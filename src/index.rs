use bitcoincore_rpc::RpcApi;
use bitcoin::{Transaction, BlockHash};
use bitcoin::blockdata::script::Instruction;
use bitcoin::blockdata::opcodes;

pub struct Tracker {
    client: bitcoincore_rpc::Client,
}

impl Tracker {
    pub fn new(client: bitcoincore_rpc::Client) -> Self {
        Tracker { client }
    }

    /// Returns sidechain transactions indexes for giving bitcoin block id.
    pub fn check_bitcoin_block_with_id(&self, id: u64) -> Vec<BitcoinMintOutput> {
        let hash = self.client.get_block_hash(id).unwrap();
        self.check_bitcoin_block_with_hash(hash)
    }

    /// Returns sidechain transactions indexes for giving bitcoin block hash.
    pub fn check_bitcoin_block_with_hash(&self, hash: BlockHash) -> Vec<BitcoinMintOutput> {
        let block = self.client.get_block(&hash).unwrap();
        let txs = block.txdata;

        let mint_txs = txs.into_iter().enumerate().filter_map(|(tx_pos, tx)| {
            parse_mint_transaction(&hash, tx_pos as u64, tx)
        }).collect();

        mint_txs
    }
}

pub fn parse_mint_transaction(block_hash: &BlockHash, tx_pos: u64, tx: Transaction) -> Option<BitcoinMintOutput> {
    // TODO: Can be multiply OP_RETURN outputs?
    tx.output.iter().enumerate().filter_map(|(out_pos, out)| {
        let mut instructions = out.script_pubkey.instructions();
        let first_instruction = instructions.next().and_then(|res| res.ok());
        match first_instruction {
            Some(Instruction::Op(opcodes::all::OP_RETURN)) => {
                let push_bytes_instr = instructions.next().and_then(|res| res.ok());
                let bytes = match push_bytes_instr {
                    Some(Instruction::PushBytes(bytes)) => bytes,
                    _ => return None
                };
                let amount = out.value;
                let bytes = Box::<[u8]>::from(bytes);
                Some(BitcoinMintOutput {
                    index: BitcoinMintOutputIndex {
                        block_hash: block_hash.clone(),
                        transaction_position: tx_pos,
                        output_position: out_pos as u64,
                    },
                    amount,
                    bytes
                })
            }
            _ => { None }
        }
    }).next()
}

pub struct BitcoinMintOutput {
    index: BitcoinMintOutputIndex,
    amount: u64,
    bytes: Box<[u8]>
}

pub struct BitcoinMintOutputIndex {
    block_hash: BlockHash,
    transaction_position: u64,
    output_position: u64,
}
