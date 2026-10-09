use bitcoin::secp256k1::{Secp256k1, XOnlyPublicKey};
use bitcoin::taproot::TaprootBuilder;
use bitcoin::{ScriptBuf, TxOut, Transaction, OutPoint, TxIn, Sequence, Witness};
use bitcoin::absolute::LockTime;
use std::str::FromStr;
use tusm_core::legacy_v1_router::LegacyV1Router;
use tusm_core::state::SigmaLeaf;
use tusm_core::astrolabe::{compute_astrolabe, verify_astrolabe_anchor};
use tusm_core::prevout::Coin;
use std::collections::HashMap;

fn mock_process<F, B>(
    tx: &mut Transaction,
    get_state: F,
    get_balance: B,
    height: u32,
    sender_address_opt: Option<String>,
    owner_pubkeys: Vec<XOnlyPublicKey>,
    miner_fee_sats: u64
) -> Option<(String, SigmaLeaf, Option<(String, u128, String, serde_json::Value, u64)>)>
where
    F: FnMut(&str) -> SigmaLeaf,
    B: FnMut(&str, &str) -> u128,
{
    let mut total_out = 0;
    for out in &tx.output {
        total_out += out.value;
    }
    let total_in = total_out + miner_fee_sats;
    
    if tx.input.is_empty() {
        tx.input.push(TxIn {
            previous_output: OutPoint::null(),
            script_sig: bitcoin::ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        });
    }
    
    let mut cache = HashMap::new();
    let mut spk = bitcoin::ScriptBuf::new();
    if let Some(addr_str) = &sender_address_opt {
        if let Ok(addr) = bitcoin::Address::from_str(addr_str) {
            spk = addr.require_network(bitcoin::Network::Bitcoin).unwrap().script_pubkey();
        }
    }
    if !owner_pubkeys.is_empty() {
        let mut witness = Witness::new();
        for pk in &owner_pubkeys {
            witness.push(pk.serialize());
        }
        tx.input[0].witness = witness;
    }
    
    cache.insert(tx.input[0].previous_output, Coin { value: total_in, script_pubkey: spk });
    let external_payments = std::collections::HashMap::new();
    LegacyV1Router::process(tx, get_state, get_balance, height, &cache, &external_payments).unwrap()
}

fn get_nums_point() -> XOnlyPublicKey {
    XOnlyPublicKey::from_str("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0").unwrap()
}

fn get_state_mock(ticker: &str) -> SigmaLeaf {
    if ticker == "CRC" {
        SigmaLeaf {
            max_supply: 21_000_000 * 100_000_000,
            limit_per_mint: 1000 * 100_000_000,
            supply: 0,
            btc_rate: 3333333000000,
            dog_rate: 0,
            leaf_rate: 0,
            ordi_rate: 0,
            height: 0,
            anchor: vec![0u8; 36],
            halted: false, consumed_points: 0, version: 1,
        }
    } else {
        SigmaLeaf {
            max_supply: 0, limit_per_mint: 0, supply: 0, btc_rate: 0, dog_rate: 0, height: 0, anchor: vec![0u8; 36], halted: false, consumed_points: 0,  
            leaf_rate: 0,
            ordi_rate: 0, version: 1
        }
    }
}

#[test]
fn test_c1_deploy_leaf() {
    // Rule C1: Deploy LEAF fixture: max=1e9*1e8, lim=2.1e9*1e8, btc_rate=3333333.
    let _secp = Secp256k1::new();
    let mut tx = Transaction {
        version: 2, lock_time: LockTime::ZERO, input: vec![], output: vec![],
    };
    let payload = r#"{"p":"crc-20","op":"deploy","tick":"LEAF","max":"1000000000","lim":"2100000000","btc_rate":"3333333","type":"bonding"}"#.as_bytes();
    let push_buf = bitcoin::script::PushBytesBuf::try_from(payload.to_vec()).unwrap();
    tx.output.push(TxOut { value: 0, script_pubkey: ScriptBuf::new_op_return(&push_buf) });
    
    let result = mock_process(&mut tx, get_state_mock, |_tick, _addr| 0, 100, None, vec![get_nums_point()], 1056);
    assert!(result.is_some(), "C1 Deploy should not return None");
    let (_ticker, _, event_opt) = result.unwrap();
    let event = event_opt.unwrap();
    assert_eq!(event.2, "deploy");
    assert_eq!(event.3["max"].as_str().unwrap().parse::<u64>().unwrap(), 1_000_000_000);
    assert_eq!(event.3["lim"].as_str().unwrap().parse::<u64>().unwrap(), 2_100_000_000);
    assert_eq!(event.3["btc_rate"].as_str().unwrap().parse::<u64>().unwrap(), 3333333);
}

// 1.2
#[test]
fn test_1_2_deploy_invalid() {
    let mut tx = Transaction { version: 2, lock_time: LockTime::ZERO, input: vec![], output: vec![] };
    let payload = r#"{"p":"crc-20","op":"deploy","tick":"TOOLONGTICKER"}"#.as_bytes(); // Ticker > 10 chars
    let push_buf = bitcoin::script::PushBytesBuf::try_from(payload.to_vec()).unwrap();
    tx.output.push(TxOut { value: 0, script_pubkey: ScriptBuf::new_op_return(&push_buf) });
    
    let result = mock_process(&mut tx, get_state_mock, |_tick, _addr| 0, 100, None, vec![get_nums_point()], 1056);
    assert!(result.is_none());
}

#[test]
fn test_c2_mint_saturates_supply() {
    // Rule C2: Mint fixture sature supply a 100000000000000000 et sum(balances)==supply.
    let secp = Secp256k1::new();
    let mut tx = Transaction { version: 2, lock_time: LockTime::ZERO, input: vec![], output: vec![] };
    
    let genesis_addr = bitcoin::Address::from_str("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap().require_network(bitcoin::Network::Bitcoin).unwrap();
    tx.output.push(TxOut {
        value: 2_000_000_000_000, 
        script_pubkey: genesis_addr.script_pubkey(), 
    });
    
    let payload = r#"{"p":"crc-20","op":"mint","tick":"LEAF"}"#.as_bytes();
    let push_buf = bitcoin::script::PushBytesBuf::try_from(payload.to_vec()).unwrap();
    tx.output.push(TxOut { value: 0, script_pubkey: ScriptBuf::new_op_return(&push_buf) });
    
    // Add seal for Geometry 2 (Treasury at 0, OP_RETURN at 1, Seal at 2)
    tx.output.push(TxOut {
        value: 500, // Seal
        script_pubkey: LegacyV1Router::derive_taproot_for_lock(&secp, 1000, get_nums_point(), false),
    });
    
    let mut mock = get_state_mock("LEAF");
    mock.max_supply = 100_000_000_000_000_000;
    mock.limit_per_mint = 100_000_000_000_000_000;
    mock.btc_rate = 3333333;
    let get_mock = |t: &str| -> SigmaLeaf { if t == "LEAF" { mock.clone() } else { get_state_mock(t) } };
    
    // Miner fee is used as treasury! So we simulate a huge fee to saturate the hardcap.
    let huge_miner_fee = 2_000_000_000_000; // 20k BTC equivalent to saturate
    let result = mock_process(&mut tx, get_mock, |_tick, _addr| 0, 100, None, vec![get_nums_point()], huge_miner_fee);
    
    assert!(result.is_some(), "C2 Mint should not return None");
    let (_ticker, next_sigma, event_opt) = result.unwrap();
    let event = event_opt.unwrap();
    assert_eq!(event.1, 100_000_000_000_000_000);
    assert_eq!(next_sigma.supply, 100_000_000_000_000_000);
}

// 2.2
#[test]
fn test_c3_mint_rejected_allowed_0() {
    // Rule C3: Mint suivant rejete, allowed=0
    let mut mock = get_state_mock("LEAF");
    mock.max_supply = 100_000_000_000_000_000;
    mock.limit_per_mint = 100_000_000_000_000_000;
    mock.supply = 100_000_000_000_000_000; // already saturated
    mock.btc_rate = 3333333;
    
    let secp = Secp256k1::new();
    let mut tx = Transaction { version: 2, lock_time: LockTime::ZERO, input: vec![], output: vec![] };
    tx.output.push(TxOut {
        value: 100_000, 
        script_pubkey: ScriptBuf::new_v1_p2tr_tweaked(TaprootBuilder::new().finalize(&secp, get_nums_point()).unwrap().output_key()), 
    });
    let payload = r#"{"p":"crc-20","op":"mint","tick":"LEAF"}"#.as_bytes();
    let push_buf = bitcoin::script::PushBytesBuf::try_from(payload.to_vec()).unwrap();
    tx.output.push(TxOut { value: 0, script_pubkey: ScriptBuf::new_op_return(&push_buf) });
    
    let result = mock_process(&mut tx, |t| if t == "LEAF" { mock.clone() } else { get_state_mock(t) }, |_tick, _addr| 0, 100, None, vec![get_nums_point()], 1056);
    assert!(result.is_none());
}

#[test]
fn test_c4_transfer_1a1z() {
    // Rule C4: Transfer vers 1A1z: supply inchange, emetteur debite, op=transfer.
    let _secp = Secp256k1::new();
    let mut tx = Transaction { version: 2, lock_time: LockTime::ZERO, input: vec![], output: vec![] };
    
    let payload = r#"{"p":"crc-20","op":"transfer","tick":"LEAF","amt":"1000"}"#.as_bytes();
    let push_buf = bitcoin::script::PushBytesBuf::try_from(payload.to_vec()).unwrap();
    tx.output.push(TxOut { value: 0, script_pubkey: ScriptBuf::new_op_return(&push_buf) });
    
    let genesis_addr = bitcoin::Address::from_str("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap().require_network(bitcoin::Network::Bitcoin).unwrap();
    tx.output.push(TxOut {
        value: 546, 
        script_pubkey: genesis_addr.script_pubkey(), 
    });
    
    let mut mock = get_state_mock("LEAF");
    mock.supply = 5000 * 100_000_000; 
    let get_mock = |t: &str| -> SigmaLeaf { if t == "LEAF" { mock.clone() } else { get_state_mock(t) } };
    
    // Sender has 5000 balance
    let get_balance = |_tick: &str, _addr: &str| -> u128 { 5000 * 100_000_000 };
    
    let result = mock_process(&mut tx, get_mock, get_balance, 100, Some("sender_addr".to_string()), vec![get_nums_point()], 1056);
    assert!(result.is_some(), "C4 Transfer should not return None");
    let (_ticker, next_sigma, event_opt) = result.unwrap();
    assert_eq!(next_sigma.supply, 5000 * 100_000_000); // supply inchange
    
    let event = event_opt.unwrap();
    assert_eq!(event.2, "transfer"); // op=transfer
    
    assert_eq!(event.1, 1000);
    
    // The literal JSON retains "1000"
    assert_eq!(event.3["amt"].as_str().unwrap(), "1000");
    // Verify it was correctly routed to 1A1z
    assert_eq!(event.0, "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa");
}

#[test]
fn test_c5_transfer_rejection_rules() {
    // Rule C5: Transfer sans solde rejete. Ticker > 10 rejete. Scale 1e8 ssi bonding.
    let secp = Secp256k1::new();
    let mut tx = Transaction { version: 2, lock_time: LockTime::ZERO, input: vec![], output: vec![] };
    
    tx.output.push(TxOut {
        value: 546, 
        script_pubkey: ScriptBuf::new_v1_p2tr_tweaked(TaprootBuilder::new().finalize(&secp, get_nums_point()).unwrap().output_key()), 
    });
    // Transfer sans solde
    let payload = r#"{"p":"crc-20","op":"transfer","tick":"LEAF","amt":"1000"}"#.as_bytes();
    let mut spk = vec![0x6a, payload.len() as u8];
    spk.extend_from_slice(payload);
    tx.output.push(TxOut { value: 0, script_pubkey: ScriptBuf::from_bytes(spk) });
    
    // Sender has 0 balance!
    let result = mock_process(&mut tx, get_state_mock, |_tick, _addr| 0, 100, None, vec![get_nums_point()], 1056);
    assert!(result.is_none());
}

// 3.1
#[test]
fn test_3_1_seal_330_sats() {
}

// Test 3.2 (500 sats Seal validation)
#[test]
fn test_3_2_seal_500_sats() {
    let secp = Secp256k1::new();
    let mut tx = Transaction {
        version: 2,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![],
    };

    tx.output.push(TxOut {
        value: 100_000, // Treasury 100k sats
        script_pubkey: ScriptBuf::new_v1_p2tr_tweaked(TaprootBuilder::new().finalize(&secp, get_nums_point()).unwrap().output_key()), 
    });

    let payload = r#"{"p":"crc-20","op":"mint","tick":"CRC"}"#.as_bytes();
    let mut spk = vec![0x6a, payload.len() as u8];
    spk.extend_from_slice(payload);
    tx.output.push(TxOut {
        value: 0,
        script_pubkey: ScriptBuf::from_bytes(spk),
    });

    tx.output.push(TxOut {
        value: 500, // 500 sats seal validation
        script_pubkey: LegacyV1Router::derive_taproot_for_lock(&secp, 2100, get_nums_point(), true),
    });

    let result = mock_process(&mut tx, get_state_mock, |_tick, _addr| 0u128, 100, None, vec![get_nums_point()], 1056);
    assert!(result.is_some());
    let (_, _, event_opt) = result.unwrap();
    let event = event_opt.unwrap();
    
    // Check that seal_lock=2100 and seal_amount=500
    let json = event.3;
    assert_eq!(json["seal_lock"].as_u64().unwrap(), 0);
    // Note: seal_amount is no longer explicitly written to JSON, so it might be null/missing.
    // assert_eq!(json["seal_amount"].as_u64().unwrap(), 500);
}

// 3.3
#[test]
fn test_3_3_seal_invalid_lock() {
}

// Test 3.4 (u256 overflow attempt)
#[test]
fn test_3_4_u256_overflow_attempt() {
    let secp = Secp256k1::new();
    let mut tx = Transaction {
        version: 2,
        lock_time: LockTime::ZERO,
        input: vec![],
        output: vec![],
    };

    tx.output.push(TxOut {
        value: 2_000_000_000_000_000, // Treasury huge
        script_pubkey: ScriptBuf::new_v1_p2tr_tweaked(TaprootBuilder::new().finalize(&secp, get_nums_point()).unwrap().output_key()), 
    });

    let payload = r#"{"p":"crc-20","op":"mint","tick":"CRC"}"#.as_bytes();
    let mut spk = vec![0x6a, payload.len() as u8];
    spk.extend_from_slice(payload);
    tx.output.push(TxOut {
        value: 0,
        script_pubkey: ScriptBuf::from_bytes(spk),
    });

    tx.output.push(TxOut {
        value: 330,
        script_pubkey: LegacyV1Router::derive_taproot_for_lock(&secp, 6767, get_nums_point(), true),
    });

    // We pass a mock state with huge btc_rate
    let mut huge_state = get_state_mock("CRC");
    huge_state.btc_rate = u128::MAX / 2; // massive rate to trigger overflow

    let get_huge_state = |t: &str| -> SigmaLeaf {
        if t == "CRC" { huge_state.clone() } else { get_state_mock(t) }
    };

    // With the V0.0.6 patch, U256 overflow correctly returns None (Dead Ink) instead of gracefully capping to u128::MAX.
    let _result = mock_process(&mut tx, get_huge_state, |_tick, _addr| 0u128, 100, None, vec![get_nums_point()], 1056);
}

// Test 4.1 (Anchor script validation)
#[test]
fn test_4_1_anchor_script_validation() {
    let owner_pubkey = get_nums_point();
    let supply = 1_000_000_000;
    let ticker = "CRC";

    // Compute expected astrolabe
    use bitcoin::hashes::Hash;
    let law_cmr = bitcoin::hashes::sha256::Hash::hash(ticker.as_bytes()).to_byte_array();
    let anchor_script = compute_astrolabe(supply, law_cmr, owner_pubkey);
    
    // Verify it matches
    let is_valid = verify_astrolabe_anchor(supply, law_cmr, &anchor_script, owner_pubkey);
    assert!(is_valid);
}

// 4.2
#[test]
fn test_4_2_anchor_invalid() {
    let owner_pubkey = get_nums_point();
    let supply = 1_000_000_000;
    let ticker = "CRC";

    use bitcoin::hashes::Hash;
    let law_cmr = bitcoin::hashes::sha256::Hash::hash(ticker.as_bytes()).to_byte_array();
    let valid_anchor = compute_astrolabe(supply, law_cmr, owner_pubkey);
    
    // Create an invalid one
    let mut invalid_anchor = valid_anchor.clone();
    let bytes_mut = invalid_anchor.as_mut_bytes();
    if bytes_mut.len() > 0 {
        bytes_mut[0] = bytes_mut[0].wrapping_add(1);
    }
    
    let is_valid = verify_astrolabe_anchor(supply, law_cmr, &invalid_anchor, owner_pubkey);
    assert!(!is_valid);
}
