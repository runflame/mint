use crate::bag_id::BagId;
use crate::record::BidEntry;
use bitcoin::BlockHash;
use std::error::Error;
use thiserror::Error;

/// A storage that store unconfirmed and confirmed bids.
pub trait BidStorage {
    type Err: Error;

    fn insert_bid(&self, record: BidEntry) -> Result<(), BidStorageError<Self::Err>>;
    fn insert_unconfirmed_bag(&self, bag: BagId) -> Result<(), BidStorageError<Self::Err>>;
    /// Update bid info there exists this bag id. Otherwise, return `BidStorageError::BagDoesNotExists`.
    fn update_bid(&self, record: BidEntry) -> Result<(), BidStorageError<Self::Err>>;

    /// Remove all confirmed bags with that block hash.
    fn remove_confirmation_with_block_hash(
        &self,
        hash: &BlockHash,
    ) -> Result<(), BidStorageError<Self::Err>>;
    /// Remove the bag, whether it is confirmed or unconfirmed. Return `BidStorageError::BagDoesNotExists` if bag does not exists.
    fn remove_bag(&self, bag: &BagId) -> Result<(), BidStorageError<Self::Err>>;

    /// Return count of unique blocks in the storage. It is used only in tests.
    fn get_blocks_count(&self) -> Result<u64, BidStorageError<Self::Err>>;
    /// Return bids with specified block hash. It is used only in tests.
    fn get_records_by_block_hash(
        &self,
        hash: &BlockHash,
    ) -> Result<Vec<BidEntry>, BidStorageError<Self::Err>>;

    fn is_bag_exists(&self, bag: &BagId) -> Result<bool, BidStorageError<Self::Err>>;
}

#[derive(Debug, Error)]
pub enum BidStorageError<T: Error> {
    #[error("Bag with id {0} does not exists.")]
    BagDoesNotExists(BagId),

    #[error("Bid has wrong format of record.")]
    WrongFormat,

    #[error(transparent)]
    Other(#[from] T),
}
