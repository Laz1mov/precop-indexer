use tusm_core::legacy_v1_router::LegacyV1Router;
use tusm_core::state::SigmaLeaf;
use bitcoin::{Transaction, TxOut, ScriptBuf, TxIn, OutPoint, Sequence, Witness, Txid};
use bitcoin::hashes::Hash;
use std::str::FromStr;
use tusm_core::prevout::Coin;
use bitcoin::secp256k1::XOnlyPublicKey;
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

fn create_op_return_output(json_str: &str) -> TxOut {
    let payload = json_str.as_bytes();
    let push_bytes = <&bitcoin::script::PushBytes>::try_from(payload).unwrap();
    let spk = bitcoin::script::Builder::new()
        .push_opcode(bitcoin::opcodes::all::OP_RETURN)
        .push_slice(push_bytes)
        .into_script();
    TxOut {
        value: 0,
        script_pubkey: spk,
    }
}

fn create_tx(outputs: Vec<TxOut>) -> Transaction {
    Transaction {
        version: 2,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint { txid: Txid::all_zeros(), vout: 0 },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: outputs,
    }
}

#[test]
fn test_pob_migration_satoshi_intercept() {
    // A mock tx outputting 546 sats to Satoshi's 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa
    let satoshi_addr = bitcoin::Address::from_str("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap().assume_checked();
    let outputs = vec![
        create_op_return_output(r#"{"p":"crc-20","op":"deploy","tick":"MOCK","max":"21000000","lim":"1000","btc":"546","type":"bonding"}"#),
        TxOut {
            value: 546,
            script_pubkey: satoshi_addr.script_pubkey(),
        },
    ];
    let mut tx = create_tx(outputs);

    let get_state = |_tick: &str| SigmaLeaf {
        supply: 0, max_supply: 0, limit_per_mint: 0, btc_rate: 0, dog_rate: 0, anchor: vec![0u8; 36], height: 0, version: 1, halted: false, consumed_points: 0,  
            leaf_rate: 0,
            ordi_rate: 0,
    };
    let get_balance = |_tick: &str, _addr: &str| 0u128;

    let result = mock_process(&mut tx, get_state, get_balance, 1000, Some("sender".to_string()), vec![], 1056);
    
    assert!(result.is_some());
    let (ticker, state, _) = result.unwrap();
    assert_eq!(ticker, "MOCK");
    assert_eq!(state.max_supply, 2100000000000000);
}

#[test]
fn test_enforce_mint_limits() {
    let outputs = vec![
        create_op_return_output(r#"{"p":"crc-20","op":"mint","tick":"MOCK","amt":"21000000"}"#),
    ];
    let mut tx = create_tx(outputs);

    let get_state = |_tick: &str| SigmaLeaf {
        supply: 0, 
        max_supply: 21000000 * 100_000_000, 
        limit_per_mint: 1000 * 100_000_000, // Strict limit of 1000
        btc_rate: 0, dog_rate: 0, anchor: vec![0u8; 36], height: 0, version: 1, halted: false, consumed_points: 0,  
            leaf_rate: 0,
            ordi_rate: 0,
    };
    let get_balance = |_tick: &str, _addr: &str| 0u128;

    let result = mock_process(&mut tx, get_state, get_balance, 1000, Some("sender".to_string()), vec![], 1056);
    
    assert!(result.is_some());
    let (ticker, state, event) = result.unwrap();
    assert_eq!(ticker, "MOCK");
    // Proof that the engine enforces the limit of 1000 instead of granting 21000000
    assert_eq!(state.supply, 1000 * 100_000_000);
    
    let (_, amt, op, _, _) = event.unwrap();
    assert_eq!(op, "mint");
    assert_eq!(amt, 1000 * 100_000_000);
}

#[test]
fn test_ico20_burn_triggers_crc20_mint() {
    let satoshi_addr = bitcoin::Address::from_str("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap().assume_checked();
    let outputs = vec![
        create_op_return_output(r#"{"p":"crc-20","op":"transfer","tick":"LEAF","amt":"1000"}"#),
        TxOut {
            value: 546,
            script_pubkey: satoshi_addr.script_pubkey(),
        },
    ];
    let mut tx = create_tx(outputs);

    let get_state = |_tick: &str| SigmaLeaf {
        supply: 0, 
        max_supply: 21000000 * 100_000_000, 
        limit_per_mint: 0, 
        btc_rate: 0, dog_rate: 0, anchor: vec![0u8; 36], height: 0, version: 1, halted: false, consumed_points: 0,  
            leaf_rate: 0,
            ordi_rate: 0,
    };
    let get_balance = |_tick: &str, _addr: &str| 1000u128 * 100_000_000;

    let result = mock_process(&mut tx, get_state, get_balance, 1000, Some("bc1q_sender_test".to_string()), vec![], 1056);
    
    assert!(result.is_some());
    let (ticker, state, event) = result.unwrap();
    assert_eq!(ticker, "LEAF");
    // Proof that the supply is NOT inflated (Patch 805 fixed)
    assert_eq!(state.supply, 0);
    
    let (dest, amt, op, _, _) = event.unwrap();
    assert_eq!(op, "transfer");
    assert_eq!(amt, 1000);
    assert_eq!(dest, "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"); 
}
