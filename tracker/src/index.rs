use crate::bitcoin_client::BitcoinClient;
use bitcoin::blockdata::opcodes;
use bitcoin::blockdata::script::Instruction;
use bitcoin::{BlockHash, Transaction, TxOut, Txid};
use bitcoincore_rpc::json::GetBlockHeaderResult;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::TryInto;

type BagId = [u8; 32];

pub struct Index<C: BitcoinClient> {
    client: C,
    height: u64,
    checked_chain: Vec<BlockHash>,
    index: HashMap<BlockHash, Vec<BitcoinMintOutput>>,
    bags: Vec<BagId>,
}

impl<C: BitcoinClient> Index<C> {
    pub fn new(client: C, base_height: Option<u64>) -> Self {
        let info = client.get_blockchain_info().unwrap();
        let height = base_height.unwrap_or(info.blocks);
        let checked_chain = Vec::new();
        let index = HashMap::new();
        let bags = vec![];
        Index {
            client,
            height,
            checked_chain,
            index,
            bags,
        }
    }

    pub fn checked_height(&self) -> u64 {
        self.height
    }

    pub fn add_bag(&mut self, bag: BagId) {
        self.bags.push(bag);
    }

    pub fn check_last_blocks(&mut self) {
        let new_blockchain_info = self.client.get_blockchain_info().unwrap();
        let new_height = new_blockchain_info.blocks;
        match self.height.cmp(&new_height) {
            Ordering::Equal => return,
            Ordering::Greater => {
                unimplemented!("TODO: is it possible that chain will be less than previous?")
            }
            Ordering::Less => self.check_blocks(new_height),
        }
    }

    pub fn get_index(&self) -> &HashMap<BlockHash, Vec<BitcoinMintOutput>> {
        &self.index
    }

    fn check_blocks(&mut self, new_height: u64) {
        let reorg_info = self.check_for_reorgs();

        let old_height = match reorg_info {
            Some(reorg_info) => self.remove_blocks_when_fork(reorg_info),
            None => self.height,
        };
        self.add_blocks(old_height, new_height);
        self.height = new_height;
    }

    fn remove_blocks_when_fork(&mut self, reorg_info: ReorgInfo) -> u64 {
        for discarded_block in reorg_info.discarded_blocks.iter() {
            self.index
                .remove(discarded_block)
                .expect("Discarded block hashes must be given from the index");
        }
        let fork_position_in_checked_chain =
            self.checked_chain.len() - reorg_info.discarded_blocks.len();
        self.checked_chain.drain(..fork_position_in_checked_chain);
        reorg_info.height_when_fork
    }

    fn add_blocks(&mut self, old_height: u64, new_height: u64) {
        for index in old_height + 1..new_height + 1 {
            let hash = self.client.get_block_hash(index).unwrap();
            self.add_next_block_to_index(hash);
        }
    }

    fn check_for_reorgs(&self) -> Option<ReorgInfo> {
        let tip = self.checked_chain.last()?;
        let last_tip_block = self.client.get_block_header_info(tip).unwrap();
        if is_block_in_main_chain(&last_tip_block) {
            return None;
        }
        let mut discarded_blocks = vec![tip.clone()];
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

        let mint_txs = txs
            .into_iter()
            .filter_map(|tx| self.parse_mint_transaction(tx))
            .collect();

        mint_txs
    }

    #[cfg(test)]
    fn tip(&self) -> BlockHash {
        self.checked_chain.last().unwrap().clone()
    }

    fn parse_mint_transaction(&self, tx: Transaction) -> Option<BitcoinMintOutput> {
        let txid = tx.txid();
        tx.output
            .iter()
            .enumerate()
            .filter_map(|(out_pos, out)| self.parse_mint_output(txid, out_pos as u64, out))
            .next()
    }

    fn parse_mint_output(
        &self,
        txid: Txid,
        out_pos: u64,
        out: &TxOut,
    ) -> Option<BitcoinMintOutput> {
        let mut instructions = out.script_pubkey.instructions();

        let first_instruction = instructions.next().and_then(|res| res.ok())?;
        assert_instruction_return(first_instruction)?;

        let push_bytes_instr = instructions.next().and_then(|res| res.ok())?;
        let bag_id = parse_push_32_bytes(push_bytes_instr)?;

        if !self.bags.iter().any(|bag| *bag == bag_id) {
            return None;
        }
        let amount = out.value;
        Some(BitcoinMintOutput {
            index: BitcoinMintOutputIndex {
                txid,
                output_position: out_pos,
            },
            amount,
            bag_id,
        })
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

#[derive(Debug)]
pub struct BitcoinMintOutput {
    pub index: BitcoinMintOutputIndex,
    pub amount: u64,
    pub bag_id: BagId,
}

#[derive(Debug)]
pub struct BitcoinMintOutputIndex {
    txid: Txid,
    output_position: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use bitcoin::blockdata::script;
    use bitcoin::hashes::sha256d;
    use bitcoin::hashes::Hash;
    use bitcoin::{BlockHeader, TxOut};
    use bitcoincore_rpc::bitcoincore_rpc_json::GetBlockchainInfoResult;
    use std::cell::RefCell;
    use std::convert::Infallible;
    use std::rc::Rc;

    #[test]
    fn test_new_blocks() {
        let initial_block = create_test_block(0, [1]);
        let block2 = create_test_block(1, [2]);

        let blocks = Rc::new(RefCell::new(vec![initial_block.clone()]));
        let client = TestBitcoinClient {
            blocks: blocks.clone(),
        };
        let mut index = Index::new(client, None);

        assert_eq!(index.height, 0);
        assert_eq!(index.checked_chain, vec![]);

        blocks.borrow_mut().push(block2.clone());
        index.check_last_blocks();

        assert_eq!(index.height, 1);
        assert_eq!(index.checked_chain, vec![block2.block_hash.clone()]);
        assert_eq!(index.tip(), block2.block_hash.clone());
        assert_eq!(index.index.len(), 0);
    }

    #[test]
    fn test_new_blocks_with_mint_txs() {
        let initial_block = create_test_block(0, [1]);
        let block2 = create_test_block_with_mint_tx(1, [2], [1; 32]);

        let blocks = Rc::new(RefCell::new(vec![initial_block.clone()]));
        let client = TestBitcoinClient {
            blocks: blocks.clone(),
        };
        let mut index = Index::new(client, None);

        index.add_bag([1; 32]);

        blocks.borrow_mut().push(block2.clone());
        index.check_last_blocks();

        assert_eq!(index.index.len(), 1);

        let txs_in_index = index.index.get(&block2.block_hash).unwrap();
        let block2_tx_out = &txs_in_index[0];

        assert_eq!(block2_tx_out.amount, 10);
        assert_eq!(block2_tx_out.bag_id, [1; 32]);
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

        let blocks = Rc::new(RefCell::new(vec![initial_block.clone()]));
        let client = TestBitcoinClient {
            blocks: blocks.clone(),
        };
        let mut index = Index::new(client, None);

        index.add_bag([1; 32]);
        index.add_bag([2; 32]);
        index.add_bag([3; 32]);

        *blocks.borrow_mut() = blocks_chain_1;
        index.check_last_blocks();

        assert_eq!(index.index.len(), 1);

        *blocks.borrow_mut() = blocks_chain_2;
        index.check_last_blocks();

        assert_eq!(index.index.len(), 2);

        let txs_in_index = index.index.get(&forked_block.block_hash).unwrap();
        let forked_block_tx_out = &txs_in_index[0];
        assert_eq!(forked_block_tx_out.bag_id, [2; 32]);

        let txs_in_index = index.index.get(&forked_block2.block_hash).unwrap();
        let forked_block2_tx_out = &txs_in_index[0];
        assert_eq!(forked_block2_tx_out.bag_id, [3; 32]);
    }
}
