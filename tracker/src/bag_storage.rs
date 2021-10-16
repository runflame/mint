mod memory;

pub use memory::{BagMemoryStorage, BagMemoryStorageError};

use crate::index::BagId;
use std::error::Error;
use crate::record::{BagProof, Outpoint};

pub trait BagStorage {
    type Err: Error;
    fn insert_unconfirmed_bag(&self, bag: BagId) -> Result<(), Self::Err>;
    fn insert_confirmed_bag(&self, bag: BagProof) -> Result<(), Self::Err>;
    fn update_confirm_bag(&self, bag: &BagId, outpoint: Outpoint) -> Result<(), Self::Err>;
    fn delete_bag(&self, bag: &BagId) -> Result<(), Self::Err>;
    fn is_bag_exists(&self, bag: &BagId) -> Result<bool, Self::Err>;
    fn is_bag_confirmed(&self, bag: &BagId) -> Result<bool, Self::Err>;
    fn count_bags(&self) -> Result<u64, Self::Err>;
}
