use bitcoin::hashes::hex::ToHex;
use bitcoin::hashes::sha256d::Hash;
use bitcoin::{Address, BlockHash, Txid};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde_json::Value;
use std::process::{Child, Stdio};
use std::str::FromStr;
use std::time::Duration;
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

// TODO: kill bitcoind process after test
fn test_new_blocks_with_mint_txs<S: IndexStorage>(storage: S, dir: &str, offset: u32) {
    let (_dir, _child, client, address) = init_client(dir, offset);

    // create mint transaction
    let tx_id = client.send_mint_transaction(10, &[1; 32]).unwrap();
    let mint_block = generate_block(&client, &address, &tx_id);

    let mut index = Index::new(client, storage, Some(119));
    index.add_bag([1; 32]);

    index.check_last_btc_blocks();
    assert_eq!(index.checked_btc_height(), GENERATED_BLOCKS + 1);

    let txs = index.get_storage();
    assert_eq!(txs.get_blocks_count().unwrap(), 1); // we have only one mint transaction

    let txs1 = txs.get_blocks_by_hash(&mint_block).unwrap();
    assert_eq!(txs1.last().unwrap().data.bag_id, [1; 32]);
}

fn init_client(path: &str, offset: u32) -> (TempDir, Child, Client, Address) {
    let rpc_port = 12001 + offset;
    let dir = TempDir::new(path.to_string());
    let node = setup_bitcoin_node(18444 + offset, rpc_port, path);
    std::thread::sleep(Duration::from_secs(1)); // Wait while node will started
    let client = Client::new(
        format!("http://localhost:{}", rpc_port),
        Auth::UserPass("rt".to_string(), "rt".to_string()),
    )
    .unwrap();

    client
        .create_wallet("testwal", None, None, None, None)
        .unwrap();

    let address = client.get_new_address(Some("testwal"), None).unwrap();
    client
        .generate_to_address(GENERATED_BLOCKS, &address)
        .unwrap();

    (dir, node, client, address)
}

fn setup_bitcoin_node(port: u32, rpcport: u32, datadir: &str) -> Child {
    let child = std::process::Command::new("bash")
        .arg("./tests/setup_single_node.sh")
        .arg(port.to_string())
        .arg(rpcport.to_string())
        .arg(datadir)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    child
}

pub struct TempDir {
    path: String,
}

impl TempDir {
    pub fn new(path: String) -> Self {
        std::fs::create_dir(path.as_str()).unwrap_or_else(|_| {
            std::fs::remove_dir_all(path.as_str()).unwrap();
            std::fs::create_dir(path.as_str()).unwrap();
        });
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.path.as_str()).unwrap_or_else(|_| ());
    }
}

// TODO: remove when https://github.com/rust-bitcoin/rust-bitcoincore-rpc/pull/189 is accepted
fn generate_block(client: &Client, address: &Address, tx_id: &Txid) -> BlockHash {
    let value: Value = client
        .call(
            "generateblock",
            &[
                Value::String(address.to_string()),
                Value::Array(vec![Value::String(tx_id.as_hash().to_hex())]),
            ],
        )
        .unwrap();

    let hash = match value {
        Value::Object(m) => match m.get("hash").unwrap() {
            Value::String(hash) => hash.clone(),
            _ => unreachable!(),
        },
        _ => unreachable!(),
    };

    BlockHash::from_hash(Hash::from_str(&hash).unwrap())
}
