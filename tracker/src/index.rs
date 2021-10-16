use crate::bag_storage::BagStorage;
use crate::bitcoin_client::BitcoinClient;
use crate::record::{BagEntry, BagEntryData, BagProof, Outpoint};
use crate::storage::IndexStorage;
use bitcoin::{BlockHash, Transaction, TxOut};
use bitcoincore_rpc::json::GetBlockHeaderResult;
use std::convert::TryFrom;

pub type BagId = [u8; 32];

pub struct Index<C: BitcoinClient, S: IndexStorage, B: BagStorage> {
    btc_client: C,
    bids_storage: S,
    bags_storage: B,

    current_height: u64,
    current_tip: BlockHash,
}

impl<C: BitcoinClient, S: IndexStorage, B: BagStorage> Index<C, S, B> {
    pub fn new(client: C, storage: S, bags: B, base_height: Option<u64>) -> Self {
        let info = client.get_blockchain_info().unwrap();
        let height = base_height.unwrap_or(info.blocks);
        let tip = client.get_block_hash(height).unwrap();
        Index {
            btc_client: client,
            current_height: height,
            current_tip: tip,
            bids_storage: storage,
            bags_storage: bags,
        }
    }

    pub fn add_bag(&self, bag: BagId) -> Result<(), B::Err> {
        self.bags_storage.insert_bag(bag)
    }

    // TODO: better API for indexing
    pub fn get_storage(&self) -> &S {
        &self.bids_storage
    }

    pub fn btc_client(&self) -> &C {
        &self.btc_client
    }

    pub fn current_height(&self) -> &u64 {
        &self.current_height
    }
    pub fn current_tip(&self) -> &BlockHash {
        &self.current_tip
    }
}

impl<C: BitcoinClient, S: IndexStorage, B: BagStorage> Index<C, S, B> {
    /// Check existence of the bid in the bitcoin chain, and if it is then add it to the store
    pub fn add_bid(&mut self, proof: BagProof) -> Result<(), ()> {
        let response = self
            .btc_client
            .get_transaction(&proof.outpoint.txid)
            .unwrap();
        let tx = response.transaction().unwrap();
        let bid_data = parse_mint_transaction_btc_block(&tx, proof.outpoint.out_pos).unwrap();
        let bid = BagEntry {
            btc_block: response.info.blockhash.unwrap(),
            btc_outpoint: proof.outpoint,
            data: bid_data,
        };
        self.bags_storage
            .insert_bag(bid.data.bag_id.clone())
            .unwrap();
        self.bids_storage.store_record(bid).unwrap();
        if response.info.blockheight.unwrap() as u64 > self.current_height {
            self.current_height = response.info.blockheight.unwrap() as u64;
            self.current_tip = response.info.blockhash.unwrap();
        }

        Ok(())
    }

    /// Check chain for the reorgs, and if it happened, delete old bids and check for them in new chain
    pub fn check_reorgs(&mut self) -> Option<ReorgInfo> {
        let new_btc_info = self.btc_client.get_blockchain_info().unwrap();
        let new_height = new_btc_info.blocks;

        match self.check_btc_for_reorgs() {
            Some(reorg) => {
                self.remove_btc_blocks_when_fork(&reorg);
                self.add_btc_blocks(reorg.height_when_fork, new_height);
                Some(reorg)
            }
            None => {
                self.add_btc_blocks(self.current_height, new_height);
                None
            }
        }
    }

    fn remove_btc_blocks_when_fork(&mut self, reorg_info: &ReorgInfo) {
        for discarded_block in reorg_info.discarded_blocks.iter() {
            self.bids_storage
                .remove_with_block_hash(discarded_block)
                .unwrap();
        }
    }

    fn add_btc_blocks(&mut self, old_height: u64, new_height: u64) {
        for index in old_height + 1..new_height + 1 {
            let hash = self.btc_client.get_block_hash(index).unwrap();
            self.add_btc_block_to_index(hash).unwrap();

            self.current_height = index;
            self.current_tip = hash;
        }
    }

    fn check_btc_for_reorgs(&self) -> Option<ReorgInfo> {
        let tip = &self.current_tip;

        let mut discarded_blocks = vec![];
        let mut block_hash = tip.clone();
        let mut height;
        let mut reorg = false;
        loop {
            let block_header_info = self.btc_client.get_block_header_info(&block_hash).unwrap();
            height = block_header_info.height;
            if is_block_in_main_chain(&block_header_info) {
                break;
            } else {
                reorg = true;
                discarded_blocks.push(block_hash);
                // Bitcoin core api does not provide information when it is None, so I suppose it will be None only
                // in case of block with height 0, and in that case block _must_ be in the main chain.
                block_hash = block_header_info.previous_block_hash.unwrap();
            }
        }

        if reorg {
            Some(ReorgInfo {
                height_when_fork: height as u64,
                discarded_blocks,
            })
        } else {
            None
        }
    }

    fn add_btc_block_to_index(&mut self, block_hash: BlockHash) -> Result<(), S::Err> {
        let transactions = self.check_btc_block_with_hash(block_hash.clone());
        transactions
            .into_iter()
            .map(|tx| self.bids_storage.store_record(tx))
            .collect::<Result<Vec<()>, S::Err>>()?;
        Ok(())
    }

    fn check_btc_block_with_hash(&self, hash: BlockHash) -> Vec<BagEntry> {
        let block = self.btc_client.get_block(&hash).unwrap();
        let txs = block.txdata;

        let mint_txs = txs
            .into_iter()
            .filter_map(|tx| {
                parse_mint_transaction_btc_block_unknown_pos(tx)
                    .filter_map(|(outpoint, bid_data)| {
                        let bag_exists = self
                            .bags_storage
                            .is_bag_exists(&bid_data.bag_id)
                            .unwrap_or(false);
                        if bag_exists {
                            Some(BagEntry {
                                btc_block: hash,
                                btc_outpoint: outpoint,
                                data: bid_data,
                            })
                        } else {
                            None
                        }
                    })
                    .next()
            })
            .collect();

        mint_txs
    }
}

fn parse_mint_transaction_btc_block(tx: &Transaction, out_pos: u64) -> Option<BagEntryData> {
    let output = tx.output.get(out_pos as usize)?;
    parse_mint_btc_output(output)
}

fn parse_mint_transaction_btc_block_unknown_pos(
    tx: Transaction,
) -> impl Iterator<Item = (Outpoint, BagEntryData)> {
    let txid = tx.txid();
    tx.output
        .into_iter()
        .enumerate()
        .filter_map(move |(out_pos, out)| {
            parse_mint_btc_output(&out).map(|data| {
                let outpoint = Outpoint {
                    txid: txid.clone(),
                    out_pos: out_pos as u64,
                };
                (outpoint, data)
            })
        })
}

fn parse_mint_btc_output(out: &TxOut) -> Option<BagEntryData> {
    match out.script_pubkey.is_v0_p2wsh() {
        true => {
            let bag_id = BagId::try_from(&out.script_pubkey.as_bytes()[2..34])
                .expect("Script is in p2wsh form");
            let amount = out.value;
            Some(BagEntryData { bag_id, amount })
        }
        false => None,
    }
}

fn is_block_in_main_chain(block: &GetBlockHeaderResult) -> bool {
    block.confirmations != -1
}

#[derive(Debug, PartialEq)]
pub struct ReorgInfo {
    height_when_fork: u64,
    discarded_blocks: Vec<BlockHash>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bag_storage::BagHashSetStorage;
    use crate::storage::memory::MemoryIndexStorage;
    use crate::test_utils::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_new_blocks_with_mint_txs() {
        let initial_block = create_test_block(0, [1], None);
        let (block2, prf) =
            create_test_block_with_mint_tx(1, [2], Some(initial_block.block_hash), [1; 32]);

        let blocks = Rc::new(RefCell::new(vec![initial_block.clone()]));
        let client = TestBitcoinClient {
            blocks: blocks.clone(),
        };
        let storage = MemoryIndexStorage::new();
        let bags = BagHashSetStorage::new();
        let mut index = Index::new(client, storage, bags, None);

        index.add_bag([1; 32]).unwrap();

        blocks.borrow_mut().push(block2.clone());
        index.add_bid(prf).unwrap();

        assert_eq!(index.bids_storage.get_blocks_count().unwrap(), 1);

        let txs_in_index = index
            .bids_storage
            .get_records_by_block_hash(&block2.block_hash)
            .unwrap();
        let block2_tx_out = &txs_in_index[0];

        assert_eq!(block2_tx_out.data.amount, 10);
        assert_eq!(block2_tx_out.data.bag_id, [1; 32]);
    }

    #[test]
    fn test_new_blocks_with_mint_txs_invalid_bags() {
        let initial_block = create_test_block(0, [1], None);
        let (mut block2, prf1) =
            create_test_block_with_mint_tx(1, [2], Some(initial_block.block_hash), [1; 32]);
        let (tx2, prf2) = create_test_mint_transaction([2; 32]);
        block2.txs.push(tx2);

        let blocks = Rc::new(RefCell::new(vec![initial_block.clone()]));
        let client = TestBitcoinClient {
            blocks: blocks.clone(),
        };

        let bags = BagHashSetStorage::new();
        let storage = MemoryIndexStorage::new();
        let mut index = Index::new(client, storage, bags, None);
        blocks.borrow_mut().push(block2.clone());

        index.add_bid(prf1).unwrap();
        index.add_bid(prf2).unwrap();
        assert_eq!(index.bids_storage.get_blocks_count().unwrap(), 1);

        let mut txs_in_index = index
            .bids_storage
            .get_records_by_block_hash(&block2.block_hash)
            .unwrap();
        txs_in_index.sort_by(|x, y| x.data.bag_id.cmp(&y.data.bag_id));

        assert_eq!(txs_in_index.len(), 2);
        assert_eq!(
            txs_in_index[0].data,
            BagEntryData {
                bag_id: [1; 32],
                amount: 10
            }
        );
        assert_eq!(
            txs_in_index[1].data,
            BagEntryData {
                bag_id: [2; 32],
                amount: 10
            }
        );
    }

    #[test]
    fn test_reorg() {
        let initial_block = create_test_block(0, [1], None);
        let (block2, _prf1) =
            create_test_block_with_mint_tx(1, [2], Some(initial_block.block_hash), [1; 32]);
        let (forked_block, _prf2) =
            create_test_block_with_mint_tx(1, [3], Some(initial_block.block_hash), [2; 32]);
        let (forked_block2, _prf3) =
            create_test_block_with_mint_tx(2, [4], Some(forked_block.block_hash), [3; 32]);

        let initial_blocks = vec![initial_block.clone()];
        let blocks_chain_1 = vec![initial_block.clone(), block2.clone()];
        let blocks_chain_2 = vec![
            initial_block.clone(), // first block in both chains
            TestBlock {
                in_main_chain: false, // was in the main chain, after reorg is not
                ..block2
            },
            forked_block.clone(),
            forked_block2.clone(),
        ];

        let blocks = Rc::new(RefCell::new(initial_blocks));
        let client = TestBitcoinClient {
            blocks: blocks.clone(),
        };
        let storage = MemoryIndexStorage::new();
        let bags = BagHashSetStorage::new();
        let mut index = Index::new(client, storage, bags, None);

        index.add_bag([1; 32]).unwrap();
        index.add_bag([2; 32]).unwrap();
        index.add_bag([3; 32]).unwrap();

        *blocks.borrow_mut() = blocks_chain_1.clone();
        index.check_reorgs();

        assert_eq!(index.current_height, 1);
        assert_eq!(index.bids_storage.get_blocks_count().unwrap(), 1);

        *blocks.borrow_mut() = blocks_chain_2.clone();
        index.check_reorgs();

        assert_eq!(index.current_height, 2);
        assert_eq!(index.bids_storage.get_blocks_count().unwrap(), 2);

        let txs_in_index = index
            .bids_storage
            .get_records_by_block_hash(&forked_block.block_hash)
            .unwrap();
        let forked_block_tx_out = &txs_in_index[0];
        assert_eq!(forked_block_tx_out.data.bag_id, [2; 32]);

        let txs_in_index = index
            .bids_storage
            .get_records_by_block_hash(&forked_block2.block_hash)
            .unwrap();
        let forked_block2_tx_out = &txs_in_index[0];
        assert_eq!(forked_block2_tx_out.data.bag_id, [3; 32]);
    }
}
