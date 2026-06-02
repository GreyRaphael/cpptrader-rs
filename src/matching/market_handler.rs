// ---------------------------------------------------------------------------
//  CppTrader Rust Port — MarketHandler trait
//  Mirrors: include/trader/matching/market_handler.h
// ---------------------------------------------------------------------------

use crate::matching::level::Level;
use crate::matching::order::Order;
use crate::matching::order_book::OrderBook;
use crate::matching::symbol::Symbol;

/// Market event handler.
///
/// Implement this trait to receive callbacks from [`MarketManager`](crate::matching::market_manager::MarketManager).
/// All methods have default no-op implementations — override only what you need.
pub trait MarketHandler {
    // Symbol events
    fn on_add_symbol(&mut self, _symbol: &Symbol) {}
    fn on_delete_symbol(&mut self, _symbol: &Symbol) {}

    // Order-book events
    fn on_add_order_book(&mut self, _order_book: &OrderBook) {}
    fn on_update_order_book(&mut self, _order_book: &OrderBook, _top: bool) {}
    fn on_delete_order_book(&mut self, _order_book: &OrderBook) {}

    // Price-level events
    fn on_add_level(&mut self, _order_book: &OrderBook, _level: &Level, _top: bool) {}
    fn on_update_level(&mut self, _order_book: &OrderBook, _level: &Level, _top: bool) {}
    fn on_delete_level(&mut self, _order_book: &OrderBook, _level: &Level, _top: bool) {}

    // Order events
    fn on_add_order(&mut self, _order: &Order) {}
    fn on_update_order(&mut self, _order: &Order) {}
    fn on_delete_order(&mut self, _order: &Order) {}

    // Execution event
    fn on_execute_order(&mut self, _order: &Order, _price: u64, _quantity: u64) {}
}

/// Default no-op handler (corresponds to C++ `static MarketHandler _default`).
pub struct NoOpHandler;
impl MarketHandler for NoOpHandler {}
