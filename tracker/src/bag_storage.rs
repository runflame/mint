use crate::index::BagId;
use std::cell::RefCell;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::rc::Rc;

pub trait BagStorage {
    type Err: Error;
    fn insert_bag(&self, bag: BagId) -> Result<(), Self::Err>;
    fn delete_bag(&self, bag: &BagId) -> Result<(), Self::Err>;
    fn is_bag_exists(&self, bag: &BagId) -> Result<bool, Self::Err>;
    fn count_bags(&self) -> Result<u64, Self::Err>;
}

#[derive(Debug)]
pub struct BagHashSetStorage {
    map: Rc<RefCell<HashSet<BagId>>>,
}

impl BagHashSetStorage {
    pub fn new() -> Self {
        BagHashSetStorage {
            map: Rc::new(RefCell::new(HashSet::new())),
        }
    }
}

impl BagStorage for BagHashSetStorage {
    type Err = BagHashSetStorageError;

    fn insert_bag(&self, bag: BagId) -> Result<(), Self::Err> {
        self.map.borrow_mut().insert(bag);
        Ok(())
    }

    fn delete_bag(&self, bag: &BagId) -> Result<(), Self::Err> {
        match self.map.borrow_mut().remove(bag) {
            true => Ok(()),
            false => Err(BagHashSetStorageError::BagNotExists),
        }
    }

    fn is_bag_exists(&self, bag: &BagId) -> Result<bool, Self::Err> {
        Ok(self.map.borrow().contains(bag))
    }

    fn count_bags(&self) -> Result<u64, Self::Err> {
        Ok(self.map.borrow().len() as u64)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BagHashSetStorageError {
    BagNotExists,
}

impl Display for BagHashSetStorageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BagHashSetStorageError::BagNotExists => f.write_str("Bag does not exists."),
        }
    }
}

impl Error for BagHashSetStorageError {}
