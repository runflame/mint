use crate::bag_storage::BagStorage;
use crate::index::BagId;
use crate::record::{BagEntry, BagProof, Outpoint};
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::rc::Rc;

#[derive(Debug)]
pub struct BagMemoryStorage {
    map: Rc<RefCell<HashMap<BagId, BagEntry>>>,
}

impl BagMemoryStorage {
    pub fn new() -> Self {
        BagMemoryStorage {
            map: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}

impl BagStorage for BagMemoryStorage {
    type Err = BagMemoryStorageError;

    fn insert_unconfirmed_bag(&self, bag: BagId) -> Result<(), Self::Err> {
        self.map
            .borrow_mut()
            .insert(bag, BagEntry::Unconfirmed(bag));
        Ok(())
    }

    fn insert_confirmed_bag(&self, bag: BagProof) -> Result<(), Self::Err> {
        self.map
            .borrow_mut()
            .insert(bag.bag_id, BagEntry::Confirmed(bag));
        Ok(())
    }

    fn update_confirm_bag(&self, bag: &BagId, outpoint: Outpoint) -> Result<(), Self::Err> {
        let mut this = self.map.borrow_mut();
        let bag_entry = this
            .get_mut(bag)
            .ok_or(BagMemoryStorageError::BagNotExists)?;
        *bag_entry = BagEntry::Confirmed(BagProof::new(outpoint, bag.clone()));
        Ok(())
    }

    fn delete_bag(&self, bag: &BagId) -> Result<(), Self::Err> {
        match self.map.borrow_mut().remove(bag) {
            Some(_) => Ok(()),
            None => Err(BagMemoryStorageError::BagNotExists),
        }
    }

    fn is_bag_exists(&self, bag: &BagId) -> Result<bool, Self::Err> {
        Ok(self.map.borrow().contains_key(bag))
    }

    fn is_bag_confirmed(&self, bag: &BagId) -> Result<bool, Self::Err> {
        let this = self.map.borrow();
        let entry = this.get(bag).ok_or(BagMemoryStorageError::BagNotExists)?;
        Ok(matches!(entry, BagEntry::Confirmed(_)))
    }

    fn count_bags(&self) -> Result<u64, Self::Err> {
        Ok(self.map.borrow().len() as u64)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BagMemoryStorageError {
    BagNotExists,
}

impl Display for BagMemoryStorageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BagMemoryStorageError::BagNotExists => f.write_str("Bag does not exists."),
        }
    }
}

impl Error for BagMemoryStorageError {}
