use crate::index::BagId;
use crate::record::BidEntry;
use bitcoin::BlockHash;
use std::error::Error;
use thiserror::Error;

pub trait BidStorage {
    type Err: Error;

    fn insert_bid(&self, record: BidEntry) -> Result<(), BidStorageError<Self::Err>>;
    fn insert_unconfirmed_bag(&self, bag: BagId) -> Result<(), BidStorageError<Self::Err>>;
    fn update_bid(&self, record: BidEntry) -> Result<(), BidStorageError<Self::Err>>;

    fn remove_with_block_hash(&self, hash: &BlockHash) -> Result<(), BidStorageError<Self::Err>>;
    fn remove_bag(&self, bag: &BagId) -> Result<(), BidStorageError<Self::Err>>;

    fn get_blocks_count(&self) -> Result<u64, BidStorageError<Self::Err>>;
    fn get_records_by_block_hash(
        &self,
        hash: &BlockHash,
    ) -> Result<Vec<BidEntry>, BidStorageError<Self::Err>>;

    fn is_bag_exists(&self, bag: &BagId) -> Result<bool, BidStorageError<Self::Err>>;
    fn is_bag_confirmed(&self, bag: &BagId) -> Result<bool, BidStorageError<Self::Err>>;
}

#[derive(Debug, Error)]
pub enum BidStorageError<T: Error> {
    #[error("Bag with id TODO does not exists.")]
    BagDoesNotExists(BagId),

    #[error(transparent)]
    Other(#[from] T),
}
