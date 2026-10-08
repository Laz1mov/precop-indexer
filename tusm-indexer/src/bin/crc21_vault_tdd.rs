use bitcoin::hashes::{sha256, Hash, HashEngine};
use bitcoin::secp256k1::XOnlyPublicKey;
use std::str::FromStr;

fn tagged_hash(tag: &str, data: &[u8]) -> [u8; 32] {
    let tag_hash = sha256::Hash::hash(tag.as_bytes());
    let mut engine = sha256::Hash::engine();
    engine.input(&tag_hash[..]);
    engine.input(&tag_hash[..]);
    engine.input(data);
    sha256::Hash::from_engine(engine).to_byte_array()
}

fn build_claim_leaf(c: &[u8], h_a: &[u8], t: &XOnlyPublicKey) -> bitcoin::ScriptBuf {
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

fn build_owner_leaf(c: &[u8], o: &XOnlyPublicKey) -> bitcoin::ScriptBuf {
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

fn build_public_leaf(c: &[u8]) -> bitcoin::ScriptBuf {
    let c_push = bitcoin::script::PushBytesBuf::try_from(c.to_vec()).unwrap();
    bitcoin::script::Builder::new()
        .push_slice(c_push.as_push_bytes())
        .push_opcode(bitcoin::opcodes::all::OP_DROP)
        .push_int(21000)
        .push_opcode(bitcoin::opcodes::all::OP_CSV)
        .into_script()
}

fn compute_crc21_vault(
    t: XOnlyPublicKey,
    o: XOnlyPublicKey,
    h_a: &[u8],
    c: &[u8],
) -> bitcoin::Address {
    use bitcoin::secp256k1::Secp256k1;
    use bitcoin::taproot::TaprootBuilder;

    let secp = Secp256k1::verification_only();
    let internal_key = XOnlyPublicKey::from_str(
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
    bitcoin::Address::p2tr_tweaked(spend_info.output_key(), bitcoin::Network::Bitcoin)
}

fn main() {
    let s_hex = "0200f9b31aed5f7cc86eff6f6a13d3dccd0e5839a0d676e958684bd2aaebb5583e0d0000000001090000881308527b20f23162982bc0e7888bfd060135ee53bb93bffec051a5d96fe46cf38dcf760100000000000000000000000000000000000000000000000000000000000000002202000000000000225120f702b8764e9fcd751ea6eb2468bbe24d34f5396c35d2c61fdb08fee3112d3d0792609d0f51e36b24c74d34fec65976e20f1a497a267df317d871c669f1f2be11f702b8764e9fcd751ea6eb2468bbe24d34f5396c35d2c61fdb08fee3112d3d071027";
    let s_bytes = hex::decode(s_hex).unwrap();
    
    let collection_outpoint = &s_bytes[2..38];
    let item_id = &s_bytes[38..42];
    let t_bytes = hex::decode("7b20f23162982bc0e7888bfd060135ee53bb93bffec051a5d96fe46cf38dcf76").unwrap();
    let o_bytes = hex::decode("f702b8764e9fcd751ea6eb2468bbe24d34f5396c35d2c61fdb08fee3112d3d07").unwrap();
    let h_a_bytes = hex::decode("92609d0f51e36b24c74d34fec65976e20f1a497a267df317d871c669f1f2be11").unwrap();
    
    // Find offsets in s_bytes
    let t_idx = s_bytes.windows(32).position(|w| w == t_bytes.as_slice()).unwrap();
    let o_idx = s_bytes.windows(32).position(|w| w == o_bytes.as_slice()).unwrap();
    let h_a_idx = s_bytes.windows(32).position(|w| w == h_a_bytes.as_slice()).unwrap();
    
    println!("T index: {}", t_idx);
    println!("h_A index: {}", h_a_idx);
    println!("O index: {}", o_idx);
    
    let cmr_bytes = &s_bytes[t_idx+32 .. t_idx+32+32];
    println!("CMR bytes: {:?}", hex::encode(cmr_bytes));
    
    let state_root = tagged_hash("crc-21/VaultState/v2", &s_bytes);
    
    let mut commitment_data = vec![];
    commitment_data.extend_from_slice(collection_outpoint);
    commitment_data.extend_from_slice(item_id);
    commitment_data.extend_from_slice(cmr_bytes);
    commitment_data.extend_from_slice(&state_root);
    
    let c = tagged_hash("crc-21/VaultCommitment/v2", &commitment_data);
    
    let vault_pubkey = hex::decode("d9bd0d2c915dd52375ebf127474cfbb8c3ac21e1956cbf26ea8d2de147b55ef4").unwrap();
    if let Some(idx) = s_bytes.windows(32).position(|w| w == vault_pubkey.as_slice()) {
        println!("Vault pubkey found at index {}", idx);
    } else {
        println!("Vault pubkey NOT found in S");
    }
}
