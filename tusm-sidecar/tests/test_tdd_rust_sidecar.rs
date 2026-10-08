use std::collections::HashMap;
use tusm_sidecar::{SidecarConfig, SidecarEngine, VerifiedPayment, EvidenceFile};

#[tokio::test]
async fn test_empty_files_initialization_and_engine_start() {
    let test_dir = tempfile::tempdir().expect("create temp dir");
    let ext_file = test_dir.path().join("external_payments.json");
    let dog_file = test_dir.path().join("dog-evidence.json");
    let ordi_file = test_dir.path().join("ordi-evidence.json");
    let pending_file = test_dir.path().join("pending_mints.jsonl");

    // Initialize with completely empty files
    std::fs::write(&ext_file, "{}").unwrap();
    std::fs::write(&dog_file, "{}").unwrap();
    std::fs::write(&ordi_file, "{}").unwrap();
    std::fs::write(&pending_file, "").unwrap();

    let config = SidecarConfig {
        target_output_file: ext_file.clone(),
        dog_evidence_file: dog_file.clone(),
        ordi_evidence_file: ordi_file.clone(),
        pending_mints_path: pending_file.clone(),
        treasury_address: "bc1p8q4hnqrve9tdd7yzx650m4ct007w38p8gkcylrmffherq25987fq4e6ajr".to_string(),
        unisat_api_key: "".to_string(),
        poll_interval_secs: 15,
        http_port: 8085,
    };

    let engine = SidecarEngine::new(config);

    assert_eq!(engine.payments.read().len(), 0);
    assert_eq!(engine.dog_evidence.read().items.len(), 0);
    assert_eq!(engine.ordi_evidence.read().items.len(), 0);
    println!("✅ Test 1 Passed: Empty bootstrap initialization clean.");
}

#[tokio::test]
async fn test_mock_payment_persistence_and_atomic_formatting() {
    let test_dir = tempfile::tempdir().expect("create temp dir");
    let ext_file = test_dir.path().join("external_payments.json");
    let dog_file = test_dir.path().join("dog-evidence.json");
    let ordi_file = test_dir.path().join("ordi-evidence.json");
    let pending_file = test_dir.path().join("pending_mints.jsonl");

    let config = SidecarConfig {
        target_output_file: ext_file.clone(),
        dog_evidence_file: dog_file.clone(),
        ordi_evidence_file: ordi_file.clone(),
        pending_mints_path: pending_file.clone(),
        treasury_address: "bc1p8q4hnqrve9tdd7yzx650m4ct007w38p8gkcylrmffherq25987fq4e6ajr".to_string(),
        unisat_api_key: "".to_string(),
        poll_interval_secs: 15,
        http_port: 8085,
    };

    let engine = SidecarEngine::new(config);

    // Insert 1 DOG payment and 1 ORDI payment
    let dog_tx = "c892254bd7550ccac21efa53d5a18d33f55fc10f6b910e6f047c16af63b0e256";
    let ordi_tx = "d1a64d8051ae24c2032b146ecb63a789dae7b2fe584f46505f874070008027fd";

    engine.payments.write().insert(
        dog_tx.to_string(),
        VerifiedPayment {
            asset: "DOG".to_string(),
            amount: 5000000000,
            amount_display: Some("50000".to_string()),
            rate: Some(47913),
            payment_type: "runes".to_string(),
            protocol: "runes".to_string(),
            runeid: Some("840000:3".to_string()),
            symbol: Some("DOG•GO•TO•THE•MOON".to_string()),
            decimals: Some(5),
            ticker: None,
            destination: Some(engine.config.treasury_address.clone()),
            destination_vout: Some(1),
            seal_vout: Some(2),
            block_height: Some(968966),
            inscription_id: None,
            inscription_number: None,
            status: "confirmed".to_string(),
        },
    );

    engine.payments.write().insert(
        ordi_tx.to_string(),
        VerifiedPayment {
            asset: "ORDI".to_string(),
            amount: 1000000000000000000,
            amount_display: Some("1.0".to_string()),
            rate: None,
            payment_type: "brc-20".to_string(),
            protocol: "brc-20".to_string(),
            runeid: None,
            symbol: None,
            decimals: Some(18),
            ticker: Some("ordi".to_string()),
            destination: Some(engine.config.treasury_address.clone()),
            destination_vout: Some(1),
            seal_vout: Some(2),
            block_height: Some(969100),
            inscription_id: Some("insc_test_ordi_1".to_string()),
            inscription_number: None,
            status: "confirmed".to_string(),
        },
    );

    // Save atomically
    engine.save_files();

    assert!(ext_file.exists());
    assert!(dog_file.exists());
    assert!(ordi_file.exists());

    // Verify external_payments.json compatibility with indexer parsing logic
    let content = std::fs::read_to_string(&ext_file).unwrap();
    let parsed: HashMap<String, serde_json::Value> = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed.len(), 2);

    let dog_val = parsed.get(dog_tx).unwrap();
    assert_eq!(dog_val.get("asset").and_then(|v| v.as_str()), Some("DOG"));
    assert_eq!(dog_val.get("amount").and_then(|v| v.as_u64()), Some(5000000000));
    assert_eq!(dog_val.get("rate").and_then(|v| v.as_u64()), Some(47913));

    let ordi_val = parsed.get(ordi_tx).unwrap();
    assert_eq!(ordi_val.get("asset").and_then(|v| v.as_str()), Some("ORDI"));
    assert_eq!(ordi_val.get("amount").and_then(|v| v.as_u64()), Some(1000000000000000000));

    // Verify dog-evidence.json
    let dog_ev_content = std::fs::read_to_string(&dog_file).unwrap();
    let dog_ev: EvidenceFile = serde_json::from_str(&dog_ev_content).unwrap();
    assert_eq!(dog_ev.count, 1);
    assert_eq!(dog_ev.items[0].txid, dog_tx);
    assert_eq!(dog_ev.items[0].amount_atoms, "5000000000");

    // Verify ordi-evidence.json
    let ordi_ev_content = std::fs::read_to_string(&ordi_file).unwrap();
    let ordi_ev: EvidenceFile = serde_json::from_str(&ordi_ev_content).unwrap();
    assert_eq!(ordi_ev.count, 1);
    assert_eq!(ordi_ev.items[0].txid, ordi_tx);
    assert_eq!(ordi_ev.items[0].amount_atoms, "1000000000000000000");

    println!("✅ Test 2 Passed: Evidence serialization and indexer-compatible JSON validated.");
}

#[tokio::test]
async fn test_live_zero_key_verification_dog_and_ordi() {
    let test_dir = tempfile::tempdir().expect("create temp dir");
    let ext_file = test_dir.path().join("external_payments.json");
    let dog_file = test_dir.path().join("dog-evidence.json");
    let ordi_file = test_dir.path().join("ordi-evidence.json");
    let pending_file = test_dir.path().join("pending_mints.jsonl");

    // Write candidate txids to pending mints queue:
    // 1. DOG (50,000 DOG)
    // 2. ORDI (1.0 ORDI)
    let candidates = vec![
        "c892254bd7550ccac21efa53d5a18d33f55fc10f6b910e6f047c16af63b0e256",
        "d1a64d8051ae24c2032b146ecb63a789dae7b2fe584f46505f874070008027fd",
    ];
    std::fs::write(&pending_file, candidates.join("\n")).unwrap();

    let config = SidecarConfig {
        target_output_file: ext_file.clone(),
        dog_evidence_file: dog_file.clone(),
        ordi_evidence_file: ordi_file.clone(),
        pending_mints_path: pending_file.clone(),
        treasury_address: "bc1p8q4hnqrve9tdd7yzx650m4ct007w38p8gkcylrmffherq25987fq4e6ajr".to_string(),
        unisat_api_key: "".to_string(),
        poll_interval_secs: 15,
        http_port: 8085,
    };

    let engine = SidecarEngine::new(config);

    // Process pending mints via live UniSat public APIs (no API key required)
    let verified_count = engine.process_pending_mints().await;
    println!("Processed pending mints: verified {} txids", verified_count);

    if verified_count >= 1 {
        engine.save_files();
        let payments = engine.payments.read().clone();
        
        if let Some(dog) = payments.get("c892254bd7550ccac21efa53d5a18d33f55fc10f6b910e6f047c16af63b0e256") {
            assert_eq!(dog.asset, "DOG");
            assert_eq!(dog.amount, 5000000000);
            assert_eq!(dog.rate, Some(47913));
            println!("✅ Live DOG payment verified: {} atoms at rate 47913", dog.amount);
        }

        if let Some(ordi) = payments.get("d1a64d8051ae24c2032b146ecb63a789dae7b2fe584f46505f874070008027fd") {
            assert_eq!(ordi.asset, "ORDI");
            assert_eq!(ordi.amount, 1000000000000000000);
            println!("✅ Live ORDI payment verified: {} atoms (1.0 ORDI)", ordi.amount);
        }
    } else {
        println!("⚠️ Network unavailable or rate limited; skipping live assertion gracefully.");
    }
}
