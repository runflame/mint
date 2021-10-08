use blockchain::utreexo;

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
    fn verify_btc_bag_exists(&self, bag_id: &[u8]) -> Option<BagId>;

    fn bid_from_btc_tx(&self, tx: bitcoin::Transaction, output_number: u64) -> Option<Bid> {
        let tx_id = tx.txid();
        let out = &tx.output[output_number as usize];
        let amount = out.value;

        let script = &out.script_pubkey;
        let bid_bag_id = parse_bag_id(script)?;

        let bag_id = self.verify_btc_bag_exists(bid_bag_id)?;
        let bid = Bid::new(BitcoinOutputLink::new(tx_id, output_number), amount, bag_id);

        Some(bid)
    }
}

fn parse_bag_id(script: &bitcoin::Script) -> Option<&[u8]> {
    if script.is_v0_p2wsh() {
        Some(&script.as_bytes()[3..23])
    }
    else {
        None
    }
}

type BlockId = [u8; 32];

pub struct Block {
    height: u64,
    prev: BlockId,
    timestamp_ms: u64,
    txroot: merkle::Hash,
    state: merkle::Hash,
    bags: Vec<BagId>,
    ext: Vec<u8>,
}

impl Block {

}
