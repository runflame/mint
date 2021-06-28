#!/usr/bin/env ruby

require 'set'
require 'pp'

# This is a prototype of the reorg scenarios involving sidechain and bitcoin chain.
# In this prototype we assume all transactions well-formed and correctly signed.
# Links between objects normally identified by hashes are represented with direct references.
# Bitcoin blocks are modelled as only containing sidechain bids: all regular transactions are ignored.

# Some bitcoin block height when the network is launched.
# All sidechain block heights are in sync with
LAUNCH_HEIGHT = 600000

class BitcoinBlock
  attr_reader :id
  attr_accessor :height
  attr_accessor :prev_bitcoin_block
  attr_accessor :bid_refs
end

# Bitcoin network can only reference the bid by id and knows its weight (amount of btc burned)
# but we can't assume data to be available because it comes from another, sidechain, network.
class BidRef
  attr_accessor :amount
  attr_accessor :bid_id
end

# Sidechain network exchanges bids that 
class Bid
  attr_reader :id
  attr_accessor :height
  attr_accessor :prev_sidechain_block
  attr_accessor :sidechain_txs
  attr_accessor :amount
end

class SidechainBlock
  attr_reader :id
  attr_accessor :height
  attr_accessor :prev_sidechain_block
  attr_accessor :bids
  attr_accessor :state # entire state of the chain when this block is applied
end

class SidechainTx
  attr_accessor :inputs
  attr_accessor :outputs
end

# This is an item in UTXO set, simply a unique object within the runtime.
# This does not hold any data because we are not validating transactions in this prototype.
class SidechainContract
  attr_reader :id
  def ==(other)
    self.class == other.class &&
    self.id == other.id
  end
end

# State of the blockchain. For simplicity, it's a simple set of unspent contracts,
# but in real life it is compressed using Utreexo.
class State
  def initialize
    @utxos = Set.new
  end
  
  def inspect
    "#<State #{@utxos.to_a.inspect}>"
  end
end

# Represents the state of the node
class Node
  attr_accessor :bitcoin_tip_block
  attr_accessor :sidechain_tip_block
end


# Utility methods

class SidechainContract
  @@id = 0
  def initialize
    @@id += 1
    @id = @@id
  end
  def inspect
    "#<Contract:#{@id}>"
  end
end

class Bid
  @@id = 0
  def initialize
    @@id += 1
    @id = @@id
  end
  
  def to_ref
    BidRef.new.tap do |bidref|
      bidref.amount = self.amount
      bidref.bid_id = self.id
    end
  end

  # Returns an array of txs with duplicates removed in round-robin fashion.
  def flatten_bids(bids)
    txs = []
    # flatten txs, picking one from each bid
    bids_txs = bids.map{|b| b.sidechain_txs}.flatten_round_robin
    # remove duplicates
    bid_txs.unique
  end

  def inspect
    "#<Bid:#{id} sats:#{amount} height:#{height} prev:#{prev_sidechain_block.id} txs:#{sidechain_txs}>"
  end
end

class BidRef
  def inspect
    "#<BidRef:#{bid_id} sats:#{amount}>"
  end
end

class SidechainBlock
  @@id = 0
  def initialize
    @@id += 1
    @id = @@id
  end

  def self.genesis
    new.tap do |b|
      b.height = LAUNCH_HEIGHT
      b.prev_sidechain_block = nil
      b.bids = []
      b.state = State.new
    end
  end

  def self.from_bids(bids)
    raise "TBD"
  end

  def build_next_block(bids)
    SidechainBlock.new.tap do |b|
      b.height = self.height + 1
      b.prev_sidechain_block = self
      b.bids = bids
      b.state = self.state.apply_txs(Bid.flatten_bids(bids))
    end
  end

  def inspect
    "#<SidechainBlock:#{@id} height:#{height} prev:#{prev_sidechain_block ? prev_sidechain_block.id : nil} bids:#{bids}>"
  end
end

class BitcoinBlock
  @@id = 0
  def initialize
    @@id += 1
    @id = @@id
  end

  def self.launch_block
    new.tap do |b|
      b.height = LAUNCH_HEIGHT
      b.prev_bitcoin_block = nil # we ignore all reorgs before the launch blocks for now
      b.bid_refs = []
    end
  end

  # Constructs the next block with the given bid refs.
  def build_next_block(bid_refs)
    BitcoinBlock.new.tap do |b|
      b.height = self.height + 1
      b.prev_bitcoin_block = self
      b.bid_refs = bid_refs
    end
  end

  # Within one difficulty retarget interval the weight of the chain is determined by its height,
  # so for simplicity of the prototype we assume all reorgs happening within one retarget window.
  def weight
    height
  end

  def inspect
    "#<BTCBlock:#{@id} height:#{height} prev:#{prev_bitcoin_block ? prev_bitcoin_block.id : nil} bids:#{bid_refs}>"
  end
end



class SidechainTx
  def initialize(inputs:, outputs:)
    @inputs = inputs
    @outputs = outputs
  end

  def ==(other)
    self.inputs == other.inputs &&
    self.outputs == other.outputs
  end 
end


class State
  def apply_tx(tx)
    apply_txs([tx])
  end

  def apply_txs(txs)
    new_utxos = @utxos.dup
    txs.each do |tx|
      tx.inputs.each do |contract|
        if !@utxos.include? contract
          raise "Invalid tx: contract #{contract.id} is not in the utxo set"
        end
        new_utxos.delete contract
      end
      tx.outputs.each do |contract|
        new_utxos.add contract
      end
    end
    @utxos = new_utxos
    self
  end
end

class Node
  def initialize
    @bitcoin_tip_block = BitcoinBlock.launch_block
    @sidechain_tip_block = SidechainBlock.genesis
  end

  def btc_height
    @bitcoin_tip_block.height
  end

  def sidechain_height
    @sidechain_tip_block.height
  end

  # Receives a BitcoinBlock or SidechainBlock
  def accept_message(msg)
    if msg.is_a?(BitcoinBlock) 
      self.accept_bitcoin_block(msg)
    elsif msg.is_a?(SidechainBlock) 
      self.accept_sidechain_block(msg)
    else
      raise "Unknown msg kind: #{msg.class}"
    end
  end

  def accept_bitcoin_block(btc_block)
    if btc_block.weight <= self.bitcoin_tip_block.weight
      raise "New block is not heavier than the current tip."
    end

    # If we cleanly extend the current block, simply update the tip
    # TBD: replace this with a generic reorg code that handles -M, +N extension of the chain
    if btc_block.height == self.bitcoin_tip_block.height + 1 &&
      btc_block.prev_bitcoin_block == self.bitcoin_tip_block
      self.bitcoin_tip_block = btc_block
      return
    end
    raise "TBD: handle the generic -M/+N reorg logic"
  end

  def accept_sidechain_block(sdc_block)
    raise "TBD"
  end

end


class Array
  # assumes array of arrays, shifts items one by one from each array.
  # [ [1,2,3], [7,8] ].flatten_round_robin => [1,7,2,8,3]
  def flatten_round_robin
    result = []
    while list = self.shift
      if item = list.shift
        # put the item into the result array
        result.push item
        # put the list back in the list
        self.push list
      else
        # the list is empty - don't put it back
      end
    end
    result
  end
end


########################## TESTS ##########################

node = Node.new

pp node

# prepare a bid

bid1 = Bid.new.tap do |bid|
  bid.height = node.btc_height + 1
  bid.prev_sidechain_block = node.sidechain_tip_block
  bid.sidechain_txs = []
  bid.amount = 10
end

btc_block2 = node.bitcoin_tip_block.build_next_block([ bid1.to_ref ])

node.accept_message(btc_block2)

sdc_block2 = SidechainBlock.new.tap do |b|
  b.height = node.sidechain_height + 1
  b.prev_sidechain_block = node.sidechain_tip_block
  b.bids = [ bid1 ]
  b.state # entire state of the chain when this block is applied
end

node.accept_message(sdc_block2)

pp node


c1 = SidechainContract.new
c2 = SidechainContract.new
c3 = SidechainContract.new

tx1 = SidechainTx.new(inputs: [c1], outputs: [c2])
tx2 = SidechainTx.new(inputs: [c1.dup], outputs: [c2])
tx3 = SidechainTx.new(inputs: [c1], outputs: [c3])

pp [tx1 == tx2, tx1 == tx3]