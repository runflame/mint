use crate::index::BagId;
use bitcoin::{BlockHash, Txid};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct BidEntry {
    pub btc_block: BlockHash,
    pub btc_outpoint: Outpoint,
    pub data: BidEntryData,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Outpoint {
    pub txid: Txid,
    pub out_pos: u64,
}

impl Outpoint {
    pub fn new(txid: Txid, out_pos: u64) -> Self {
        Outpoint { txid, out_pos }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct BidEntryData {
    pub bag_id: BagId,
    pub amount: u64,
}

// TODO: naming
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct BagProof {
    pub outpoint: Outpoint,
    pub bag_id: BagId,
}

impl BagProof {
    pub fn new(outpoint: Outpoint, bag_id: [u8; 32]) -> Self {
        BagProof { outpoint, bag_id }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum BagEntry {
    Confirmed(BagProof),
    Unconfirmed(BagId),
}
