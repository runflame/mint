use crate::index::BagId;
use crate::record::BidEntry;
use bitcoin::BlockHash;
use std::error::Error;

pub trait BidStorage {
    type Err: Error;
    fn store_record(&self, record: BidEntry) -> Result<(), Self::Err>;
    fn get_blocks_count(&self) -> Result<u64, Self::Err>;
    fn remove_with_block_hash(&self, hash: &BlockHash) -> Result<(), Self::Err>;
    fn get_records_by_block_hash(&self, hash: &BlockHash) -> Result<Vec<BidEntry>, Self::Err>;
    fn remove_records_with_bag(&self, bag: &BagId) -> Result<(), Self::Err>;
}
