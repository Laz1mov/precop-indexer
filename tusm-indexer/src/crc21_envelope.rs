//! Ordinals inscription envelope parser.
//!
//! Extracts:
//! - Tags (1: content type, 5: metadata CBOR, 17: properties CBOR, etc.)
//! - Multi-push concatenation for tag values and body
//! - Body sha256
//! - Tag 5 parsed vault state if present

use bitcoin::script::Instruction;
use bitcoin::ScriptBuf;
use sha2::{Digest, Sha256};
use crate::crc21_cbor::CborValue;

#[derive(Debug, Clone, PartialEq)]
pub struct InscriptionEnvelope {
    pub content_type: Option<String>,
    pub metadata_raw: Option<Vec<u8>>,
    pub properties_raw: Option<Vec<u8>>,
    pub body: Vec<u8>,
    pub body_sha256: [u8; 32],
    pub metadata_cbor: Option<CborValue>,
    pub properties_cbor: Option<CborValue>,
}

impl InscriptionEnvelope {
    /// Inspect a witness script for an Ordinals envelope.
    pub fn parse_from_witness_script(script: &ScriptBuf) -> Option<Self> {
        let mut instructions = script.instructions();

        // 1. Look for OP_0 OP_IF OP_PUSH "ord"
        let mut found_envelope = false;
        let mut expect_ord = false;

        while let Some(Ok(instruction)) = instructions.next() {
            match instruction {
                Instruction::PushBytes(bytes) if bytes.is_empty() => {
                    if let Some(Ok(Instruction::Op(bitcoin::opcodes::all::OP_IF))) = instructions.next() {
                        expect_ord = true;
                    }
                }
                Instruction::Op(bitcoin::opcodes::all::OP_PUSHBYTES_0) => {
                    if let Some(Ok(Instruction::Op(bitcoin::opcodes::all::OP_IF))) = instructions.next() {
                        expect_ord = true;
                    }
                }
                Instruction::PushBytes(bytes) if expect_ord => {
                    if bytes.as_bytes() == b"ord" {
                        found_envelope = true;
                        break;
                    }
                    expect_ord = false;
                }
                _ => {
                    expect_ord = false;
                }
            }
        }

        if !found_envelope {
            return None;
        }

        // 2. We are inside the envelope. Process tags and body.
        let mut current_tag: Option<u64> = None;
        let mut tag_values: std::collections::HashMap<u64, Vec<u8>> = std::collections::HashMap::new();
        let mut in_body = false;
        let mut body_bytes: Vec<u8> = Vec::new();

        while let Some(Ok(instruction)) = instructions.next() {
            match instruction {
                Instruction::Op(bitcoin::opcodes::all::OP_ENDIF) => {
                    break;
                }
                Instruction::Op(bitcoin::opcodes::all::OP_PUSHBYTES_0) => {
                    // Empty push / OP_0 in envelope signifies start of body
                    in_body = true;
                    current_tag = None;
                }
                Instruction::PushBytes(bytes) if bytes.is_empty() => {
                    // Empty push also signifies start of body
                    in_body = true;
                    current_tag = None;
                }
                Instruction::PushBytes(bytes) => {
                    let b = bytes.as_bytes();
                    if in_body {
                        body_bytes.extend_from_slice(b);
                    } else if let Some(tag) = current_tag {
                        // Value push for the active tag (can be chunked)
                        tag_values.entry(tag).or_default().extend_from_slice(b);
                        // In Ordinals spec, each tag has a single field value (possibly chunked)
                        // After one tag value, next push is the next tag
                        current_tag = None;
                    } else {
                        // This push defines a new tag
                        let tag_num = match b.len() {
                            1 => b[0] as u64,
                            2 => u16::from_le_bytes([b[0], b[1]]) as u64,
                            4 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as u64,
                            _ => 9999, // Unknown / invalid tag
                        };
                        current_tag = Some(tag_num);
                    }
                }
                Instruction::Op(op) => {
                    // Check if it's OP_1 to OP_16 as a tag number push
                    let op_val = op.to_u8();
                    if !in_body && current_tag.is_none() && (81..=96).contains(&op_val) {
                        current_tag = Some((op_val - 80) as u64);
                    }
                }
            }
        }

        let content_type = tag_values
            .get(&1)
            .and_then(|v| String::from_utf8(v.clone()).ok());

        let metadata_raw = tag_values.remove(&5);
        let properties_raw = tag_values.remove(&17);

        let metadata_cbor = metadata_raw
            .as_deref()
            .and_then(|raw| CborValue::parse(raw).ok());
        let properties_cbor = properties_raw
            .as_deref()
            .and_then(|raw| CborValue::parse(raw).ok());

        let body_sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(&body_bytes);
            hasher.finalize().into()
        };

        Some(Self {
            content_type,
            metadata_raw,
            properties_raw,
            body: body_bytes,
            body_sha256,
            metadata_cbor,
            properties_cbor,
        })
    }

    /// Extract VaultStateV2 from metadata CBOR if present (>= 120 bytes)
    pub fn get_vault_state_bytes(&self) -> Option<&[u8]> {
        if let Some(ref cbor) = self.metadata_cbor {
            if let Some(s) = cbor.get_str_key("crc21_vault_v1").and_then(|v| v.as_bytes()) {
                if s.len() >= 120 {
                    return Some(s);
                }
            }
            if let Some(s) = cbor.get_str_key("crc21_vault_v2").and_then(|v| v.as_bytes()) {
                if s.len() >= 120 {
                    return Some(s);
                }
            }
        }
        None
    }

    /// Extract item name and traits from properties CBOR
    /// Properties format in Koala: `{ 1: { 0: "Merkel Koala #2305", 1: { traits... } } }`
    pub fn get_properties_info(&self) -> (Option<String>, serde_json::Value) {
        if let Some(ref cbor) = self.properties_cbor {
            if let Some(sub) = cbor.get_u64_key(1) {
                let name = sub.get_u64_key(0).and_then(|v| v.as_str()).map(|s| s.to_string());
                let traits = sub.get_u64_key(1).map(|v| v.to_json()).unwrap_or(serde_json::json!({}));
                return (name, traits);
            }
        }
        (None, serde_json::json!({}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_koala_2305_envelope() {
        let hex_str = include_str!("../tests/fixtures/koala_2305_reveal.hex").trim();
        let raw = hex::decode(hex_str).unwrap();
        let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&raw).unwrap();
        
        let witness = &tx.input[0].witness;
        let script_bytes = witness.iter().nth(witness.len() - 2).unwrap();
        let script = ScriptBuf::from(script_bytes.to_vec());

        let env = InscriptionEnvelope::parse_from_witness_script(&script).expect("Envelope should be found");
        assert_eq!(env.content_type.as_deref(), Some("image/svg+xml"));
        let state_bytes = env.get_vault_state_bytes().expect("Vault state 220 bytes should be found");
        assert_eq!(state_bytes.len(), 220);
        
        let (name, traits) = env.get_properties_info();
        assert_eq!(name.as_deref(), Some("Merkel Koala #2305"));
        assert_eq!(traits["Background"], "Blue");
        assert_eq!(traits["Hats"], "Orange Beanie");
        assert!(!env.body.is_empty());
        assert_eq!(env.body.len(), 520 + 259);
    }
}
