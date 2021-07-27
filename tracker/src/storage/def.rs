use crate::record::Record;
use bitcoin::BlockHash;
use std::error::Error;

pub trait IndexStorage {
    type Err: Error;
    fn store_record(&self, record: Record) -> Result<(), Self::Err>;
    fn get_blocks_count(&self) -> Result<u64, Self::Err>;
    fn remove_with_block_hash(&self, hash: &BlockHash) -> Result<(), Self::Err>;
    fn get_blocks_by_hash(&self, hash: &BlockHash) -> Result<Vec<Record>, Self::Err>;
}
