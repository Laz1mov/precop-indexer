use tusm_core::legacy_v1_router::LegacyV1Router;
use tusm_core::prevout::{Coin, PrevoutError, PrevoutResolver};
use tusm_core::state::SigmaLeaf;
use bitcoin::{Transaction, TxIn, TxOut, ScriptBuf, OutPoint, Txid, Witness, Address, Network};
use bitcoin::hashes::Hash;
use std::collections::HashMap;
use std::str::FromStr;

struct MockPrevouts {
    map: HashMap<OutPoint, Coin>,
}

impl PrevoutResolver for MockPrevouts {
    fn prevout(&self, outpoint: &OutPoint) -> Result<&Coin, PrevoutError> {
        self.map.get(outpoint).ok_or(PrevoutError::MissingPrevout(*outpoint))
    }
}

#[test]
fn test_sighash_single_psbt_rejected_without_counterparty_output() {
    let seller_addr = Address::from_str("bc1p4lve2mt0lwsmsv49kkew2dhsx0j8phmdm53dswmwgz63r8yt9req8z5g65").unwrap().assume_checked();
    let attacker_addr = Address::from_str("bc1qrmuh3yg43pyqznchwkl2stcac2y6fu6920qkev").unwrap().assume_checked();

    let prev_outpoint = OutPoint {
        txid: Txid::from_str("0073b2d75723caf2c496b3f31fc5e4ba4299212f219da7aadfa626c26e033b34").unwrap(),
        vout: 1,
    };

    let mut prevouts = HashMap::new();
    prevouts.insert(prev_outpoint, Coin {
        value: 670,
        script_pubkey: seller_addr.script_pubkey(),
    });
    let resolver = MockPrevouts { map: prevouts };

    // SIGHASH_SINGLE signature (65 bytes ending in 0x83)
    let mut sig_single = vec![1u8; 64];
    sig_single.push(0x83);

    let mut witness = Witness::new();
    witness.push(sig_single);

    let op_return_script = ScriptBuf::from_bytes(
        hex::decode("6a437b2270223a226372632d3230222c226f70223a227472616e73666572222c227469636b223a224c454146222c22616d74223a223130303030303030303030303030227d").unwrap()
    );

    // Hostile TX: output 0 is OP_RETURN, output 1 is attacker address, NO seller counterparty output
    let hostile_tx = Transaction {
        version: 2,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: prev_outpoint,
            script_sig: ScriptBuf::new(),
            sequence: bitcoin::Sequence::MAX,
            witness: witness.clone(),
        }],
        output: vec![
            TxOut { value: 0, script_pubkey: op_return_script.clone() },
            TxOut { value: 330, script_pubkey: attacker_addr.script_pubkey() },
        ],
    };

    let get_state = |_tick: &str| SigmaLeaf {
        supply: 1_000_000_000_000_000_000,
        max_supply: 1_000_000_000_000_000_000,
        limit_per_mint: 21_000_000_000_000,
        btc_rate: 0,
        dog_rate: 0,
        leaf_rate: 0,
        ordi_rate: 0,
        anchor: vec![0u8; 36],
        height: 968266,
        version: 1,
        halted: false,
        consumed_points: 0,
    };

    let get_balance = |_tick: &str, _addr: &str| -> u128 { 500_000_000_000_000_000 };
    let external_payments = HashMap::new();

    let result = LegacyV1Router::process(
        &hostile_tx,
        get_state,
        get_balance,
        968266,
        &resolver,
        &external_payments,
    ).unwrap();

    assert!(result.is_none(), "Hostile transaction must be rejected because counterparty output is missing!");

    // Legitimate TX: includes seller payout output
    let legit_tx = Transaction {
        version: 2,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: prev_outpoint,
            script_sig: ScriptBuf::new(),
            sequence: bitcoin::Sequence::MAX,
            witness,
        }],
        output: vec![
            TxOut { value: 350000, script_pubkey: seller_addr.script_pubkey() }, // Seller gets paid!
            TxOut { value: 0, script_pubkey: op_return_script },
            TxOut { value: 1000, script_pubkey: attacker_addr.script_pubkey() },
        ],
    };

    let legit_result = LegacyV1Router::process(
        &legit_tx,
        get_state,
        get_balance,
        968266,
        &resolver,
        &external_payments,
    ).unwrap();

    assert!(legit_result.is_some(), "Legitimate transaction with seller counterparty output must be accepted!");
}
