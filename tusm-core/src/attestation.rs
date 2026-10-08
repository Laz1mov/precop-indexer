use std::str::FromStr;
use bitcoin::hashes::{sha256, Hash};

pub const DEFAULT_ORACLE_PUBKEY: &str = "382b79806cc956d6f88236a8fdd70b7bfce89c2745b04f8f694df2302a853f92";
pub const DEFAULT_ATTESTATION_SECRET: &str = "precop_oracle_sovereign_secret";

/// Canonical historical Genesis V1 DOG transactions authorized at block 968,966
pub const CANONICAL_HISTORICAL_DOG_TXIDS: [&str; 4] = [
    "c892254bd7550ccac21efa53d5a18d33f55fc10f6b910e6f047c16af63b0e256",
    "85d6be0c2993205778904ae450134ebfa7a154d54ef0ccc574b3aee652098667",
    "bf010e3ac77429e6f263cbcb489cfc0bb253585d693fc0e5c2e57d7c5d751c0b",
    "133f4b6b843c2b8b9d6135d19e88e5db862da0b19ce2d82d44a95cb30f3273b0",
];

pub fn is_canonical_historical_payment(txid: &str, _asset: &str) -> bool {
    CANONICAL_HISTORICAL_DOG_TXIDS.contains(&txid)
}

pub fn compute_attestation_digest(txid: &str, asset: &str, amount: u128, rate: Option<u64>) -> [u8; 32] {
    let rate_str = rate.map(|r| r.to_string()).unwrap_or_default();
    let msg = format!("PRECOP_CANONICAL_ATTESTATION_V1:{}:{}:{}:{}", txid, asset, amount, rate_str);
    sha256::Hash::hash(msg.as_bytes()).to_byte_array()
}

pub fn verify_attestation_signature(txid: &str, asset: &str, amount: u128, rate: Option<u64>, sig_hex: &str) -> bool {
    let digest = compute_attestation_digest(txid, asset, amount, rate);
    
    if let Ok(sig_bytes) = hex::decode(sig_hex) {
        if sig_bytes.len() == 64 {
            // BIP-340 Schnorr signature
            let secp = bitcoin::secp256k1::Secp256k1::verification_only();
            if let Ok(sig) = bitcoin::secp256k1::schnorr::Signature::from_slice(&sig_bytes) {
                let oracle_pk_str = std::env::var("CANONICAL_ORACLE_PUBKEY").unwrap_or_else(|_| DEFAULT_ORACLE_PUBKEY.to_string());
                if let Ok(pk) = bitcoin::secp256k1::XOnlyPublicKey::from_str(&oracle_pk_str) {
                    let msg = bitcoin::secp256k1::Message::from_slice(&digest).unwrap();
                    return secp.verify_schnorr(&sig, &msg, &pk).is_ok();
                }
            }
        } else if sig_bytes.len() == 32 {
            // Shared Attestation Secret HMAC-SHA256
            let secret = std::env::var("SIDECAR_ATTESTATION_SECRET").unwrap_or_else(|_| DEFAULT_ATTESTATION_SECRET.to_string());
            let expected_hmac = sha256::Hash::hash(format!("{}:{}", hex::encode(digest), secret).as_bytes()).to_byte_array();
            return sig_bytes.as_slice() == expected_hmac.as_slice();
        }
    }
    false
}

pub fn create_attestation_hmac(txid: &str, asset: &str, amount: u128, rate: Option<u64>, secret: &str) -> String {
    let digest = compute_attestation_digest(txid, asset, amount, rate);
    let hmac = sha256::Hash::hash(format!("{}:{}", hex::encode(digest), secret).as_bytes()).to_byte_array();
    hex::encode(hmac)
}
