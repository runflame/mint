use std::convert::TryFrom;
use std::error::Error;

use bitcoin::{Block, BlockHash, Transaction, TxOut, Txid};
use bitcoincore_rpc::json::GetBlockHeaderResult;
use thiserror::Error;

use crate::bag_id::BagId;
use crate::bitcoin_client::{BitcoinClient, ClientError};
use crate::record::{BidEntry, BidEntryData, BidProof, BidTx, Outpoint};
use crate::storage::{BidStorage, BidStorageError};

pub struct Index<C: BitcoinClient, S: BidStorage> {
    btc_client: C,
    bids_storage: S,

    current_height: u64,
    current_tip: BlockHash,
}

impl<C: BitcoinClient, S: BidStorage> Index<C, S> {
    pub fn new(
        client: C,
        storage: S,
        base_height: Option<u64>,
    ) -> Result<Self, ClientError<C::Err>> {
        let info = client.get_blockchain_info()?;
        let height = base_height.unwrap_or(info.blocks);
        let tip = client.get_block_hash(height)?;
        Ok(Index {
            btc_client: client,
            current_height: height,
            current_tip: tip,
            bids_storage: storage,
        })
    }

    pub fn add_bag(&self, bag: impl Into<BagId>) -> Result<(), BidStorageError<S::Err>> {
        self.bids_storage.insert_unconfirmed_bag(bag.into())
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

type IError<C, S> = TrackerError<<C as BitcoinClient>::Err, <S as BidStorage>::Err>;

impl<C: BitcoinClient, S: BidStorage> Index<C, S> {
    /// Check existence of the bid in the bitcoin chain, and if it is then add it to the store
    pub fn add_bid(&mut self, proof: BidProof) -> Result<(), IError<C, S>> {
        let block = self.btc_client.get_block(&proof.btc_block)?;
        let height = self
            .btc_client
            .get_block_header_info(&proof.btc_block)?
            .height;

        let tx = find_tx(block, &proof.tx.outpoint.txid).ok_or_else(|| {
            TrackerError::TxDoesNotExists(proof.btc_block, proof.tx.outpoint.txid)
        })?;
        let bid_data = parse_mint_transaction_btc_block(&tx, proof.tx.outpoint.out_pos)
            .ok_or_else(|| TrackerError::WrongOutputFormat)?;

        if bid_data.bag_id != proof.tx.bag_id {
            return Err(TrackerError::WrongBagId {
                tx: proof.tx.outpoint.txid,
                expected: proof.tx.bag_id,
                actual: bid_data.bag_id,
            });
        }

        let bid = BidEntry {
            amount: bid_data.amount,
            proof,
        };

        if height as u64 > self.current_height {
            self.current_height = height as u64;
            self.current_tip = bid.proof.btc_block;
        }

        self.bids_storage.insert_bid(bid)?;

        Ok(())
    }

    /// Check chain for the reorgs, and if it happened, delete old bids and check for them in new chain
    pub fn check_reorgs(&mut self) -> Result<Option<ReorgInfo>, IError<C, S>> {
        let new_btc_info = self.btc_client.get_blockchain_info()?;
        let new_height = new_btc_info.blocks;

        let reorg = match self.check_btc_for_reorgs()? {
            Some(reorg) => {
                self.remove_btc_blocks_when_fork(&reorg)?;
                self.current_height = reorg.height_when_fork;
                self.current_tip = reorg.fork_root;

                Some(reorg)
            }
            None => None,
        };
        self.add_btc_blocks(self.current_height, new_height)?;

        Ok(reorg)
    }

    fn remove_btc_blocks_when_fork(
        &mut self,
        reorg_info: &ReorgInfo,
    ) -> Result<(), BidStorageError<S::Err>> {
        for discarded_block in reorg_info.discarded_blocks.iter() {
            match self.bids_storage.remove_with_block_hash(discarded_block) {
                Ok(_) | Err(BidStorageError::BagDoesNotExists(_)) => {}
                err => return err,
            }
        }
        Ok(())
    }

    fn add_btc_blocks(&mut self, old_height: u64, new_height: u64) -> Result<(), IError<C, S>> {
        for index in old_height + 1..new_height + 1 {
            let hash = self.btc_client.get_block_hash(index)?;
            self.add_btc_block_to_index(hash)?;

            self.current_height = index;
            self.current_tip = hash;
        }
        Ok(())
    }

    fn check_btc_for_reorgs(&self) -> Result<Option<ReorgInfo>, IError<C, S>> {
        let tip = &self.current_tip;

        let mut discarded_blocks = vec![];
        let mut block_hash = tip.clone();
        let mut height;
        let mut reorg = false;
        loop {
            let block_header_info = self.btc_client.get_block_header_info(&block_hash)?;
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

        Ok(if reorg {
            Some(ReorgInfo {
                height_when_fork: height as u64,
                fork_root: block_hash,
                discarded_blocks,
            })
        } else {
            None
        })
    }

    fn add_btc_block_to_index(&mut self, block_hash: BlockHash) -> Result<(), IError<C, S>> {
        let transactions = self.check_btc_block_with_hash(block_hash.clone())?;
        transactions
            .into_iter()
            .map(|bid| match self.bids_storage.update_bid(bid) {
                Ok(_) | Err(BidStorageError::BagDoesNotExists(_)) => Ok(()),
                err => err,
            })
            .collect::<Result<Vec<()>, BidStorageError<S::Err>>>()?;
        Ok(())
    }

    fn check_btc_block_with_hash(&self, hash: BlockHash) -> Result<Vec<BidEntry>, IError<C, S>> {
        let block = self.btc_client.get_block(&hash)?;
        let txs = block.txdata;

        let mint_txs = txs
            .into_iter()
            .filter_map(|tx| {
                parse_mint_transaction_btc_block_unknown_pos(tx)
                    .filter_map(|(outpoint, bid_data)| {
                        let bag_exists = self
                            .bids_storage
                            .is_bag_exists(&bid_data.bag_id)
                            .unwrap_or(false);
                        if bag_exists {
                            Some(BidEntry {
                                amount: bid_data.amount,
                                proof: BidProof {
                                    btc_block: hash,
                                    tx: BidTx {
                                        outpoint,
                                        bag_id: bid_data.bag_id,
                                    },
                                },
                            })
                        } else {
                            None
                        }
                    })
                    .next()
            })
            .collect();

        Ok(mint_txs)
    }
}

fn find_tx(block: Block, txid: &Txid) -> Option<Transaction> {
    block.txdata.into_iter().find(|tx| tx.txid() == *txid)
}

fn parse_mint_transaction_btc_block(tx: &Transaction, out_pos: u64) -> Option<BidEntryData> {
    let output = tx.output.get(out_pos as usize)?;
    parse_mint_btc_output(output)
}

fn parse_mint_transaction_btc_block_unknown_pos(
    tx: Transaction,
) -> impl Iterator<Item = (Outpoint, BidEntryData)> {
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

fn parse_mint_btc_output(out: &TxOut) -> Option<BidEntryData> {
    match out.script_pubkey.is_v0_p2wsh() {
        true => {
            let bag_id = BagId::try_from(&out.script_pubkey.as_bytes()[2..34])
                .expect("Script is in p2wsh form");
            let amount = out.value;
            Some(BidEntryData { bag_id, amount })
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
    fork_root: BlockHash, // Block that available in both chains.
    discarded_blocks: Vec<BlockHash>,
}

#[derive(Debug, Error)]
pub enum TrackerError<C: Error, S: Error> {
    #[error(transparent)]
    ClientError(#[from] ClientError<C>),

    #[error(transparent)]
    StorageError(#[from] BidStorageError<S>),

    #[error("Transaction with {1} id does not contains in block with {0} id.")]
    TxDoesNotExists(BlockHash, Txid),

    #[error("Transaction output has wrong format.")]
    WrongOutputFormat,

    #[error("Expected bag id {expected} but found bag id {actual} in transaction {tx}")]
    WrongBagId {
        tx: Txid,
        expected: BagId,
        actual: BagId,
    },
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::storage::memory::MemoryIndexStorage;
    use crate::test_utils::*;

    use super::*;

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
        let mut index = Index::new(client, storage, None).unwrap();

        index.add_bag([1; 32]).unwrap();

        blocks.borrow_mut().push(block2.clone());
        index.add_bid(prf).unwrap();

        assert_eq!(index.bids_storage.get_blocks_count().unwrap(), 1);

        let txs_in_index = index
            .bids_storage
            .get_records_by_block_hash(&block2.block_hash)
            .unwrap();
        let block2_tx_out = &txs_in_index[0];

        assert_eq!(block2_tx_out.amount, 10);
        assert_eq!(block2_tx_out.proof.tx.bag_id, [1; 32]);
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

        let storage = MemoryIndexStorage::new();
        let mut index = Index::new(client, storage, None).unwrap();
        blocks.borrow_mut().push(block2.clone());

        index.add_bid(prf1).unwrap();
        index
            .add_bid(BidProof::new(block2.block_hash, prf2))
            .unwrap();
        assert_eq!(index.bids_storage.get_blocks_count().unwrap(), 1);

        let mut txs_in_index = index
            .bids_storage
            .get_records_by_block_hash(&block2.block_hash)
            .unwrap();
        txs_in_index.sort_by(|x, y| x.proof.tx.bag_id.cmp(&y.proof.tx.bag_id));

        assert_eq!(txs_in_index.len(), 2);
        assert_eq!(txs_in_index[0].proof.tx.bag_id, [1; 32]);
        assert_eq!(txs_in_index[1].proof.tx.bag_id, [2; 32]);
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
        let mut index = Index::new(client, storage, None).unwrap();

        index.add_bag([1; 32]).unwrap();
        index.add_bag([2; 32]).unwrap();
        index.add_bag([3; 32]).unwrap();

        *blocks.borrow_mut() = blocks_chain_1.clone();
        index.check_reorgs().unwrap();

        assert_eq!(index.current_height, 1);
        assert_eq!(index.bids_storage.get_blocks_count().unwrap(), 1);

        *blocks.borrow_mut() = blocks_chain_2.clone();
        index.check_reorgs().unwrap();

        assert_eq!(index.current_height, 2);
        assert_eq!(index.bids_storage.get_blocks_count().unwrap(), 2);

        let txs_in_index = index
            .bids_storage
            .get_records_by_block_hash(&forked_block.block_hash)
            .unwrap();
        let forked_block_tx_out = &txs_in_index[0];
        assert_eq!(forked_block_tx_out.proof.tx.bag_id, [2; 32]);

        let txs_in_index = index
            .bids_storage
            .get_records_by_block_hash(&forked_block2.block_hash)
            .unwrap();
        let forked_block2_tx_out = &txs_in_index[0];
        assert_eq!(forked_block2_tx_out.proof.tx.bag_id, [3; 32]);
    }

    #[test]
    fn test_reorg_shorter_chain() {
        let initial_block = create_test_block(0, [1], None);
        let (forked_block, _prf1) =
            create_test_block_with_mint_tx(1, [2], Some(initial_block.block_hash), [1; 32]);
        let (block2, _prf2) =
            create_test_block_with_mint_tx(1, [3], Some(initial_block.block_hash), [2; 32]);
        let (block3, _prf3) =
            create_test_block_with_mint_tx(2, [4], Some(block2.block_hash), [3; 32]);

        let initial_blocks = vec![initial_block.clone()];
        let blocks_chain_1 = vec![initial_block.clone(), block2.clone(), block3.clone()];
        let blocks_chain_2 = vec![
            initial_block.clone(), // first block in both chains
            TestBlock {
                in_main_chain: false, // was in the main chain, after reorg is not
                ..block2
            },
            TestBlock {
                in_main_chain: false, // was in the main chain, after reorg is not
                ..block3
            },
            forked_block.clone(),
        ];

        let blocks = Rc::new(RefCell::new(initial_blocks));
        let client = TestBitcoinClient {
            blocks: blocks.clone(),
        };
        let storage = MemoryIndexStorage::new();
        let mut index = Index::new(client, storage, None).unwrap();

        index.add_bag([1; 32]).unwrap();
        index.add_bag([2; 32]).unwrap();
        index.add_bag([3; 32]).unwrap();

        *blocks.borrow_mut() = blocks_chain_1.clone();
        index.check_reorgs().unwrap();

        assert_eq!(index.current_height, 2);
        assert_eq!(index.bids_storage.get_blocks_count().unwrap(), 2);

        *blocks.borrow_mut() = blocks_chain_2.clone();
        index.check_reorgs().unwrap();

        assert_eq!(index.current_height, 1);
        assert_eq!(index.bids_storage.get_blocks_count().unwrap(), 1);

        let txs_in_index = index
            .bids_storage
            .get_records_by_block_hash(&forked_block.block_hash)
            .unwrap();
        let forked_block_tx_out = &txs_in_index[0];
        assert_eq!(forked_block_tx_out.proof.tx.bag_id, [1; 32]);
    }
}
