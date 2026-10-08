use bitcoin::blockdata::script::Builder;
use bitcoin::blockdata::opcodes::all::*;
use bitcoin::taproot::{TaprootBuilder, LeafVersion};
use bitcoin::secp256k1::{XOnlyPublicKey, Secp256k1};
use bitcoin::hashes::hex::FromHex;
use std::str::FromStr;

fn main() {
    let secp = Secp256k1::new();
    
    // 1. The NUMS internal key (standard BIP-341)
    let nums_hex = "50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0";
    let internal_key = XOnlyPublicKey::from_str(nums_hex).unwrap();
    
    // 2. The CMR (Commitment Merkle Root) from the user's screenshot: 3e7b33...4f43
    // We will use a dummy 32-byte CMR for the test, but we need the exact one for production.
    let cmr_hex = "3e7b330000000000000000000000000000000000000000000000000000004f43";
    let cmr = Vec::from_hex(cmr_hex).unwrap();
    
    // 3. The user's public key (Extracted from the first input's script_pubkey).
    // Let's use a dummy x-only pubkey for testing.
    let user_pubkey_hex = "50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0";
    let user_pubkey = XOnlyPublicKey::from_str(user_pubkey_hex).unwrap();
    
    let locks = vec![144, 1000, 2100, 6767];
    
    println!("Testing Taproot CSV Derivation for Pubkey: {}", user_pubkey_hex);
    
    for lock in locks {
        // Construct the leaf script WITHOUT CMR:
        // <lock> OP_CHECKSEQUENCEVERIFY OP_DROP <pubkey> OP_CHECKSIG
        let mut script_builder = Builder::new();
            
        // Push the lock correctly as a CScriptNum
        // In Bitcoin Script, integers are pushed as little-endian with special rules.
        script_builder = script_builder.push_int(lock);
            
        let script = script_builder
            .push_opcode(OP_CSV)
            .push_opcode(OP_DROP)
            .push_slice(<&bitcoin::script::PushBytes>::try_from(user_pubkey.serialize().as_slice()).unwrap())
            .push_opcode(OP_CHECKSIG)
            .into_script();
            
        // Build the Taproot tree with a single leaf
        let taproot_builder = TaprootBuilder::new()
            .add_leaf(0, script.clone())
            .expect("Failed to add leaf");
            
        let spend_info = taproot_builder.finalize(&secp, internal_key).unwrap();
        let derived_address = bitcoin::Address::p2tr_tweaked(spend_info.output_key(), bitcoin::Network::Bitcoin);
        
        println!("Lock {:4} -> Address: {}", lock, derived_address);
        println!("  Leaf Script Hex: {}", hex::encode(script.as_bytes()));
    }
}
