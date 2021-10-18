mod utils;
use bitcoincore_rpc::RpcApi;
use tracker::bitcoin_client::BitcoinMintExt;
use tracker::index::BagId;
use tracker::record::BidProof;
use tracker::storage::memory::MemoryIndexStorage;
use tracker::storage::BidStorage;
use tracker::Index;
use utils::*;

const GENERATED_BLOCKS: u64 = 120;

// Wait 2 seconds or until condition is satisfied. Used for waiting for synchronizing nodes.
macro_rules! wait {
    ($cond:expr) => {
        assert!(utils::wait_until(6, || $cond));
    };
}

#[test]
fn test_reorg_longest_chain() {
    let storage = MemoryIndexStorage::new();

    let _tempdir = TempDir::new("/tmp/test_reorg_longest_chain/".to_string());
    let dir1 = "/tmp/test_reorg_longest_chain/node1/";
    let dir2 = "/tmp/test_reorg_longest_chain/node2/";

    const OFFSET_N1: u32 = 3;
    const OFFSET_N2: u32 = 4;
    const NODE2_PORT: u32 = 18444 + OFFSET_N2;
    let node2_addr = format!("localhost:{}", NODE2_PORT);

    let (_dir1, _child1, client1, address1) = init_client(dir1, GENERATED_BLOCKS, OFFSET_N1);
    let (_dir2, _child2, client2, address2) = init_client(dir2, 0, OFFSET_N2);

    const SATOSHIES_TO_SEND: u64 = 1000;
    const HEIGHT_BEFORE_FORK: u64 = GENERATED_BLOCKS * 2;
    const HEIGHT_CHAIN1: u64 = HEIGHT_BEFORE_FORK + 2;
    const HEIGHT_CHAIN2: u64 = HEIGHT_BEFORE_FORK + 3;
    const BAG1_12: BagId = [1; 32]; // bag #1 on both chains
    const BAG2_1: BagId = [2; 32]; // bag #2 on chain #1
    const BAG2_2: BagId = [3; 32]; // bag #2 on chain #2
    const BAG3_2: BagId = [4; 32]; // bag #3 on chain #2

    // Connect node1 to node2/
    assert_eq!(client1.get_network_info().unwrap().connections, 0);
    add_node_client(&client1, &node2_addr);
    wait!(client1.get_network_info().unwrap().connections == 1);
    wait!(client2.get_blockchain_info().unwrap().blocks == GENERATED_BLOCKS);
    // Generate blocks to give node2 money for paying.
    client2
        .generate_to_address(GENERATED_BLOCKS, &address2)
        .unwrap();
    wait!(client1.get_blockchain_info().unwrap().blocks == HEIGHT_BEFORE_FORK);

    // Both nodes have mint tx.
    let bid1_12 = client1
        .send_mint_transaction(SATOSHIES_TO_SEND, &BAG1_12)
        .unwrap();
    let both_block = generate_block(&client1, &address1, &bid1_12.outpoint.txid);
    let prf1_12 = BidProof::new(both_block, bid1_12);
    // Wait before node2 receive block
    wait!(client2.get_blockchain_info().unwrap().best_block_hash == both_block);

    // Disconnect nodes.
    disconnect_node_client(&client1, &node2_addr);
    wait!(client1.get_network_info().unwrap().connections == 0);

    // Mine block with Bag2_1 on node 1.
    let (last_block_chain_1, prf2_1) = {
        let bid = client1
            .send_mint_transaction(SATOSHIES_TO_SEND, &BAG2_1)
            .unwrap();
        let block = generate_block(&client1, &address1, &bid.outpoint.txid);
        (block, BidProof::new(block, bid))
    };

    let (bag2_2block, bag3_2block, prf2_2, prf3_2) = {
        // Mine block with Bag2_2 on node 2.
        let bid2 = client2
            .send_mint_transaction(SATOSHIES_TO_SEND, &BAG2_2)
            .unwrap();
        let bag1block = generate_block(&client2, &address2, &bid2.outpoint.txid);

        // Mine block with Bag3_2 on node 2.
        let bid3 = client2
            .send_mint_transaction(SATOSHIES_TO_SEND, &BAG3_2)
            .unwrap();
        let bag2block = generate_block(&client2, &address2, &bid3.outpoint.txid);

        (
            bag1block,
            bag2block,
            BidProof::new(bag1block, bid2),
            BidProof::new(bag2block, bid3),
        )
    };

    // Track chain on node1.
    let mut index = Index::new(client1, storage, Some(HEIGHT_BEFORE_FORK - 1)).unwrap();

    // Tracker has no access to the chain #2, so it cannot prove bags 2_2 and 3_2 now.
    index.add_bid(prf1_12).unwrap();
    index.add_bid(prf2_1).unwrap();
    index.add_bag(prf2_2.tx.bag_id).unwrap();
    index.add_bag(prf3_2.tx.bag_id).unwrap();

    // Check that node1 contains only 2 bags on chain #1.
    {
        assert_eq!(*index.current_height(), HEIGHT_CHAIN1);

        let store = index.get_storage();
        assert_eq!(store.get_blocks_count().unwrap(), 2);

        let txs = store
            .get_records_by_block_hash(&last_block_chain_1)
            .unwrap();
        assert_eq!(txs.last().unwrap().proof.tx.bag_id, BAG2_1);
    }

    // Reconnect node1 with node2.
    let client1 = index.btc_client();
    add_node_client(client1, &node2_addr);
    assert_eq!(client1.get_network_info().unwrap().connections, 1);
    wait!(client1.get_blockchain_info().unwrap().blocks == HEIGHT_CHAIN2);

    // Tracker must find reorg there.
    index.check_reorgs().unwrap();

    // Check that reorg happened and chain #2 is main now.
    {
        assert_eq!(*index.current_height(), HEIGHT_CHAIN2);

        let store = index.get_storage();
        assert_eq!(store.get_blocks_count().unwrap(), 3);

        let txs = store.get_records_by_block_hash(&bag3_2block).unwrap();
        assert_eq!(txs.last().unwrap().proof.tx.bag_id, BAG3_2);

        let txs = store.get_records_by_block_hash(&bag2_2block).unwrap();
        assert_eq!(txs.last().unwrap().proof.tx.bag_id, BAG2_2);
    }
}
