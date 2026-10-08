use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tusm_core::astrolabe::{compute_astrolabe, verify_astrolabe_anchor};
use tusm_core::law::apply_mint_leaf;
use tusm_core::tx_env_bitcoin::BitcoinTxEnv;
use tusm_core::complexity_adapter::ComplexityAdapter;
use tusm_core::complexity::env::TxEnv;
use bitcoin::{Transaction, TxIn, TxOut, ScriptBuf, Sequence, Witness};
use bitcoin::secp256k1::XOnlyPublicKey;
use std::str::FromStr;

fn setup_mock_mint_env() -> (BitcoinTxEnv<'static>, [u8; 32]) {
    let mut witness = Witness::new();
    witness.push(vec![0; 32]); // Dummy pubkey
    witness.push(vec![0; 960]); // 30 hashes (depth 30) * 32 bytes

    let tx = Box::new(Transaction {
        version: 2,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: bitcoin::OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::from_consensus(144),
            witness,
        }],
        output: vec![
            TxOut { value: 0, script_pubkey: ScriptBuf::new() },
            TxOut { value: 10000, script_pubkey: ScriptBuf::from_bytes(vec![0x51, 0x20, 0x38, 0x2b]) },
        ],
    });

    let utxos = Box::new(vec![TxOut {
        value: 100000,
        script_pubkey: ScriptBuf::new(),
    }]);

    let tx = Box::leak(tx);
    let utxos = Box::leak(utxos);

    let env = BitcoinTxEnv::new(tx, utxos);
    let user_pubkey_hash = env.jet_input_utxos_hash();
    let adapter = ComplexityAdapter { physical_env: env };
    let tge_root = adapter.jet_compute_merkle_root(user_pubkey_hash, &vec![0; 960]).unwrap_or([0u8; 32]);
    let env = adapter.physical_env;

    (env, tge_root)
}

fn bench_astrolabe_anchor(c: &mut Criterion) {
    let mut group = c.benchmark_group("Astrolabe V2");
    let law_cmr = [0xabu8; 32];
    let supply = 1_000_000_000_000u128;
    let owner = XOnlyPublicKey::from_str("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0").unwrap();

    let expected_script = compute_astrolabe(supply, law_cmr, owner);

    group.bench_function("compute_astrolabe", |b| {
        b.iter(|| {
            black_box(compute_astrolabe(
                black_box(supply),
                black_box(law_cmr),
                black_box(owner),
            ))
        })
    });

    group.bench_function("verify_astrolabe_anchor", |b| {
        b.iter(|| {
            black_box(verify_astrolabe_anchor(
                black_box(supply),
                black_box(law_cmr),
                black_box(&expected_script),
                black_box(owner),
            ))
        })
    });
    group.finish();
}

fn bench_utreexo_mint(c: &mut Criterion) {
    let mut group = c.benchmark_group("Utreexo Verification");
    let (env, tge_root) = setup_mock_mint_env();
    let current_supply = 50_000_000u128;
    let max_supply = 100_000_000u128;
    let limit_per_mint = 1_000_000u128;

    group.bench_function("apply_mint_leaf_depth_30", |b| {
        b.iter(|| {
            let _ = apply_mint_leaf(
                black_box(current_supply),
                black_box(max_supply),
                black_box(limit_per_mint),
                black_box(&tge_root),
                black_box(&env),
            );
        })
    });
    group.finish();
}

criterion_group!(benches, bench_astrolabe_anchor, bench_utreexo_mint);
criterion_main!(benches);
