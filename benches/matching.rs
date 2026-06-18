use std::hint::black_box;

use cpptrader::{MarketManager, Order, OrderTimeInForce, Symbol};
use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

const SYMBOL_ID: u32 = 0;

fn make_symbol() -> Symbol {
    Symbol::new(SYMBOL_ID, b"BENCH   ")
}

fn make_manager(matching: bool) -> MarketManager {
    let mut manager = MarketManager::with_default_handler();
    let symbol = make_symbol();
    manager.add_symbol(symbol).unwrap();
    manager.add_order_book(&symbol).unwrap();
    if matching {
        manager.enable_matching();
    }
    manager
}

fn add_sell_limits(
    manager: &mut MarketManager,
    next_id: &mut u64,
    levels: usize,
    orders_per_level: usize,
    quantity: u64,
    base_price: u64,
    tick_size: u64,
) {
    for level in 0..levels {
        let price = base_price + level as u64 * tick_size;
        for _ in 0..orders_per_level {
            manager
                .add_order(Order::sell_limit(
                    *next_id,
                    SYMBOL_ID,
                    price,
                    quantity,
                    OrderTimeInForce::Gtc,
                    u64::MAX,
                ))
                .unwrap();
            *next_id += 1;
        }
    }
}

fn add_buy_limits(
    manager: &mut MarketManager,
    next_id: &mut u64,
    levels: usize,
    orders_per_level: usize,
    quantity: u64,
    base_price: u64,
    tick_size: u64,
) {
    for level in 0..levels {
        let price = base_price - level as u64 * tick_size;
        for _ in 0..orders_per_level {
            manager
                .add_order(Order::buy_limit(
                    *next_id,
                    SYMBOL_ID,
                    price,
                    quantity,
                    OrderTimeInForce::Gtc,
                    u64::MAX,
                ))
                .unwrap();
            *next_id += 1;
        }
    }
}

fn bench_limit_add_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("limit_add_only");

    for order_count in [100usize, 1_000, 10_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(order_count),
            &order_count,
            |b, &order_count| {
                b.iter_batched(
                    || make_manager(false),
                    |mut manager| {
                        for i in 0..order_count {
                            let id = i as u64 + 1;
                            let price = 10_000 + (i % 100) as u64;
                            manager
                                .add_order(Order::buy_limit(
                                    id,
                                    SYMBOL_ID,
                                    price,
                                    1,
                                    OrderTimeInForce::Gtc,
                                    u64::MAX,
                                ))
                                .unwrap();
                        }
                        black_box(manager.order_count());
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_market_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_matching");

    group.bench_function("single_level_market_sweep_1000", |b| {
        b.iter_batched(
            || {
                let mut manager = make_manager(true);
                let mut next_id = 1;
                add_sell_limits(&mut manager, &mut next_id, 1, 1_000, 1, 10_000, 1);
                (manager, next_id)
            },
            |(mut manager, next_id)| {
                manager
                    .add_order(Order::buy_market(next_id, SYMBOL_ID, 1_000, u64::MAX))
                    .unwrap();
                black_box(manager.order_count());
            },
            BatchSize::LargeInput,
        );
    });

    group.bench_function("multi_level_market_sweep_10x100", |b| {
        b.iter_batched(
            || {
                let mut manager = make_manager(true);
                let mut next_id = 1;
                add_sell_limits(&mut manager, &mut next_id, 10, 100, 1, 10_000, 1);
                (manager, next_id)
            },
            |(mut manager, next_id)| {
                manager
                    .add_order(Order::buy_market(next_id, SYMBOL_ID, 1_000, u64::MAX))
                    .unwrap();
                black_box(manager.order_count());
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn bench_fok_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("fok_matching");

    group.bench_function("fok_success_across_10_levels", |b| {
        b.iter_batched(
            || {
                let mut manager = make_manager(true);
                let mut next_id = 1;
                add_sell_limits(&mut manager, &mut next_id, 10, 100, 1, 10_000, 1);
                (manager, next_id)
            },
            |(mut manager, next_id)| {
                manager
                    .add_order(Order::buy_limit(
                        next_id,
                        SYMBOL_ID,
                        10_009,
                        1_000,
                        OrderTimeInForce::Fok,
                        u64::MAX,
                    ))
                    .unwrap();
                black_box(manager.order_count());
            },
            BatchSize::LargeInput,
        );
    });

    group.bench_function("fok_fail_scan_10_levels", |b| {
        b.iter_batched(
            || {
                let mut manager = make_manager(true);
                let mut next_id = 1;
                add_sell_limits(&mut manager, &mut next_id, 10, 100, 1, 10_000, 1);
                (manager, next_id)
            },
            |(mut manager, next_id)| {
                manager
                    .add_order(Order::buy_limit(
                        next_id,
                        SYMBOL_ID,
                        10_009,
                        1_001,
                        OrderTimeInForce::Fok,
                        u64::MAX,
                    ))
                    .unwrap();
                black_box(manager.order_count());
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn bench_reduce_and_cancel_like_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("reduce_workload");

    group.bench_function("reduce_100_non_front_orders_same_level", |b| {
        b.iter_batched(
            || {
                let mut manager = make_manager(false);
                let mut next_id = 1;
                add_buy_limits(&mut manager, &mut next_id, 1, 1_000, 1, 10_000, 1);
                manager
            },
            |mut manager| {
                for id in (501..=600).rev() {
                    manager.reduce_order(id, 1).unwrap();
                }
                black_box(manager.order_count());
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn bench_stop_activation(c: &mut Criterion) {
    let mut group = c.benchmark_group("stop_activation");

    group.bench_function("activate_1000_buy_stops", |b| {
        b.iter_batched(
            || {
                let mut manager = make_manager(false);
                let mut next_id = 1;
                for _ in 0..1_000 {
                    manager
                        .add_order(Order::buy_stop(
                            next_id,
                            SYMBOL_ID,
                            10_000,
                            1,
                            OrderTimeInForce::Gtc,
                            u64::MAX,
                        ))
                        .unwrap();
                    next_id += 1;
                }
                add_sell_limits(&mut manager, &mut next_id, 1, 1_000, 1, 9_990, 1);
                manager
            },
            |mut manager| {
                manager.enable_matching();
                black_box(manager.order_count());
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_limit_add_only,
    bench_market_matching,
    bench_fok_matching,
    bench_reduce_and_cancel_like_workload,
    bench_stop_activation
);
criterion_main!(benches);
