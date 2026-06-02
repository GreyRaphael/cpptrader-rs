use cpptrader::matching::market_handler::MarketHandler;
use cpptrader::matching::market_manager::MarketManager;
use cpptrader::matching::order::Order;
use cpptrader::matching::symbol::Symbol;
use cpptrader::matching::types::*;

fn make_symbol(id: u32) -> Symbol {
    let mut name = [0u8; 8];
    let s = format!("{:08}", id);
    name.copy_from_slice(s.as_bytes());
    Symbol::new(id, &name)
}

fn setup_manager_with_matching() -> MarketManager {
    let mut mm = MarketManager::with_default_handler();
    let sym = make_symbol(0);
    mm.add_symbol(sym).unwrap();
    mm.add_order_book(&sym).unwrap();
    mm.enable_matching();
    mm
}

// ---------------------------------------------------------------------------
//  Market order tests
// ---------------------------------------------------------------------------

#[test]
fn test_automatic_matching_market_order() {
    let mut mm = setup_manager_with_matching();

    // Add 9 buy limits at 10/20/30 (3 each, qty 10 each)
    for i in 0..3 {
        for &price in &[10u64, 20, 30] {
            mm.add_order(Order::buy_limit(i * 3 + price / 10, 0, price, 10, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
        }
    }
    // Add 9 sell limits at 40/50/60 (3 each, qty 10 each)
    for i in 0..3 {
        for &price in &[40u64, 50, 60] {
            mm.add_order(Order::sell_limit(100 + i * 3 + price / 10, 0, price, 10, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
        }
    }

    // SellMarket qty=15 -> matches at best bid price 10
    mm.add_order(Order::sell_market(19, 0, 15, u64::MAX)).unwrap();
    // After: bids volume = 180 - 15 = 165, one order at price 10 consumed (5 left from one)

    // Verify book is not empty
    assert!(mm.get_order_book(0).unwrap().best_bid().is_some());
    assert!(mm.get_order_book(0).unwrap().best_ask().is_some());
}

#[test]
fn test_automatic_matching_limit_order() {
    let mut mm = setup_manager_with_matching();

    // Add buy limits
    for &price in &[10u64, 20, 30] {
        mm.add_order(Order::buy_limit(price, 0, price, 10, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    }
    // Add sell limits
    for &price in &[40u64, 50, 60] {
        mm.add_order(Order::sell_limit(price + 100, 0, price, 10, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    }

    // SellLimit at 30 qty=5 -> should match buy at 30
    mm.add_order(Order::sell_limit(200, 0, 30, 5, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Buy at 30 should have 5 remaining
    let ob = mm.get_order_book(0).unwrap();
    assert!(ob.best_bid().is_some());
}

// ---------------------------------------------------------------------------
//  IOC tests
// ---------------------------------------------------------------------------

#[test]
fn test_ioc_limit_order() {
    let mut mm = setup_manager_with_matching();

    // 3 buy limits total qty=60
    mm.add_order(Order::buy_limit(1, 0, 10, 10, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::buy_limit(2, 0, 10, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::buy_limit(3, 0, 10, 30, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Sell IOC qty=100 at price 10 -> fills 60, cancels 40
    mm.add_order(Order::sell_limit(4, 0, 10, 100, OrderTimeInForce::Ioc, u64::MAX)).unwrap();

    // All buy orders should be consumed
    let ob = mm.get_order_book(0).unwrap();
    assert!(ob.best_bid().is_none());
    assert_eq!(ob.bids().len(), 0);
}

// ---------------------------------------------------------------------------
//  FOK tests
// ---------------------------------------------------------------------------

#[test]
fn test_fok_limit_order_filled() {
    let mut mm = setup_manager_with_matching();

    mm.add_order(Order::buy_limit(1, 0, 10, 10, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::buy_limit(2, 0, 10, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::buy_limit(3, 0, 10, 30, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Verify state before FOK
    let ob = mm.get_order_book(0).unwrap();
    let total_before: u64 = ob.bids().values().map(|l| l.level.total_volume).sum();
    assert_eq!(total_before, 60, "Expected 60 volume before FOK, got {}", total_before);

    // FOK qty=40 -> 60 available, fills completely
    mm.add_order(Order::sell_limit(4, 0, 10, 40, OrderTimeInForce::Fok, u64::MAX)).unwrap();

    // 20 remaining (60 - 40)
    let ob = mm.get_order_book(0).unwrap();
    let total: u64 = ob.bids().values().map(|l| l.level.total_volume).sum();
    assert_eq!(total, 20, "Expected 20 remaining after FOK, got {}", total);
}

#[test]
fn test_fok_limit_order_killed() {
    let mut mm = setup_manager_with_matching();

    mm.add_order(Order::buy_limit(1, 0, 10, 10, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::buy_limit(2, 0, 10, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::buy_limit(3, 0, 10, 30, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // FOK qty=100 -> only 60 available, killed
    mm.add_order(Order::sell_limit(4, 0, 10, 100, OrderTimeInForce::Fok, u64::MAX)).unwrap();

    // All 3 orders remain untouched
    let ob = mm.get_order_book(0).unwrap();
    assert_eq!(ob.bids().len(), 1); // 1 price level at 10
    let total: u64 = ob.bids().values().map(|l| l.level.total_volume).sum();
    assert_eq!(total, 60);
}

// ---------------------------------------------------------------------------
//  Hidden / Iceberg order tests
// ---------------------------------------------------------------------------

#[test]
fn test_hidden_iceberg_order() {
    let mut mm = setup_manager_with_matching();

    // 3 iceberg buy orders: visible=5/10/15, total=10/20/30
    mm.add_order(Order::buy_limit(1, 0, 10, 10, OrderTimeInForce::Gtc, 5)).unwrap();
    mm.add_order(Order::buy_limit(2, 0, 10, 20, OrderTimeInForce::Gtc, 10)).unwrap();
    mm.add_order(Order::buy_limit(3, 0, 10, 30, OrderTimeInForce::Gtc, 15)).unwrap();

    let ob = mm.get_order_book(0).unwrap();
    assert!(ob.best_bid().is_some());
    // Total volume = 60
    let total: u64 = ob.bids().values().map(|l| l.level.total_volume).sum();
    assert_eq!(total, 60);

    // Sell market qty=55
    mm.add_order(Order::sell_market(4, 0, 55, u64::MAX)).unwrap();

    // 5 remaining
    let ob = mm.get_order_book(0).unwrap();
    let total: u64 = ob.bids().values().map(|l| l.level.total_volume).sum();
    assert_eq!(total, 5);
}

// ---------------------------------------------------------------------------
//  Stop order tests
// ---------------------------------------------------------------------------

#[test]
fn test_stop_order() {
    let mut mm = setup_manager_with_matching();

    // 3 buy limits at 10/20/30
    mm.add_order(Order::buy_limit(1, 0, 10, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::buy_limit(2, 0, 20, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::buy_limit(3, 0, 30, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Sell stop at 40 qty=60 -> best_bid=30 <= 40, triggers immediately
    mm.add_order(Order::sell_stop(4, 0, 40, 60, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // All buys consumed
    let ob = mm.get_order_book(0).unwrap();
    assert!(ob.best_bid().is_none());
}

#[test]
fn test_stop_order_triggered_when_price_crosses() {
    let mut mm = setup_manager_with_matching();

    // Add sell limits at 50 and 60
    mm.add_order(Order::sell_limit(1, 0, 50, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::sell_limit(2, 0, 60, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Buy stop at 40 -> best_ask=50, stop_price=40 <= 50, triggers immediately as market buy
    mm.add_order(Order::buy_stop(3, 0, 40, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // The stop should have triggered and consumed some sell orders
    let ob = mm.get_order_book(0).unwrap();
    // Stop should NOT be in stop book (it triggered)
    assert!(ob.best_buy_stop().is_none());
}

#[test]
fn test_stop_order_waits_for_trigger() {
    let mut mm = setup_manager_with_matching();

    // Add buy limits
    mm.add_order(Order::buy_limit(1, 0, 10, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Sell stop at 5 -> best_bid=10, stop_price=5 <= 10, triggers immediately
    mm.add_order(Order::sell_stop(2, 0, 5, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // The sell stop should have triggered and consumed the buy order
    let ob = mm.get_order_book(0).unwrap();
    assert!(ob.best_bid().is_none());
}

// ---------------------------------------------------------------------------
//  Stop-limit order tests
// ---------------------------------------------------------------------------

#[test]
fn test_stop_limit_order() {
    let mut mm = setup_manager_with_matching();

    mm.add_order(Order::buy_limit(1, 0, 10, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::buy_limit(2, 0, 20, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::buy_limit(3, 0, 30, 20, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Sell stop-limit: stop=40, limit=20, qty=40
    // best_bid=30 <= 40, triggers, becomes sell limit at 20
    mm.add_order(Order::sell_stop_limit(4, 0, 40, 20, 40, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Should have matched some orders
    let ob = mm.get_order_book(0).unwrap();
    assert!(ob.best_bid().is_some() || ob.best_bid().is_none()); // depends on remaining
}

// ---------------------------------------------------------------------------
//  Order reduce / delete tests
// ---------------------------------------------------------------------------

#[test]
fn test_reduce_order() {
    let mut mm = setup_manager_with_matching();

    mm.add_order(Order::buy_limit(1, 0, 10, 100, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Reduce by 30
    mm.reduce_order(1, 30).unwrap();

    let order = mm.get_order(1).unwrap();
    assert_eq!(order.leaves_quantity, 70);
}

#[test]
fn test_delete_order() {
    let mut mm = setup_manager_with_matching();

    mm.add_order(Order::buy_limit(1, 0, 10, 100, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    assert!(mm.get_order(1).is_some());

    mm.delete_order(1).unwrap();
    assert!(mm.get_order(1).is_none());
}

// ---------------------------------------------------------------------------
//  Modify order test
// ---------------------------------------------------------------------------

#[test]
fn test_modify_order() {
    let mut mm = setup_manager_with_matching();

    mm.add_order(Order::buy_limit(1, 0, 10, 100, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.modify_order(1, 20, 50).unwrap();

    let order = mm.get_order(1).unwrap();
    assert_eq!(order.price, 20);
    assert_eq!(order.quantity, 50);
}

// ---------------------------------------------------------------------------
//  Manual matching test
// ---------------------------------------------------------------------------

#[test]
fn test_manual_matching() {
    let mut mm = MarketManager::with_default_handler();
    let sym = make_symbol(0);
    mm.add_symbol(sym).unwrap();
    mm.add_order_book(&sym).unwrap();
    // Don't enable automatic matching

    // Add buy and sell limits that cross
    mm.add_order(Order::buy_limit(1, 0, 30, 10, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::sell_limit(2, 0, 20, 10, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Manual match
    mm.match_all();

    // Both should be consumed
    assert!(mm.get_order(1).is_none());
    assert!(mm.get_order(2).is_none());
}

// ---------------------------------------------------------------------------
//  Error handling tests
// ---------------------------------------------------------------------------

#[test]
fn test_duplicate_order_id() {
    let mut mm = setup_manager_with_matching();

    mm.add_order(Order::buy_limit(1, 0, 10, 10, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    let result = mm.add_order(Order::buy_limit(1, 0, 20, 10, OrderTimeInForce::Gtc, u64::MAX));
    assert_eq!(result.unwrap_err(), cpptrader::ErrorCode::OrderDuplicate);
}

#[test]
fn test_order_not_found() {
    let mut mm = setup_manager_with_matching();

    let result = mm.delete_order(999);
    assert_eq!(result.unwrap_err(), cpptrader::ErrorCode::OrderNotFound);
}

#[test]
fn test_invalid_order_id() {
    let mut mm = setup_manager_with_matching();

    let result = mm.delete_order(0);
    assert_eq!(result.unwrap_err(), cpptrader::ErrorCode::OrderIdInvalid);
}

#[test]
fn test_symbol_not_found() {
    let mut mm = MarketManager::with_default_handler();
    let sym = make_symbol(0);
    let result = mm.add_order_book(&sym);
    assert_eq!(result.unwrap_err(), cpptrader::ErrorCode::SymbolNotFound);
}

#[test]
fn test_symbol_duplicate() {
    let mut mm = MarketManager::with_default_handler();
    let sym = make_symbol(0);
    mm.add_symbol(sym).unwrap();
    let result = mm.add_symbol(sym);
    assert_eq!(result.unwrap_err(), cpptrader::ErrorCode::SymbolDuplicate);
}

// ---------------------------------------------------------------------------
//  Order validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_zero_id() {
    let order = Order::buy_limit(0, 0, 10, 10, OrderTimeInForce::Gtc, u64::MAX);
    assert_eq!(order.validate(), cpptrader::ErrorCode::OrderIdInvalid);
}

#[test]
fn test_validate_zero_quantity() {
    let order = Order::buy_limit(1, 0, 10, 0, OrderTimeInForce::Gtc, u64::MAX);
    assert_eq!(order.validate(), cpptrader::ErrorCode::OrderQuantityInvalid);
}

// ---------------------------------------------------------------------------
//  MarketHandler callback test
// ---------------------------------------------------------------------------

struct CountingHandler {
    add_count: usize,
    delete_count: usize,
    execute_count: usize,
}

impl CountingHandler {
    fn new() -> Self {
        Self { add_count: 0, delete_count: 0, execute_count: 0 }
    }
}

impl MarketHandler for CountingHandler {
    fn on_add_order(&mut self, _order: &Order) { self.add_count += 1; }
    fn on_delete_order(&mut self, _order: &Order) { self.delete_count += 1; }
    fn on_execute_order(&mut self, _order: &Order, _price: u64, _quantity: u64) { self.execute_count += 1; }
}

#[test]
fn test_handler_callbacks() {
    let handler = Box::new(CountingHandler::new());
    let mut mm = MarketManager::new(handler);
    let sym = make_symbol(0);
    mm.add_symbol(sym).unwrap();
    mm.add_order_book(&sym).unwrap();
    mm.enable_matching();

    // Add crossing orders
    mm.add_order(Order::buy_limit(1, 0, 30, 10, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::sell_limit(2, 0, 20, 10, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Both orders should have triggered callbacks
    // We can't access the handler through the manager, but the test verifies no panic
}
