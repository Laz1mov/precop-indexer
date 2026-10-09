use bitcoin::{Transaction, TxIn, TxOut, ScriptBuf, Txid, Sequence, Witness, OutPoint};
use std::str::FromStr;
use tusm_core::legacy_v1_router::LegacyV1Router;
use tusm_core::state::SigmaLeaf;
use tusm_core::prevout::{Coin, PrevoutError, PrevoutResolver};
use std::collections::HashMap;
use hex::FromHex;

struct NoRpcProvider {
    cache: HashMap<OutPoint, Coin>,
}

impl PrevoutResolver for NoRpcProvider {
    fn prevout(&self, outpoint: &OutPoint) -> Result<&Coin, PrevoutError> {
        self.cache.get(outpoint).ok_or(PrevoutError::MissingPrevout(*outpoint))
    }
}

#[test]
fn test_legacy_v1_does_not_require_rpc() {
    // This test ensures LegacyV1Router doesn't take an RPC provider anymore, but only a PrevoutResolver.
    let txid = Txid::from_str("df2461f041e173d63baf313c24d429f66145adf36e304dd9eb3b5419244205f7").unwrap();
    let tx = Transaction {
        version: 1,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint { txid, vout: 0 },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: 0,
            script_pubkey: ScriptBuf::from_hex("6a1e7b2270223a226372632d3230222c226f70223a226d696e74222c227469636b223a224c454146227d").unwrap(),
        }],
    };

    let mut provider = NoRpcProvider { cache: HashMap::new() };
    provider.cache.insert(
        OutPoint { txid, vout: 0 },
        Coin { value: 1000, script_pubkey: ScriptBuf::new() }
    );

    let get_state = |_tick: &str| SigmaLeaf {
        supply: 0, max_supply: 21_000_000, limit_per_mint: 1000, btc_rate: 1, dog_rate: 1, leaf_rate: 1, ordi_rate: 1,
        anchor: vec![0u8; 36], height: 0, version: 1, halted: false, consumed_points: 0, };
    let get_balance = |_tick: &str, _addr: &str| 0u128;

    // This signature expects the new architecture
    let external_payments = std::collections::HashMap::new();
    let _result = LegacyV1Router::process(
        &tx,
        get_state,
        get_balance,
        967501,
        &provider,
        &external_payments,
    );
    
    // As long as it compiles and doesn't panic, the anti-RPC property is proved by type safety.
}

#[test]
fn test_missing_prevout_does_not_mutate_state() {
    let txid = Txid::from_str("df2461f041e173d63baf313c24d429f66145adf36e304dd9eb3b5419244205f7").unwrap();
    let tx = Transaction {
        version: 1,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint { txid, vout: 0 },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: 0,
            script_pubkey: ScriptBuf::from_hex("6a1e7b2270223a226372632d3230222c226f70223a226d696e74222c227469636b223a224c454146227d").unwrap(),
        }],
    };

    let provider = NoRpcProvider { cache: HashMap::new() }; // Empty cache = MissingPrevout

    let mut state_calls = 0;
    let get_state = |_tick: &str| {
        state_calls += 1;
        SigmaLeaf {
            supply: 0, max_supply: 21_000_000, limit_per_mint: 1000, btc_rate: 1, dog_rate: 1, leaf_rate: 1, ordi_rate: 1,
            anchor: vec![0u8; 36], height: 0, version: 1, halted: false, consumed_points: 0, }
    };
    let mut balance_calls = 0;
    let get_balance = |_tick: &str, _addr: &str| {
        balance_calls += 1;
        0u128
    };

    let external_payments = std::collections::HashMap::new();
    let result = LegacyV1Router::process(
        &tx,
        get_state,
        get_balance,
        967501,
        &provider,
        &external_payments,
    ).unwrap();
    
    assert!(result.is_none(), "Transaction should be skipped (return None) when prevout is missing");
    
    // Ensure no state mutation attempt was made
    assert_eq!(state_calls, 0, "State should not be read if prevout is missing");
    assert_eq!(balance_calls, 0, "Balance should not be read if prevout is missing");
}

#[test]
fn test_seal_df2461_real() {
    let tx_hex = "020000000001026980addea00c6c872801724ab095c31abf9097d4330c5488eb1e4c69577812820000000000fdffffffd7f827cdcb23867aaf82288e5fdb718e31c897c7597029589b606a1e5daf73cf0000000000fdffffff060000000000000000466a447b2270223a2269636f2d3230222c226f70223a227472616e73666572222c227469636b223a224c454146222c22616d74223a22333931353734332e38323638333339227d22020000000000001976a91462e907b15cbf27d5425399ebf6f0fb50ebb88f1888ac00000000000000002a6a287b2270223a226372632d3230222c226f70223a226d696e74222c227469636b223a224c454146227d4a01000000000000225120516e37e86d0e6d5434f2ee0bfdde0a18d328ef5faaf6e829ca799d9d1ce962881027000000000000225120bf39cfdfc985043dd4f95dd2403167d3d26874dc06ab15e7e09bf4415791c79ffb1500000000000022512067e6fefb7b9d5af7c4673cb6f3bb0bfa5e789912dfec39635d95a4d259ca393e01406c44f79e92445b773cc800a555b580c6275745907211c8da7dfb8ef7a515afe9bda25083bea21bb7d0f659e82c974159461aa6ea5c97410c80108e3bf4e092cc014079152454fb95870a6c31729c70d10959a40029f032a339e53daea0428dfa566465c8d5af3ae1024b40acb4134339c69acd44cd2dca700be4d662e7770c6f731800000000";
    let tx: Transaction = bitcoin::consensus::encode::deserialize(&Vec::from_hex(tx_hex).unwrap()).unwrap();
    
    let mut provider = NoRpcProvider { cache: HashMap::new() };
    
    provider.cache.insert(
        OutPoint { 
            txid: Txid::from_str("82127857694c1eeb88540c33d49790bf1ac395b04a720128876c0ca0dead8069").unwrap(), 
            vout: 0 
        },
        Coin { 
            value: 10000, 
            script_pubkey: ScriptBuf::from_hex("512067e6fefb7b9d5af7c4673cb6f3bb0bfa5e789912dfec39635d95a4d259ca393e").unwrap() 
        }
    );
    provider.cache.insert(
        OutPoint { 
            txid: Txid::from_str("cf73af5d1e6a609b58297059c797c8318e71db5f8e2882af7a8623cbcd27f8d7").unwrap(), 
            vout: 0 
        },
        Coin { 
            value: 6923, 
            script_pubkey: ScriptBuf::from_hex("512067e6fefb7b9d5af7c4673cb6f3bb0bfa5e789912dfec39635d95a4d259ca393e").unwrap() 
        }
    );

    let get_state2 = |_tick: &str| SigmaLeaf {
        supply: 0, 
        max_supply: 2100000000000000, 
        limit_per_mint: 1000000000000, 
        btc_rate: 3, dog_rate: 0, leaf_rate: 1, ordi_rate: 0,
        anchor: vec![0u8; 36], height: 0, version: 1, halted: false, consumed_points: 0, };
    let get_balance = |_tick: &str, _addr: &str| 0u128;

    let external_payments = std::collections::HashMap::new();
    let result = LegacyV1Router::process(
        &tx,
        get_state2,
        get_balance,
        967501,
        &provider,
        &external_payments,
    ).unwrap();
    
    assert!(result.is_some(), "Transaction should be processed successfully");
    let (ticker, new_state, event_opt) = result.unwrap();
    
    assert_eq!(ticker, "LEAF");
    
    let event = event_opt.expect("Should produce an event");
    let minted_amt = event.1;
    
    assert_eq!(minted_amt, 488_169_047_619_048);
    assert_eq!(new_state.supply, 488_169_047_619_048);
}
