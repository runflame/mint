use crate::record::{BidEntry, BidEntryData, Outpoint};
use crate::storage::IndexStorage;
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
             bag_id BLOB,
             amount INTEGER
         )",
                [],
            )
            .unwrap();
    }
}

impl IndexStorage for SqliteIndexStorage {
    type Err = rusqlite::Error;

    fn store_record(&self, record: BidEntry) -> Result<(), Self::Err> {
        self.connection.execute(
            "INSERT INTO records VALUES (?1, ?2, ?3, ?4, ?5);",
            rusqlite::params![
                record.btc_block.as_ref(),
                record.btc_outpoint.txid.as_ref(),
                record.btc_outpoint.out_pos,
                &record.data.bag_id as &[_],
                record.data.amount
            ],
        )?;
        Ok(())
    }

    fn get_blocks_count(&self) -> Result<u64, Self::Err> {
        self.connection
            .query_row("SELECT COUNT(*) FROM records;", [], |row| row.get(0))
    }

    fn remove_with_block_hash(&self, hash: &BlockHash) -> Result<(), Self::Err> {
        self.connection
            .execute("DELETE FROM records WHERE block = ?1;", [hash.as_ref()])?;
        Ok(())
    }

    fn get_records_by_block_hash(&self, hash: &BlockHash) -> Result<Vec<BidEntry>, Self::Err> {
        let mut stmt = self.connection.prepare(
            "SELECT block, txid, out_pos, bag_id, amount FROM records WHERE block = ?1;",
        )?;

        let res = stmt.query_map([hash.as_ref()], |row| {
            Ok(BidEntry {
                btc_block: {
                    let vec: Vec<u8> = row.get(0)?;
                    BlockHash::from_slice(&vec).expect("TODO: handle the error")
                },
                btc_outpoint: Outpoint {
                    txid: {
                        let vec: Vec<u8> = row.get(1)?;
                        Txid::from_slice(&vec).expect("TODO: handle the error")
                    },
                    out_pos: row.get(2)?,
                },
                data: BidEntryData {
                    bag_id: {
                        let vec: Vec<u8> = row.get(3)?;
                        TryFrom::try_from(vec.as_slice()).expect("TODO: handle the error")
                    },
                    amount: row.get(4)?,
                },
            })
        });

        res.and_then(|cursor| cursor.collect())
    }

    fn remove_records_with_bag(&self, bag: &[u8; 32]) -> Result<(), Self::Err> {
        self.connection
            .execute("DELETE FROM records WHERE bag_id = ?1;", [bag as &[_]])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::BidEntryData;
    use bitcoin::Txid;

    #[test]
    fn sqlite_storage_tests() {
        let store = SqliteIndexStorage::in_memory();

        let record = dummy_record([1; 32], [2; 32], 4, [3; 32], 5);

        store.store_record(record.clone()).unwrap();
        assert_eq!(store.get_blocks_count().unwrap(), 1);

        let records = store.get_records_by_block_hash(&record.btc_block).unwrap();
        assert_eq!(records, vec![record.clone()]);

        store.remove_with_block_hash(&record.btc_block).unwrap();
        assert_eq!(store.get_blocks_count().unwrap(), 0);

        store.store_record(record.clone()).unwrap();
        assert_eq!(store.get_blocks_count().unwrap(), 1);
        store.remove_records_with_bag(&record.data.bag_id).unwrap();
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
            btc_block: BlockHash::hash(&block),
            btc_outpoint: Outpoint {
                txid: Txid::hash(&txid),
                out_pos,
            },
            data: BidEntryData { bag_id, amount },
        }
    }
}
