use crate::bitcoin_client::BitcoinClient;
use bitcoin::blockdata::{opcodes, script};
use bitcoin::hashes::Hash;
use bitcoin::{Block, BlockHash, BlockHeader, Transaction, TxOut, Txid};
use bitcoincore_rpc::bitcoincore_rpc_json::{FundRawTransactionResult, SignRawTransactionResult};
use bitcoincore_rpc::json::GetBlockHeaderResult;
use bitcoincore_rpc::json::GetBlockchainInfoResult;
use bitcoincore_rpc::RawTx;
use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Clone)]
pub struct TestBlock {
    pub height: u64,
    pub block_hash: BlockHash,
    pub in_main_chain: bool,
    pub txs: Vec<Transaction>,
}

pub struct TestBitcoinClient {
    pub blocks: Rc<RefCell<Vec<TestBlock>>>,
}

impl BitcoinClient for TestBitcoinClient {
    type Err = Infallible;

    fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult, Self::Err> {
        let blocks = self.blocks.borrow();
        Ok(GetBlockchainInfoResult {
            chain: "rusttest".to_string(),
            blocks: blocks.last().unwrap().height,
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

    fn get_block_header_info(&self, hash: &BlockHash) -> Result<GetBlockHeaderResult, Self::Err> {
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

    fn fund_raw_transaction<R: RawTx>(
        &self,
        _tx: R,
    ) -> Result<FundRawTransactionResult, Self::Err> {
        unimplemented!()
    }

    fn sign_raw_transaction_with_wallet<R: RawTx>(
        &self,
        _tx: R,
    ) -> Result<SignRawTransactionResult, Self::Err> {
        unimplemented!()
    }

    fn send_raw_transaction<R: RawTx>(&self, _tx: R) -> Result<Txid, Self::Err> {
        unimplemented!()
    }
}

pub fn create_test_block(height: u64, data: impl AsRef<[u8]>) -> TestBlock {
    use bitcoin::hashes::sha256d;

    TestBlock {
        height,
        block_hash: BlockHash::from_hash(sha256d::Hash::hash(data.as_ref())),
        in_main_chain: true,
        txs: vec![],
    }
}

pub fn create_test_block_with_mint_tx(
    height: u64,
    data: impl AsRef<[u8]>,
    tx_data: impl AsRef<[u8]>,
) -> TestBlock {
    use bitcoin::hashes::sha256d;

    TestBlock {
        height,
        block_hash: BlockHash::from_hash(sha256d::Hash::hash(data.as_ref())),
        in_main_chain: true,
        txs: vec![create_test_mint_transaction(tx_data)],
    }
}

pub fn create_test_mint_transaction(tx_data: impl AsRef<[u8]>) -> Transaction {
    Transaction {
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
    }
}
