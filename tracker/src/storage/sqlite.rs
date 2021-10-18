use crate::index::BagId;
use crate::record::{BidEntry, BidProof, BidTx, Outpoint};
use crate::storage::def::BidStorageError;
use crate::storage::BidStorage;
use bitcoin::hashes::Hash;
use bitcoin::BlockHash;
use bitcoin::Txid;
use rusqlite::Connection;
use std::convert::TryFrom;
use std::path::Path;

#[derive(Debug)]
pub struct SqliteIndexStorage {
    connection: Connection,
}

impl SqliteIndexStorage {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self::with_connection(Connection::open(path).unwrap())
    }
    pub fn in_memory() -> Self {
        Self::with_connection(Connection::open_in_memory().unwrap())
    }
    pub fn with_connection(connection: Connection) -> Self {
        let this = SqliteIndexStorage { connection };
        this.init_tables();
        this
    }
}

impl SqliteIndexStorage {
    fn init_tables(&self) {
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS records (
             block BLOB,
             txid BLOB,
             out_pos INTEGER,
             bag_id BLOB NOT NULL,
             amount INTEGER
         )",
                [],
            )
            .unwrap();
    }
}

impl BidStorage for SqliteIndexStorage {
    type Err = rusqlite::Error;

    fn insert_bid(&self, record: BidEntry) -> Result<(), BidStorageError<Self::Err>> {
        self.connection.execute(
            "INSERT INTO records VALUES (?1, ?2, ?3, ?4, ?5);",
            rusqlite::params![
                record.proof.btc_block.as_ref(),
                record.proof.tx.outpoint.txid.as_ref(),
                record.proof.tx.outpoint.out_pos,
                &record.proof.tx.bag_id as &[_],
                record.amount
            ],
        )?;
        Ok(())
    }

    fn get_blocks_count(&self) -> Result<u64, BidStorageError<Self::Err>> {
        self.connection
            .query_row("SELECT COUNT(DISTINCT block) FROM records;", [], |row| {
                row.get(0)
            })
            .map_err(Into::into)
    }

    fn remove_with_block_hash(&self, hash: &BlockHash) -> Result<(), BidStorageError<Self::Err>> {
        self.connection
            .execute("DELETE FROM records WHERE block = ?1;", [hash.as_ref()])?;
        Ok(())
    }

    fn get_records_by_block_hash(
        &self,
        hash: &BlockHash,
    ) -> Result<Vec<BidEntry>, BidStorageError<Self::Err>> {
        let mut stmt = self.connection.prepare(
            "SELECT block, txid, out_pos, bag_id, amount FROM records WHERE block = ?1;",
        )?;

        let res = stmt.query_map([hash.as_ref()], |row| {
            Ok(BidEntry {
                amount: row.get(4)?,
                proof: BidProof {
                    btc_block: {
                        let vec: Vec<u8> = row.get(0)?;
                        BlockHash::from_slice(&vec).expect("TODO: handle the error")
                    },
                    tx: BidTx {
                        outpoint: Outpoint {
                            txid: {
                                let vec: Vec<u8> = row.get(1)?;
                                Txid::from_slice(&vec).expect("TODO: handle the error")
                            },
                            out_pos: row.get(2)?,
                        },
                        bag_id: {
                            let vec: Vec<u8> = row.get(3)?;
                            TryFrom::try_from(vec.as_slice()).expect("TODO: handle the error")
                        },
                    },
                },
            })
        });

        res.and_then(|cursor| cursor.collect()).map_err(Into::into)
    }

    fn remove_bag(&self, bag: &BagId) -> Result<(), BidStorageError<Self::Err>> {
        let deleted = self
            .connection
            .execute("DELETE FROM records WHERE bag_id = ?1;", [bag as &[_]])?;
        if deleted == 0 {
            Err(BidStorageError::BagDoesNotExists(*bag))
        } else {
            Ok(())
        }
    }

    fn insert_unconfirmed_bag(&self, bag: BagId) -> Result<(), BidStorageError<Self::Err>> {
        self.connection.execute(
            "INSERT INTO records VALUES (?1, ?2, ?3, ?4, ?5);",
            rusqlite::params![
                &rusqlite::types::Null,
                &rusqlite::types::Null,
                &rusqlite::types::Null,
                &bag as &[_],
                &rusqlite::types::Null
            ],
        )?;
        Ok(())
    }

    fn update_bid(&self, record: BidEntry) -> Result<(), BidStorageError<Self::Err>> {
        let updated = self.connection.execute(
            "UPDATE records SET block=?1, txid=?2, out_pos=?3, amount=?4 WHERE bag_id=?5;",
            rusqlite::params![
                record.proof.btc_block.as_ref(),
                record.proof.tx.outpoint.txid.as_ref(),
                record.proof.tx.outpoint.out_pos,
                record.amount,
                &record.proof.tx.bag_id as &[_]
            ],
        )?;
        if updated == 0 {
            Err(BidStorageError::BagDoesNotExists(record.proof.tx.bag_id))
        } else {
            Ok(())
        }
    }

    fn is_bag_exists(&self, bag: &[u8; 32]) -> Result<bool, BidStorageError<Self::Err>> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM records WHERE bag_id = ?1)",
                [bag as &[_]],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn is_bag_confirmed(&self, bag: &[u8; 32]) -> Result<bool, BidStorageError<Self::Err>> {
        let (exists, confirmed) = self.connection.query_row(
            "SELECT \
                EXISTS(SELECT 1 FROM records WHERE bag_id = ?1), \
                EXISTS(SELECT 1 FROM records WHERE bag_id = ?1 AND block IS NOT NULL)",
            [bag as &[_]],
            |row| -> Result<(bool, bool), _> { Ok((row.get(0)?, row.get(1)?)) },
        )?;
        if !exists {
            Err(BidStorageError::BagDoesNotExists(*bag))
        } else {
            Ok(confirmed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Txid;

    #[test]
    fn sqlite_storage_tests() {
        let store = SqliteIndexStorage::in_memory();

        let record = dummy_record([1; 32], [2; 32], 4, [3; 32], 5);

        store.insert_bid(record.clone()).unwrap();
        assert_eq!(store.get_blocks_count().unwrap(), 1);

        let records = store
            .get_records_by_block_hash(&record.proof.btc_block)
            .unwrap();
        assert_eq!(records, vec![record.clone()]);

        store
            .remove_with_block_hash(&record.proof.btc_block)
            .unwrap();
        assert_eq!(store.get_blocks_count().unwrap(), 0);

        store.insert_bid(record.clone()).unwrap();
        assert_eq!(store.get_blocks_count().unwrap(), 1);
        store.remove_bag(&record.proof.tx.bag_id).unwrap();
        assert_eq!(store.get_blocks_count().unwrap(), 0);
    }

    fn dummy_record(
        block: [u8; 32],
        txid: [u8; 32],
        out_pos: u64,
        bag_id: [u8; 32],
        amount: u64,
    ) -> BidEntry {
        BidEntry {
            amount,
            proof: BidProof::new(
                BlockHash::hash(&block),
                BidTx::new(Outpoint::new(Txid::hash(&txid), out_pos), bag_id),
            ),
        }
    }
}
