// ---------------------------------------------------------------------------
//  CppTrader Rust Port — OrderBook
//  Mirrors: include/trader/matching/order_book.h + source/.../order_book.cpp
// ---------------------------------------------------------------------------

use std::collections::{BTreeMap, VecDeque};

use crate::matching::level::{Level, LevelUpdate};
use crate::matching::order::{Order, OrderId};
use crate::matching::symbol::Symbol;
use crate::matching::types::{LevelType, UpdateType};

// ---------------------------------------------------------------------------
//  LevelData — per-price-level state (replaces C++ LevelNode)
// ---------------------------------------------------------------------------

/// Internal per-price-level data.
#[derive(Debug)]
pub struct LevelData {
    pub level: Level,
    /// FIFO queue of order IDs at this level (time-priority).
    pub order_queue: VecDeque<OrderId>,
}

impl LevelData {
    fn new(level_type: LevelType, price: u64) -> Self {
        Self {
            level: Level::new(level_type, price),
            order_queue: VecDeque::new(),
        }
    }
}

// ---------------------------------------------------------------------------
//  OrderBook
// ---------------------------------------------------------------------------

/// An order book for a single symbol.
///
/// Contains six `BTreeMap` collections:
/// - `bids` / `asks` — resting limit orders
/// - `buy_stop` / `sell_stop` — stop orders
/// - `trailing_buy_stop` / `trailing_sell_stop` — trailing-stop orders
///
/// Not thread-safe.
#[derive(Debug)]
pub struct OrderBook {
    symbol: Symbol,

    bids: BTreeMap<u64, LevelData>,
    asks: BTreeMap<u64, LevelData>,
    buy_stop: BTreeMap<u64, LevelData>,
    sell_stop: BTreeMap<u64, LevelData>,
    trailing_buy_stop: BTreeMap<u64, LevelData>,
    trailing_sell_stop: BTreeMap<u64, LevelData>,

    // Market price tracking
    last_bid_price: u64,
    last_ask_price: u64,
    matching_bid_price: u64,
    matching_ask_price: u64,
    _trailing_bid_price: u64,
    _trailing_ask_price: u64,
}

impl OrderBook {
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            buy_stop: BTreeMap::new(),
            sell_stop: BTreeMap::new(),
            trailing_buy_stop: BTreeMap::new(),
            trailing_sell_stop: BTreeMap::new(),
            last_bid_price: 0,
            last_ask_price: u64::MAX,
            matching_bid_price: 0,
            matching_ask_price: u64::MAX,
            _trailing_bid_price: 0,
            _trailing_ask_price: u64::MAX,
        }
    }

    // -- Public getters --------------------------------------------------------

    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    pub fn size(&self) -> usize {
        self.bids.len()
            + self.asks.len()
            + self.buy_stop.len()
            + self.sell_stop.len()
            + self.trailing_buy_stop.len()
            + self.trailing_sell_stop.len()
    }

    pub fn best_bid(&self) -> Option<&LevelData> {
        self.bids.values().next_back()
    }
    pub fn best_ask(&self) -> Option<&LevelData> {
        self.asks.values().next()
    }
    pub fn bids(&self) -> &BTreeMap<u64, LevelData> {
        &self.bids
    }
    pub fn asks(&self) -> &BTreeMap<u64, LevelData> {
        &self.asks
    }

    pub fn best_buy_stop(&self) -> Option<&LevelData> {
        self.buy_stop.values().next()
    }
    pub fn best_sell_stop(&self) -> Option<&LevelData> {
        self.sell_stop.values().next_back()
    }
    pub fn buy_stop(&self) -> &BTreeMap<u64, LevelData> {
        &self.buy_stop
    }
    pub fn sell_stop(&self) -> &BTreeMap<u64, LevelData> {
        &self.sell_stop
    }

    pub fn best_trailing_buy_stop(&self) -> Option<&LevelData> {
        self.trailing_buy_stop.values().next()
    }
    pub fn best_trailing_sell_stop(&self) -> Option<&LevelData> {
        self.trailing_sell_stop.values().next_back()
    }
    pub fn trailing_buy_stop(&self) -> &BTreeMap<u64, LevelData> {
        &self.trailing_buy_stop
    }
    pub fn trailing_sell_stop(&self) -> &BTreeMap<u64, LevelData> {
        &self.trailing_sell_stop
    }

    pub fn get_bid(&self, price: u64) -> Option<&LevelData> {
        self.bids.get(&price)
    }
    pub fn get_ask(&self, price: u64) -> Option<&LevelData> {
        self.asks.get(&price)
    }

    // -- Market price helpers --------------------------------------------------

    pub fn get_market_price_bid(&self) -> u64 {
        let best = self.best_bid().map_or(0, |l| l.level.price);
        self.matching_bid_price.max(best)
    }

    pub fn get_market_price_ask(&self) -> u64 {
        let best = self.best_ask().map_or(u64::MAX, |l| l.level.price);
        self.matching_ask_price.min(best)
    }

    pub fn get_market_trailing_stop_price_bid(&self) -> u64 {
        let best = self.best_bid().map_or(0, |l| l.level.price);
        self.last_bid_price.min(best)
    }

    pub fn get_market_trailing_stop_price_ask(&self) -> u64 {
        let best = self.best_ask().map_or(u64::MAX, |l| l.level.price);
        self.last_ask_price.max(best)
    }

    pub fn update_last_price(&mut self, order: &Order, price: u64) {
        if order.is_buy() {
            self.last_bid_price = price;
        } else {
            self.last_ask_price = price;
        }
    }

    pub fn update_matching_price(&mut self, order: &Order, price: u64) {
        if order.is_buy() {
            self.matching_bid_price = price;
        } else {
            self.matching_ask_price = price;
        }
    }

    pub fn reset_matching_price(&mut self) {
        self.matching_bid_price = 0;
        self.matching_ask_price = u64::MAX;
    }

    // -- Trailing stop price calculation ---------------------------------------

    pub fn calculate_trailing_stop_price(&self, order: &Order) -> u64 {
        let market_price = if order.is_buy() {
            self.get_market_trailing_stop_price_ask()
        } else {
            self.get_market_trailing_stop_price_bid()
        };

        // If market price is at sentinel value (no valid price yet), keep original stop price
        if market_price == u64::MAX || market_price == 0 {
            return order.stop_price;
        }

        let mut trailing_distance = order.trailing_distance;
        let mut trailing_step = order.trailing_step;

        // Convert percentage to absolute
        if trailing_distance < 0 {
            trailing_distance = (-(trailing_distance) as u64 * market_price / 10_000) as i64;
            trailing_step = (-(trailing_step) as u64 * market_price / 10_000) as i64;
        }

        let old_price = order.stop_price;

        if order.is_buy() {
            let new_price = market_price.saturating_add(trailing_distance as u64);
            if new_price < old_price && (old_price - new_price) >= trailing_step as u64 {
                return new_price;
            }
        } else {
            let new_price = market_price.saturating_sub(trailing_distance as u64);
            if new_price > old_price && (new_price - old_price) >= trailing_step as u64 {
                return new_price;
            }
        }

        old_price
    }

    // -- Limit order operations -----------------------------------------------

    /// Compute `top` flag for a given price and side (immutable access only).
    fn is_top(&self, order: &Order) -> bool {
        if order.is_buy() {
            self.bids
                .values()
                .next_back()
                .is_some_and(|l| l.level.price == order.price)
        } else {
            self.asks
                .values()
                .next()
                .is_some_and(|l| l.level.price == order.price)
        }
    }

    /// Add a limit order to the book. Returns the level update notification.
    pub fn add_order(&mut self, order: &Order) -> LevelUpdate {
        let top = self.is_top(order);
        let levels = if order.is_buy() {
            &mut self.bids
        } else {
            &mut self.asks
        };
        let level_type = if order.is_buy() {
            LevelType::Bid
        } else {
            LevelType::Ask
        };

        let is_new = !levels.contains_key(&order.price);
        let level = levels
            .entry(order.price)
            .or_insert_with(|| LevelData::new(level_type, order.price));

        level.level.total_volume += order.leaves_quantity;
        level.level.hidden_volume += order.hidden_quantity();
        level.level.visible_volume += order.visible_quantity();
        level.order_queue.push_back(order.id);
        level.level.orders += 1;

        let update_type = if is_new {
            UpdateType::Add
        } else {
            UpdateType::Update
        };
        LevelUpdate::new(update_type, level.level.clone(), top)
    }

    /// Reduce a limit order by `quantity`. Returns the level update notification.
    pub fn reduce_order(
        &mut self,
        order: &Order,
        quantity: u64,
        hidden: u64,
        visible: u64,
    ) -> LevelUpdate {
        let top = self.is_top(order);
        let levels = if order.is_buy() {
            &mut self.bids
        } else {
            &mut self.asks
        };

        let mut delete_level = false;
        let mut level_snapshot = None;

        if let Some(level) = levels.get_mut(&order.price) {
            level.level.total_volume -= quantity;
            level.level.hidden_volume -= hidden;
            level.level.visible_volume -= visible;

            if order.leaves_quantity == 0 {
                level.order_queue.retain(|&id| id != order.id);
                level.level.orders -= 1;
            }

            level_snapshot = Some(level.level.clone());

            if level.level.total_volume == 0 {
                delete_level = true;
            }
        }

        let snapshot = level_snapshot.unwrap_or_else(|| Level::new(LevelType::Bid, order.price));

        if delete_level {
            levels.remove(&order.price);
            LevelUpdate::new(UpdateType::Delete, snapshot, top)
        } else {
            LevelUpdate::new(UpdateType::Update, snapshot, top)
        }
    }

    /// Delete a limit order entirely. Returns the level update notification.
    pub fn delete_order(&mut self, order: &Order) -> LevelUpdate {
        let top = self.is_top(order);
        let levels = if order.is_buy() {
            &mut self.bids
        } else {
            &mut self.asks
        };

        let mut delete_level = false;
        let mut level_snapshot = None;

        if let Some(level) = levels.get_mut(&order.price) {
            level.level.total_volume -= order.leaves_quantity;
            level.level.hidden_volume -= order.hidden_quantity();
            level.level.visible_volume -= order.visible_quantity();
            level.order_queue.retain(|&id| id != order.id);
            level.level.orders -= 1;

            level_snapshot = Some(level.level.clone());

            if level.level.total_volume == 0 {
                delete_level = true;
            }
        }

        let snapshot = level_snapshot.unwrap_or_else(|| Level::new(LevelType::Bid, order.price));

        if delete_level {
            levels.remove(&order.price);
            LevelUpdate::new(UpdateType::Delete, snapshot, top)
        } else {
            LevelUpdate::new(UpdateType::Update, snapshot, top)
        }
    }

    // -- Stop order operations -------------------------------------------------

    pub fn add_stop_order(&mut self, order: &Order) {
        let (levels, level_type) = if order.is_buy() {
            (&mut self.buy_stop, LevelType::Ask)
        } else {
            (&mut self.sell_stop, LevelType::Bid)
        };
        let level = levels
            .entry(order.stop_price)
            .or_insert_with(|| LevelData::new(level_type, order.stop_price));
        level.level.total_volume += order.leaves_quantity;
        level.level.hidden_volume += order.hidden_quantity();
        level.level.visible_volume += order.visible_quantity();
        level.order_queue.push_back(order.id);
        level.level.orders += 1;
    }

    pub fn reduce_stop_order(&mut self, order: &Order, quantity: u64, hidden: u64, visible: u64) {
        let levels = if order.is_buy() {
            &mut self.buy_stop
        } else {
            &mut self.sell_stop
        };
        let mut remove = false;
        if let Some(level) = levels.get_mut(&order.stop_price) {
            level.level.total_volume -= quantity;
            level.level.hidden_volume -= hidden;
            level.level.visible_volume -= visible;
            if order.leaves_quantity == 0 {
                level.order_queue.retain(|&id| id != order.id);
                level.level.orders -= 1;
            }
            if level.level.total_volume == 0 {
                remove = true;
            }
        }
        if remove {
            levels.remove(&order.stop_price);
        }
    }

    pub fn delete_stop_order(&mut self, order: &Order) {
        let levels = if order.is_buy() {
            &mut self.buy_stop
        } else {
            &mut self.sell_stop
        };
        let mut remove = false;
        if let Some(level) = levels.get_mut(&order.stop_price) {
            level.level.total_volume -= order.leaves_quantity;
            level.level.hidden_volume -= order.hidden_quantity();
            level.level.visible_volume -= order.visible_quantity();
            level.order_queue.retain(|&id| id != order.id);
            level.level.orders -= 1;
            if level.level.total_volume == 0 {
                remove = true;
            }
        }
        if remove {
            levels.remove(&order.stop_price);
        }
    }

    // -- Trailing stop order operations ----------------------------------------

    pub fn add_trailing_stop_order(&mut self, order: &Order) {
        let (levels, level_type) = if order.is_buy() {
            (&mut self.trailing_buy_stop, LevelType::Ask)
        } else {
            (&mut self.trailing_sell_stop, LevelType::Bid)
        };
        let level = levels
            .entry(order.stop_price)
            .or_insert_with(|| LevelData::new(level_type, order.stop_price));
        level.level.total_volume += order.leaves_quantity;
        level.level.hidden_volume += order.hidden_quantity();
        level.level.visible_volume += order.visible_quantity();
        level.order_queue.push_back(order.id);
        level.level.orders += 1;
    }

    pub fn reduce_trailing_stop_order(
        &mut self,
        order: &Order,
        quantity: u64,
        hidden: u64,
        visible: u64,
    ) {
        let levels = if order.is_buy() {
            &mut self.trailing_buy_stop
        } else {
            &mut self.trailing_sell_stop
        };
        let mut remove = false;
        if let Some(level) = levels.get_mut(&order.stop_price) {
            level.level.total_volume -= quantity;
            level.level.hidden_volume -= hidden;
            level.level.visible_volume -= visible;
            if order.leaves_quantity == 0 {
                level.order_queue.retain(|&id| id != order.id);
                level.level.orders -= 1;
            }
            if level.level.total_volume == 0 {
                remove = true;
            }
        }
        if remove {
            levels.remove(&order.stop_price);
        }
    }

    pub fn delete_trailing_stop_order(&mut self, order: &Order) {
        let levels = if order.is_buy() {
            &mut self.trailing_buy_stop
        } else {
            &mut self.trailing_sell_stop
        };
        let mut remove = false;
        if let Some(level) = levels.get_mut(&order.stop_price) {
            level.level.total_volume -= order.leaves_quantity;
            level.level.hidden_volume -= order.hidden_quantity();
            level.level.visible_volume -= order.visible_quantity();
            level.order_queue.retain(|&id| id != order.id);
            level.level.orders -= 1;
            if level.level.total_volume == 0 {
                remove = true;
            }
        }
        if remove {
            levels.remove(&order.stop_price);
        }
    }
}

impl std::fmt::Display for OrderBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "OrderBook({} bids={} asks={})",
            self.symbol,
            self.bids.len(),
            self.asks.len(),
        )
    }
}
