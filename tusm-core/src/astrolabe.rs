
fn encode_law(law_cmr: &[u8; 32], supply: u128, version: u8, owner_pubkey: &bitcoin::secp256k1::XOnlyPublicKey) -> bitcoin::ScriptBuf {
    // Canonical serialization: OwnerPubKey || OP_CHECKSIGVERIFY || StateCommit(CMR_comf || supply as u128 LE || version)
    let mut state = Vec::new();
    state.extend_from_slice(law_cmr);
    state.extend_from_slice(&supply.to_le_bytes());
    state.push(version);
    
    let state_push = bitcoin::script::PushBytesBuf::try_from(state).unwrap();
    bitcoin::script::Builder::new()
        .push_x_only_key(owner_pubkey)
        .push_opcode(bitcoin::opcodes::all::OP_CHECKSIGVERIFY)
        .push_slice(state_push.as_push_bytes())
        .push_opcode(bitcoin::opcodes::all::OP_DROP)
        .push_opcode(bitcoin::opcodes::OP_TRUE)
        .into_script()
}

// 5. Compute Q
pub fn compute_astrolabe(supply: u128, law_cmr: [u8; 32], owner_pubkey: bitcoin::secp256k1::XOnlyPublicKey) -> bitcoin::ScriptBuf {
    use bitcoin::secp256k1::{Secp256k1, XOnlyPublicKey};
    use bitcoin::taproot::TaprootBuilder;
    use std::str::FromStr;

    let tap_script = encode_law(&law_cmr, supply, 1, &owner_pubkey);

    let secp = Secp256k1::verification_only();

    // Standard NUMS XOnlyPublicKey from BIP-341 (H(uncompressed G))
    let internal_key = XOnlyPublicKey::from_str(
        "50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0",
    )
    .unwrap();

    let builder = TaprootBuilder::new().add_leaf(0, tap_script).unwrap();
    let spend_info = builder.finalize(&secp, internal_key).unwrap();

    // Q = P + t*G is mathematically executed inside new_v1_p2tr / TaprootSpendInfo
    bitcoin::ScriptBuf::new_v1_p2tr(&secp, internal_key, spend_info.merkle_root())
}

pub fn verify_astrolabe_anchor(
    supply: u128,
    law_cmr: [u8; 32],
    actual_anchor_script: &bitcoin::ScriptBuf,
    owner_pubkey: bitcoin::secp256k1::XOnlyPublicKey,
) -> bool {
    let expected = compute_astrolabe(supply, law_cmr, owner_pubkey);
    &expected == actual_anchor_script
}

/// CRC-21: Encodage de la feuille de Claim (Propriétaire / Vault)
/// claim = PUSH32(C) DROP 5000 CSV DROP SHA256 PUSH32(h_A) EQUALVERIFY PUSH32(T) CHECKSIG
pub fn build_claim_leaf(
    c: &[u8],
    h_a: &[u8],
    t: &bitcoin::secp256k1::XOnlyPublicKey,
) -> bitcoin::ScriptBuf {
    let c_push = bitcoin::script::PushBytesBuf::try_from(c.to_vec()).unwrap();
    let h_a_push = bitcoin::script::PushBytesBuf::try_from(h_a.to_vec()).unwrap();
    
    bitcoin::script::Builder::new()
        .push_slice(c_push.as_push_bytes())
        .push_opcode(bitcoin::opcodes::all::OP_DROP)
        .push_int(5000)
        .push_opcode(bitcoin::opcodes::all::OP_CSV)
        .push_opcode(bitcoin::opcodes::all::OP_DROP)
        .push_opcode(bitcoin::opcodes::all::OP_SHA256)
        .push_slice(h_a_push.as_push_bytes())
        .push_opcode(bitcoin::opcodes::all::OP_EQUALVERIFY)
        .push_x_only_key(t)
        .push_opcode(bitcoin::opcodes::all::OP_CHECKSIG)
        .into_script()
}

/// CRC-21: Encodage de la feuille de Owner rescue
/// owner = PUSH32(C) DROP 10000 CSV DROP PUSH32(O) CHECKSIG
pub fn build_owner_leaf(
    c: &[u8],
    o: &bitcoin::secp256k1::XOnlyPublicKey,
) -> bitcoin::ScriptBuf {
    let c_push = bitcoin::script::PushBytesBuf::try_from(c.to_vec()).unwrap();
    
    bitcoin::script::Builder::new()
        .push_slice(c_push.as_push_bytes())
        .push_opcode(bitcoin::opcodes::all::OP_DROP)
        .push_int(10000)
        .push_opcode(bitcoin::opcodes::all::OP_CSV)
        .push_opcode(bitcoin::opcodes::all::OP_DROP)
        .push_x_only_key(o)
        .push_opcode(bitcoin::opcodes::all::OP_CHECKSIG)
        .into_script()
}

/// CRC-21: Encodage de la feuille de Public recovery
/// public = PUSH32(C) DROP 21000 CSV
pub fn build_public_leaf(
    c: &[u8],
) -> bitcoin::ScriptBuf {
    let c_push = bitcoin::script::PushBytesBuf::try_from(c.to_vec()).unwrap();
    
    bitcoin::script::Builder::new()
        .push_slice(c_push.as_push_bytes())
        .push_opcode(bitcoin::opcodes::all::OP_DROP)
        .push_int(21000)
        .push_opcode(bitcoin::opcodes::all::OP_CSV)
        .into_script()
}

/// CRC-21: Génère le Vault (Covenant) MAST avec ses 3 branches temporelles
pub fn compute_crc21_vault(
    t: bitcoin::secp256k1::XOnlyPublicKey,
    o: bitcoin::secp256k1::XOnlyPublicKey,
    h_a: &[u8],
    c: &[u8],
) -> (bitcoin::Address, bitcoin::taproot::TapNodeHash) {
    use bitcoin::secp256k1::Secp256k1;
    use bitcoin::taproot::TaprootBuilder;
    use std::str::FromStr;

    let secp = Secp256k1::verification_only();
    let internal_key = bitcoin::secp256k1::XOnlyPublicKey::from_str(
        "50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0",
    ).unwrap();

    let claim_script = build_claim_leaf(c, h_a, &t);
    let owner_script = build_owner_leaf(c, &o);
    let public_script = build_public_leaf(c);

    let builder = TaprootBuilder::new()
        .add_leaf(1, claim_script).unwrap()
        .add_leaf(2, owner_script).unwrap()
        .add_leaf(2, public_script).unwrap();

    let spend_info = builder.finalize(&secp, internal_key).unwrap();
    let merkle_root = spend_info.merkle_root().unwrap();

    let address = bitcoin::Address::p2tr_tweaked(spend_info.output_key(), bitcoin::Network::Bitcoin);

    (address, merkle_root)
}

/// Public accessor for the law-anchor leaf (used by the indexer to publish the covenant topology).
pub fn encode_law_public(law_cmr: &[u8; 32], supply: u128, version: u8, owner_pubkey: &bitcoin::secp256k1::XOnlyPublicKey) -> bitcoin::ScriptBuf {
    encode_law(law_cmr, supply, version, owner_pubkey)
}
