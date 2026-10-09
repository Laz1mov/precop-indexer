use bitcoin::secp256k1::{Secp256k1, XOnlyPublicKey};
use bitcoin::taproot::TaprootBuilder;
use bitcoin::{ScriptBuf, TxOut, Transaction, OutPoint, TxIn, Sequence, Witness};
use bitcoin::absolute::LockTime;
use std::str::FromStr;
use tusm_core::legacy_v1_router::LegacyV1Router;
use tusm_core::state::SigmaLeaf;
use tusm_core::prevout::Coin;
use std::collections::HashMap;

fn mock_process_with_external<F, B>(
    tx: &mut Transaction,
    get_state: F,
    get_balance: B,
    height: u32,
    sender_address_opt: Option<String>,
    owner_pubkeys: Vec<XOnlyPublicKey>,
    miner_fee_sats: u64,
    external_payments: &HashMap<String, (String, u128)>
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
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        });
    }
    
    let mut cache = HashMap::new();
    let mut spk = ScriptBuf::new();
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
    LegacyV1Router::process(tx, get_state, get_balance, height, &cache, external_payments).unwrap()
}

fn get_nums_point() -> XOnlyPublicKey {
    XOnlyPublicKey::from_str("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0").unwrap()
}

fn derive_taproot_for_lock(lock: u16) -> ScriptBuf {
    let secp = Secp256k1::new();
    let internal_key = get_nums_point();
    LegacyV1Router::derive_taproot_for_lock(&secp, lock, internal_key, true)
}

fn default_crc_state() -> SigmaLeaf {
    SigmaLeaf {
        max_supply: 1000000000_00000000, // 1 billion CRC = 10^17 atoms
        limit_per_mint: 10000000000000_00000000,
        supply: 0,
        btc_rate: 3333333000000,
        dog_rate: 47917,
        leaf_rate: 0,
        ordi_rate: 0,
        height: 968894,
        anchor: vec![0u8; 36],
        halted: false,
        consumed_points: 0,
        version: 1,
    }
}

#[test]
fn test_crc_mint_btc_30506778() {
    // Tx 305067783e3c74aa708f870648225e909d9c5eea8bdc90ce1f5979bf4572eb8e
    // Bloc: 968 967
    // Paiement à la trésorerie: 1 000 sats
    // Lock: 6767 (x1.309017)
    // Seal: sortie :1
    // CRC alloués: 8 726,77912732 CRC = 872 677 912 732 atoms
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

    // vout[0]: OP_RETURN
    let mut spk = vec![0x6a];
    let payload = r#"{"p":"crc-20","op":"mint","tick":"CRC"}"#.as_bytes();
    spk.push(payload.len() as u8);
    spk.extend_from_slice(payload);
    tx.output.push(TxOut {
        value: 0,
        script_pubkey: ScriptBuf::from_bytes(spk),
    });

    // vout[1]: Seal lock 6767
    tx.output.push(TxOut {
        value: 330,
        script_pubkey: derive_taproot_for_lock(6767),
    });

    // vout[2]: Treasury 1 000 sats
    tx.output.push(TxOut {
        value: 1_000,
        script_pubkey: ScriptBuf::new_v1_p2tr_tweaked(TaprootBuilder::new().finalize(&secp, get_nums_point()).unwrap().output_key()),
    });

    let get_state = |ticker: &str| -> SigmaLeaf {
        if ticker == "CRC" {
            default_crc_state()
        } else {
            let mut s = default_crc_state();
            s.max_supply = 0;
            s
        }
    };

    let external_payments = HashMap::new();
    let result = mock_process_with_external(
        &mut tx,
        get_state,
        |_tick, _addr| 0u128,
        968967,
        None,
        vec![get_nums_point()],
        1847,
        &external_payments
    );

    assert!(result.is_some(), "Router failed to process transaction");
    let (ticker, mutated_state, event) = result.unwrap();
    assert_eq!(ticker, "CRC");

    // Exact expected: 1 000 sats x 6.666666 CRC/sat x 1.309017 = 8 726.77912732 CRC = 872 677 912 732 atoms
    let expected_atoms = 872_677_912_732u128;
    assert_eq!(mutated_state.supply, expected_atoms);

    let ev = event.unwrap();
    assert_eq!(ev.1, expected_atoms);
}

#[test]
fn test_crc_mint_dog_c892254b() {
    // Tx c892254bd7550ccac21efa53d5a18d33f55fc10f6b910e6f047c16af63b0e256
    // Bloc: 968 966
    // DOG reçus par la trésorerie: 50 000 DOG (5 000 000 000 unités)
    // Sats sur la sortie trésorerie: 10 546 (fee 10 000 + postage 546, ne comptent pas)
    // Taux DOG appliqué: 47 913 (prix de marché à ce bloc +15%, juste sous le plafond 47 917)
    // Lock: 2100 (x1.136010)
    // Seal: sortie :2
    // CRC alloués: 544 296,4713 CRC = 54 429 647 130 000 atoms
    let _secp = Secp256k1::new();
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

    // vout[0]: OP_RETURN {"p":"crc-20","op":"mint","tick":"CRC"}
    let mut spk = vec![0x6a];
    let payload = r#"{"p":"crc-20","op":"mint","tick":"CRC"}"#.as_bytes();
    spk.push(payload.len() as u8);
    spk.extend_from_slice(payload);
    tx.output.push(TxOut {
        value: 0,
        script_pubkey: ScriptBuf::from_bytes(spk),
    });

    // vout[1]: Treasury with 10 546 sats (bc1p8q4hnqrve9tdd7yzx650m4ct007w38p8gkcylrmffherq25987fq4e6ajr)
    let treasury_spk = ScriptBuf::from_hex("5120382b79806cc956d6f88236a8fdd70b7bfce89c2745b04f8f694df2302a853f92").unwrap();
    tx.output.push(TxOut {
        value: 10_546,
        script_pubkey: treasury_spk,
    });

    // vout[2]: Seal lock 2100
    let secp_tmp = Secp256k1::new();
    let sender_pk = XOnlyPublicKey::from_str("538c8b98fe918e36a3bed6e72044c68e65f6fc5b80796886860bc5701355aba0").unwrap();
    let seal_cmr = LegacyV1Router::derive_taproot_for_lock(&secp_tmp, 2100, sender_pk, true);
    let seal_nocmr = LegacyV1Router::derive_taproot_for_lock(&secp_tmp, 2100, sender_pk, false);
    println!("DEBUG_SEAL_WITH_CMR: {}", seal_cmr.to_hex_string());
    println!("DEBUG_SEAL_NO_CMR:   {}", seal_nocmr.to_hex_string());
    let seal_spk = derive_taproot_for_lock(2100);
    tx.output.push(TxOut {
        value: 330,
        script_pubkey: seal_spk,
    });

    let get_state = |ticker: &str| -> SigmaLeaf {
        if ticker == "CRC" {
            default_crc_state()
        } else {
            let mut s = default_crc_state();
            s.max_supply = 0;
            s
        }
    };

    let mut external_payments = HashMap::new();
    let txid = tx.txid().to_string();
    // Dog units: 50 000 DOG = 5 000 000 000 units with 5 decimals. Rate applied: 47913.
    external_payments.insert(txid, ("DOG:47913".to_string(), 5_000_000_000u128));

    let result = mock_process_with_external(
        &mut tx,
        get_state,
        |_tick, _addr| 0u128,
        968966,
        None,
        vec![get_nums_point()],
        330,
        &external_payments
    );

    assert!(result.is_some(), "Router failed to process transaction");
    let (ticker, mutated_state, event) = result.unwrap();
    assert_eq!(ticker, "CRC");

    // Exact expected: 50 000 DOG x (47 913 / 5 000) x 1.136010 = 544 296.4713 CRC = 54 429 647 130 000 atoms
    let expected_atoms = 54_429_647_130_000u128;
    assert_eq!(mutated_state.supply, expected_atoms);

    let ev = event.unwrap();
    assert_eq!(ev.1, expected_atoms);
}

#[test]
fn test_crc_mint_btc_first_mint_07fb() {
    // Tx 07fb060fa12010bb03aaecfaf45477769c6461873a9b58e9eeb10943b6214351
    // Bloc: 968 894
    // 100,000 sats, lock 6767
    // Proves that the first mint DOES NOT exhaust the entire supply
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

    // vout[0]: OP_RETURN
    let mut spk = vec![0x6a];
    let payload = r#"{"p":"crc-20","op":"mint","tick":"CRC"}"#.as_bytes();
    spk.push(payload.len() as u8);
    spk.extend_from_slice(payload);
    tx.output.push(TxOut {
        value: 0,
        script_pubkey: ScriptBuf::from_bytes(spk),
    });

    // vout[1]: Seal lock 6767
    tx.output.push(TxOut {
        value: 330,
        script_pubkey: derive_taproot_for_lock(6767),
    });

    // vout[2]: Treasury 100,000 sats
    tx.output.push(TxOut {
        value: 100_000,
        script_pubkey: ScriptBuf::new_v1_p2tr_tweaked(TaprootBuilder::new().finalize(&secp, get_nums_point()).unwrap().output_key()),
    });

    let get_state = |_ticker: &str| -> SigmaLeaf {
        default_crc_state()
    };

    let external_payments = HashMap::new();
    let result = mock_process_with_external(
        &mut tx,
        get_state,
        |_tick, _addr| 0u128,
        968894,
        None,
        vec![get_nums_point()],
        314,
        &external_payments
    );

    assert!(result.is_some(), "Router failed to process transaction");
    let (ticker, mutated_state, event) = result.unwrap();
    assert_eq!(ticker, "CRC");

    // 100,000 sats x 6.666666 x 1.309017 = 872 677.9127322 CRC = 87 267 791 273 220 atoms
    let expected_atoms = 87_267_791_273_220u128;
    assert_eq!(mutated_state.supply, expected_atoms);
    assert!(mutated_state.supply < default_crc_state().max_supply);

    let ev = event.unwrap();
    assert_eq!(ev.1, expected_atoms);
}

#[test]
fn test_crc_120_canonical_mints_exact_convergence() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let canonical_path = manifest_dir.join("tests/fixtures/canonical_mints.json");
    let raw_cache_path = manifest_dir.join("tests/fixtures/raw_mints_cache.json");
    
    if !canonical_path.exists() || !raw_cache_path.exists() {
        println!("Skipping 120-mint convergence test: fixture files not present.");
        return;
    }
    
    let canonical_raw = std::fs::read_to_string(canonical_path).unwrap();
    let canonical: Vec<serde_json::Value> = serde_json::from_str(&canonical_raw).unwrap();
    
    let raw_cache_str = std::fs::read_to_string(raw_cache_path).unwrap();
    let raw_cache: HashMap<String, String> = serde_json::from_str(&raw_cache_str).unwrap();
    
    assert_eq!(canonical.len(), 120);
    assert_eq!(raw_cache.len(), 120);
    
    // Build external_payments for DOG mints
    let mut external_payments = HashMap::new();
    for m in &canonical {
        if m.get("asset").and_then(|v| v.as_str()) == Some("DOG") {
            let txid = m.get("txid").and_then(|v| v.as_str()).unwrap().to_string();
            let dog_units: u128 = m.get("dogUnits").and_then(|v| v.as_str()).unwrap().parse().unwrap();
            let dog_rate: u64 = m.get("dogRate").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(47913);
            let lock: u16 = m.get("lock").and_then(|v| v.as_u64()).unwrap() as u16;
            let asset_key = format!("DOG:{}:{}", dog_rate, lock);
            external_payments.insert(txid, (asset_key, dog_units));
        }
    }
    assert_eq!(external_payments.len(), 39);
    
    let mut current_state = default_crc_state();
    let mut total_minted_atoms: u128 = 0;
    let mut processed_count = 0;
    
    // Sort canonical mints by block height, then txid
    let mut sorted_mints = canonical.clone();
    sorted_mints.sort_by_key(|m| {
        (
            m.get("height").and_then(|v| v.as_u64()).unwrap_or(0),
            m.get("txid").and_then(|v| v.as_str()).unwrap_or("").to_string()
        )
    });
    
    for m in &sorted_mints {
        let txid = m.get("txid").and_then(|v| v.as_str()).unwrap();
        let height = m.get("height").and_then(|v| v.as_u64()).unwrap_or(968900) as u32;
        let expected_atoms: u128 = m.get("atoms").and_then(|v| v.as_str()).unwrap().parse().unwrap();
        
        let raw_hex = raw_cache.get(txid).expect("Raw tx missing from cache");
        let raw_bytes = hex::decode(raw_hex).unwrap();
        let tx: Transaction = bitcoin::consensus::deserialize(&raw_bytes).unwrap();
        
        let empty_cache: HashMap<OutPoint, Coin> = HashMap::new();
        
        let get_state = |_ticker: &str| -> SigmaLeaf {
            current_state.clone()
        };
        
        let res = LegacyV1Router::process(
            &tx,
            get_state,
            |_t, _a| 0u128,
            height,
            &empty_cache,
            &external_payments
        ).expect("Process error");
        
        assert!(res.is_some(), "Mint rejected by router for txid: {}", txid);
        let (ticker, mutated_state, event_opt) = res.unwrap();
        assert_eq!(ticker, "CRC");
        
        let ev = event_opt.expect("Event missing for mint");
        let allocated_atoms = ev.1;
        assert_eq!(allocated_atoms, expected_atoms, "Atom allocation mismatch for txid {}", txid);
        
        total_minted_atoms += allocated_atoms;
        current_state = mutated_state;
        processed_count += 1;
    }
    
    assert_eq!(processed_count, 120);
    // Target consensus ground truth: exactly 162,053,369.45 CRC = 16,205,336,945,176,514 atoms
    assert_eq!(total_minted_atoms, 16_205_336_945_176_514u128);
    assert_eq!(current_state.supply, 16_205_336_945_176_514u128);
    
    println!("SUCCESS: All 120 canonical CRC mints converged to EXACTLY {} atoms (162,053,369.45 CRC)!", current_state.supply);
}
