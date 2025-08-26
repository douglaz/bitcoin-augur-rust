use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction};
use chrono::{Duration, Utc};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

/// Generate a mempool snapshot with the specified number of transactions
fn generate_mempool_snapshot(num_transactions: usize) -> MempoolSnapshot {
    let mut transactions = Vec::with_capacity(num_transactions);
    let timestamp = Utc::now();

    for i in 0..num_transactions {
        // Create a diverse range of fee rates (1-100 sat/vB)
        let fee_rate = (i as f64 % 100.0) + 1.0;
        let weight = (1000 + (i % 4000)) as u64; // Vary weight between 1000 and 5000
        let fee = (fee_rate * weight as f64 / 4.0) as u64;

        transactions.push(MempoolTransaction::new(weight, fee));
    }

    MempoolSnapshot::from_transactions(transactions, 800000, timestamp)
}

/// Generate multiple snapshots over time
fn generate_snapshot_history(
    num_snapshots: usize,
    txs_per_snapshot: usize,
) -> Vec<MempoolSnapshot> {
    let mut snapshots = Vec::with_capacity(num_snapshots);
    let mut base_time = Utc::now() - Duration::hours(24);
    let mut height = 800000;

    for _ in 0..num_snapshots {
        let transactions: Vec<MempoolTransaction> = (0..txs_per_snapshot)
            .map(|i| {
                let fee_rate = (i as f64 % 100.0) + 1.0;
                let weight = (1000 + (i % 4000)) as u64;
                let fee = (fee_rate * weight as f64 / 4.0) as u64;
                MempoolTransaction::new(weight, fee)
            })
            .collect();

        let snapshot = MempoolSnapshot::from_transactions(transactions, height, base_time);
        snapshots.push(snapshot);

        base_time += Duration::minutes(10);
        height += 1;
    }

    snapshots
}

fn benchmark_fee_estimation(c: &mut Criterion) {
    let mut group = c.benchmark_group("fee_estimation");

    // Test with different mempool sizes
    for size in [1000, 10000, 50000, 100000].iter() {
        let snapshots = vec![generate_mempool_snapshot(*size)];
        let estimator = FeeEstimator::new();

        group.bench_with_input(BenchmarkId::new("single_snapshot", size), size, |b, _| {
            b.iter(|| estimator.calculate_estimates(&snapshots, None));
        });
    }

    group.finish();
}

fn benchmark_multi_snapshot_estimation(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_snapshot_estimation");

    // Test with different numbers of snapshots
    for num_snapshots in [10, 50, 100, 144].iter() {
        let snapshots = generate_snapshot_history(*num_snapshots, 10000);
        let estimator = FeeEstimator::new();

        group.bench_with_input(
            BenchmarkId::new("snapshots", num_snapshots),
            num_snapshots,
            |b, _| {
                b.iter(|| estimator.calculate_estimates(&snapshots, None));
            },
        );
    }

    group.finish();
}

// Bucket creation is tested implicitly through fee estimation
// since it's an internal implementation detail

fn benchmark_confidence_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("confidence_levels");

    let snapshots = generate_snapshot_history(50, 10000);

    // Test different confidence levels
    for confidence in [0.05, 0.2, 0.5, 0.8, 0.95].iter() {
        let estimator = FeeEstimator::with_config(
            vec![*confidence],
            FeeEstimator::DEFAULT_BLOCK_TARGETS.to_vec(),
            Duration::minutes(30),
            Duration::hours(24),
        )
        .expect("Valid config");

        group.bench_with_input(
            BenchmarkId::new("confidence", format!("{:.2}", confidence)),
            confidence,
            |b, _| {
                b.iter(|| estimator.calculate_estimates(&snapshots, None));
            },
        );
    }

    group.finish();
}

// Poisson calculation is tested implicitly through fee estimation
// since it's an internal implementation detail

criterion_group!(
    benches,
    benchmark_fee_estimation,
    benchmark_multi_snapshot_estimation,
    benchmark_confidence_levels
);
criterion_main!(benches);
