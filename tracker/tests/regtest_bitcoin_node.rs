use bitcoincore_rpc::{Client, Auth, RpcApi};
use std::collections::HashMap;
use bitcoin::{TxOut, Transaction, TxIn, Block, BlockHeader, BlockHash};
use bitcoin::blockdata::{script, opcodes};
use bitcoin::consensus::{Encodable, Decodable};
use std::time::Duration;
use bitcoin::hashes::hex::ToHex;
use tracker::Index;
use std::process::Stdio;
use serde::Serialize;
use serde_json::Value;
use bitcoin::hashes::sha256d::Hash;
use std::str::FromStr;

fn setup_node(port: u32, rpcport: u32, datadir: &str) -> std::process::Child {
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

struct TempDir {
    path: String,
}

impl TempDir {
    fn new(path: String) -> Self {
        std::fs::create_dir(path.as_str()).unwrap_or_else(|_| {
            std::fs::remove_dir_all(path.as_str()).unwrap();
            std::fs::create_dir(path.as_str()).unwrap();
        });
        Self {
            path
        }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.path.as_str()).unwrap_or_else(|_|());
    }
}

#[test]
fn test_new_blocks_with_mint_txs() {
    const DIR: &'static str = "/tmp/bitcoin_test_node";
    let dir = TempDir::new(DIR.to_string());
    let node = setup_node(18444, 12001, DIR);
    std::thread::sleep(Duration::from_secs(1)); // Wait while node will started
    let client = Client::new(
        "http://localhost:12001".to_string(),
        Auth::UserPass("rt".to_string(), "rt".to_string())
    ).unwrap();

    client.create_wallet(
        "testwal",
        None,
        None,
        None,
        None,
    ).unwrap();

    let address = client.get_new_address(Some("testwal"), None).unwrap();
    client.generate_to_address(120, &address).unwrap();

    let tx = Transaction {
        version: 2,
        lock_time: 0,
        input: vec![],
        output: vec![TxOut {
            value: 10,
            script_pubkey: script::Script::new_op_return(&[1; 32])
        }],
    };

    let mut bytes = Vec::new();
    consensus_encode_tx(&tx, &mut bytes).unwrap();

    let funded = client.fund_raw_transaction(&bytes, None, None).unwrap();
    let signed = client.sign_raw_transaction_with_wallet(&funded.hex, None, None).unwrap();
    assert!(signed.complete);
    let tx_id = client.send_raw_transaction(&signed.hex).unwrap();

    let value: Value = client.call(
        "generateblock",
        &[
            Value::String(address.to_string()),
            Value::Array(vec![Value::String(tx_id.as_hash().to_hex())])
        ]).unwrap();

    let hash = match value {
        Value::Object(m) => {
            match m.get("hash").unwrap() {
                Value::String(hash) => hash.clone(),
                _ => unreachable!()
            }
        }
        _ => unreachable!()
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

fn consensus_encode_tx<S: std::io::Write>(
    tx: &Transaction,
    mut s: S,
) -> Result<usize, std::io::Error> {
    let mut len = 0;
    len += tx.version.consensus_encode(&mut s)?;
    len += tx.input.consensus_encode(&mut s)?;
    len += tx.output.consensus_encode(&mut s)?;
    len += tx.lock_time.consensus_encode(s)?;
    Ok(len)
}
