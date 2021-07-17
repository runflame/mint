use bitcoincore_rpc::RpcApi;
use bitcoin::{Transaction, BlockHash, Txid, Block};
use bitcoin::blockdata::script::Instruction;
use bitcoin::blockdata::opcodes;
use std::collections::HashMap;
use std::cmp::Ordering;
use bitcoincore_rpc::json::GetBlockHeaderResult;

pub struct Index {
    client: bitcoincore_rpc::Client,
    tip: BlockHash,
    blocks: u64,
    checked_chain: Vec<BlockHash>,
    index: HashMap<BlockHash, Vec<BitcoinMintOutput>>,
}

impl Index {
    pub fn new(client: bitcoincore_rpc::Client) -> Self {
        let info = client.get_blockchain_info().unwrap();
        let tip = info.best_block_hash;
        let blocks = info.blocks;
        let checked_chain = Vec::new();
        let index = HashMap::new();
        Index { client, tip, blocks, checked_chain, index }
    }

    pub fn check_last_blocks(&mut self) {
        let new_blockchain_info = self.client.get_blockchain_info().unwrap();
        match self.blocks.cmp(&new_blockchain_info.blocks) {
            Ordering::Equal => return,
            Ordering::Greater => unimplemented!("TODO: is it possible that chain will be less than previous?"),
            Ordering::Less => {
                let reorg_info = self.check_for_reorgs();
                let old_height = match reorg_info {
                    Some(reorg_info) => {
                        for discarded_block in reorg_info.discarded_blocks {
                            self.index.remove(&discarded_block).expect("Discarded block hashes must be given from the index");
                        }
                        let fork_position_in_checked_chain = self.checked_chain.len()-reorg_info.discarded_blocks.len();
                        self.checked_chain.drain(..fork_position_in_checked_chain).collect();
                        reorg_info.height_when_fork
                    }
                    None => self.blocks
                };
                for index in old_height+1..new_blockchain_info.blocks+1 {
                    let hash = self.client.get_block_hash(index).unwrap();
                    self.add_next_block_to_index(hash);
                }
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
            }
            else {
                discarded_blocks.push(block_header_info.hash);
            }
        }
        let height_when_fork = self.blocks - discarded_blocks.len() as u64;
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
    }

    fn check_bitcoin_block_with_id(&self, id: u64) -> Vec<BitcoinMintOutput> {
        let hash = self.client.get_block_hash(id).unwrap();
        self.check_bitcoin_block_with_hash(hash)
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

pub fn parse_mint_transaction(tx: Transaction) -> Option<BitcoinMintOutput> {
    let txid = tx.txid();
    tx.output.iter().enumerate().filter_map(|(out_pos, out)| {
        let mut instructions = out.script_pubkey.instructions();
        let first_instruction = instructions.next().and_then(|res| res.ok());
        match first_instruction {
            Some(Instruction::Op(opcodes::all::OP_RETURN)) => {
                let push_bytes_instr = instructions.next().and_then(|res| res.ok());
                let bytes = match push_bytes_instr {
                    Some(Instruction::PushBytes(bytes)) => bytes,
                    _ => return None
                };
                let amount = out.value;
                let bytes = Box::<[u8]>::from(bytes);
                Some(BitcoinMintOutput {
                    index: BitcoinMintOutputIndex {
                        txid,
                        output_position: out_pos as u64,
                    },
                    amount,
                    bytes
                })
            }
            _ => { None }
        }
    }).next()
}

pub struct BitcoinMintOutput {
    index: BitcoinMintOutputIndex,
    amount: u64,
    bytes: Box<[u8]>
}

pub struct BitcoinMintOutputIndex {
    txid: Txid,
    output_position: u64,
}
