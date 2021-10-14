mod program;

use blockchain::{BlockTx};
use zkvm::curve25519_dalek::scalar::Scalar;
use zkvm::{Program, Contract, Predicate, PortableItem, Value, Commitment, Anchor, Prover, Signature, TxHeader, UnsignedTx, Multisignature};
use merlin::Transcript;
use zkvm::bulletproofs::BulletproofGens;
use std::convert::TryFrom;

/// An outpoint - a combination of a transaction hash and an index n into its vout.
pub struct OutPoint {
    pub tx_id: bitcoin::Txid,
    pub output: u64,
}

impl OutPoint {
    pub fn new(tx_id: bitcoin::Txid, output: u64) -> Self {
        OutPoint { tx_id, output }
    }
}

// TBD: maybe newtype?
pub type Satoshi = u64;
pub type BagId = [u8; 32];

pub struct Bid {
    pub link: OutPoint,
    pub amount: Satoshi,
    pub bag: BagId,
}

impl Bid {
    pub fn new(link: OutPoint, amount: Satoshi, bag: BagId) -> Self {
        Bid { link, amount, bag }
    }
}

// TODO: not sure if this abstraction is good, but i have not find other way to create verified bids yet
pub trait TrackerImpl {
    fn verify_btc_bag_exists(&self, bag_id: &BagId) -> Option<()>;

    fn bid_from_btc_tx(&self, tx: bitcoin::Transaction, output_number: u64) -> Option<Bid> {
        let tx_id = tx.txid();
        let out = &tx.output[output_number as usize];
        let amount = out.value;

        let script = &out.script_pubkey;
        let bag_id = parse_bag_id(script)?;

        self.verify_btc_bag_exists(&bag_id)?;
        let bid = Bid::new(OutPoint::new(tx_id, output_number), amount, bag_id);

        Some(bid)
    }
}

fn parse_bag_id(script: &bitcoin::Script) -> Option<BagId> {
    if script.is_v0_p2wsh() {
        Some(BagId::try_from(&script.as_bytes()[2..34]).expect("BagId have 32 bytes"))
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

pub fn sidecoin_flavor() -> Scalar {
    // Scalar has no const fns to make this as a constant instead of a function.
    Scalar::zero()
}

pub struct Tx {
    pub block_tx: BlockTx
}

impl Tx {

}

pub fn create_mint_contract(privkey: Scalar, qty: u64, bag_id: &BagId, bid_index: u64) -> Contract {
    let anchor = make_bid_anchor(bag_id, bid_index);
    Contract {
        predicate: Predicate::with_witness(privkey),
        payload: vec![PortableItem::Value(Value {
            qty: Commitment::blinded(qty),
            flv: Commitment::blinded(sidecoin_flavor()),
        })],
        anchor,
    }
}

fn make_bid_anchor(bag_id: &BagId, bid_index: u64) -> Anchor {
    let mut t = Transcript::new(b"Flame.Reward");
    t.append_message(b"bag_id", bag_id);
    t.append_u64(b"index", bid_index);

    let mut anchor = [0u8; 32];
    t.challenge_bytes(b"anchor", &mut anchor);
    Anchor(anchor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zkvm::{TxID, TxLog, VMError, TxHeader, Prover, Signature, Multisignature};
    use zkvm::bulletproofs::BulletproofGens;
    use zkvm::curve25519_dalek::ristretto::CompressedRistretto;
    use zkvm::curve25519_dalek::traits::Identity;

    #[test]
    fn use_mint_output() {
        let privkey = Scalar::from(1u8);
        let predicate = Predicate::with_witness(privkey);
        let qty = 10;
        let bag_id = [0u8; 32];
        let bid_index = 0;

        let mint_contract = create_mint_contract(privkey, qty, &bag_id, bid_index);

        let program = make_1_1_program(mint_contract, predicate, qty);

        match build_and_verify(program) {
            Err(err) => panic!("{}", err.to_string()),
            _ => (),
        }
    }

    fn make_1_1_program(utxo: Contract, output_predicate: Predicate, qty: u64) -> Program {
        Program::build(|b| {
            b.push(utxo)
                .input()
                .signtx()
                .push(Commitment::blinded(qty))
                .push(Commitment::blinded(sidecoin_flavor()))
                .cloak(1, 1)
                .push(output_predicate)
                .output(1);
        })
    }

    fn build_and_verify(program: Program) -> Result<(TxID, TxLog), VMError> {
        let (txlog, tx) = {
            // Build tx
            let bp_gens = BulletproofGens::new(256, 1);
            let header = TxHeader {
                version: 0u64,
                mintime_ms: 0u64,
                maxtime_ms: 0u64,
            };
            let utx = Prover::build_tx(program, header, &bp_gens)?;

            let sig = if utx.signing_instructions.len() == 0 {
                Signature {
                    R: CompressedRistretto::identity(),
                    s: Scalar::zero(),
                }
            } else {
                // find all the secret scalars for the pubkeys used in the VM
                let privkeys: Vec<Scalar> = utx
                    .signing_instructions
                    .iter()
                    .map(|(predicate, _msg)| predicate_privkey(predicate))
                    .collect();

                let mut signtx_transcript = Transcript::new(b"ZkVM.signtx");
                signtx_transcript.append_message(b"txid", &utx.txid.0);
                Signature::sign_multi(
                    privkeys,
                    utx.signing_instructions
                        .iter()
                        .map(|(p, m)| (p.verification_key(), m))
                        .collect(),
                    &mut signtx_transcript,
                )
                .unwrap()
            };

            (utx.txlog.clone(), utx.sign(sig))
        };

        // Verify tx
        let bp_gens = BulletproofGens::new(256, 1);

        let vtx = tx.verify(&bp_gens)?;
        Ok((vtx.id, txlog))
    }

    fn predicate_privkey(pred: &Predicate) -> Scalar {
        if let Some(privkey) = pred.verification_key_witness::<Scalar>() {
            return *privkey;
        }
        panic!("Expect witness in the Predicate in these tests");
    }
}