// ---------------------------------------------------------------------------
//  CppTrader Rust Port — Core enumerations
//  Mirrors: include/trader/matching/errors.h, update.h, order.h, level.h
// ---------------------------------------------------------------------------

/// Order side (buy / sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
    TrailingStop,
    TrailingStopLimit,
}

/// Time-in-force policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OrderTimeInForce {
    /// Good-Till-Cancelled
    Gtc,
    /// Immediate-Or-Cancel
    Ioc,
    /// Fill-Or-Kill
    Fok,
    /// All-Or-None
    Aon,
}

/// Price-level side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LevelType {
    Bid,
    Ask,
}

/// Update notification kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum UpdateType {
    None,
    Add,
    Update,
    Delete,
}

// -- Display implementations --------------------------------------------------

impl std::fmt::Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Buy => write!(f, "Buy"),
            Self::Sell => write!(f, "Sell"),
        }
    }
}

impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Market => write!(f, "Market"),
            Self::Limit => write!(f, "Limit"),
            Self::Stop => write!(f, "Stop"),
            Self::StopLimit => write!(f, "StopLimit"),
            Self::TrailingStop => write!(f, "TrailingStop"),
            Self::TrailingStopLimit => write!(f, "TrailingStopLimit"),
        }
    }
}

impl std::fmt::Display for OrderTimeInForce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gtc => write!(f, "GTC"),
            Self::Ioc => write!(f, "IOC"),
            Self::Fok => write!(f, "FOK"),
            Self::Aon => write!(f, "AON"),
        }
    }
}

impl std::fmt::Display for LevelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bid => write!(f, "Bid"),
            Self::Ask => write!(f, "Ask"),
        }
    }
}

impl std::fmt::Display for UpdateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Add => write!(f, "Add"),
            Self::Update => write!(f, "Update"),
            Self::Delete => write!(f, "Delete"),
        }
    }
}
