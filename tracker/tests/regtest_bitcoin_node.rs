mod utils;

use crate::utils::generate_block;
use crate::utils::init_client;

use tracker::bag_id::BagId;
use tracker::bitcoin_client::BitcoinMintExt;
use tracker::record::BidProof;
use tracker::storage::memory::MemoryIndexStorage;
#[cfg(feature = "sqlite-storage")]
use tracker::storage::sqlite::SqliteIndexStorage;
use tracker::storage::BidStorage;
use tracker::Index;

const GENERATED_BLOCKS: u64 = 120;

#[test]
fn regtest_bitcoin_node_memory_storage() {
    test_new_blocks_with_mint_txs(MemoryIndexStorage::new(), "/tmp/test_memory_storage/", 0);
}

#[test]
#[cfg(feature = "sqlite-storage")]
fn regtest_bitcoin_node_sqlite_storage() {
    test_new_blocks_with_mint_txs(
        SqliteIndexStorage::in_memory(),
        "/tmp/test_sqlite_storage/",
        1,
    );
}

fn test_new_blocks_with_mint_txs<S: BidStorage>(storage: S, dir: &str, offset: u32) {
    let (_dir, _child, client, address) = init_client(dir, GENERATED_BLOCKS, offset);

    // create mint transaction
    let bid_tx = client.send_mint_transaction(1000, &BagId([1; 32])).unwrap();
    let mint_block = generate_block(&client, &address, &bid_tx.outpoint.txid);

    let mut index = Index::new(client, storage, Some(119)).unwrap();

    index.add_bid(BidProof::new(mint_block, bid_tx)).unwrap();

    assert_eq!(*index.current_height(), GENERATED_BLOCKS + 1);

    let txs = index.get_storage();
    assert_eq!(txs.get_blocks_count().unwrap(), 1); // we have only one mint transaction

    let txs1 = txs.get_records_by_block_hash(&mint_block).unwrap();
    assert_eq!(txs1.last().unwrap().proof.tx.bag_id, [1; 32]);
}
