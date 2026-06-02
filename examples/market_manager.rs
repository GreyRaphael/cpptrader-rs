//! Market manager example — demonstrates MarketHandler callbacks.
//!
//! This example creates a MarketManager, adds symbols and order books,
//! places various orders, and prints all market events via the handler.
//!
//! Usage:
//!   cargo run --example market_manager

use cpptrader::matching::level::Level;
use cpptrader::matching::market_handler::MarketHandler;
use cpptrader::matching::market_manager::MarketManager;
use cpptrader::matching::order::Order;
use cpptrader::matching::order_book::OrderBook;
use cpptrader::matching::symbol::Symbol;
use cpptrader::matching::types::*;

struct LoggingHandler;

impl MarketHandler for LoggingHandler {
    fn on_add_symbol(&mut self, symbol: &Symbol) {
        println!("[AddSymbol] {}", symbol);
    }
    fn on_delete_symbol(&mut self, symbol: &Symbol) {
        println!("[DeleteSymbol] {}", symbol);
    }
    fn on_add_order_book(&mut self, ob: &OrderBook) {
        println!("[AddOrderBook] {}", ob);
    }
    fn on_update_order_book(&mut self, ob: &OrderBook, top: bool) {
        println!("[UpdateOrderBook] {} top={}", ob.symbol(), top);
    }
    fn on_delete_order_book(&mut self, ob: &OrderBook) {
        println!("[DeleteOrderBook] {}", ob);
    }
    fn on_add_level(&mut self, ob: &OrderBook, level: &Level, top: bool) {
        println!("[AddLevel] {} {} top={}", ob.symbol(), level, top);
    }
    fn on_update_level(&mut self, ob: &OrderBook, level: &Level, top: bool) {
        println!("[UpdateLevel] {} {} top={}", ob.symbol(), level, top);
    }
    fn on_delete_level(&mut self, ob: &OrderBook, level: &Level, top: bool) {
        println!("[DeleteLevel] {} {} top={}", ob.symbol(), level, top);
    }
    fn on_add_order(&mut self, order: &Order) {
        println!("[AddOrder] {}", order);
    }
    fn on_update_order(&mut self, order: &Order) {
        println!("[UpdateOrder] {}", order);
    }
    fn on_delete_order(&mut self, order: &Order) {
        println!("[DeleteOrder] {}", order);
    }
    fn on_execute_order(&mut self, order: &Order, price: u64, quantity: u64) {
        println!("[ExecuteOrder] {} @ {} qty={}", order.id, price, quantity);
    }
}

fn main() {
    println!("=== CppTrader Market Manager Example ===\n");

    let handler = Box::new(LoggingHandler);
    let mut mm = MarketManager::new(handler);

    // -- Setup symbol and order book --
    let sym = Symbol::new(1, b"AAPL    ");
    mm.add_symbol(sym).unwrap();
    mm.add_order_book(&sym).unwrap();

    // -- Add limit orders (no matching yet) --
    println!("\n--- Adding limit orders ---");
    mm.add_order(Order::buy_limit(1, 1, 15000, 100, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::buy_limit(2, 1, 14900, 200, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::buy_limit(3, 1, 14800, 300, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    mm.add_order(Order::sell_limit(4, 1, 15100, 100, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::sell_limit(5, 1, 15200, 200, OrderTimeInForce::Gtc, u64::MAX)).unwrap();
    mm.add_order(Order::sell_limit(6, 1, 15300, 300, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Print book
    println!("\n--- Order Book ---");
    let ob = mm.get_order_book(1).unwrap();
    println!("Bids:");
    for (price, level) in ob.bids().iter().rev() {
        println!("  {} : vol={}", price, level.level.total_volume);
    }
    println!("Asks:");
    for (price, level) in ob.asks() {
        println!("  {} : vol={}", price, level.level.total_volume);
    }

    // -- Enable matching and add crossing orders --
    println!("\n--- Enabling matching ---");
    mm.enable_matching();

    println!("\n--- Adding crossing limit order ---");
    mm.add_order(Order::buy_limit(7, 1, 15100, 50, OrderTimeInForce::Gtc, u64::MAX)).unwrap();

    // Print updated book
    println!("\n--- Updated Order Book ---");
    let ob = mm.get_order_book(1).unwrap();
    println!("Bids:");
    for (price, level) in ob.bids().iter().rev() {
        println!("  {} : vol={}", price, level.level.total_volume);
    }
    println!("Asks:");
    for (price, level) in ob.asks() {
        println!("  {} : vol={}", price, level.level.total_volume);
    }

    // -- Add IOC order --
    println!("\n--- Adding IOC sell order ---");
    mm.add_order(Order::sell_limit(8, 1, 15000, 500, OrderTimeInForce::Ioc, u64::MAX)).unwrap();

    // -- Reduce and modify --
    println!("\n--- Reduce and modify ---");
    mm.reduce_order(2, 50).unwrap();
    mm.modify_order(5, 15150, 100).unwrap();

    // -- Final book state --
    println!("\n--- Final Order Book ---");
    let ob = mm.get_order_book(1).unwrap();
    println!("Bids:");
    for (price, level) in ob.bids().iter().rev() {
        println!("  {} : vol={}", price, level.level.total_volume);
    }
    println!("Asks:");
    for (price, level) in ob.asks() {
        println!("  {} : vol={}", price, level.level.total_volume);
    }
    println!("Active orders: {}", mm.order_count());

    println!("\n=== Done ===");
}
