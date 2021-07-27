#[cfg(feature = "sqlite-storage")]
pub mod sqlite;

use crate::index::BagId;
use bitcoin::{BlockHash, Txid};
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Infallible;
use std::error::Error;

pub trait IndexStorage {
    type Err: Error;
    fn store_record(&self, record: Record) -> Result<(), Self::Err>;
    fn get_blocks_count(&self) -> Result<u64, Self::Err>;
    fn remove_with_block_hash(&self, hash: &BlockHash) -> Result<(), Self::Err>;
    fn get_blocks_by_hash(&self, hash: &BlockHash) -> Result<Vec<Record>, Self::Err>;
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Record {
    pub bitcoin_block: BlockHash,
    pub bitcoin_tx_id: Txid,
    pub bitcoin_output_position: u64,
    pub data: RecordData,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct RecordData {
    pub bag_id: BagId,
    pub amount: u64,
}

#[derive(Debug)]
pub struct MemoryIndexStorage(RefCell<HashMap<BlockHash, Vec<Record>>>);

impl MemoryIndexStorage {
    pub fn new() -> Self {
        MemoryIndexStorage(RefCell::new(HashMap::new()))
    }
}

impl IndexStorage for MemoryIndexStorage {
    type Err = Infallible;

    fn store_record(&self, record: Record) -> Result<(), Self::Err> {
        let mut this = self.0.borrow_mut();
        let vec = this.entry(record.bitcoin_block).or_default();
        vec.push(record);
        Ok(())
    }

    fn get_blocks_count(&self) -> Result<u64, Self::Err> {
        Ok(self.0.borrow().len() as u64)
    }

    fn remove_with_block_hash(&self, hash: &BlockHash) -> Result<(), Self::Err> {
        self.0.borrow_mut().remove(hash);
        Ok(())
    }

    fn get_blocks_by_hash(&self, hash: &BlockHash) -> Result<Vec<Record>, Self::Err> {
        let this = self.0.borrow();
        let records = this.get(hash).map(Clone::clone).unwrap();
        Ok(records)
    }
}
