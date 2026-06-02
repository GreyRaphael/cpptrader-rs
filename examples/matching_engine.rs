//! Interactive command-line matching engine.
//!
//! Usage:
//!   cargo run --example matching_engine
//!
//! Commands:
//!   add_symbol <id> <name>              — Add a symbol
//!   add_book <symbol_id>                — Add an order book
//!   add <id> <symbol> <side> <type> <price> <qty> [tif] [extra]
//!   delete <order_id>                   — Delete an order
//!   reduce <order_id> <qty>             — Reduce an order
//!   modify <order_id> <price> <qty>     — Modify an order
//!   execute <order_id> <qty>            — Execute an order
//!   match                               — Trigger manual matching
//!   enable                              — Enable automatic matching
//!   disable                             — Disable automatic matching
//!   book <symbol_id>                    — Print order book
//!   quit                                — Exit

use std::io::{self, BufRead, Write};

use cpptrader::matching::market_handler::MarketHandler;
use cpptrader::matching::market_manager::MarketManager;
use cpptrader::matching::order::Order;
use cpptrader::matching::order_book::OrderBook;
use cpptrader::matching::symbol::Symbol;
use cpptrader::matching::types::*;

struct PrintHandler;

impl MarketHandler for PrintHandler {
    fn on_add_symbol(&mut self, symbol: &Symbol) {
        println!("[EVENT] AddSymbol: {}", symbol);
    }
    fn on_delete_symbol(&mut self, symbol: &Symbol) {
        println!("[EVENT] DeleteSymbol: {}", symbol);
    }
    fn on_add_order_book(&mut self, ob: &OrderBook) {
        println!("[EVENT] AddOrderBook: {}", ob);
    }
    fn on_delete_order_book(&mut self, ob: &OrderBook) {
        println!("[EVENT] DeleteOrderBook: {}", ob);
    }
    fn on_add_order(&mut self, order: &Order) {
        println!("[EVENT] AddOrder: {}", order);
    }
    fn on_update_order(&mut self, order: &Order) {
        println!("[EVENT] UpdateOrder: {}", order);
    }
    fn on_delete_order(&mut self, order: &Order) {
        println!("[EVENT] DeleteOrder: {}", order);
    }
    fn on_execute_order(&mut self, order: &Order, price: u64, quantity: u64) {
        println!("[EVENT] ExecuteOrder: {} @ {} qty={}", order.id, price, quantity);
    }
}

fn print_book(ob: &OrderBook) {
    println!("=== Order Book: {} ===", ob.symbol());
    println!("  Bids:");
    for (price, level) in ob.bids().iter().rev() {
        println!("    {} : vol={} orders={}", price, level.level.total_volume, level.level.orders);
    }
    println!("  Asks:");
    for (price, level) in ob.asks() {
        println!("    {} : vol={} orders={}", price, level.level.total_volume, level.level.orders);
    }
    if let Some(bid) = ob.best_bid() {
        print!("  Best Bid: {}", bid.level.price);
    } else {
        print!("  Best Bid: none");
    }
    if let Some(ask) = ob.best_ask() {
        println!("  Best Ask: {}", ask.level.price);
    } else {
        println!("  Best Ask: none");
    }
}

fn parse_side(s: &str) -> Option<OrderSide> {
    match s.to_lowercase().as_str() {
        "buy" | "b" => Some(OrderSide::Buy),
        "sell" | "s" => Some(OrderSide::Sell),
        _ => None,
    }
}

fn parse_tif(s: &str) -> Option<OrderTimeInForce> {
    match s.to_uppercase().as_str() {
        "GTC" => Some(OrderTimeInForce::Gtc),
        "IOC" => Some(OrderTimeInForce::Ioc),
        "FOK" => Some(OrderTimeInForce::Fok),
        "AON" => Some(OrderTimeInForce::Aon),
        _ => None,
    }
}

fn main() {
    let handler = Box::new(PrintHandler);
    let mut mm = MarketManager::new(handler);

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    print!("> ");
    stdout.flush().unwrap();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            print!("> ");
            stdout.flush().unwrap();
            continue;
        }

        match parts[0] {
            "add_symbol" => {
                if parts.len() < 3 {
                    println!("Usage: add_symbol <id> <name>");
                } else {
                    let id: u32 = parts[1].parse().unwrap();
                    let mut name = [0u8; 8];
                    let bytes = parts[2].as_bytes();
                    let len = bytes.len().min(8);
                    name[..len].copy_from_slice(&bytes[..len]);
                    let sym = Symbol::new(id, &name);
                    match mm.add_symbol(sym) {
                        Ok(()) => println!("OK"),
                        Err(e) => println!("Error: {}", e),
                    }
                }
            }
            "add_book" => {
                if parts.len() < 2 {
                    println!("Usage: add_book <symbol_id>");
                } else {
                    let id: u32 = parts[1].parse().unwrap();
                    let sym = match mm.get_symbol(id) {
                        Some(s) => *s,
                        None => { println!("Symbol not found"); print!("> "); stdout.flush().unwrap(); continue; }
                    };
                    match mm.add_order_book(&sym) {
                        Ok(()) => println!("OK"),
                        Err(e) => println!("Error: {}", e),
                    }
                }
            }
            "add" => {
                if parts.len() < 7 {
                    println!("Usage: add <id> <symbol> <side> <type> <price> <qty> [tif] [extra]");
                    println!("  type: market, limit, stop, stop_limit, trailing_stop, trailing_stop_limit");
                    println!("  tif: GTC, IOC, FOK, AON (default GTC)");
                    println!("  extra: max_visible (limit/stop_limit) or slippage (market/stop)");
                } else {
                    let id: u64 = parts[1].parse().unwrap();
                    let symbol_id: u32 = parts[2].parse().unwrap();
                    let side = match parse_side(parts[3]) {
                        Some(s) => s,
                        None => { println!("Invalid side"); print!("> "); stdout.flush().unwrap(); continue; }
                    };
                    let price: u64 = parts[5].parse().unwrap();
                    let qty: u64 = parts[6].parse().unwrap();
                    let tif = if parts.len() > 7 { parse_tif(parts[7]).unwrap_or(OrderTimeInForce::Gtc) } else { OrderTimeInForce::Gtc };
                    let extra: u64 = if parts.len() > 8 { parts[8].parse().unwrap_or(u64::MAX) } else { u64::MAX };

                    let order = match parts[4].to_lowercase().as_str() {
                        "market" => Order::market(id, symbol_id, side, qty, extra),
                        "limit" => Order::limit(id, symbol_id, side, price, qty, tif, extra),
                        "stop" => Order::stop(id, symbol_id, side, price, qty, tif, extra),
                        "stop_limit" => Order::stop_limit(id, symbol_id, side, price, price, qty, tif, extra),
                        "trailing_stop" => Order::trailing_stop(id, symbol_id, side, price, qty, extra as i64, 0, tif, u64::MAX),
                        "trailing_stop_limit" => Order::trailing_stop_limit(id, symbol_id, side, price, price, qty, extra as i64, 0, tif, u64::MAX),
                        _ => { println!("Unknown order type"); print!("> "); stdout.flush().unwrap(); continue; }
                    };

                    match mm.add_order(order) {
                        Ok(()) => println!("OK"),
                        Err(e) => println!("Error: {}", e),
                    }
                }
            }
            "delete" => {
                if parts.len() < 2 {
                    println!("Usage: delete <order_id>");
                } else {
                    let id: u64 = parts[1].parse().unwrap();
                    match mm.delete_order(id) {
                        Ok(()) => println!("OK"),
                        Err(e) => println!("Error: {}", e),
                    }
                }
            }
            "reduce" => {
                if parts.len() < 3 {
                    println!("Usage: reduce <order_id> <qty>");
                } else {
                    let id: u64 = parts[1].parse().unwrap();
                    let qty: u64 = parts[2].parse().unwrap();
                    match mm.reduce_order(id, qty) {
                        Ok(()) => println!("OK"),
                        Err(e) => println!("Error: {}", e),
                    }
                }
            }
            "modify" => {
                if parts.len() < 4 {
                    println!("Usage: modify <order_id> <price> <qty>");
                } else {
                    let id: u64 = parts[1].parse().unwrap();
                    let price: u64 = parts[2].parse().unwrap();
                    let qty: u64 = parts[3].parse().unwrap();
                    match mm.modify_order(id, price, qty) {
                        Ok(()) => println!("OK"),
                        Err(e) => println!("Error: {}", e),
                    }
                }
            }
            "execute" => {
                if parts.len() < 3 {
                    println!("Usage: execute <order_id> <qty>");
                } else {
                    let id: u64 = parts[1].parse().unwrap();
                    let qty: u64 = parts[2].parse().unwrap();
                    match mm.execute_order(id, qty) {
                        Ok(()) => println!("OK"),
                        Err(e) => println!("Error: {}", e),
                    }
                }
            }
            "match" => {
                mm.match_all();
                println!("OK");
            }
            "enable" => {
                mm.enable_matching();
                println!("Matching enabled");
            }
            "disable" => {
                mm.disable_matching();
                println!("Matching disabled");
            }
            "book" => {
                if parts.len() < 2 {
                    println!("Usage: book <symbol_id>");
                } else {
                    let id: u32 = parts[1].parse().unwrap();
                    match mm.get_order_book(id) {
                        Some(ob) => print_book(ob),
                        None => println!("Order book not found"),
                    }
                }
            }
            "quit" | "exit" => break,
            "help" => {
                println!("Commands:");
                println!("  add_symbol <id> <name>");
                println!("  add_book <symbol_id>");
                println!("  add <id> <symbol> <side> <type> <price> <qty> [tif] [extra]");
                println!("  delete <order_id>");
                println!("  reduce <order_id> <qty>");
                println!("  modify <order_id> <price> <qty>");
                println!("  execute <order_id> <qty>");
                println!("  match");
                println!("  enable / disable");
                println!("  book <symbol_id>");
                println!("  quit");
            }
            _ => println!("Unknown command. Type 'help' for usage."),
        }

        print!("> ");
        stdout.flush().unwrap();
    }
}
