use bitcoin::hashes::hex::ToHex;
use bitcoin::hashes::sha256d::Hash;
use bitcoin::{Address, BlockHash, Txid};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde_json::Value;
use std::str::FromStr;
use std::time::{Duration, Instant};

pub fn init_client(
    path: &str,
    block_num: u64,
    offset: u32,
) -> (TempDir, KillBitcoind, Client, Address) {
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
    client.generate_to_address(block_num, &address).unwrap();

    (dir, node, client, address)
}

pub fn setup_bitcoin_node(port: u32, rpcport: u32, datadir: &str) -> KillBitcoind {
    let output = std::process::Command::new("bash")
        .arg("./tests/setup_single_node.sh")
        .arg(port.to_string())
        .arg(rpcport.to_string())
        .arg(datadir)
        .output()
        .unwrap();
    let id = String::from_utf8(output.stdout).unwrap().trim().to_string();
    KillBitcoind { id }
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
pub fn generate_block(client: &Client, address: &Address, tx_id: &Txid) -> BlockHash {
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

pub struct KillBitcoind {
    id: String,
}

impl Drop for KillBitcoind {
    fn drop(&mut self) {
        std::process::Command::new("kill")
            .arg(&self.id)
            .spawn()
            .unwrap();
    }
}

#[allow(unused)]
pub fn add_node_client(client: &Client, addr: &str) {
    let _: Value = client
        .call(
            "addnode",
            &[
                Value::String(addr.to_string()),
                Value::String("onetry".to_string()),
            ],
        )
        .unwrap();
}

#[allow(unused)]
pub fn disconnect_node_client(client: &Client, addr: &str) {
    let _: Value = client
        .call("disconnectnode", &[Value::String(addr.to_string())])
        .unwrap();
}

#[allow(unused)]
pub fn wait_until(seconds: u64, condition: impl Fn() -> bool) -> bool {
    let instant = Instant::now();
    loop {
        let now = instant.elapsed();
        if now.as_secs() >= seconds {
            return condition();
        } else {
            if condition() {
                return true;
            } else {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}
