pub mod matching;

// Re-export the most commonly used types at crate root.
pub use matching::error::{ErrorCode, Result};
pub use matching::level::{Level, LevelUpdate};
pub use matching::market_handler::MarketHandler;
pub use matching::market_manager::MarketManager;
pub use matching::order::{Order, OrderId};
pub use matching::order_book::OrderBook;
pub use matching::symbol::Symbol;
pub use matching::types::*;
