use merlin::Transcript;
use std::convert::TryFrom;

pub struct BitcoinOutputLink {
    pub tx_id: bitcoin::Txid,
    pub output: u64,
}

impl BitcoinOutputLink {
    pub fn new(tx_id: bitcoin::Txid, output: u64) -> Self {
        BitcoinOutputLink { tx_id, output }
    }
}

// TBD: maybe newtype?
pub type Satoshi = u64;
pub type BagId = [u8; 32];

pub struct Bid {
    pub link: BitcoinOutputLink,
    pub amount: Satoshi,
    pub bag: BagId,
}

impl Bid {
    pub fn new(link: BitcoinOutputLink, amount: Satoshi, bag: BagId) -> Self {
        Bid { link, amount, bag }
    }
}

// TODO: not sure if this abstraction is good, but i have not find other way to create verified bids yet
pub trait TrackerImpl {
    fn verify_btc_bag_exists(&self, pubkey_hash: bitcoin::PubkeyHash) -> Option<BagId>;

    fn bid_from_btc_tx(&self, tx: bitcoin::Transaction, output_number: u64) -> Option<Bid> {
        use bitcoin::hashes::Hash;

        let tx_id = tx.txid();
        let out = &tx.output[output_number as usize];
        let amount = out.value;

        let script = &out.script_pubkey;
        let pubkey_hash = if script.is_p2pkh() {
            bitcoin::PubkeyHash::from_slice(&script.as_bytes()[3..23]).unwrap()
        }
        else {
            return None;
        };

        let bag_id = self.verify_btc_bag_exists(pubkey_hash)?;
        let bid = Bid::new(BitcoinOutputLink::new(tx_id, output_number), amount, bag_id);

        Some(bid)
    }
}
