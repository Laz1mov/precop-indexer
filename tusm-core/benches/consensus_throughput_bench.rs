use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tusm_core::crc_canonical::get_canonical_crc_lock;
use tusm_core::attestation::{compute_attestation_digest, compute_hmac_rfc2104};
use tusm_core::legacy_v1_router::LegacyV1Router;
use primitive_types::U256;

fn bench_canonical_lock_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("Canonical CRC");
    let txid = "67bc208baad985b438a053a969e3a0ed2ffda172d30b21d4e22670372e3b8032";
    
    group.bench_function("lock_lookup_hit", |b| {
        b.iter(|| {
            black_box(get_canonical_crc_lock(black_box(txid)))
        });
    });

    let miss_txid = "0000000000000000000000000000000000000000000000000000000000000000";
    group.bench_function("lock_lookup_miss", |b| {
        b.iter(|| {
            black_box(get_canonical_crc_lock(black_box(miss_txid)))
        });
    });
    group.finish();
}

fn bench_attestation_crypto(c: &mut Criterion) {
    let mut group = c.benchmark_group("Attestation Security");
    let txid = "c892254bd7550ccac21efa53d5a18d33f55fc10f6b910e6f047c16af63b0e256";
    let asset = "DOG";
    let amount = 5_000_000_000u128;
    let rate = Some(47917u64);
    let secret = b"e9635c8657a15f14d06d8752bf19afecd3289b834d728345434590cfca2b8df0";

    group.bench_function("digest_computation", |b| {
        b.iter(|| {
            black_box(compute_attestation_digest(
                black_box(txid),
                black_box(asset),
                black_box(amount),
                black_box(rate),
            ))
        });
    });

    let digest = compute_attestation_digest(txid, asset, amount, rate);
    group.bench_function("hmac_rfc2104_verification", |b| {
        b.iter(|| {
            black_box(compute_hmac_rfc2104(
                black_box(secret),
                black_box(&digest),
            ))
        });
    });
    group.finish();
}

fn bench_consensus_math(c: &mut Criterion) {
    let mut group = c.benchmark_group("Consensus Math");

    group.bench_function("amount_parsing", |b| {
        let amt_str = "100000000000.12345678";
        b.iter(|| {
            black_box(LegacyV1Router::parse_amount(black_box(amt_str)))
        });
    });

    group.bench_function("crc_bonding_curve_u256", |b| {
        let p_atoms = U256::from(50_000_000_000u64);
        let lock_mult = U256::from(16_180_339u64);
        let applied_rate = U256::from(47_917u64);
        let denom = U256::from(50_000_000u64);

        b.iter(|| {
            let num = black_box(p_atoms) * black_box(applied_rate) * black_box(lock_mult);
            black_box(num / denom)
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_canonical_lock_lookup,
    bench_attestation_crypto,
    bench_consensus_math
);
criterion_main!(benches);
