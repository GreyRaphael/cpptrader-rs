// ---------------------------------------------------------------------------
//  CppTrader Rust Port — Level / LevelUpdate
//  Mirrors: include/trader/matching/level.h
// ---------------------------------------------------------------------------

use crate::matching::types::{LevelType, UpdateType};

/// A price level in the order book.
#[derive(Debug, Clone)]
pub struct Level {
    pub level_type: LevelType,
    pub price: u64,
    pub total_volume: u64,
    pub hidden_volume: u64,
    pub visible_volume: u64,
    pub orders: usize,
}

impl Level {
    pub fn new(level_type: LevelType, price: u64) -> Self {
        Self {
            level_type,
            price,
            total_volume: 0,
            hidden_volume: 0,
            visible_volume: 0,
            orders: 0,
        }
    }

    #[inline]
    pub fn is_bid(&self) -> bool {
        self.level_type == LevelType::Bid
    }
    #[inline]
    pub fn is_ask(&self) -> bool {
        self.level_type == LevelType::Ask
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Level({} {} vol={} orders={})",
            self.level_type, self.price, self.total_volume, self.orders,
        )
    }
}

/// Notification about a price-level change.
#[derive(Debug, Clone)]
pub struct LevelUpdate {
    pub update_type: UpdateType,
    pub level: Level,
    pub top: bool,
}

impl LevelUpdate {
    pub fn new(update_type: UpdateType, level: Level, top: bool) -> Self {
        Self {
            update_type,
            level,
            top,
        }
    }
}
