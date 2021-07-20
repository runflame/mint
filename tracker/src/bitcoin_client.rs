use bitcoin::{Block, BlockHash};
use bitcoincore_rpc::json::{GetBlockHeaderResult, GetBlockchainInfoResult};
use std::error::Error;

/// Trait is used to abstract from the concrete implementation of a client.
pub trait BitcoinClient {
    type Err: Error;
    fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult, Self::Err>;
    fn get_block_hash(&self, height: u64) -> Result<BlockHash, Self::Err>;
    fn get_block_header_info(&self, hash: &BlockHash) -> Result<GetBlockHeaderResult, Self::Err>;
    fn get_block(&self, hash: &BlockHash) -> Result<Block, Self::Err>;
}

impl BitcoinClient for bitcoincore_rpc::Client {
    type Err = bitcoincore_rpc::Error;

    fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult, Self::Err> {
        self.get_blockchain_info()
    }

    fn get_block_hash(&self, height: u64) -> Result<BlockHash, Self::Err> {
        self.get_block_hash(height)
    }

    fn get_block_header_info(&self, hash: &BlockHash) -> Result<GetBlockHeaderResult, Self::Err> {
        self.get_block_header_info(hash)
    }

    fn get_block(&self, hash: &BlockHash) -> Result<Block, Self::Err> {
        self.get_block(hash)
    }
}
