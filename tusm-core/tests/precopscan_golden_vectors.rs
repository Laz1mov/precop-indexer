//! Cross-language parity vectors: tusm-core (Rust, consensus) -> Precopscan (TypeScript, verifier).
//!
//! Run:  cargo test -p tusm-core --test precopscan_golden_vectors -- --nocapture
//! It (re)writes `precopscan/tests/fixtures/tusm_core_golden.json`, which the Precopscan
//! vitest suite consumes. Any divergence between the VM and the explorer fails CI on both sides.

use bitcoin::secp256k1::{Secp256k1, XOnlyPublicKey};
use std::str::FromStr;
use tusm_core::astrolabe::{compute_astrolabe, compute_crc21_vault};
use tusm_core::legacy_v1_router::LegacyV1Router;

const OWNER: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"; // BIP-340 vector #1 pubkey
const T_KEY: &str = "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"; // BIP-340 vector #2 pubkey
const C: &str = "6c4f39bb3240dd0295de04eed4a70bddb6ad150f362617f8bcb48a2b670c1f2e";
const H_A: &str = "116f5736d68aef805656c2fb3b2740ebef9cd9a428bd5f62611588550f27a28a";
const LAW_CMR: &str = "3e7b332b5816d972abe801cc0e012179d2299c0d2d9924ff0a36cc990fb14f43";

#[test]
fn emit_precopscan_golden_vectors() {
    let secp = Secp256k1::new();
    let owner = XOnlyPublicKey::from_str(OWNER).unwrap();
    let t_key = XOnlyPublicKey::from_str(T_KEY).unwrap();

    let mut tge = Vec::new();
    for lock in [144u16, 1000, 2100, 6767] {
        for has_cmr in [false, true] {
            let spk = LegacyV1Router::derive_taproot_for_lock(&secp, lock, owner, has_cmr);
            let addr = bitcoin::Address::from_script(&spk, bitcoin::Network::Bitcoin).unwrap();
            tge.push(serde_json::json!({
                "lock": lock, "withCmr": has_cmr,
                "scriptPubKey": hex::encode(spk.as_bytes()), "address": addr.to_string()
            }));
        }
    }

    let (crc21_addr, crc21_root) =
        compute_crc21_vault(t_key, owner, &hex::decode(H_A).unwrap(), &hex::decode(C).unwrap());

    let mut law = [0u8; 32];
    law.copy_from_slice(&hex::decode(LAW_CMR).unwrap());
    let supply: u128 = 100_000_000_000_000_000;
    let anchor = compute_astrolabe(supply, law, owner);
    let anchor_addr = bitcoin::Address::from_script(&anchor, bitcoin::Network::Bitcoin).unwrap();

    let out = serde_json::json!({
        "generator": "tusm-core/tests/precopscan_golden_vectors.rs",
        "inputs": { "owner": OWNER, "tKey": T_KEY, "c": C, "hA": H_A, "lawCmr": LAW_CMR, "supply": supply.to_string() },
        "tgeCsv": tge,
        "crc21Vault": { "address": crc21_addr.to_string(), "merkleRoot": crc21_root.to_string() },
        "lawAnchor": { "scriptPubKey": hex::encode(anchor.as_bytes()), "address": anchor_addr.to_string() }
    });

    let pretty = serde_json::to_string_pretty(&out).unwrap();
    println!("{}", pretty);
    let target = concat!(env!("CARGO_MANIFEST_DIR"), "/../../precopscan/tests/fixtures/tusm_core_golden.json");
    if std::path::Path::new(target).parent().map(|p| p.exists()).unwrap_or(false) {
        std::fs::write(target, pretty).unwrap();
    }
}
