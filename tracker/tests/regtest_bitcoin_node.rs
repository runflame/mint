mod utils;

use crate::utils::generate_block;
use crate::utils::init_client;

use tracker::bitcoin_client::BitcoinMintExt;
use tracker::storage::memory::MemoryIndexStorage;
use tracker::storage::sqlite::SqliteIndexStorage;
use tracker::storage::IndexStorage;
use tracker::Index;

const GENERATED_BLOCKS: u64 = 120;

#[test]
fn regtest_bitcoin_node_memory_storage() {
    test_new_blocks_with_mint_txs(MemoryIndexStorage::new(), "/tmp/test_memory_storage/", 0);
}

#[test]
fn regtest_bitcoin_node_sqlite_storage() {
    test_new_blocks_with_mint_txs(
        SqliteIndexStorage::in_memory(),
        "/tmp/test_sqlite_storage/",
        1,
    );
}

fn test_new_blocks_with_mint_txs<S: IndexStorage>(storage: S, dir: &str, offset: u32) {
    let (_dir, _child, client, address) = init_client(dir, GENERATED_BLOCKS, offset);

    // create mint transaction
    let tx_id = client.send_mint_transaction(10, &[1; 32]).unwrap();
    let mint_block = generate_block(&client, &address, &tx_id);

    let mut index = Index::new(client, storage, Some(119));

    index.check_last_btc_blocks();

    index.add_bag([1; 32]);
    index.validate_bags().unwrap();

    assert_eq!(index.checked_btc_height(), GENERATED_BLOCKS + 1);

    let txs = index.get_storage();
    assert_eq!(txs.get_blocks_count().unwrap(), 1); // we have only one mint transaction

    let txs1 = txs.get_records_by_block_hash(&mint_block).unwrap();
    assert_eq!(txs1.last().unwrap().data.bag_id, [1; 32]);
}
