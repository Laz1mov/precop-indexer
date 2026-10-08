use bitcoin::{Block, BlockHash, OutPoint, Transaction};
use std::collections::{HashMap, HashSet};
use tusm_core::prevout::{Coin, PrevoutError, PrevoutProvider};
use serde_json::Value;

#[derive(Debug)]
pub struct ResolvedBlock {
    pub block_hash: BlockHash,
    pub height: u32,
    pub prevouts: HashMap<OutPoint, Coin>,
}

pub struct CandidateExtractor;

impl CandidateExtractor {
    pub fn is_candidate(tx: &Transaction) -> bool {
        for out in &tx.output {
            if out.script_pubkey.is_op_return() {
                for instruction in out.script_pubkey.instructions() {
                    if let Ok(bitcoin::script::Instruction::PushBytes(bytes)) = instruction {
                        if let Ok(json_str) = std::str::from_utf8(bytes.as_bytes()) {
                            if let Ok(v) = serde_json::from_str::<Value>(json_str) {
                                if let Some(p) = v.get("p").and_then(|p| p.as_str()) {
                                    if p == "crc-20" || p == "ico-20" || p == "crc-21" {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    pub fn extract_candidate_outpoints(block: &Block) -> Vec<OutPoint> {
        let mut outpoints = HashSet::new();
        for tx in &block.txdata {
            if !tx.is_coin_base() {
                for out in &tx.output {
                    if out.script_pubkey.is_op_return() {
                        for instruction in out.script_pubkey.instructions() {
                            if let Ok(bitcoin::script::Instruction::PushBytes(bytes)) = instruction {
                                if let Ok(json_str) = std::str::from_utf8(bytes.as_bytes()) {
                                    if let Ok(v) = serde_json::from_str::<Value>(json_str) {
                                        if let Some(p) = v.get("p").and_then(|p| p.as_str()) {
                                            if p == "crc-20" || p == "crc-21" {
                                                for input in &tx.input {
                                                    outpoints.insert(input.previous_output);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        outpoints.into_iter().collect()
    }
}

pub struct BlockBridge;

impl BlockBridge {
    pub fn resolve(
        block: &Block,
        height: u32,
        provider: &impl PrevoutProvider,
    ) -> Result<ResolvedBlock, PrevoutError> {
        let outpoints = CandidateExtractor::extract_candidate_outpoints(block);
        
        // resolve_prevouts automatically deduplicates since extract returns a HashSet-deduped Vec
        // and batch_lookup returns an atomic error if ANY outpoint is missing.
        let prevouts = provider.batch_lookup(&outpoints)?;
        
        Ok(ResolvedBlock {
            block_hash: block.block_hash(),
            height,
            prevouts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{TxIn, TxOut, ScriptBuf, Txid, Sequence, Witness};
    use std::str::FromStr;
    use bitcoin::hashes::Hash;

    struct TestPrevoutProvider {
        cache: HashMap<OutPoint, Coin>,
    }
    
    impl PrevoutProvider for TestPrevoutProvider {
        fn lookup(&self, outpoint: OutPoint) -> Result<Coin, PrevoutError> {
            self.cache.get(&outpoint).cloned().ok_or(PrevoutError::MissingPrevout(outpoint))
        }
    }

    fn make_op_return_tx_with_inputs(json_payload: &str, inputs: Vec<OutPoint>) -> Transaction {
        let mut op_return_script = vec![0x6a]; // OP_RETURN
        let payload_bytes = json_payload.as_bytes();
        if payload_bytes.len() <= 75 {
            op_return_script.push(payload_bytes.len() as u8);
        }
        op_return_script.extend_from_slice(payload_bytes);
        
        Transaction {
            version: 1,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: inputs.into_iter().map(|op| TxIn {
                previous_output: op,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }).collect(),
            output: vec![TxOut {
                value: 0,
                script_pubkey: ScriptBuf::from_bytes(op_return_script),
            }],
        }
    }

    #[test]
    fn test_candidate_extraction_mints_and_deploys_ignore_prevouts() {
        let txid1 = Txid::from_str("1111111111111111111111111111111111111111111111111111111111111111").unwrap();
        let txid2 = Txid::from_str("2222222222222222222222222222222222222222222222222222222222222222").unwrap();

        let tx_deploy = make_op_return_tx_with_inputs(r#"{"p":"crc-20","op":"deploy"}"#, vec![OutPoint { txid: txid1, vout: 0 }]);
        let tx_mint = make_op_return_tx_with_inputs(r#"{"p":"crc-20","op":"mint"}"#, vec![OutPoint { txid: txid2, vout: 0 }]);

        let block = Block {
            header: bitcoin::block::Header { version: bitcoin::block::Version::from_consensus(1), prev_blockhash: bitcoin::BlockHash::all_zeros(), merkle_root: bitcoin::hash_types::TxMerkleNode::all_zeros(), time: 0, bits: bitcoin::CompactTarget::from_consensus(0), nonce: 0 },
            txdata: vec![tx_deploy.clone(), tx_mint.clone()],
        };

        assert!(CandidateExtractor::is_candidate(&tx_deploy));
        assert!(CandidateExtractor::is_candidate(&tx_mint));

        let outpoints = CandidateExtractor::extract_candidate_outpoints(&block);
        assert_eq!(outpoints.len(), 2, "Deploy and Mint should require all prevouts in V2");
    }

    #[test]
    fn test_candidate_extraction_transfer_requires_only_first_prevout() {
        let txid1 = Txid::from_str("1111111111111111111111111111111111111111111111111111111111111111").unwrap();
        let txid2 = Txid::from_str("2222222222222222222222222222222222222222222222222222222222222222").unwrap();
        
        let op1 = OutPoint { txid: txid1, vout: 0 };
        let op2 = OutPoint { txid: txid2, vout: 0 };

        let tx_transfer = make_op_return_tx_with_inputs(r#"{"p":"crc-20","op":"transfer"}"#, vec![op1, op2]);

        let block = Block {
            header: bitcoin::block::Header { version: bitcoin::block::Version::from_consensus(1), prev_blockhash: bitcoin::BlockHash::all_zeros(), merkle_root: bitcoin::hash_types::TxMerkleNode::all_zeros(), time: 0, bits: bitcoin::CompactTarget::from_consensus(0), nonce: 0 },
            txdata: vec![tx_transfer.clone()],
        };

        assert!(CandidateExtractor::is_candidate(&tx_transfer));

        let outpoints = CandidateExtractor::extract_candidate_outpoints(&block);
        assert_eq!(outpoints.len(), 2, "Transfer should require all prevouts");
        assert!(outpoints.contains(&op1));
        assert!(outpoints.contains(&op2));
    }
}
