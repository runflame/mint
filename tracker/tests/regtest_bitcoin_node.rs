use bitcoin::blockdata::script;
use bitcoin::consensus::Encodable;
use bitcoin::hashes::hex::ToHex;
use bitcoin::hashes::sha256d::Hash;
use bitcoin::{BlockHash, Transaction, TxOut};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde_json::Value;
use std::process::Stdio;
use std::str::FromStr;
use std::time::Duration;
use tracker::bitcoin_client::BitcoinMintExt;
use tracker::Index;

#[test]
fn test_new_blocks_with_mint_txs() {
    const DIR: &'static str = "/tmp/bitcoin_test_node";
    let _dir = TempDir::new(DIR.to_string());
    let _node = setup_bitcoin_node(18444, 12001, DIR);
    std::thread::sleep(Duration::from_secs(1)); // Wait while node will started
    let client = Client::new(
        "http://localhost:12001".to_string(),
        Auth::UserPass("rt".to_string(), "rt".to_string()),
    )
    .unwrap();

    client
        .create_wallet("testwal", None, None, None, None)
        .unwrap();

    let address = client.get_new_address(Some("testwal"), None).unwrap();
    client.generate_to_address(120, &address).unwrap();

    let tx_id = client.send_mint_transaction(10, &[1; 32]).unwrap();

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

    let mint_block = BlockHash::from_hash(Hash::from_str(&hash).unwrap());

    let mut index = Index::new(client, Some(119));
    index.add_bag([1; 32]);

    index.check_last_blocks();
    assert_eq!(index.checked_height(), 121);

    let txs = index.get_index();

    assert_eq!(txs.len(), 1);

    let txs1 = txs.get(&mint_block).unwrap();
    assert_eq!(txs1.last().unwrap().bag_id, [1; 32])
}

pub fn setup_bitcoin_node(port: u32, rpcport: u32, datadir: &str) -> std::process::Child {
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
