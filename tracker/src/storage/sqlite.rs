use crate::bag_id::BagId;
use crate::record::{BidEntry, BidProof, BidTx, Outpoint};
use crate::storage::def::BidStorageError;
use crate::storage::BidStorage;
use bitcoin::hashes::Hash;
use bitcoin::BlockHash;
use bitcoin::Txid;
use rusqlite::Connection;
use std::convert::TryFrom;
use std::path::Path;

/// Bid storage used `sqlite`.
#[derive(Debug)]
pub struct BidSqliteStorage {
    connection: Connection,
}

impl BidSqliteStorage {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self::with_connection(Connection::open(path).unwrap())
    }
    pub fn in_memory() -> Self {
        Self::with_connection(Connection::open_in_memory().unwrap())
    }
    pub fn with_connection(connection: Connection) -> Self {
        let this = BidSqliteStorage { connection };
        this.init_tables();
        this
    }
}

impl BidSqliteStorage {
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

impl BidStorage for BidSqliteStorage {
    type Err = rusqlite::Error;

    fn insert_bid(&self, record: BidEntry) -> Result<(), BidStorageError<Self::Err>> {
        self.connection.execute(
            "INSERT INTO records VALUES (?1, ?2, ?3, ?4, ?5);",
            rusqlite::params![
                record.proof.btc_block.as_ref(),
                record.proof.tx.outpoint.txid.as_ref(),
                record.proof.tx.outpoint.out_pos,
                &record.proof.tx.bag_id,
                record.amount
            ],
        )?;
        Ok(())
    }

    fn insert_unconfirmed_bag(&self, bag: BagId) -> Result<(), BidStorageError<Self::Err>> {
        self.connection.execute(
            "INSERT INTO records VALUES (?1, ?2, ?3, ?4, ?5);",
            rusqlite::params![
                &rusqlite::types::Null,
                &rusqlite::types::Null,
                &rusqlite::types::Null,
                &bag,
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
                &record.proof.tx.bag_id,
            ],
        )?;
        if updated == 0 {
            Err(BidStorageError::BagDoesNotExists(record.proof.tx.bag_id))
        } else {
            Ok(())
        }
    }

    fn remove_confirmation_with_block_hash(
        &self,
        hash: &BlockHash,
    ) -> Result<(), BidStorageError<Self::Err>> {
        self.connection.execute(
            "UPDATE records SET block=?1, txid=?2, out_pos=?3, amount=?4 WHERE block=?5;",
            rusqlite::params![
                &rusqlite::types::Null,
                &rusqlite::types::Null,
                &rusqlite::types::Null,
                &rusqlite::types::Null,
                hash.as_ref()
            ],
        )?;
        Ok(())
    }

    fn remove_bag(&self, bag: &BagId) -> Result<(), BidStorageError<Self::Err>> {
        let deleted = self
            .connection
            .execute("DELETE FROM records WHERE bag_id = ?1;", [bag])?;
        if deleted == 0 {
            Err(BidStorageError::BagDoesNotExists(*bag))
        } else {
            Ok(())
        }
    }

    fn get_blocks_count(&self) -> Result<u64, BidStorageError<Self::Err>> {
        self.connection
            .query_row("SELECT COUNT(DISTINCT block) FROM records;", [], |row| {
                row.get(0)
            })
            .map_err(Into::into)
    }

    fn get_records_by_block_hash(
        &self,
        hash: &BlockHash,
    ) -> Result<Vec<BidEntry>, BidStorageError<Self::Err>> {
        let mut stmt = self.connection.prepare(
            "SELECT block, txid, out_pos, bag_id, amount FROM records WHERE block = ?1;",
        )?;

        let res = stmt.query_map([hash.as_ref()], |row| {
            Ok(BidEntryRaw {
                btc_block: row.get(0)?,
                txid: row.get(1)?,
                out_pos: row.get(2)?,
                bag_id: row.get(3)?,
                amount: row.get(4)?,
            })
        });

        let raw = res.and_then(|cursor| cursor.collect::<Result<Vec<_>, _>>())?;
        raw.into_iter()
            .map(|raw| raw.try_into_bid().ok_or(BidStorageError::WrongFormat))
            .collect()
    }

    fn is_bag_exists(&self, bag: &BagId) -> Result<bool, BidStorageError<Self::Err>> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM records WHERE bag_id = ?1)",
                [bag],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }
}

struct BidEntryRaw {
    amount: u64,
    btc_block: Vec<u8>,
    bag_id: Vec<u8>,
    txid: Vec<u8>,
    out_pos: u64,
}

impl BidEntryRaw {
    fn try_into_bid(self) -> Option<BidEntry> {
        Some(BidEntry {
            amount: self.amount,
            proof: BidProof {
                btc_block: BlockHash::from_slice(&self.btc_block).ok()?,
                tx: BidTx {
                    outpoint: Outpoint {
                        txid: Txid::from_slice(&self.txid).ok()?,
                        out_pos: self.out_pos,
                    },
                    bag_id: TryFrom::try_from(self.bag_id.as_slice()).ok()?,
                },
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Txid;

    #[test]
    fn sqlite_storage_tests() {
        let store = BidSqliteStorage::in_memory();

        let record = dummy_record([1; 32], [2; 32], 4, [3; 32], 5);

        store.insert_bid(record.clone()).unwrap();
        assert_eq!(store.get_blocks_count().unwrap(), 1);

        let records = store
            .get_records_by_block_hash(&record.proof.btc_block)
            .unwrap();
        assert_eq!(records, vec![record.clone()]);

        store
            .remove_confirmation_with_block_hash(&record.proof.btc_block)
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
                BidTx::new(Outpoint::new(Txid::hash(&txid), out_pos), BagId(bag_id)),
            ),
        }
    }
}
