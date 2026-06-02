// ---------------------------------------------------------------------------
//  CppTrader Rust Port — Order
//  Mirrors: include/trader/matching/order.h + order.inl
// ---------------------------------------------------------------------------

use crate::matching::error::ErrorCode;
use crate::matching::types::{OrderSide, OrderTimeInForce, OrderType};

/// Order identifier (u64).
pub type OrderId = u64;

/// An order to buy or sell.
#[derive(Debug, Clone)]
pub struct Order {
    pub id: OrderId,
    pub symbol_id: u32,
    pub order_type: OrderType,
    pub side: OrderSide,
    pub price: u64,
    pub stop_price: u64,
    pub quantity: u64,
    pub executed_quantity: u64,
    pub leaves_quantity: u64,
    pub time_in_force: OrderTimeInForce,
    pub max_visible_quantity: u64,
    pub slippage: u64,
    pub trailing_distance: i64,
    pub trailing_step: i64,
}

// -- Computed properties -------------------------------------------------------

impl Order {
    /// Hidden (non-visible) portion of the remaining quantity.
    #[inline]
    pub fn hidden_quantity(&self) -> u64 {
        self.leaves_quantity.saturating_sub(self.max_visible_quantity)
    }

    /// Visible portion of the remaining quantity.
    #[inline]
    pub fn visible_quantity(&self) -> u64 {
        self.leaves_quantity.min(self.max_visible_quantity)
    }
}

// -- Type predicates -----------------------------------------------------------

impl Order {
    #[inline] pub fn is_market(&self) -> bool { self.order_type == OrderType::Market }
    #[inline] pub fn is_limit(&self) -> bool { self.order_type == OrderType::Limit }
    #[inline] pub fn is_stop(&self) -> bool { self.order_type == OrderType::Stop }
    #[inline] pub fn is_stop_limit(&self) -> bool { self.order_type == OrderType::StopLimit }
    #[inline] pub fn is_trailing_stop(&self) -> bool { self.order_type == OrderType::TrailingStop }
    #[inline] pub fn is_trailing_stop_limit(&self) -> bool { self.order_type == OrderType::TrailingStopLimit }
    #[inline] pub fn is_buy(&self) -> bool { self.side == OrderSide::Buy }
    #[inline] pub fn is_sell(&self) -> bool { self.side == OrderSide::Sell }
    #[inline] pub fn is_gtc(&self) -> bool { self.time_in_force == OrderTimeInForce::Gtc }
    #[inline] pub fn is_ioc(&self) -> bool { self.time_in_force == OrderTimeInForce::Ioc }
    #[inline] pub fn is_fok(&self) -> bool { self.time_in_force == OrderTimeInForce::Fok }
    #[inline] pub fn is_aon(&self) -> bool { self.time_in_force == OrderTimeInForce::Aon }
    #[inline] pub fn is_hidden(&self) -> bool { self.max_visible_quantity == 0 }
    #[inline] pub fn is_iceberg(&self) -> bool { self.max_visible_quantity < u64::MAX }
    #[inline] pub fn is_slippage(&self) -> bool { self.slippage < u64::MAX }
}

// -- Validation ----------------------------------------------------------------

impl Order {
    /// Validate order parameters. Returns `ErrorCode::Ok` on success.
    pub fn validate(&self) -> ErrorCode {
        if self.id == 0 {
            return ErrorCode::OrderIdInvalid;
        }
        if self.quantity < self.leaves_quantity {
            return ErrorCode::OrderQuantityInvalid;
        }
        if self.leaves_quantity == 0 {
            return ErrorCode::OrderQuantityInvalid;
        }
        match self.order_type {
            OrderType::Market => {
                if self.time_in_force != OrderTimeInForce::Ioc
                    && self.time_in_force != OrderTimeInForce::Fok
                {
                    return ErrorCode::OrderParameterInvalid;
                }
                if self.is_iceberg() {
                    return ErrorCode::OrderParameterInvalid;
                }
            }
            OrderType::Limit => {
                if self.is_slippage() {
                    return ErrorCode::OrderParameterInvalid;
                }
            }
            OrderType::Stop | OrderType::TrailingStop => {
                if self.is_aon() {
                    return ErrorCode::OrderParameterInvalid;
                }
                if self.is_iceberg() {
                    return ErrorCode::OrderParameterInvalid;
                }
            }
            OrderType::StopLimit | OrderType::TrailingStopLimit => {
                if self.is_slippage() {
                    return ErrorCode::OrderParameterInvalid;
                }
            }
        }
        if self.is_trailing_stop() || self.is_trailing_stop_limit() {
            if self.trailing_distance == 0 {
                return ErrorCode::OrderParameterInvalid;
            }
            if self.trailing_distance > 0 {
                if self.trailing_step < 0 || self.trailing_step >= self.trailing_distance {
                    return ErrorCode::OrderParameterInvalid;
                }
            } else {
                // negative = percentage, -10000..=-1  (0.01%..100%)
                if !(-10_000..=-1).contains(&self.trailing_distance) {
                    return ErrorCode::OrderParameterInvalid;
                }
                if self.trailing_step > 0 || self.trailing_step < self.trailing_distance {
                    return ErrorCode::OrderParameterInvalid;
                }
            }
        }
        ErrorCode::Ok
    }
}

// -- Factory methods -----------------------------------------------------------

// Sentinel for "not set"
const NONE: u64 = u64::MAX;

impl Order {
    // ---- Market orders ----

    pub fn market(id: OrderId, symbol_id: u32, side: OrderSide, quantity: u64, slippage: u64) -> Self {
        Self {
            id, symbol_id, order_type: OrderType::Market, side,
            price: 0, stop_price: 0, quantity,
            executed_quantity: 0, leaves_quantity: quantity,
            time_in_force: OrderTimeInForce::Ioc,
            max_visible_quantity: NONE, slippage,
            trailing_distance: 0, trailing_step: 0,
        }
    }

    pub fn buy_market(id: OrderId, symbol_id: u32, quantity: u64, slippage: u64) -> Self {
        Self::market(id, symbol_id, OrderSide::Buy, quantity, slippage)
    }

    pub fn sell_market(id: OrderId, symbol_id: u32, quantity: u64, slippage: u64) -> Self {
        Self::market(id, symbol_id, OrderSide::Sell, quantity, slippage)
    }

    // ---- Limit orders ----

    pub fn limit(
        id: OrderId, symbol_id: u32, side: OrderSide,
        price: u64, quantity: u64,
        tif: OrderTimeInForce, max_visible_quantity: u64,
    ) -> Self {
        Self {
            id, symbol_id, order_type: OrderType::Limit, side,
            price, stop_price: 0, quantity,
            executed_quantity: 0, leaves_quantity: quantity,
            time_in_force: tif,
            max_visible_quantity, slippage: NONE,
            trailing_distance: 0, trailing_step: 0,
        }
    }

    pub fn buy_limit(
        id: OrderId, symbol_id: u32, price: u64, quantity: u64,
        tif: OrderTimeInForce, max_visible_quantity: u64,
    ) -> Self {
        Self::limit(id, symbol_id, OrderSide::Buy, price, quantity, tif, max_visible_quantity)
    }

    pub fn sell_limit(
        id: OrderId, symbol_id: u32, price: u64, quantity: u64,
        tif: OrderTimeInForce, max_visible_quantity: u64,
    ) -> Self {
        Self::limit(id, symbol_id, OrderSide::Sell, price, quantity, tif, max_visible_quantity)
    }

    // ---- Stop orders ----

    pub fn stop(
        id: OrderId, symbol_id: u32, side: OrderSide,
        stop_price: u64, quantity: u64,
        tif: OrderTimeInForce, slippage: u64,
    ) -> Self {
        Self {
            id, symbol_id, order_type: OrderType::Stop, side,
            price: 0, stop_price, quantity,
            executed_quantity: 0, leaves_quantity: quantity,
            time_in_force: tif,
            max_visible_quantity: NONE, slippage,
            trailing_distance: 0, trailing_step: 0,
        }
    }

    pub fn buy_stop(id: OrderId, symbol_id: u32, stop_price: u64, quantity: u64, tif: OrderTimeInForce, slippage: u64) -> Self {
        Self::stop(id, symbol_id, OrderSide::Buy, stop_price, quantity, tif, slippage)
    }

    pub fn sell_stop(id: OrderId, symbol_id: u32, stop_price: u64, quantity: u64, tif: OrderTimeInForce, slippage: u64) -> Self {
        Self::stop(id, symbol_id, OrderSide::Sell, stop_price, quantity, tif, slippage)
    }

    // ---- Stop-limit orders ----

    #[allow(clippy::too_many_arguments)]
    pub fn stop_limit(
        id: OrderId, symbol_id: u32, side: OrderSide,
        stop_price: u64, price: u64, quantity: u64,
        tif: OrderTimeInForce, max_visible_quantity: u64,
    ) -> Self {
        Self {
            id, symbol_id, order_type: OrderType::StopLimit, side,
            price, stop_price, quantity,
            executed_quantity: 0, leaves_quantity: quantity,
            time_in_force: tif,
            max_visible_quantity, slippage: NONE,
            trailing_distance: 0, trailing_step: 0,
        }
    }

    pub fn buy_stop_limit(id: OrderId, symbol_id: u32, stop_price: u64, price: u64, quantity: u64, tif: OrderTimeInForce, max_visible_quantity: u64) -> Self {
        Self::stop_limit(id, symbol_id, OrderSide::Buy, stop_price, price, quantity, tif, max_visible_quantity)
    }

    pub fn sell_stop_limit(id: OrderId, symbol_id: u32, stop_price: u64, price: u64, quantity: u64, tif: OrderTimeInForce, max_visible_quantity: u64) -> Self {
        Self::stop_limit(id, symbol_id, OrderSide::Sell, stop_price, price, quantity, tif, max_visible_quantity)
    }

    // ---- Trailing-stop orders ----

    #[allow(clippy::too_many_arguments)]
    pub fn trailing_stop(
        id: OrderId, symbol_id: u32, side: OrderSide,
        stop_price: u64, quantity: u64,
        trailing_distance: i64, trailing_step: i64,
        tif: OrderTimeInForce, slippage: u64,
    ) -> Self {
        Self {
            id, symbol_id, order_type: OrderType::TrailingStop, side,
            price: 0, stop_price, quantity,
            executed_quantity: 0, leaves_quantity: quantity,
            time_in_force: tif,
            max_visible_quantity: NONE, slippage,
            trailing_distance, trailing_step,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn trailing_buy_stop(id: OrderId, symbol_id: u32, stop_price: u64, quantity: u64, trailing_distance: i64, trailing_step: i64, tif: OrderTimeInForce, slippage: u64) -> Self {
        Self::trailing_stop(id, symbol_id, OrderSide::Buy, stop_price, quantity, trailing_distance, trailing_step, tif, slippage)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn trailing_sell_stop(id: OrderId, symbol_id: u32, stop_price: u64, quantity: u64, trailing_distance: i64, trailing_step: i64, tif: OrderTimeInForce, slippage: u64) -> Self {
        Self::trailing_stop(id, symbol_id, OrderSide::Sell, stop_price, quantity, trailing_distance, trailing_step, tif, slippage)
    }

    // ---- Trailing-stop-limit orders ----

    #[allow(clippy::too_many_arguments)]
    pub fn trailing_stop_limit(
        id: OrderId, symbol_id: u32, side: OrderSide,
        stop_price: u64, price: u64, quantity: u64,
        trailing_distance: i64, trailing_step: i64,
        tif: OrderTimeInForce, max_visible_quantity: u64,
    ) -> Self {
        Self {
            id, symbol_id, order_type: OrderType::TrailingStopLimit, side,
            price, stop_price, quantity,
            executed_quantity: 0, leaves_quantity: quantity,
            time_in_force: tif,
            max_visible_quantity, slippage: NONE,
            trailing_distance, trailing_step,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn trailing_buy_stop_limit(id: OrderId, symbol_id: u32, stop_price: u64, price: u64, quantity: u64, trailing_distance: i64, trailing_step: i64, tif: OrderTimeInForce, max_visible_quantity: u64) -> Self {
        Self::trailing_stop_limit(id, symbol_id, OrderSide::Buy, stop_price, price, quantity, trailing_distance, trailing_step, tif, max_visible_quantity)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn trailing_sell_stop_limit(id: OrderId, symbol_id: u32, stop_price: u64, price: u64, quantity: u64, trailing_distance: i64, trailing_step: i64, tif: OrderTimeInForce, max_visible_quantity: u64) -> Self {
        Self::trailing_stop_limit(id, symbol_id, OrderSide::Sell, stop_price, price, quantity, trailing_distance, trailing_step, tif, max_visible_quantity)
    }
}

// -- Display -------------------------------------------------------------------

impl std::fmt::Display for Order {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Order({} {} {} {} @ {} qty={} exec={} leaves={})",
            self.id, self.side, self.order_type, self.time_in_force,
            self.price, self.quantity, self.executed_quantity, self.leaves_quantity,
        )
    }
}
