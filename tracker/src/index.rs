use crate::bitcoin_client::BitcoinClient;
use crate::record::{Record, RecordData};
use crate::storage::IndexStorage;
use bitcoin::blockdata::opcodes;
use bitcoin::blockdata::script::Instruction;
use bitcoin::{BlockHash, Transaction, TxOut};
use bitcoincore_rpc::json::GetBlockHeaderResult;
use std::cmp::Ordering;
use std::convert::TryInto;

pub type BagId = [u8; 32];

pub struct Index<C: BitcoinClient, S: IndexStorage> {
    btc_client: C,
    btc_height: u64,
    btc_checked_chain: Vec<BlockHash>,
    storage: S,
    bags: Vec<BagId>,
}

impl<C: BitcoinClient, S: IndexStorage> Index<C, S> {
    pub fn new(client: C, storage: S, base_height: Option<u64>) -> Self {
        let info = client.get_blockchain_info().unwrap();
        let height = base_height.unwrap_or(info.blocks);
        let checked_chain = Vec::new();
        let bags = vec![];
        Index {
            btc_client: client,
            btc_height: height,
            btc_checked_chain: checked_chain,
            storage,
            bags,
        }
    }

    pub fn checked_btc_height(&self) -> u64 {
        self.btc_height
    }

    pub fn add_bag(&mut self, bag: BagId) {
        self.bags.push(bag);
    }

    // TODO: better API for indexing
    pub fn get_storage(&self) -> &S {
        &self.storage
    }
}

impl<C: BitcoinClient, S: IndexStorage> Index<C, S> {
    pub fn check_last_btc_blocks(&mut self) {
        let new_btc_info = self.btc_client.get_blockchain_info().unwrap();
        let new_height = new_btc_info.blocks;
        match self.btc_height.cmp(&new_height) {
            Ordering::Equal => return,
            Ordering::Greater => {
                unimplemented!("TODO: is it possible that chain will be less than previous?")
            }
            Ordering::Less => self.check_btc_blocks_from(new_height),
        }
    }

    fn check_btc_blocks_from(&mut self, new_height: u64) {
        let reorg_info = self.check_btc_for_reorgs();

        let old_height = match reorg_info {
            Some(reorg_info) => self.remove_btc_blocks_when_fork(reorg_info),
            None => self.btc_height,
        };
        self.add_btc_blocks(old_height, new_height);
        self.btc_height = new_height;
    }

    fn remove_btc_blocks_when_fork(&mut self, reorg_info: ReorgInfo) -> u64 {
        for discarded_block in reorg_info.discarded_blocks.iter() {
            self.storage
                .remove_with_block_hash(discarded_block)
                .unwrap();
        }
        let fork_position_in_checked_chain =
            self.btc_checked_chain.len() - reorg_info.discarded_blocks.len();
        self.btc_checked_chain
            .drain(..fork_position_in_checked_chain);
        reorg_info.height_when_fork
    }

    fn add_btc_blocks(&mut self, old_height: u64, new_height: u64) {
        for index in old_height + 1..new_height + 1 {
            let hash = self.btc_client.get_block_hash(index).unwrap();
            self.add_btc_block_to_index(hash).unwrap();
        }
    }

    fn check_btc_for_reorgs(&self) -> Option<ReorgInfo> {
        let tip = self.btc_checked_chain.last()?;
        let last_tip_block = self.btc_client.get_block_header_info(tip).unwrap();
        if is_block_in_main_chain(&last_tip_block) {
            return None;
        }
        let mut discarded_blocks = vec![tip.clone()];
        for block_hash in self.btc_checked_chain.iter().rev().skip(1) {
            let block_header_info = self.btc_client.get_block_header_info(&block_hash).unwrap();
            if is_block_in_main_chain(&block_header_info) {
                break;
            } else {
                discarded_blocks.push(block_header_info.hash);
            }
        }
        let height_when_fork = self.btc_height - discarded_blocks.len() as u64;
        Some(ReorgInfo {
            height_when_fork,
            discarded_blocks,
        })
    }

    fn add_btc_block_to_index(&mut self, block_hash: BlockHash) -> Result<(), S::Err> {
        let transactions = self.check_btc_block_with_hash(block_hash.clone());
        transactions
            .into_iter()
            .map(|tx| self.storage.store_record(tx))
            .collect::<Result<Vec<()>, S::Err>>()?;
        self.btc_checked_chain.push(block_hash);
        Ok(())
    }

    fn check_btc_block_with_hash(&self, hash: BlockHash) -> Vec<Record> {
        let block = self.btc_client.get_block(&hash).unwrap();
        let txs = block.txdata;

        let mint_txs = txs
            .into_iter()
            .filter_map(|tx| self.parse_mint_transaction_btc_block(hash, tx))
            .collect();

        mint_txs
    }
}

impl<C: BitcoinClient, S: IndexStorage> Index<C, S> {
    fn parse_mint_transaction_btc_block(
        &self,
        block: BlockHash,
        tx: Transaction,
    ) -> Option<Record> {
        let txid = tx.txid();
        tx.output
            .iter()
            .enumerate()
            .filter_map(|(out_pos, out)| {
                self.parse_mint_btc_output(out).map(|data| Record {
                    bitcoin_block: block,
                    bitcoin_tx_id: txid.clone(),
                    bitcoin_output_position: out_pos as u64,
                    data,
                })
            })
            .next()
    }

    fn parse_mint_btc_output(&self, out: &TxOut) -> Option<RecordData> {
        let mut instructions = out.script_pubkey.instructions();

        let first_instruction = instructions.next().and_then(|res| res.ok())?;
        assert_instruction_return(first_instruction)?;

        let push_bytes_instr = instructions.next().and_then(|res| res.ok())?;
        let bag_id = parse_push_32_bytes(push_bytes_instr)?;

        if !self.bags.iter().any(|bag| *bag == bag_id) {
            return None;
        }
        let amount = out.value;
        Some(RecordData { bag_id, amount })
    }
}

fn assert_instruction_return(instr: Instruction) -> Option<()> {
    match instr {
        Instruction::Op(opcodes::all::OP_RETURN) => Some(()),
        _ => None,
    }
}

fn parse_push_32_bytes(instr: Instruction) -> Option<[u8; 32]> {
    let bytes = match instr {
        Instruction::PushBytes(bytes) => bytes,
        _ => return None,
    };
    let array = TryInto::<&[u8; 32]>::try_into(bytes).ok()?.clone();
    Some(array)
}

fn is_block_in_main_chain(block: &GetBlockHeaderResult) -> bool {
    block.confirmations != -1
}

struct ReorgInfo {
    height_when_fork: u64,
    discarded_blocks: Vec<BlockHash>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::memory::MemoryIndexStorage;
    use crate::test_utils::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    impl<C: BitcoinClient, S: IndexStorage> Index<C, S> {
        fn tip(&self) -> BlockHash {
            self.btc_checked_chain.last().unwrap().clone()
        }
    }

    #[test]
    fn test_new_blocks() {
        let initial_block = create_test_block(0, [1]);
        let block2 = create_test_block(1, [2]);

        let blocks = Rc::new(RefCell::new(vec![initial_block.clone()]));
        let client = TestBitcoinClient {
            blocks: blocks.clone(),
        };
        let storage = MemoryIndexStorage::new();
        let mut index = Index::new(client, storage, None);

        assert_eq!(index.btc_height, 0);
        assert_eq!(index.btc_checked_chain, vec![]);

        blocks.borrow_mut().push(block2.clone());
        index.check_last_btc_blocks();

        assert_eq!(index.btc_height, 1);
        assert_eq!(index.btc_checked_chain, vec![block2.block_hash.clone()]);
        assert_eq!(index.tip(), block2.block_hash.clone());
        assert_eq!(index.storage.get_blocks_count().unwrap(), 0);
    }

    #[test]
    fn test_new_blocks_with_mint_txs() {
        let initial_block = create_test_block(0, [1]);
        let block2 = create_test_block_with_mint_tx(1, [2], [1; 32]);

        let blocks = Rc::new(RefCell::new(vec![initial_block.clone()]));
        let client = TestBitcoinClient {
            blocks: blocks.clone(),
        };
        let storage = MemoryIndexStorage::new();
        let mut index = Index::new(client, storage, None);

        index.add_bag([1; 32]);

        blocks.borrow_mut().push(block2.clone());
        index.check_last_btc_blocks();

        assert_eq!(index.storage.get_blocks_count().unwrap(), 1);

        let txs_in_index = index
            .storage
            .get_blocks_by_hash(&block2.block_hash)
            .unwrap();
        let block2_tx_out = &txs_in_index[0];

        assert_eq!(block2_tx_out.data.amount, 10);
        assert_eq!(block2_tx_out.data.bag_id, [1; 32]);
    }

    #[test]
    fn test_reorg() {
        let initial_block = create_test_block(0, [1]);
        let block2 = create_test_block_with_mint_tx(1, [2], [1; 32]);
        let forked_block = create_test_block_with_mint_tx(1, [3], [2; 32]);
        let forked_block2 = create_test_block_with_mint_tx(2, [4], [3; 32]);

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
        let mut index = Index::new(client, storage, None);

        index.add_bag([1; 32]);
        index.add_bag([2; 32]);
        index.add_bag([3; 32]);

        *blocks.borrow_mut() = blocks_chain_1;
        index.check_last_btc_blocks();

        assert_eq!(index.storage.get_blocks_count().unwrap(), 1);

        *blocks.borrow_mut() = blocks_chain_2;
        index.check_last_btc_blocks();

        assert_eq!(index.storage.get_blocks_count().unwrap(), 2);

        let txs_in_index = index
            .storage
            .get_blocks_by_hash(&forked_block.block_hash)
            .unwrap();
        let forked_block_tx_out = &txs_in_index[0];
        assert_eq!(forked_block_tx_out.data.bag_id, [2; 32]);

        let txs_in_index = index
            .storage
            .get_blocks_by_hash(&forked_block2.block_hash)
            .unwrap();
        let forked_block2_tx_out = &txs_in_index[0];
        assert_eq!(forked_block2_tx_out.data.bag_id, [3; 32]);
    }
}
