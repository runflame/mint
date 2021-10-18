use crate::index::BagId;
use crate::record::BidEntry;
use crate::storage::def::BidStorageError;
use crate::storage::BidStorage;
use bitcoin::BlockHash;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;

/// Use it only for tests purposes.
#[derive(Debug)]
pub struct MemoryIndexStorage {
    confirmed: RefCell<HashMap<BlockHash, HashMap<BagId, BidEntry>>>,
    unconfirmed: RefCell<HashSet<BagId>>,
}

impl MemoryIndexStorage {
    pub fn new() -> Self {
        MemoryIndexStorage {
            confirmed: RefCell::new(HashMap::new()),
            unconfirmed: RefCell::new(HashSet::new()),
        }
    }
}

impl BidStorage for MemoryIndexStorage {
    type Err = Infallible;

    fn insert_bid(&self, record: BidEntry) -> Result<(), BidStorageError<Self::Err>> {
        let mut this = self.confirmed.borrow_mut();
        let set = this.entry(record.proof.btc_block).or_default();
        set.insert(record.proof.tx.bag_id, record);
        Ok(())
    }

    fn insert_unconfirmed_bag(&self, bag: [u8; 32]) -> Result<(), BidStorageError<Self::Err>> {
        let mut this = self.unconfirmed.borrow_mut();
        this.insert(bag);
        Ok(())
    }

    fn update_bid(&self, record: BidEntry) -> Result<(), BidStorageError<Self::Err>> {
        self.remove_bag(&record.proof.tx.bag_id)?;
        self.insert_bid(record)
    }

    fn remove_with_block_hash(&self, hash: &BlockHash) -> Result<(), BidStorageError<Self::Err>> {
        self.confirmed.borrow_mut().remove(hash);
        Ok(())
    }

    fn remove_bag(&self, bag: &BagId) -> Result<(), BidStorageError<Self::Err>> {
        // MemoryIndexStorage is used only to debug so no need to worry about performance
        let mut confirmed = self.confirmed.borrow_mut();
        let mut unconfirmed = self.unconfirmed.borrow_mut();

        if unconfirmed.remove(bag) {
            Ok(())
        } else {
            let mut discarded_block = None;
            let mut is_bag_exists = false;
            for (block, bids) in confirmed.iter_mut() {
                if bids.contains_key(bag) {
                    is_bag_exists = true;
                    bids.remove(bag);
                    if bids.len() == 0 {
                        discarded_block = Some(*block);
                    }
                    break;
                }
            }
            if let Some(discarded_block) = discarded_block {
                confirmed.remove(&discarded_block);
            }
            if is_bag_exists {
                Ok(())
            } else {
                Err(BidStorageError::BagDoesNotExists(*bag))
            }
        }
    }

    fn get_blocks_count(&self) -> Result<u64, BidStorageError<Self::Err>> {
        Ok(self.confirmed.borrow().len() as u64)
    }

    fn get_records_by_block_hash(
        &self,
        hash: &BlockHash,
    ) -> Result<Vec<BidEntry>, BidStorageError<Self::Err>> {
        let this = self.confirmed.borrow();
        let records = this
            .get(hash)
            .map(Clone::clone)
            .map(|x| x.into_iter().map(|(_, v)| v).collect())
            .unwrap();
        Ok(records)
    }

    fn is_bag_exists(&self, bag: &BagId) -> Result<bool, BidStorageError<Self::Err>> {
        Ok(self.is_bag_confirmed(bag)? || self.unconfirmed.borrow().contains(bag))
    }

    fn is_bag_confirmed(&self, bag: &BagId) -> Result<bool, BidStorageError<Self::Err>> {
        Ok(self
            .confirmed
            .borrow()
            .iter()
            .find(|(_, bids)| bids.get(bag).is_some())
            .is_some())
    }
}
