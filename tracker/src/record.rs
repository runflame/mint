use crate::bag_id::BagId;
use bitcoin::{BlockHash, Txid};
use std::hash::Hash;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct BidEntry {
    pub amount: u64,
    pub proof: BidProof,
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

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct BidProof {
    pub btc_block: BlockHash,
    pub tx: BidTx,
}

impl BidProof {
    pub fn new(btc_block: BlockHash, tx: BidTx) -> Self {
        BidProof { btc_block, tx }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct BidTx {
    pub outpoint: Outpoint,
    pub bag_id: BagId,
}

impl BidTx {
    pub fn new(outpoint: Outpoint, bag_id: BagId) -> Self {
        BidTx { outpoint, bag_id }
    }
}
