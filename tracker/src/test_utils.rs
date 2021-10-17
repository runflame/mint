use crate::bitcoin_client::BitcoinClient;
use crate::record::{BagProof, BidTx, Outpoint};
use bitcoin::blockdata::script;
use bitcoin::hashes::{sha256, Hash};
use bitcoin::{Block, BlockHash, BlockHeader, Transaction, TxOut, Txid, WScriptHash};
use bitcoincore_rpc::bitcoincore_rpc_json::{FundRawTransactionResult, SignRawTransactionResult};
use bitcoincore_rpc::json::GetBlockHeaderResult;
use bitcoincore_rpc::json::GetBlockchainInfoResult;
use bitcoincore_rpc::RawTx;
use std::cell::RefCell;
use std::convert::{Infallible, TryInto};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct TestBlock {
    pub height: u64,
    pub block_hash: BlockHash,
    pub in_main_chain: bool,
    pub txs: Vec<Transaction>,
    pub prev: Option<BlockHash>,
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
            previous_block_hash: block.prev,
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

pub fn create_test_block(
    height: u64,
    data: impl AsRef<[u8]>,
    prev: Option<BlockHash>,
) -> TestBlock {
    use bitcoin::hashes::sha256d;

    TestBlock {
        height,
        block_hash: BlockHash::from_hash(sha256d::Hash::hash(data.as_ref())),
        in_main_chain: true,
        txs: vec![],
        prev,
    }
}

pub fn create_test_block_with_mint_tx(
    height: u64,
    data: impl AsRef<[u8]>,
    prev: Option<BlockHash>,
    tx_data: impl AsRef<[u8]>,
) -> (TestBlock, BagProof) {
    use bitcoin::hashes::sha256d;

    let (tx, bid_tx) = create_test_mint_transaction(tx_data);
    let block = TestBlock {
        height,
        block_hash: BlockHash::from_hash(sha256d::Hash::hash(data.as_ref())),
        in_main_chain: true,
        txs: vec![tx],
        prev,
    };
    let prf = BagProof::new(block.block_hash, bid_tx);
    (block, prf)
}

pub fn create_test_mint_transaction(tx_data: impl AsRef<[u8]>) -> (Transaction, BidTx) {
    let tx = Transaction {
        version: 0,
        lock_time: 0,
        input: vec![],
        output: vec![TxOut {
            value: 10,
            script_pubkey: script::Script::new_v0_wsh(&WScriptHash::from_hash(
                sha256::Hash::from_slice(tx_data.as_ref()).unwrap(),
            )),
        }],
    };
    let bid_tx = BidTx::new(
        Outpoint::new(tx.txid(), 0),
        tx_data.as_ref().try_into().unwrap(),
    );
    (tx, bid_tx)
}
