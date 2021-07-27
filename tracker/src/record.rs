use crate::index::BagId;
use bitcoin::{BlockHash, Txid};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Record {
    pub bitcoin_block: BlockHash,
    pub bitcoin_tx_id: Txid,
    pub bitcoin_output_position: u64,
    pub data: RecordData,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct RecordData {
    pub bag_id: BagId,
    pub amount: u64,
}
