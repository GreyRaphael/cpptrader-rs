// ---------------------------------------------------------------------------
//  CppTrader Rust Port — Error codes
//  Mirrors: include/trader/matching/errors.h
// ---------------------------------------------------------------------------

/// Error codes returned by market-manager operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ErrorCode {
    Ok,
    SymbolDuplicate,
    SymbolNotFound,
    OrderBookDuplicate,
    OrderBookNotFound,
    OrderDuplicate,
    OrderNotFound,
    OrderIdInvalid,
    OrderTypeInvalid,
    OrderParameterInvalid,
    OrderQuantityInvalid,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "OK"),
            Self::SymbolDuplicate => write!(f, "Symbol duplicate"),
            Self::SymbolNotFound => write!(f, "Symbol not found"),
            Self::OrderBookDuplicate => write!(f, "Order book duplicate"),
            Self::OrderBookNotFound => write!(f, "Order book not found"),
            Self::OrderDuplicate => write!(f, "Order duplicate"),
            Self::OrderNotFound => write!(f, "Order not found"),
            Self::OrderIdInvalid => write!(f, "Order ID invalid"),
            Self::OrderTypeInvalid => write!(f, "Order type invalid"),
            Self::OrderParameterInvalid => write!(f, "Order parameter invalid"),
            Self::OrderQuantityInvalid => write!(f, "Order quantity invalid"),
        }
    }
}

impl std::error::Error for ErrorCode {}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, ErrorCode>;
