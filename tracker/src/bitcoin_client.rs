use crate::index::BagId;
use crate::record::{BidTx, Outpoint};
use bitcoin::blockdata::script;
use bitcoin::consensus::Encodable;
use bitcoin::{Block, BlockHash, Transaction, TxOut, Txid, WScriptHash};
use bitcoincore_rpc::json::{
    FundRawTransactionResult, GetBlockHeaderResult, GetBlockchainInfoResult,
    SignRawTransactionResult,
};
use bitcoincore_rpc::{RawTx, RpcApi};
use std::error::Error;

/// Trait is used to abstract from the concrete implementation of a client.
pub trait BitcoinClient {
    type Err: Error;
    fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult, Self::Err>;
    fn get_block_hash(&self, height: u64) -> Result<BlockHash, Self::Err>;
    fn get_block_header_info(&self, hash: &BlockHash) -> Result<GetBlockHeaderResult, Self::Err>;
    fn get_block(&self, hash: &BlockHash) -> Result<Block, Self::Err>;
    fn fund_raw_transaction<R: RawTx>(&self, tx: R) -> Result<FundRawTransactionResult, Self::Err>;
    fn sign_raw_transaction_with_wallet<R: RawTx>(
        &self,
        tx: R,
    ) -> Result<SignRawTransactionResult, Self::Err>;
    fn send_raw_transaction<R: RawTx>(&self, tx: R) -> Result<Txid, Self::Err>;
}

impl BitcoinClient for bitcoincore_rpc::Client {
    type Err = bitcoincore_rpc::Error;

    fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult, Self::Err> {
        RpcApi::get_blockchain_info(self)
    }

    fn get_block_hash(&self, height: u64) -> Result<BlockHash, Self::Err> {
        RpcApi::get_block_hash(self, height)
    }

    fn get_block_header_info(&self, hash: &BlockHash) -> Result<GetBlockHeaderResult, Self::Err> {
        RpcApi::get_block_header_info(self, hash)
    }

    fn get_block(&self, hash: &BlockHash) -> Result<Block, Self::Err> {
        RpcApi::get_block(self, hash)
    }

    fn fund_raw_transaction<R: RawTx>(&self, tx: R) -> Result<FundRawTransactionResult, Self::Err> {
        RpcApi::fund_raw_transaction(self, tx, None, None)
    }

    fn sign_raw_transaction_with_wallet<R: RawTx>(
        &self,
        tx: R,
    ) -> Result<SignRawTransactionResult, Self::Err> {
        RpcApi::sign_raw_transaction_with_wallet(self, tx, None, None)
    }

    fn send_raw_transaction<R: RawTx>(&self, tx: R) -> Result<Txid, Self::Err> {
        RpcApi::send_raw_transaction(self, tx)
    }
}

pub trait BitcoinMintExt: BitcoinClient {
    fn send_mint_transaction(&self, satoshies: u64, bag_id: &BagId) -> Result<BidTx, Self::Err> {
        use bitcoin::hashes::sha256;
        use bitcoin::hashes::Hash;

        let hash = sha256::Hash::from_slice(bag_id).expect("Bag id has 32 bytes, as sha256");
        let tx = Transaction {
            version: 2,
            lock_time: 0,
            input: vec![],
            output: vec![TxOut {
                value: satoshies,
                script_pubkey: script::Script::new_v0_wsh(&WScriptHash::from_hash(hash)),
            }],
        };

        let mut bytes = Vec::new();
        consensus_encode_tx(&tx, &mut bytes)
            .expect("We write to the vector so it cannot return error");

        let funded = self.fund_raw_transaction(&bytes)?;
        let signed = self.sign_raw_transaction_with_wallet(&funded.hex)?;

        let txid = self.send_raw_transaction(&signed.hex)?;

        let raw_signed_tx = signed.transaction().expect("Bitcoin node accept it");
        let out_pos = find_out_pos_mint_tx(&raw_signed_tx, bag_id);

        Ok(BidTx::new(Outpoint::new(txid, out_pos), bag_id.clone()))
    }
}

// Bitcoin node does not guarantee that change of outputs does not change, so we must find bid
// output from signed transaction
fn find_out_pos_mint_tx(tx: &Transaction, bag: &BagId) -> u64 {
    tx.output
        .iter()
        .enumerate()
        .find_map(|(i, out)| match out.script_pubkey.is_v0_p2wsh() {
            true => {
                if &out.script_pubkey.as_bytes()[2..34] == bag {
                    Some(i as u64)
                } else {
                    None
                }
            }
            false => None,
        })
        .unwrap()
}

impl<T: BitcoinClient> BitcoinMintExt for T {}

// `bitcoin` crate provide uparseable output with it's `consensus_encode` method for transactions
// with zero inputs.
fn consensus_encode_tx<S: std::io::Write>(
    tx: &Transaction,
    mut s: S,
) -> Result<usize, std::io::Error> {
    let mut len = 0;
    len += tx.version.consensus_encode(&mut s)?;
    len += tx.input.consensus_encode(&mut s)?;
    len += tx.output.consensus_encode(&mut s)?;
    len += tx.lock_time.consensus_encode(s)?;
    Ok(len)
}
