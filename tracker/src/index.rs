use crate::bitcoin_client::BitcoinClient;
use bitcoin::blockdata::opcodes;
use bitcoin::blockdata::script::Instruction;
use bitcoin::{Block, BlockHash, Transaction, Txid};
use bitcoincore_rpc::json::GetBlockHeaderResult;
use std::cmp::Ordering;
use std::collections::HashMap;

pub struct Index<C: BitcoinClient> {
    client: C,
    tip: BlockHash,
    height: u64,
    checked_chain: Vec<BlockHash>,
    index: HashMap<BlockHash, Vec<BitcoinMintOutput>>,
}

impl<C: BitcoinClient> Index<C> {
    pub fn new(client: C) -> Self {
        let info = client.get_blockchain_info().unwrap();
        let tip = info.best_block_hash;
        let height = info.blocks - 1;
        let checked_chain = Vec::new();
        let index = HashMap::new();
        Index {
            client,
            tip,
            height,
            checked_chain,
            index,
        }
    }

    pub fn check_last_blocks(&mut self) {
        let new_blockchain_info = self.client.get_blockchain_info().unwrap();
        let new_height = new_blockchain_info.blocks - 1;
        match self.height.cmp(&new_height) {
            Ordering::Equal => return,
            Ordering::Greater => {
                unimplemented!("TODO: is it possible that chain will be less than previous?")
            }
            Ordering::Less => {
                let reorg_info = self.check_for_reorgs();
                let old_height = match reorg_info {
                    Some(reorg_info) => {
                        for discarded_block in reorg_info.discarded_blocks.iter() {
                            self.index
                                .remove(discarded_block)
                                .expect("Discarded block hashes must be given from the index");
                        }
                        let fork_position_in_checked_chain =
                            self.checked_chain.len() - reorg_info.discarded_blocks.len();
                        self.checked_chain
                            .drain(..fork_position_in_checked_chain)
                            .for_each(drop);
                        reorg_info.height_when_fork
                    }
                    None => self.height,
                };
                for index in old_height + 1..new_height + 1 {
                    let hash = self.client.get_block_hash(index).unwrap();
                    self.add_next_block_to_index(hash);
                }
                self.height = new_height;
                self.tip = new_blockchain_info.best_block_hash;
            }
        }
    }

    fn check_for_reorgs(&self) -> Option<ReorgInfo> {
        let last_tip_block = self.client.get_block_header_info(&self.tip).unwrap();
        if is_block_in_main_chain(&last_tip_block) {
            return None;
        }
        let mut discarded_blocks = vec![self.tip];
        for block_hash in self.checked_chain.iter().rev().skip(1) {
            let block_header_info = self.client.get_block_header_info(&block_hash).unwrap();
            if is_block_in_main_chain(&block_header_info) {
                break;
            } else {
                discarded_blocks.push(block_header_info.hash);
            }
        }
        let height_when_fork = self.height - discarded_blocks.len() as u64;
        Some(ReorgInfo {
            height_when_fork,
            discarded_blocks,
        })
    }

    fn add_next_block_to_index(&mut self, block_hash: BlockHash) {
        let transactions = self.check_bitcoin_block_with_hash(block_hash.clone());
        if transactions.len() != 0 {
            self.index.insert(block_hash, transactions);
        }
        self.checked_chain.push(block_hash);
    }

    fn check_bitcoin_block_with_hash(&self, hash: BlockHash) -> Vec<BitcoinMintOutput> {
        let block = self.client.get_block(&hash).unwrap();
        let txs = block.txdata;

        let mint_txs = txs.into_iter().filter_map(parse_mint_transaction).collect();

        mint_txs
    }
}

fn is_block_in_main_chain(block: &GetBlockHeaderResult) -> bool {
    block.confirmations != -1
}

struct ReorgInfo {
    height_when_fork: u64,
    discarded_blocks: Vec<BlockHash>,
}

fn parse_mint_transaction(tx: Transaction) -> Option<BitcoinMintOutput> {
    let txid = tx.txid();
    tx.output
        .iter()
        .enumerate()
        .filter_map(|(out_pos, out)| {
            let mut instructions = out.script_pubkey.instructions();
            let first_instruction = instructions.next().and_then(|res| res.ok());
            match first_instruction {
                Some(Instruction::Op(opcodes::all::OP_RETURN)) => {
                    let push_bytes_instr = instructions.next().and_then(|res| res.ok());
                    let bytes = match push_bytes_instr {
                        Some(Instruction::PushBytes(bytes)) => bytes,
                        _ => return None,
                    };
                    let amount = out.value;
                    let bytes = Box::<[u8]>::from(bytes);
                    Some(BitcoinMintOutput {
                        index: BitcoinMintOutputIndex {
                            txid,
                            output_position: out_pos as u64,
                        },
                        amount,
                        bytes,
                    })
                }
                _ => None,
            }
        })
        .next()
}

pub struct BitcoinMintOutput {
    index: BitcoinMintOutputIndex,
    amount: u64,
    bytes: Box<[u8]>,
}

pub struct BitcoinMintOutputIndex {
    txid: Txid,
    output_position: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::blockdata::script;
    use bitcoin::hashes::sha256d;
    use bitcoin::hashes::Hash;
    use bitcoin::{BlockHeader, TxOut};
    use bitcoincore_rpc::bitcoincore_rpc_json::GetBlockchainInfoResult;
    use std::cell::RefCell;
    use std::convert::Infallible;
    use std::rc::Rc;

    #[derive(Clone)]
    struct TestBlock {
        height: u64,
        block_hash: BlockHash,
        in_main_chain: bool,
        txs: Vec<Transaction>,
    }

    struct TestBitcoinClient {
        blocks: Rc<RefCell<Vec<TestBlock>>>,
    }

    impl BitcoinClient for TestBitcoinClient {
        type Err = Infallible;

        fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult, Self::Err> {
            let blocks = self.blocks.borrow();
            Ok(GetBlockchainInfoResult {
                chain: "rusttest".to_string(),
                blocks: blocks.last().unwrap().height + 1,
                headers: 0,
                best_block_hash: blocks.last().unwrap().block_hash.clone(),
                difficulty: 0.0,
                median_time: 0,
                verification_progress: 0.0,
                initial_block_download: false,
                chain_work: vec![],
                size_on_disk: 0,
                pruned: false,
                prune_height: None,
                automatic_pruning: None,
                prune_target_size: None,
                softforks: Default::default(),
                warnings: "".to_string(),
            })
        }

        fn get_block_hash(&self, height: u64) -> Result<BlockHash, Self::Err> {
            Ok(self
                .blocks
                .borrow()
                .iter()
                .find(|block| block.height == height && block.in_main_chain)
                .unwrap()
                .block_hash)
        }

        fn get_block_header_info(
            &self,
            hash: &BlockHash,
        ) -> Result<GetBlockHeaderResult, Self::Err> {
            let blocks = self.blocks.borrow();
            let index = blocks
                .iter()
                .position(|block| block.block_hash == *hash)
                .unwrap();
            let block = &blocks[index];
            Ok(GetBlockHeaderResult {
                hash: block.block_hash.clone(),
                confirmations: if block.in_main_chain { 1 } else { -1 },
                height: block.height as usize,
                version: 0,
                version_hex: None,
                merkle_root: Default::default(),
                time: 0,
                median_time: None,
                nonce: 0,
                bits: "".to_string(),
                difficulty: 0.0,
                chainwork: vec![],
                n_tx: 0,
                previous_block_hash: None,
                next_block_hash: None,
            })
        }

        fn get_block(&self, hash: &BlockHash) -> Result<Block, Self::Err> {
            let blocks = self.blocks.borrow();
            let index = blocks
                .iter()
                .position(|block| block.block_hash == *hash)
                .unwrap();
            let block = &blocks[index];
            Ok(Block {
                header: BlockHeader {
                    version: 0,
                    prev_blockhash: Default::default(),
                    merkle_root: Default::default(),
                    time: 0,
                    bits: 0,
                    nonce: 0,
                },
                txdata: block.txs.clone(),
            })
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
        let mut index = Index::new(client);

        assert_eq!(index.height, 0);
        assert_eq!(index.checked_chain, vec![]);
        assert_eq!(index.tip, initial_block.block_hash);

        blocks.borrow_mut().push(block2.clone());
        index.check_last_blocks();

        assert_eq!(index.height, 1);
        assert_eq!(index.checked_chain, vec![block2.block_hash.clone()]);
        assert_eq!(index.tip, block2.block_hash.clone());
        assert_eq!(index.index.len(), 0);
    }

    #[test]
    fn test_new_blocks_with_mint_txs() {
        let initial_block = create_test_block(0, [1]);
        let block2 = create_test_block_with_mint_tx(1, [2], [1, 2, 3, 4]);

        let blocks = Rc::new(RefCell::new(vec![initial_block.clone()]));
        let client = TestBitcoinClient {
            blocks: blocks.clone(),
        };
        let mut index = Index::new(client);

        blocks.borrow_mut().push(block2.clone());
        index.check_last_blocks();

        assert_eq!(index.index.len(), 1);
        let txs_in_index = index.index.get(&block2.block_hash).unwrap();
        let block2_tx_out = &txs_in_index[0];
        assert_eq!(block2_tx_out.amount, 10);
        assert_eq!(block2_tx_out.bytes, (&[1u8, 2, 3, 4] as &[_]).into());
    }

    #[test]
    fn test_reorg() {
        let initial_block = create_test_block(0, [1]);
        let block2 = create_test_block_with_mint_tx(1, [2], [1, 2, 3, 4]);
        let forked_block = create_test_block_with_mint_tx(1, [3], [6, 7, 8, 9]);
        let forked_block2 = create_test_block_with_mint_tx(2, [4], [10]);

        let blocks = Rc::new(RefCell::new(vec![initial_block.clone()]));
        let client = TestBitcoinClient {
            blocks: blocks.clone(),
        };
        let mut index = Index::new(client);

        blocks.borrow_mut().push(block2.clone());
        index.check_last_blocks();

        assert_eq!(index.index.len(), 1);

        *blocks.borrow_mut() = vec![
            initial_block.clone(), // first block in both chains
            TestBlock {
                in_main_chain: false,
                ..block2
            }, // was in the main chain, after reorg is not
            forked_block.clone(),
            forked_block2.clone(),
        ];

        index.check_last_blocks();

        assert_eq!(index.index.len(), 2);

        let txs_in_index = index.index.get(&forked_block.block_hash).unwrap();
        let forked_block_tx_out = &txs_in_index[0];
        assert_eq!(forked_block_tx_out.bytes, (&[6u8, 7, 8, 9] as &[_]).into());

        let txs_in_index = index.index.get(&forked_block2.block_hash).unwrap();
        let forked_block2_tx_out = &txs_in_index[0];
        assert_eq!(forked_block2_tx_out.bytes, (&[10u8] as &[_]).into());
    }

    fn create_test_block(height: u64, data: impl AsRef<[u8]>) -> TestBlock {
        TestBlock {
            height,
            block_hash: BlockHash::from_hash(sha256d::Hash::hash(data.as_ref())),
            in_main_chain: true,
            txs: vec![],
        }
    }

    fn create_test_block_with_mint_tx(
        height: u64,
        data: impl AsRef<[u8]>,
        tx_data: impl AsRef<[u8]>,
    ) -> TestBlock {
        TestBlock {
            height,
            block_hash: BlockHash::from_hash(sha256d::Hash::hash(data.as_ref())),
            in_main_chain: true,
            txs: vec![Transaction {
                version: 0,
                lock_time: 0,
                input: vec![],
                output: vec![TxOut {
                    value: 10,
                    script_pubkey: script::Builder::new()
                        .push_opcode(opcodes::all::OP_RETURN)
                        .push_slice(tx_data.as_ref())
                        .into_script(),
                }],
            }],
        }
    }
}
