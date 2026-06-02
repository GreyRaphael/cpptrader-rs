use criterion::{criterion_group, criterion_main, Criterion};

fn bench_noop(_c: &mut Criterion) {}

criterion_group!(benches, bench_noop);
criterion_main!(benches);
