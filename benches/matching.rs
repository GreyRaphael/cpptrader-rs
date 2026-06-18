use criterion::{Criterion, criterion_group, criterion_main};

fn bench_noop(_c: &mut Criterion) {}

criterion_group!(benches, bench_noop);
criterion_main!(benches);
