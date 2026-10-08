use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tusm_core::astrolabe::verify_astrolabe_anchor;
use tusm_core::law::apply_mint_leaf;
use tusm_core::tx_env_bitcoin::BitcoinTxEnv;
use bitcoin::{Transaction, TxIn, TxOut, ScriptBuf, Amount, Sequence, Witness};
use bitcoin::hashes::{sha256, Hash};

fn setup_mock_mint_env() -> (BitcoinTxEnv<'static>, [u8; 32]) {
    // Generate a mock transaction with 1 input and 2 outputs to simulate a V2 Mint
    // The input needs a witness of length >= 2 where the second element (index 1) is a 960-byte Utreexo proof (depth 30)
    let mut witness = Witness::new();
    witness.push(vec![0; 32]); // Dummy pubkey
    witness.push(vec![0; 960]); // 30 hashes (depth 30) * 32 bytes

    let tx = Box::new(Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: bitcoin::OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::from_consensus(144),
            witness,
        }],
        output: vec![
            TxOut { value: Amount::ZERO, script_pubkey: ScriptBuf::new() },
            TxOut { value: Amount::from_sat(10000), script_pubkey: ScriptBuf::from_bytes(vec![0x51, 0x20, 0x38, 0x2b]) },
        ],
    });

    let utxos = Box::new(vec![TxOut {
        value: Amount::from_sat(100000),
        script_pubkey: ScriptBuf::new(),
    }]);

    // Leak to get static references for the bench
    let tx = Box::leak(tx);
    let utxos = Box::leak(utxos);

    let env = BitcoinTxEnv::new(tx, utxos);
    
    // We mock the tge_root by running the actual compute once
    use tusm_core::complexity_adapter::complexity::env::TxEnv;
    let adapter = tusm_core::complexity_adapter::ComplexityAdapter { physical_env: env };
    
    // Grab the utxos hash the law will use
    let user_pubkey_hash = adapter.physical_env.jet_input_utxos_hash();

    // The dummy proof has length 960 (30 elements)
    let tge_root = adapter.jet_compute_merkle_root(user_pubkey_hash, &vec![0; 960]).unwrap_or([0u8; 32]);
    let env = adapter.physical_env;
    
    (env, tge_root)
}

fn bench_astrolabe_anchor(c: &mut Criterion) {
    let mut group = c.benchmark_group("Astrolabe V2");
    // Dummy parameters
    let ticker = "LEAF";
    let supply = 1_000_000_000_000;
    // Compute expected once so we can benchmark verify
    let expected_script = tusm_core::astrolabe::compute_astrolabe(supply, ticker);
    
    group.bench_function("verify_astrolabe_anchor", |b| {
        b.iter(|| {
            verify_astrolabe_anchor(
                black_box(supply),
                black_box(ticker),
                black_box(&expected_script)
            )
        })
    });
    group.finish();
}

fn bench_utreexo_mint(c: &mut Criterion) {
    let mut group = c.benchmark_group("Utreexo Verification");
    let (env, tge_root) = setup_mock_mint_env();
    let current_supply = 50_000_000;
    
    group.bench_function("apply_mint_leaf_depth_30", |b| {
        b.iter(|| {
            apply_mint_leaf(
                black_box(current_supply),
                black_box(&tge_root),
                black_box(&env)
            ).unwrap()
        })
    });
    group.finish();
}

criterion_group!(benches, bench_astrolabe_anchor, bench_utreexo_mint);
criterion_main!(benches);
