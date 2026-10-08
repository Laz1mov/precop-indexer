use serde::{Serialize, Deserialize};
use rustreexo::stump::Stump;
use rustreexo::proof::Proof;
use rustreexo::node_hash::BitcoinNodeHash;
use bitcoin_hashes::{sha256, Hash};

pub const MAGIC_TUSMWIT1: &str = "TUSMWIT1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WitnessPacket {
    pub magic: String,
    pub block_hash: String,
    pub height: u32,
    pub parent_hash: String,
    pub schema: String,
    
    // The transaction and the prevouts needed to evaluate it
    pub raw_tx: String, // hex encoded
    pub prevouts: Vec<String>, // hex encoded
    
    // The true Utreexo leaf hashes corresponding to the prevouts (computed by Floresta)
    pub leaf_hashes: Option<Vec<String>>, // hex encoded BitcoinNodeHash
    
    // Utreexo proof components (optional or required depending on the validation mode)
    // Encoded as a hex string of the serialized rustreexo Proof
    pub utreexo_proof: Option<String>,
}

impl WitnessPacket {
    pub fn new(
        block_hash: String,
        height: u32,
        parent_hash: String,
        schema: String,
        raw_tx: String,
        prevouts: Vec<String>,
        leaf_hashes: Option<Vec<String>>,
        utreexo_proof: Option<String>
    ) -> Self {
        Self {
            magic: MAGIC_TUSMWIT1.to_string(),
            block_hash,
            height,
            parent_hash,
            schema,
            raw_tx,
            prevouts,
            leaf_hashes,
            utreexo_proof,
        }
    }

    pub fn serialize(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn deserialize(json: &str) -> Result<Self, serde_json::Error> {
        let packet: Self = serde_json::from_str(json)?;
        if packet.magic != MAGIC_TUSMWIT1 {
            return Err(serde::de::Error::custom("Invalid magic string, expected TUSMWIT1"));
        }
        Ok(packet)
    }

    pub fn verify_utreexo(&self, stump: &Stump) -> bool {
        let proof_hex = match &self.utreexo_proof {
            Some(p) => p,
            None => return false,
        };

        let proof_bytes = match hex::decode(proof_hex) {
            Ok(b) => b,
            Err(_) => return false,
        };

        // 1. Rejet si la preuve est tronquée ou malformée
        let proof = match Proof::deserialize(proof_bytes.as_slice()) {
            Ok(p) => p,
            Err(_) => return false,
        };

        // 2. Rejet si plus profonde que 32
        if proof.hashes.len() > 32 {
            return false;
        }

        if self.prevouts.is_empty() {
            return false;
        }

        // 3. Verify leaf_hashes match prevouts
        let leaf_hashes_hex = match &self.leaf_hashes {
            Some(hashes) => hashes,
            None => return false,
        };
        
        if leaf_hashes_hex.len() != self.prevouts.len() {
            return false;
        }

        // Parse leaf hashes
        let mut parsed_leaf_hashes = Vec::new();
        for hash_hex in leaf_hashes_hex {
            let bytes = match hex::decode(hash_hex) {
                Ok(b) => b,
                Err(_) => return false,
            };
            let mut array = [0u8; 32];
            let len = bytes.len().min(32);
            array[..len].copy_from_slice(&bytes[..len]);
            let node_hash = BitcoinNodeHash::from(array);
            parsed_leaf_hashes.push(node_hash);
        }

        // Verify the proof against the provided Stump (the current accumulator state)
        // This will reject if the proof opens another output, or if it doesn't compute the root.
        match stump.verify(&proof, &parsed_leaf_hashes) {
            Ok(is_valid) => is_valid,
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_w1_codec_canonical() {
        let packet = WitnessPacket::new(
            "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f".to_string(),
            0,
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            "crc20-bonding-v1".to_string(),
            "01000000000000000000".to_string(),
            vec!["0000".to_string()],
            Some(vec!["0000".to_string()]),
            None
        );
        let ser = packet.serialize().unwrap();
        let de = WitnessPacket::deserialize(&ser).unwrap();
        assert_eq!(packet, de);
    }

    #[test]
    fn test_w1_invalid_magic() {
        let json = r#"{"magic":"FAKEMAGIC","block_hash":"0000","height":0,"parent_hash":"0000","schema":"crc20","raw_tx":"00","prevouts":[],"leaf_hashes":null,"utreexo_proof":null}"#;
        let res = WitnessPacket::deserialize(json);
        assert!(res.is_err());
    }

    #[test]
    fn test_w2_utreexo_proof_verification() {
        let fixture_prevout = hex::encode(b"c0ffee");
        let bytes = hex::decode(&fixture_prevout).unwrap();
        let leaf_hash = sha256::Hash::hash(&bytes);
        let leaf_hash_hex = hex::encode(leaf_hash.to_byte_array());
        let node_hash = BitcoinNodeHash::from(leaf_hash.to_byte_array());

        // Create a valid Stump with the fixture_prevout as its only leaf
        let empty_stump = Stump::new();
        // modify returns (Stump, Vec<NodeHash>) in 0.6.0
        let stump = empty_stump.modify(&[node_hash.clone()], &[], &Proof::default()).unwrap().0;
        
        // Valid proof for leaf 0 in a 1-leaf tree has no sibling hashes
        let valid_proof = Proof::new(vec![0], vec![]);
        let mut valid_proof_bytes = Vec::new();
        valid_proof.serialize(&mut valid_proof_bytes).unwrap();
        let valid_proof_hex = hex::encode(&valid_proof_bytes);

        let mut packet = WitnessPacket::new(
            "00".to_string(), 0, "00".to_string(), "crc20".to_string(), "00".to_string(), vec![fixture_prevout.to_string()], Some(vec![leaf_hash_hex.clone()]), None
        );

        // Rejet 1: Missing proof
        assert_eq!(packet.verify_utreexo(&stump), false, "Missing proof should fail");

        // Rejet 2: Preuve tronquée (invalid format)
        packet.utreexo_proof = Some("deadbeef".to_string());
        assert_eq!(packet.verify_utreexo(&stump), false, "Truncated proof should fail");

        // Rejet 3: Profondeur > 32
        let too_deep_proof = Proof::new(vec![0], vec![BitcoinNodeHash::default(); 33]);
        let mut too_deep_bytes = Vec::new();
        too_deep_proof.serialize(&mut too_deep_bytes).unwrap();
        packet.utreexo_proof = Some(hex::encode(&too_deep_bytes));
        assert_eq!(packet.verify_utreexo(&stump), false, "Depth > 32 should fail");

        // Rejet 4: Ouvre une autre sortie (mauvais prevout)
        packet.utreexo_proof = Some(valid_proof_hex.clone());
        packet.leaf_hashes = Some(vec![hex::encode([0u8; 32])]);
        assert_eq!(packet.verify_utreexo(&stump), false, "Opening another output should fail");

        // Cas Valide
        packet.leaf_hashes = Some(vec![leaf_hash_hex.clone()]);
        assert_eq!(packet.verify_utreexo(&stump), true, "Valid proof computing to root should pass");
    }
}
