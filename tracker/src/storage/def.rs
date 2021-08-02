use crate::index::BagId;
use crate::record::Record;
use bitcoin::BlockHash;
use std::error::Error;

pub trait IndexStorage {
    type Err: Error;
    fn store_record(&self, record: Record) -> Result<(), Self::Err>;
    fn get_blocks_count(&self) -> Result<u64, Self::Err>;
    fn remove_with_block_hash(&self, hash: &BlockHash) -> Result<(), Self::Err>;
    fn get_records_by_block_hash(&self, hash: &BlockHash) -> Result<Vec<Record>, Self::Err>;
    fn remove_records_with_bag(&self, bag: &BagId) -> Result<(), Self::Err>;
}
