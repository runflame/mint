# Sidechain mint protocol

* [Overview](#overview)
* [Definitions](#definitions)
  * [Bid](#bid)
  * [Bag](#bag)
  * [Bag ID](#bag-id)
  * [Height](#height)
  * [Reward](#reward)
  * [Fee](#fee)
  * [Inflation](#inflation)
  * [Maturation period](#maturation-period)
  * [Block](#block)
  * [Block ID](#block-id)
  * [Block size](#block-size)
  * [Transcript](#transcript)
  * [Merkle binary tree](#merkle-binary-tree)
* [Security](#security)


## Overview

The mint protocol aims to be a derivative of the proof-of-work protocol. 
Proof-of-work burns energy in exchange for bitcoins, whereas proof-of-mint burns _bitcoins_ in exchange for _sidecoins_.

The protocol is designed to turn continuous investment in a sidecoins into a consensus mechanism.
Sidecoins are released at a fixed schedule, just like bitcoins, in exchange for sacrifice at every block.
Users can join and leave the network. Any node can be a mint node without the need for specialized hardware.

The sidechain is a directed acyclic graph of [blocks](#block). 
The chain with the largest [weight](#weight) is automatically selected as a main chain.

Each [block](#block) consists of [bags](#bag) of transactions that do not contain double-spends.
Individual minters compose [bags](#bag) and commit to them by making [bids](#bid) on bitcoin chain.
[Bids](#bid)

## Definitions

#### Bid

A bitcoin transaction output that destroys some number of coins, that commits to a [bag ID](#bag-id).

Destination address for transaction must be set to [bag ID](#bag-id).

Bid transaction **lock time** is set to the bag’s height expressed in **blocks**.

#### Bag

A set of [sidechain transactions](#transaction) selected as a sidechain block candidate. 

One or more bags are used to construct a [sidechain block](#block).

TBD: a bag cannot contain information about a bid because the bid must contain bag id, so it produces an interdependence.
```
struct Bag {
    height: u64,             // height of the new block
    ancestors: Vec<BagID>,   // hashes of the ancestors bags
    timestamp_ms: u64,       // bag timestamp
    reward_address: Address, // sidechain predicate that receives the sidechain reward
    bid_tx: BitcoinTxID,     // bitcoin tx id that bids on this bag
    bid_output: u64,         // bid tx output index that commits to this bag
    bid_amount: u64,         // amount of satoshis bid on this amount
    txs: Vec<Tx>,            // sidechain txs in the new block
    ext: Vec<u8>,
}
```

##### Height

Each bag is identified by the block height to which it belongs. Sidechain blocks have the same height as their corresponding bitcoin blocks, 
although in practice their [bids](#bid) may be shifted due to delays or reorgs.

The first sidechain block contains no transactions, empty state and has height `...` (TBD).

In this specification height will be labeled as `H`. Note like `H-M` should be read as _the height of the bag located M below_.

##### Compatibility of bags

Bags are called compatible if:
1. They have the same height.
2. They have the same set of ancestors from height (H-1) to height (H-M), where M is maturation period.
3. Their transactions do not contain double spends.

##### Ancestors

Each bag has ancestors - a set of bags for which the bag is a child in the chain. An ancestor must have a height strictly less than a child height. Ancestors can be connected directly by adding them to field `Bag.ancestors` and also indirectly. Ancestor is indirectly connected if it is not contained in the `Bag.ansectors`.

Ancestor `H-M` must be read as an _ancestor located at the height H-M_. Ancestors `H-M..H-K` must be read as _a set of ancestors located from height H-M to height H-K_, where each ancestor can be not connected directly, but from another ancestor.

Each ancestor H-K for bag B must fulfill the following rules:
1. Ancestors H-K-1..H-M should be the same for the ancestor H-K and for other ancestors connected to the bag B at the height H-K.


#### Bag ID

Bag ID is a hash of the contents of the [bag](#bag).

Defined via a [transcript](#transcript):

```
T = Transcript("Flame.Bag")
T.append_u64le("height", bag.height)
// TBD: add ancestors to the transcript.
T.append("address", bag.reward_address)
T.append("bid_tx", bag.bid_tx)
T.append_u64le("bid_output", bag.bid_output)
T.append_u64le("bid_amount", bag.bid_amount)
T.append("txroot", MerkleRoot(bag.txs))
T.append("ext", bag.ext)
bag_id = T.challenge_bytes("id")
bag_id[0..1] = [0xf1, 0xae]
```

Note that the first 2 bytes are set to the bytes `F1 AE` indicating the Flame mainnet.
Testnet uses `F1 XX` prefix where the second byte `XX` indicates the version of the testnet.

Tx root is defined as [Merkle root hash](zkvm-spec.md#merkle-binary-tree) of the sidechain transactions including the witness data.


#### Reward

Reward is an asset with flavor `0` that’s given to the creators of the [block](#block) in proportion to their [bids](#bid). 

Reward consists of [transaction fees](#fee) and [inflation](#inflation). 

For each transaction, the fee is distributed among bids that include that transaction.

Inflation is distributed among all the bids.

For a block with a given height `h` the reward is computed as follows:

1. Sum up all bid amounts (in satoshis) into `X`.
2. For each transaction `tx_k`, sum up bid amounts from the bags that contain that transaction into `Z_k`.
3. For each bid with amount `x_i` and transaction `tx_k`, compute the fee reward `F_{i,k} = fee * x_i / Z_k` (128-bit division of 64-bit unsigned integers, rounding down).
4. Compute the inflation amount `R` according to the block height:
    1. Subtract the initial block [height](#height): `h' = h - INITIAL_HEIGHT`
    2. Start with inflation `R = 50'000'000` and while the `h' > 210'000` and inflation is greater than zero, divide the inflation by 2 (rounding down) and subtract 210'000 from `h'`.
    3. For each bid with amount `x_i` compute inflation reward `R_i = R * x_i / X` (128-bit division of 64-bit unsigned integers, rounding down).
5. For each bid `i`, sum up `F_{i,k}` from each transaction `tx_k` and add inflation reward `R_i`. The resulting amount is the total award `T_i` per bid. 
6. For each bid `bid_i` and total award `T_i` create a UTXO with the predicate `bid_i.address`. Unique anchor is computed as follows using the [transcript](#transcript): 

   ```
   T = Transcript("Flame.Reward")
   T.append("bag_id", bag.id)
   T.append_u64le("index", i)
   bag_id = T.challenge_bytes("anchor")
   ```

7. The resulting contract ID is stored in the _maturation list_ until block height = height + [maturation period](#maturation-period).


#### Fee

Each transaction pays the fee with asset flavor `0`. Fees are collected by creators of [bids](#bid) as a part of the [reward](#reward).


#### Inflation

Inflation is a distribution of units of asset `0` over time according to the schedule starting with block at [initial height](#height) H0.

Inflation for block `h` is computed as follows:

1. Start with inflation 50'000'000.
2. Subtract the [initial height](#height): `h' = h - INITIAL_HEIGHT`
3. While the `h' > 210'000` and inflation is greater than zero, divide the inflation by 2 (rounding down) and subtract 210'000 from `h'`.

Greater or equal  | Less than     | Inflation per block
------------------|---------------|----------------------
H0                | H0 + 210'000  | 50'000'000
H0 + 210'000      | H0 + 420'000  | 25'000'000
H0 + 420'000      | H0 + 630'000  | 12'500'000
H0 + 630'000      | H0 + 840'000  |  6'250'000
H0 + 840'000      | H0 + 1050'000 |  3'125'000
...               | ...           |        ...


#### Maturation period

Number of blocks that must pass before the [reward](#reward) contract can be spent.

For mainnet maturity period is set to `100` blocks.

Rewards are created immediately at each block, but are stored in a _maturation list_ preventing their use until they mature.


#### Block

Block is a set of transactions produced by deterministically merging one or more [bags](#bag).

```
struct Block {
    height: u64,
    prev: BlockID,
    timestamp_ms: u64,
    txroot: <Hash of merkle root of included txs>,
    state: <Utreexo root>
    bags: Vec<BagID>,
    ext: Vec<u8>,
}
```

The algorithm for deterministically merging bags into a block is as follows:

1. For each bag, compute the [reward](#reward) and add resulting contract IDs to the _maturation list_.
2. All bags must be [_compatible_](#compatibility-of-bags). Otherwise, fail validation.
3. All bags must have the same `ext` value. The `block.ext` is set to that value. Otherwise, fail validation.
4. Validate bag timestamps: 
    1. If the bag timestamp is less or equal to the median time past of the block to which it belongs (via [height](#height)), fail validation.
    2. If the bag timestamp is greater than current wallclock + 2 hours, fail validation.
5. Compute the block timestamp as a median of all bag timestamps. For even number of bags, the earlier timestamp is chosen (”rounding down”).
   Individual bag timestamps are within the required interval, therefore the median also lies within the same interval.
6. Iterate all the transactions in the bags in round-robin, skipping bags where no transactions left.
7. If a transaction with the same ID (that excludes witness data) was already added to the block, skip it.
8. Fail if transaction is invalid. Use the block timestamp for validation of time bounds.
9. Add transaction with its witness data to the block and apply to the current blockchain state (as specified by the previous block). 
   Fail if double-spend detected.
10. Compute the merkle root `txroot` over a list of collected transactions.
11. Add reward contract IDs to the utreexo state.
12. Compute the new utreexo state root for the block.


#### Block ID

TBD: hash the block fields.


#### Block size

Block size is limited to prevent denial-of-service attacks, but may slowly expand for future capacity.

For the purposes of this specification exact block size limit is unimportant.
Instead we define an abstract function `size(height) -> bytes` that maps every block height to its maximum size in bytes.


#### Transcript

Transcript is an instance of the [Merlin](https://doc.dalek.rs/merlin/) construction,
which is itself based on [STROBE](https://strobe.sourceforge.io/) and [Keccak-f](https://keccak.team/keccak.html)
with 128-bit security parameter.

Transcripts have the following operations, each taking a label for domain separation:

1. **Initialize** transcript:
    ```
    T := Transcript(label)
    ```
2. **Append bytes** of arbitrary length prefixed with a label:
    ```
    T.append(label, bytes)
    ```
3. **Challenge bytes**
    ```    
    T.challenge_bytes<size>(label) -> bytes
    ```
4. **Challenge scalar** is defined as generating 64 challenge bytes and reducing the 512-bit little-endian integer modulo Ristretto group order `|G|`:
    ```    
    T.challenge_scalar(label) -> scalar
    T.challenge_scalar(label) == T.challenge_bytes<64>(label) mod |G|
    ```

Labeled instances of the transcript can be precomputed
to reduce number of Keccak-f permutations to just one per challenge.


#### Merkle binary tree

The construction of a merkle binary tree is based on the [RFC 6962 Section 2.1](https://tools.ietf.org/html/rfc6962#section-2.1)
with hash function replaced with a [transcript](#transcript).

Leafs and nodes in the tree use the same instance of a transcript provided by the upstream protocol:

```
T = Transcript(<label>)
```

The hash of an empty list is a 32-byte challenge string with the label `merkle.empty`:

```
MerkleHash(T, {}) = T.challenge_bytes("merkle.empty")
```

The hash of a list with one entry (also known as a leaf hash) is
computed by committing the entry to the transcript (defined by the item type),
and then generating 32-byte challenge string the label `merkle.leaf`:

```
MerkleHash(T, {item}) = {
    T.append(<field1 name>, item.field1)
    T.append(<field2 name>, item.field2)
    ...
    T.challenge_bytes("merkle.leaf")
}
```

For n > 1, let k be the largest power of two smaller than n (i.e., k < n ≤ 2k). The merkle hash of an n-element list is then defined recursively as:

```
MerkleHash(T, list) = {
    T.append("L", MerkleHash(list[0..k]))
    T.append("R", MerkleHash(list[k..n]))
    T.challenge_bytes("merkle.node")
}
```

Note that we do not require the length of the input list to be a power of two.
The resulting merkle binary tree may thus not be balanced; however,
its shape is uniquely determined by the number of leaves.



### Sidechain validation


The algorithm for validating the block is the following:

1. In case of Bitcoin reorg, roll back the sidechain to the point valid at the latest non-reorged Bitcoin block. Then process sidechain blocks, validating them against the new Bitcoin main chain.
2. Check that block.height is +1 to the current tip.
3. Check that block.prev points to the current tip.
4. Check that bags are ordered primarily by BTC amount burned, secondarily by BitcoinTxID lexicographically (lowest hash first).
5. For each bag in the list:
    1. Check that tx with `BitcoinTxID` exists in bitcoin main chain and its first output correctly commits to the sidechain ID and the bag ID.
    2. Check that tx locktime is expressed in block height and equals H0 + block.height.
    3. Check that bag.prev == block.prev and bag.height == block.height.
    4. Check bag size to be less or equals the current [blocksize limit](#block-size).
    5. Apply transactions, skipping duplicates from the previously processed commit within this block.
    6. Check block size to be less or equals the current [blocksize limit](#block-size).
    7. Fail the entire block if invalid tx is encountered (including double-spend attempt).
6. Compute allocations of [block reward](#reward) in the following manner:
    1. [Inflation](#inflation) units are divided in proportion to the satoshis burned in all the [bags](#bag) constituting the block.
    2. Transaction fees are divided in proportion to the satoshis burned in the [bags](#bag) that contain the transaction. 
       If the transaction is contained in a single [bag](#bag), the entirety of its fee is allocated to that bag's address.
    3. All the rewards (fees + inflation) are not spendable until 100 blocks in the future (approx. 16 hours).


### Consensus

Due to double-spends and conflicting bags, there could be multiple valid sidechains.
To resolve which one is the main one, the following consensus algorithm is proposed:

1. Each chain has **total weight** as a sum of weights of each of its blocks. If a better chain appears, reorganization procedure is used to switch from the current chain to another one: blocks are rolled back one after another to the common block, and then another chain's blocks are applied per usual rules. If the new chain violates rules, it is banned and the reorganization is performed back to the valid chain.
2. Each block's weight is a base-2 logarithm of the sum of each bag's **adjusted burn**.
3. Each bid's adjusted burn is amount of satoshis burned, **halved N times for N bitcoin blocks delay** comparing to the target block height (sidechain block height + H0). In other words, bid being "late" by N bitcoin blocks has its BTC value divided by 2^N for the purposes of weight calculation.

**Note 1.** Because of the adjustment due to position in bitcoin chain, total weight may change as Bitcoin reorgs happen and transactions become more concentrated or spread out. This may affect the choice of the main sidechain and cause its reorganzation. In other words, Bitcoin miners are capable of affecting the finality of sidechain transactions: after all, sidechain relies on Bitcoin for its security.

**Note 2.** Position-dependent weight calculation does not affect [block reward distribution](#reward). This is intentional so that reward allocations are stable across Bitcoin reorganizations.

**Note 3.** Defining total weight as sum of logarithms of adjusted burns helps with fighting long-range attacks. Logarithm is monotonic and locally linear, so real-time conflicts are not amplified, but over the long term makes it exponentially expensive to produce a fork. For instance, making a fork N blocks late requires 2^N amount of capital.

### Minter

By analogy with _miners_, minters are nodes in the network that [mint](#minting) [sidecoins](#sidecoin) by publishing [bids](#bid).

### Minting

A process of performing [bids](#bid) that mint [sidecoins](#sidecoin). By analogy with bitcoin _mining_.

1. First, receive the latest BTC block and all available [bags](#bag) in the sidechain network.
2. Second, wait to receive all available [bids](#bid) in this block over the sidechain p2p network.
3. Choose the maximum-weight combination of bids that fit under the block limit, which defines the previous block.
4. Apply that block to the current state, removing conflicting and duplicate txs from the mempool.
5. Prepare a bag with an under-the-limit subset of most-paying txs from mempool and a reference to the previous block ID as computed speculatively from the known bids.
6. Create a "bid transaction" with next block's timelock and the bag ID in the output script.
7. Broadcast the transaction ASAP so it gets mined in the next block.







### Security

TODO: how does greedy selection of bags work? What if one user withhelds his bag, so they make the next bag link to the different and higher-value prevblock, so it's incompatible with everyone else?

                        block 1      block 2
    honest players:     N1 btc       N2
            weight:     log2(N1)     log2(N2) ...

    dishonest player:   N1+A1 btc    A2
            weight:     log2(N1+A1)  log2(A2) ...

Honest weight for K blocks:   
    
    \sum_{ log2(N_i) }  i = 1 ... k

Dishonest weight of K blocks: (changing N1+A1 for N1*(1 + f1) where f1 is capital share of the attacker

    log2(N1*(1+f1)) == log2(N1) + log2(1+f1)
    A2 => N2*f2, etc.
    
    
    log2(1+f1) + \sum_{ log2(f_j) } + \sum_{ log2(N_i) }

      ( > 0 )          ( < 0 if f_j < 1.0)  <- unless there's a 51% attack, each block diminishes the value of the initial advantage.

So let's say we want to have one-block reorg: at block 1 we withhold a bag and for block 2 create a new bag that's 
               
    








