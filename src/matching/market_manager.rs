// ---------------------------------------------------------------------------
//  CppTrader Rust Port — MarketManager
//  Mirrors: include/trader/matching/market_manager.h + source/.../market_manager.cpp
// ---------------------------------------------------------------------------

use hashbrown::HashMap;

use crate::matching::error::{ErrorCode, Result};
use crate::matching::level::LevelUpdate;
use crate::matching::market_handler::{MarketHandler, NoOpHandler};
use crate::matching::order::{Order, OrderId};
use crate::matching::order_book::OrderBook;
use crate::matching::symbol::Symbol;
use crate::matching::types::{LevelType, OrderType, UpdateType};

// ---------------------------------------------------------------------------
//  OrderSlot — stored in the orders HashMap (replaces C++ OrderNode + pool)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct OrderSlot {
    order: Order,
}

// ---------------------------------------------------------------------------
//  MarketManager
// ---------------------------------------------------------------------------

/// Central manager for symbols, order books, and orders.
///
/// Not thread-safe.
pub struct MarketManager {
    handler: Box<dyn MarketHandler>,

    symbols: Vec<Option<Symbol>>,
    order_books: Vec<Option<OrderBook>>,
    orders: HashMap<OrderId, OrderSlot>,

    matching: bool,
}

impl MarketManager {
    #[inline]
    fn on_add_symbol(&mut self, symbol: &Symbol) {
        self.handler.on_add_symbol(symbol);
    }

    #[inline]
    fn on_delete_symbol(&mut self, symbol: &Symbol) {
        self.handler.on_delete_symbol(symbol);
    }

    #[inline]
    fn on_add_order_book(&mut self, order_book: &OrderBook) {
        self.handler.on_add_order_book(order_book);
    }

    #[inline]
    fn on_delete_order_book(&mut self, order_book: &OrderBook) {
        self.handler.on_delete_order_book(order_book);
    }

    #[inline]
    fn on_add_order(&mut self, order: &Order) {
        self.handler.on_add_order(order);
    }

    #[inline]
    fn on_update_order(&mut self, order: &Order) {
        self.handler.on_update_order(order);
    }

    #[inline]
    fn on_delete_order(&mut self, order: &Order) {
        self.handler.on_delete_order(order);
    }

    #[inline]
    fn on_execute_order(&mut self, order: &Order, price: u64, quantity: u64) {
        self.handler.on_execute_order(order, price, quantity);
    }

    /// Create with a custom event handler.
    pub fn new(handler: Box<dyn MarketHandler>) -> Self {
        Self {
            handler,
            symbols: Vec::new(),
            order_books: Vec::new(),
            orders: HashMap::new(),
            matching: false,
        }
    }

    /// Create with the default no-op handler.
    pub fn with_default_handler() -> Self {
        Self::new(Box::new(NoOpHandler))
    }

    // -- Query -----------------------------------------------------------------

    pub fn get_symbol(&self, id: u32) -> Option<&Symbol> {
        self.symbols.get(id as usize).and_then(|s| s.as_ref())
    }

    pub fn get_order_book(&self, id: u32) -> Option<&OrderBook> {
        self.order_books.get(id as usize).and_then(|ob| ob.as_ref())
    }

    pub fn get_order(&self, id: OrderId) -> Option<&Order> {
        self.orders.get(&id).map(|s| &s.order)
    }

    /// Iterate over all active orders.
    pub fn iter_orders(&self) -> impl Iterator<Item = (OrderId, &Order)> {
        self.orders.iter().map(|(&id, slot)| (id, &slot.order))
    }

    /// Number of active orders.
    pub fn order_count(&self) -> usize {
        self.orders.len()
    }

    // -- Symbol management -----------------------------------------------------

    pub fn add_symbol(&mut self, symbol: Symbol) -> Result<()> {
        let idx = symbol.id as usize;
        if self.symbols.len() <= idx {
            self.symbols.resize_with(idx + 1, || None);
        }
        if self.symbols[idx].is_some() {
            return Err(ErrorCode::SymbolDuplicate);
        }
        self.on_add_symbol(&symbol);
        self.symbols[idx] = Some(symbol);
        Ok(())
    }

    pub fn delete_symbol(&mut self, id: u32) -> Result<()> {
        let idx = id as usize;
        let symbol = self.symbols.get(idx).and_then(|s| *s);
        let symbol = symbol.ok_or(ErrorCode::SymbolNotFound)?;
        self.on_delete_symbol(&symbol);
        self.symbols[idx] = None;
        Ok(())
    }

    // -- Order-book management -------------------------------------------------

    pub fn add_order_book(&mut self, symbol: &Symbol) -> Result<()> {
        let idx = symbol.id as usize;
        if self.symbols.get(idx).and_then(|s| *s).is_none() {
            return Err(ErrorCode::SymbolNotFound);
        }
        if self.order_books.len() <= idx {
            self.order_books.resize_with(idx + 1, || None);
        }
        if self.order_books[idx].is_some() {
            return Err(ErrorCode::OrderBookDuplicate);
        }
        let ob = OrderBook::new(*symbol);
        self.on_add_order_book(&ob);
        self.order_books[idx] = Some(ob);
        Ok(())
    }

    pub fn delete_order_book(&mut self, id: u32) -> Result<()> {
        let idx = id as usize;
        if self
            .order_books
            .get(idx)
            .and_then(|ob| ob.as_ref())
            .is_none()
        {
            return Err(ErrorCode::OrderBookNotFound);
        }
        // Borrow-checker-friendly: take ownership, notify, drop.
        let ob = self.order_books[idx].take().unwrap();
        self.on_delete_order_book(&ob);
        drop(ob);
        Ok(())
    }

    // -- Matching control ------------------------------------------------------

    pub fn is_matching_enabled(&self) -> bool {
        self.matching
    }

    pub fn enable_matching(&mut self) {
        self.matching = true;
        self.match_all();
    }

    pub fn disable_matching(&mut self) {
        self.matching = false;
    }

    pub fn match_all(&mut self) {
        // Collect non-null order-book indices first to satisfy the borrow checker.
        let ids: Vec<u32> = self
            .order_books
            .iter()
            .enumerate()
            .filter_map(|(i, ob)| ob.as_ref().map(|_| i as u32))
            .collect();
        for id in ids {
            self.match_book(id);
        }
    }

    // -- Order lifecycle -------------------------------------------------------

    pub fn add_order(&mut self, order: Order) -> Result<()> {
        let err = order.validate();
        if err != ErrorCode::Ok {
            return Err(err);
        }
        match order.order_type {
            OrderType::Market => self.add_market_order(order, false),
            OrderType::Limit => self.add_limit_order(order, false),
            OrderType::Stop | OrderType::TrailingStop => self.add_stop_order(order, false),
            OrderType::StopLimit | OrderType::TrailingStopLimit => {
                self.add_stop_limit_order(order, false)
            }
        }
    }

    pub fn reduce_order(&mut self, id: OrderId, quantity: u64) -> Result<()> {
        self.reduce_order_impl(id, quantity, false)
    }

    pub fn modify_order(&mut self, id: OrderId, new_price: u64, new_quantity: u64) -> Result<()> {
        self.modify_order_impl(id, new_price, new_quantity, false, false)
    }

    pub fn mitigate_order(&mut self, id: OrderId, new_price: u64, new_quantity: u64) -> Result<()> {
        self.modify_order_impl(id, new_price, new_quantity, true, false)
    }

    pub fn replace_order(
        &mut self,
        id: OrderId,
        new_id: OrderId,
        new_price: u64,
        new_quantity: u64,
    ) -> Result<()> {
        self.replace_order_impl(id, new_id, new_price, new_quantity, false)
    }

    pub fn replace_order_with(&mut self, id: OrderId, new_order: Order) -> Result<()> {
        if id == 0 {
            return Err(ErrorCode::OrderIdInvalid);
        }
        if !self.orders.contains_key(&id) {
            return Err(ErrorCode::OrderNotFound);
        }

        let err = new_order.validate();
        if err != ErrorCode::Ok {
            return Err(err);
        }
        if new_order.id != id && self.orders.contains_key(&new_order.id) {
            return Err(ErrorCode::OrderDuplicate);
        }

        let idx = new_order.symbol_id as usize;
        if self.symbols.get(idx).and_then(|s| s.as_ref()).is_none() {
            return Err(ErrorCode::SymbolNotFound);
        }
        if self
            .order_books
            .get(idx)
            .and_then(|ob| ob.as_ref())
            .is_none()
        {
            return Err(ErrorCode::OrderBookNotFound);
        }

        let old_order = self.orders[&id].order;
        self.delete_order_impl(id, true)?;

        match self.add_order(new_order) {
            Ok(()) => Ok(()),
            Err(err) => {
                let _ = self.add_order(old_order);
                Err(err)
            }
        }
    }

    pub fn delete_order(&mut self, id: OrderId) -> Result<()> {
        self.delete_order_impl(id, false)
    }

    pub fn execute_order(&mut self, id: OrderId, quantity: u64) -> Result<()> {
        if id == 0 {
            return Err(ErrorCode::OrderIdInvalid);
        }
        if quantity == 0 {
            return Err(ErrorCode::OrderQuantityInvalid);
        }

        let slot = self.orders.get_mut(&id).ok_or(ErrorCode::OrderNotFound)?;
        let symbol_id = slot.order.symbol_id;
        let price = slot.order.price;

        self.do_execute(symbol_id, id, price, quantity)?;

        // match_book and reset_matching_price are handled inside do_execute.
        Ok(())
    }

    pub fn execute_order_at(&mut self, id: OrderId, price: u64, quantity: u64) -> Result<()> {
        if id == 0 {
            return Err(ErrorCode::OrderIdInvalid);
        }
        if quantity == 0 {
            return Err(ErrorCode::OrderQuantityInvalid);
        }

        let symbol_id = self
            .orders
            .get(&id)
            .ok_or(ErrorCode::OrderNotFound)?
            .order
            .symbol_id;
        self.do_execute(symbol_id, id, price, quantity)?;

        // match_book and reset_matching_price are handled inside do_execute.
        Ok(())
    }

    // ===========================================================================
    //  Private helpers
    // ===========================================================================

    /// Execute `quantity` of order `id` at `price`. Removes the order from
    /// the book and fires handler callbacks.
    fn do_execute(
        &mut self,
        symbol_id: u32,
        id: OrderId,
        price: u64,
        mut quantity: u64,
    ) -> Result<()> {
        let order_before_execute = self.orders.get(&id).ok_or(ErrorCode::OrderNotFound)?.order;
        quantity = quantity.min(order_before_execute.leaves_quantity);

        // Notify handler
        self.on_execute_order(&order_before_execute, price, quantity);

        // Update market prices
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            ob.update_last_price(&order_before_execute, price);
            ob.update_matching_price(&order_before_execute, price);
        }

        // Update order quantities
        let hidden_before = order_before_execute.hidden_quantity();
        let visible_before = order_before_execute.visible_quantity();
        let slot = self.orders.get_mut(&id).ok_or(ErrorCode::OrderNotFound)?;
        slot.order.executed_quantity += quantity;
        slot.order.leaves_quantity -= quantity;
        let hidden_after = slot.order.hidden_quantity();
        let visible_after = slot.order.visible_quantity();
        let hidden_delta = hidden_before - hidden_after;
        let visible_delta = visible_before - visible_after;

        // Update order book level
        let order = slot.order;
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            match order.order_type {
                OrderType::Limit => {
                    ob.reduce_order(&order, quantity, hidden_delta, visible_delta);
                }
                OrderType::Stop | OrderType::StopLimit => {
                    ob.reduce_stop_order(&order, quantity, hidden_delta, visible_delta);
                }
                OrderType::TrailingStop | OrderType::TrailingStopLimit => {
                    ob.reduce_trailing_stop_order(&order, quantity, hidden_delta, visible_delta);
                }
                _ => {}
            }
        }

        if order.leaves_quantity == 0 {
            self.on_delete_order(&order);
            self.orders.remove(&id);
        } else {
            self.on_update_order(&order);
        }

        if self.matching {
            self.match_book(symbol_id);
        }
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            ob.reset_matching_price();
        }
        Ok(())
    }

    // -- Add order variants ----------------------------------------------------

    fn add_market_order(&mut self, mut order: Order, recursive: bool) -> Result<()> {
        let symbol_id = order.symbol_id;
        self.on_add_order(&order);

        if self.matching && !recursive {
            self.match_market(symbol_id, &mut order);
        }

        self.on_delete_order(&order);

        if self.matching && !recursive {
            self.match_book(symbol_id);
        }
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            ob.reset_matching_price();
        }
        Ok(())
    }

    fn add_limit_order(&mut self, mut order: Order, recursive: bool) -> Result<()> {
        let symbol_id = order.symbol_id;
        self.on_add_order(&order);

        if self.matching && !recursive {
            self.match_limit(symbol_id, &mut order);
        }

        if order.leaves_quantity > 0 && !order.is_ioc() && !order.is_fok() {
            if self.orders.contains_key(&order.id) {
                self.on_delete_order(&order);
                return Err(ErrorCode::OrderDuplicate);
            }
            let order_clone = order;
            self.orders.insert(order.id, OrderSlot { order });

            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                let update = ob.add_order(&order_clone);
                self.update_level(symbol_id, &update);
            }
        } else {
            self.on_delete_order(&order);
        }

        if self.matching && !recursive {
            self.match_book(symbol_id);
        }
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            ob.reset_matching_price();
        }
        Ok(())
    }

    fn add_stop_order(&mut self, mut order: Order, recursive: bool) -> Result<()> {
        let symbol_id = order.symbol_id;

        // Recalculate trailing stop price
        if (order.is_trailing_stop() || order.is_trailing_stop_limit())
            && let Some(ob) = self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
        {
            order.stop_price = ob.calculate_trailing_stop_price(&order);
        }

        self.on_add_order(&order);

        if self.matching && !recursive {
            let stop_price = if order.is_buy() {
                self.order_books
                    .get(symbol_id as usize)
                    .and_then(|o| o.as_ref())
                    .map_or(u64::MAX, |ob| ob.get_market_price_ask())
            } else {
                self.order_books
                    .get(symbol_id as usize)
                    .and_then(|o| o.as_ref())
                    .map_or(0, |ob| ob.get_market_price_bid())
            };
            let arbitrage = if order.is_buy() {
                order.stop_price <= stop_price
            } else {
                order.stop_price >= stop_price
            };
            if arbitrage {
                order.order_type = OrderType::Market;
                order.price = 0;
                order.stop_price = 0;
                order.time_in_force = if order.is_fok() {
                    crate::matching::types::OrderTimeInForce::Fok
                } else {
                    crate::matching::types::OrderTimeInForce::Ioc
                };
                self.on_update_order(&order);
                self.match_market(symbol_id, &mut order);
                self.on_delete_order(&order);
                if self.matching {
                    self.match_book(symbol_id);
                }
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    ob.reset_matching_price();
                }
                return Ok(());
            }
        }

        if order.leaves_quantity > 0 {
            if self.orders.contains_key(&order.id) {
                self.on_delete_order(&order);
                return Err(ErrorCode::OrderDuplicate);
            }
            let order_clone = order;
            self.orders
                .insert(order.id, OrderSlot { order: order_clone });

            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                if order.is_trailing_stop() || order.is_trailing_stop_limit() {
                    ob.add_trailing_stop_order(&order);
                } else {
                    ob.add_stop_order(&order);
                }
            }
        } else {
            self.on_delete_order(&order);
        }

        if self.matching && !recursive {
            self.match_book(symbol_id);
        }
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            ob.reset_matching_price();
        }
        Ok(())
    }

    fn add_stop_limit_order(&mut self, mut order: Order, recursive: bool) -> Result<()> {
        let symbol_id = order.symbol_id;

        if (order.is_trailing_stop() || order.is_trailing_stop_limit())
            && let Some(ob) = self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
        {
            let diff = order.price as i64 - order.stop_price as i64;
            order.stop_price = ob.calculate_trailing_stop_price(&order);
            order.price = (order.stop_price as i64 + diff) as u64;
        }

        self.on_add_order(&order);

        if self.matching && !recursive {
            let stop_price = if order.is_buy() {
                self.order_books
                    .get(symbol_id as usize)
                    .and_then(|o| o.as_ref())
                    .map_or(u64::MAX, |ob| ob.get_market_price_ask())
            } else {
                self.order_books
                    .get(symbol_id as usize)
                    .and_then(|o| o.as_ref())
                    .map_or(0, |ob| ob.get_market_price_bid())
            };
            let arbitrage = if order.is_buy() {
                order.stop_price <= stop_price
            } else {
                order.stop_price >= stop_price
            };
            if arbitrage {
                order.order_type = OrderType::Limit;
                order.stop_price = 0;
                self.on_update_order(&order);
                self.match_limit(symbol_id, &mut order);

                if order.leaves_quantity > 0 && !order.is_ioc() && !order.is_fok() {
                    if self.orders.contains_key(&order.id) {
                        self.on_delete_order(&order);
                        return Err(ErrorCode::OrderDuplicate);
                    }
                    let oc = order;
                    self.orders.insert(order.id, OrderSlot { order });
                    if let Some(ob) = self
                        .order_books
                        .get_mut(symbol_id as usize)
                        .and_then(|o| o.as_mut())
                    {
                        let update = ob.add_order(&oc);
                        self.update_level(symbol_id, &update);
                    }
                } else {
                    self.on_delete_order(&order);
                }

                if self.matching {
                    self.match_book(symbol_id);
                }
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    ob.reset_matching_price();
                }
                return Ok(());
            }
        }

        if order.leaves_quantity > 0 {
            if self.orders.contains_key(&order.id) {
                self.on_delete_order(&order);
                return Err(ErrorCode::OrderDuplicate);
            }
            let order_clone = order;
            self.orders
                .insert(order.id, OrderSlot { order: order_clone });

            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                if order.is_trailing_stop() || order.is_trailing_stop_limit() {
                    ob.add_trailing_stop_order(&order);
                } else {
                    ob.add_stop_order(&order);
                }
            }
        } else {
            self.on_delete_order(&order);
        }

        if self.matching && !recursive {
            self.match_book(symbol_id);
        }
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            ob.reset_matching_price();
        }
        Ok(())
    }

    // -- Reduce / Modify / Replace / Delete ------------------------------------

    fn reduce_order_impl(&mut self, id: OrderId, quantity: u64, recursive: bool) -> Result<()> {
        if id == 0 {
            return Err(ErrorCode::OrderIdInvalid);
        }
        if quantity == 0 {
            return Err(ErrorCode::OrderQuantityInvalid);
        }

        let symbol_id = self
            .orders
            .get(&id)
            .ok_or(ErrorCode::OrderNotFound)?
            .order
            .symbol_id;
        let quantity = quantity.min(self.orders[&id].order.leaves_quantity);

        let hidden_before = self.orders[&id].order.hidden_quantity();
        let visible_before = self.orders[&id].order.visible_quantity();

        {
            let slot = self.orders.get_mut(&id).unwrap();
            slot.order.leaves_quantity -= quantity;
        }

        let hidden_after = self.orders[&id].order.hidden_quantity();
        let visible_after = self.orders[&id].order.visible_quantity();
        let hidden_delta = hidden_before - hidden_after;
        let visible_delta = visible_before - visible_after;

        let order = self.orders[&id].order;

        if order.leaves_quantity > 0 {
            self.on_update_order(&order);
            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                match order.order_type {
                    OrderType::Limit => {
                        ob.reduce_order(&order, quantity, hidden_delta, visible_delta);
                    }
                    OrderType::Stop | OrderType::StopLimit => {
                        ob.reduce_stop_order(&order, quantity, hidden_delta, visible_delta);
                    }
                    OrderType::TrailingStop | OrderType::TrailingStopLimit => {
                        ob.reduce_trailing_stop_order(
                            &order,
                            quantity,
                            hidden_delta,
                            visible_delta,
                        );
                    }
                    _ => {}
                }
            }
        } else {
            self.on_delete_order(&order);
            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                match order.order_type {
                    OrderType::Limit => {
                        ob.reduce_order(&order, quantity, hidden_delta, visible_delta);
                    }
                    OrderType::Stop | OrderType::StopLimit => {
                        ob.reduce_stop_order(&order, quantity, hidden_delta, visible_delta);
                    }
                    OrderType::TrailingStop | OrderType::TrailingStopLimit => {
                        ob.reduce_trailing_stop_order(
                            &order,
                            quantity,
                            hidden_delta,
                            visible_delta,
                        );
                    }
                    _ => {}
                }
            }
            self.orders.remove(&id);
        }

        if self.matching && !recursive {
            self.match_book(symbol_id);
        }
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            ob.reset_matching_price();
        }
        Ok(())
    }

    fn modify_order_impl(
        &mut self,
        id: OrderId,
        new_price: u64,
        new_quantity: u64,
        mitigate: bool,
        recursive: bool,
    ) -> Result<()> {
        if id == 0 {
            return Err(ErrorCode::OrderIdInvalid);
        }
        if new_quantity == 0 {
            return Err(ErrorCode::OrderQuantityInvalid);
        }

        let symbol_id = self
            .orders
            .get(&id)
            .ok_or(ErrorCode::OrderNotFound)?
            .order
            .symbol_id;

        // Delete from book
        {
            let order = self.orders[&id].order;
            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                match order.order_type {
                    OrderType::Limit => {
                        ob.delete_order(&order);
                    }
                    OrderType::Stop | OrderType::StopLimit => {
                        ob.delete_stop_order(&order);
                    }
                    OrderType::TrailingStop | OrderType::TrailingStopLimit => {
                        ob.delete_trailing_stop_order(&order);
                    }
                    _ => {}
                }
            }
        }

        // Modify
        {
            let slot = self.orders.get_mut(&id).unwrap();
            slot.order.price = new_price;
            slot.order.quantity = new_quantity;
            slot.order.leaves_quantity = new_quantity;
            if mitigate {
                if new_quantity > slot.order.executed_quantity {
                    slot.order.leaves_quantity = new_quantity - slot.order.executed_quantity;
                } else {
                    slot.order.leaves_quantity = 0;
                }
            }
        }

        let mut order = self.orders[&id].order;

        if order.leaves_quantity > 0 {
            self.on_update_order(&order);

            if self.matching && !recursive {
                self.match_limit(symbol_id, &mut order);
                // Update the stored order after matching
                if let Some(slot) = self.orders.get_mut(&id) {
                    slot.order.leaves_quantity = order.leaves_quantity;
                    slot.order.executed_quantity = order.executed_quantity;
                }
            }

            if self
                .orders
                .get(&id)
                .is_some_and(|s| s.order.leaves_quantity > 0)
            {
                let order = self.orders[&id].order;
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    match order.order_type {
                        OrderType::Limit => {
                            ob.add_order(&order);
                        }
                        OrderType::Stop | OrderType::StopLimit => {
                            ob.add_stop_order(&order);
                        }
                        OrderType::TrailingStop | OrderType::TrailingStopLimit => {
                            ob.add_trailing_stop_order(&order);
                        }
                        _ => {}
                    }
                }
            }
        }

        if self
            .orders
            .get(&id)
            .is_some_and(|s| s.order.leaves_quantity == 0)
        {
            let order = self.orders[&id].order;
            self.on_delete_order(&order);
            self.orders.remove(&id);
        }

        if self.matching && !recursive {
            self.match_book(symbol_id);
        }
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            ob.reset_matching_price();
        }
        Ok(())
    }

    fn replace_order_impl(
        &mut self,
        id: OrderId,
        new_id: OrderId,
        new_price: u64,
        new_quantity: u64,
        recursive: bool,
    ) -> Result<()> {
        if id == 0 {
            return Err(ErrorCode::OrderIdInvalid);
        }
        if new_id == 0 {
            return Err(ErrorCode::OrderIdInvalid);
        }
        if new_quantity == 0 {
            return Err(ErrorCode::OrderQuantityInvalid);
        }

        let slot = self.orders.get(&id).ok_or(ErrorCode::OrderNotFound)?;
        if slot.order.order_type != OrderType::Limit {
            return Err(ErrorCode::OrderTypeInvalid);
        }
        if new_id != id && self.orders.contains_key(&new_id) {
            return Err(ErrorCode::OrderDuplicate);
        }
        let symbol_id = slot.order.symbol_id;

        // Delete old from book
        {
            let order = self.orders[&id].order;
            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                ob.delete_order(&order);
            }
        }

        let old_order = self.orders[&id].order;
        self.on_delete_order(&old_order);

        // Transform in-place
        {
            let slot = self.orders.get_mut(&id).unwrap();
            slot.order.id = new_id;
            slot.order.price = new_price;
            slot.order.quantity = new_quantity;
            slot.order.executed_quantity = 0;
            slot.order.leaves_quantity = new_quantity;
        }

        let order = self.orders.remove(&id).unwrap().order;
        self.on_add_order(&order);

        if self.matching && !recursive {
            // Need to match, but we just removed it. Re-insert temporarily.
            self.orders.insert(order.id, OrderSlot { order });
            let mut order_mut = order;
            self.match_limit(symbol_id, &mut order_mut);

            // Update stored order
            if let Some(slot) = self.orders.get_mut(&order_mut.id) {
                slot.order.leaves_quantity = order_mut.leaves_quantity;
                slot.order.executed_quantity = order_mut.executed_quantity;
            }

            if self
                .orders
                .get(&order_mut.id)
                .is_some_and(|s| s.order.leaves_quantity > 0)
            {
                if self.orders.contains_key(&order_mut.id) && order_mut.id != id {
                    // Re-insert under new ID (already done)
                }
                let order = self.orders[&order_mut.id].order;
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    ob.add_order(&order);
                }
            } else {
                let order = self.orders.remove(&order_mut.id).map(|s| s.order);
                if let Some(order) = order {
                    self.on_delete_order(&order);
                }
            }
        } else {
            // No matching — just insert
            if self.orders.contains_key(&order.id) {
                self.on_delete_order(&order);
                return Err(ErrorCode::OrderDuplicate);
            }
            if order.leaves_quantity > 0 {
                self.orders.insert(order.id, OrderSlot { order });
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    ob.add_order(&order);
                }
            } else {
                self.on_delete_order(&order);
            }
        }

        if self.matching && !recursive {
            self.match_book(symbol_id);
        }
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            ob.reset_matching_price();
        }
        Ok(())
    }

    fn delete_order_impl(&mut self, id: OrderId, recursive: bool) -> Result<()> {
        if id == 0 {
            return Err(ErrorCode::OrderIdInvalid);
        }
        let symbol_id = self
            .orders
            .get(&id)
            .ok_or(ErrorCode::OrderNotFound)?
            .order
            .symbol_id;

        let order = self.orders.remove(&id).unwrap().order;
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            match order.order_type {
                OrderType::Limit => {
                    ob.delete_order(&order);
                }
                OrderType::Stop | OrderType::StopLimit => {
                    ob.delete_stop_order(&order);
                }
                OrderType::TrailingStop | OrderType::TrailingStopLimit => {
                    ob.delete_trailing_stop_order(&order);
                }
                _ => {}
            }
        }

        self.on_delete_order(&order);

        if self.matching && !recursive {
            self.match_book(symbol_id);
        }
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            ob.reset_matching_price();
        }
        Ok(())
    }

    // ===========================================================================
    //  Matching engine
    // ===========================================================================

    fn match_book(&mut self, symbol_id: u32) {
        loop {
            // Check for crossed book
            let has_cross = self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
                .is_some_and(|ob| match (ob.best_bid(), ob.best_ask()) {
                    (Some(bid), Some(ask)) => bid.level.price >= ask.level.price,
                    _ => false,
                });

            if !has_cross {
                break;
            }

            // Get first orders from best bid/ask
            let info = self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
                .and_then(|ob| {
                    let bid = ob.best_bid()?;
                    let ask = ob.best_ask()?;
                    let bid_order = bid.front_order_id()?;
                    let ask_order = ask.front_order_id()?;
                    Some((bid_order, ask_order, bid.level.price, ask.level.price))
                });

            let (bid_id, ask_id, bid_price, _ask_price) = match info {
                Some(v) => v,
                None => break,
            };

            // Check AON
            let bid_aon = self.orders.get(&bid_id).is_some_and(|s| s.order.is_aon());
            let ask_aon = self.orders.get(&ask_id).is_some_and(|s| s.order.is_aon());

            if bid_aon || ask_aon {
                // For AON: check if both sides can be fully filled
                let bid_total: u64 = self
                    .order_books
                    .get(symbol_id as usize)
                    .and_then(|o| o.as_ref())
                    .map_or(0, |ob| {
                        ob.bids().values().map(|l| l.level.total_volume).sum()
                    });
                let ask_total: u64 = self
                    .order_books
                    .get(symbol_id as usize)
                    .and_then(|o| o.as_ref())
                    .map_or(0, |ob| {
                        ob.asks().values().map(|l| l.level.total_volume).sum()
                    });

                let bid_qty = self
                    .orders
                    .get(&bid_id)
                    .map_or(0, |s| s.order.leaves_quantity);
                let ask_qty = self
                    .orders
                    .get(&ask_id)
                    .map_or(0, |s| s.order.leaves_quantity);

                // AON bid needs ask_total >= bid_qty; AON ask needs bid_total >= ask_qty
                let can_fill = if bid_aon && ask_aon {
                    bid_qty <= ask_total && ask_qty <= bid_total
                } else if bid_aon {
                    bid_qty <= ask_total
                } else {
                    ask_qty <= bid_total
                };

                if !can_fill {
                    break;
                }

                // Both sides can be filled — execute the smaller side
                let exec_qty = bid_qty.min(ask_qty);
                self.execute_matching_chain(symbol_id, bid_price, exec_qty);
            } else {
                let bid_qty = self
                    .orders
                    .get(&bid_id)
                    .map_or(0, |s| s.order.leaves_quantity);
                let ask_qty = self
                    .orders
                    .get(&ask_id)
                    .map_or(0, |s| s.order.leaves_quantity);

                let (exec_id, reduce_id, exec_qty, exec_price) = if bid_qty > ask_qty {
                    (ask_id, bid_id, ask_qty, ask_id) // exec at ask price
                } else {
                    (bid_id, ask_id, bid_qty, bid_id) // exec at bid price
                };

                // Get exec price from the executing order
                let price = self.orders.get(&exec_price).map_or(0, |s| s.order.price);

                // Execute the smaller order
                let exec_order_for_callback = self.orders[&exec_id].order;
                self.on_execute_order(&exec_order_for_callback, price, exec_qty);
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    ob.update_last_price(&self.orders[&exec_id].order, price);
                    ob.update_matching_price(&self.orders[&exec_id].order, price);
                }
                {
                    let slot = self.orders.get_mut(&exec_id).unwrap();
                    slot.order.executed_quantity += exec_qty;
                }
                let exec_order = self.orders.remove(&exec_id).unwrap().order;
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                    && exec_order.order_type == OrderType::Limit
                {
                    ob.delete_order(&exec_order);
                }
                self.on_delete_order(&exec_order);

                // Reduce the larger order
                let reduce_order_for_callback = self.orders[&reduce_id].order;
                self.on_execute_order(&reduce_order_for_callback, price, exec_qty);
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    ob.update_last_price(&self.orders[&reduce_id].order, price);
                    ob.update_matching_price(&self.orders[&reduce_id].order, price);
                }
                self.reduce_order_impl(reduce_id, exec_qty, true).ok();
            }

            // Activate stop orders
            self.activate_stop_orders_at(symbol_id);
        }

        // Activate stop orders until no more
        while self.activate_stop_orders(symbol_id) {}
    }

    fn match_market(&mut self, symbol_id: u32, order: &mut Order) {
        let price = if order.is_buy() {
            let best = self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
                .and_then(|ob| ob.best_ask());
            match best {
                None => return,
                Some(l) => l.level.price.saturating_add(order.slippage),
            }
        } else {
            let best = self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
                .and_then(|ob| ob.best_bid());
            match best {
                None => return,
                Some(l) => l.level.price.saturating_sub(order.slippage),
            }
        };
        order.price = price;
        self.match_order_impl(symbol_id, order);
    }

    fn match_limit(&mut self, symbol_id: u32, order: &mut Order) {
        self.match_order_impl(symbol_id, order);
    }

    fn match_order_impl(&mut self, symbol_id: u32, order: &mut Order) {
        loop {
            // Get the best opposing level
            let level_info = self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
                .and_then(|ob| {
                    if order.is_buy() {
                        let ask = ob.best_ask()?;
                        Some((ask.level.price, ask.front_order_id()?))
                    } else {
                        let bid = ob.best_bid()?;
                        Some((bid.level.price, bid.front_order_id()?))
                    }
                });

            let (level_price, opposing_id) = match level_info {
                Some(v) => v,
                None => return,
            };

            // Check arbitrage
            let arbitrage = if order.is_buy() {
                order.price >= level_price
            } else {
                order.price <= level_price
            };
            if !arbitrage {
                return;
            }

            // FOK/AON special case
            if order.is_fok() || order.is_aon() {
                let chain = self.calculate_matching_chain_volume(
                    symbol_id,
                    order.price,
                    order.leaves_quantity,
                    order.is_buy(),
                );
                if chain < order.leaves_quantity {
                    return;
                }

                self.execute_matching_chain_at(symbol_id, order.price, order.is_buy(), order);
                return;
            }

            // Check AON on opposing
            let opposing_aon = self
                .orders
                .get(&opposing_id)
                .is_some_and(|s| s.order.is_aon());
            let opposing_qty = self
                .orders
                .get(&opposing_id)
                .map_or(0, |s| s.order.leaves_quantity);

            if opposing_aon && opposing_qty > order.leaves_quantity {
                return;
            }

            let quantity = opposing_qty.min(order.leaves_quantity);
            let price = self.orders.get(&opposing_id).map_or(0, |s| s.order.price);

            // Execute opposing order
            let opposing_order_for_callback = self.orders[&opposing_id].order;
            self.on_execute_order(&opposing_order_for_callback, price, quantity);
            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                ob.update_last_price(&self.orders[&opposing_id].order, price);
                ob.update_matching_price(&self.orders[&opposing_id].order, price);
            }
            // Update executed_quantity on the opposing order before reducing
            if let Some(slot) = self.orders.get_mut(&opposing_id) {
                slot.order.executed_quantity += quantity;
            }
            self.reduce_order_impl(opposing_id, quantity, true).ok();

            // Update incoming order
            self.on_execute_order(order, price, quantity);
            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                ob.update_last_price(order, price);
                ob.update_matching_price(order, price);
            }
            order.executed_quantity += quantity;
            order.leaves_quantity -= quantity;

            if order.leaves_quantity == 0 {
                return;
            }
        }
    }

    // -- Stop order activation -------------------------------------------------

    fn activate_stop_orders(&mut self, symbol_id: u32) -> bool {
        let mut result = false;
        let mut stop = false;

        while !stop {
            stop = true;

            let ask_price = self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
                .map_or(u64::MAX, |ob| ob.get_market_price_ask());
            let bid_price = self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
                .map_or(0, |ob| ob.get_market_price_bid());

            if self.activate_stop_at(symbol_id, true, ask_price) {
                result = true;
                stop = false;
            }
            if self.activate_trailing_stop_at(symbol_id, true, ask_price) {
                result = true;
                stop = false;
            }
            self.recalculate_trailing_stop_price(symbol_id, LevelType::Ask);
            if self.activate_stop_at(symbol_id, false, bid_price) {
                result = true;
                stop = false;
            }
            if self.activate_trailing_stop_at(symbol_id, false, bid_price) {
                result = true;
                stop = false;
            }
            self.recalculate_trailing_stop_price(symbol_id, LevelType::Bid);
        }

        result
    }

    fn activate_stop_orders_at(&mut self, symbol_id: u32) {
        let ask_price = self
            .order_books
            .get(symbol_id as usize)
            .and_then(|o| o.as_ref())
            .map_or(u64::MAX, |ob| ob.get_market_price_ask());
        let bid_price = self
            .order_books
            .get(symbol_id as usize)
            .and_then(|o| o.as_ref())
            .map_or(0, |ob| ob.get_market_price_bid());
        self.activate_stop_at(symbol_id, true, ask_price);
        self.activate_stop_at(symbol_id, false, bid_price);
    }

    fn activate_stop_at(&mut self, symbol_id: u32, is_buy: bool, market_price: u64) -> bool {
        // Get the best stop level and check if it should be activated
        let order_ids: Vec<OrderId> = {
            let ob = match self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
            {
                Some(ob) => ob,
                None => return false,
            };
            let level = if is_buy {
                ob.best_buy_stop()
            } else {
                ob.best_sell_stop()
            };
            let level = match level {
                Some(l) => l,
                None => return false,
            };
            let arbitrage = if level.level.is_bid() {
                market_price >= level.level.price
            } else {
                market_price <= level.level.price
            };
            if !arbitrage {
                return false;
            }
            if level.has_tombstones() {
                level.order_ids().collect()
            } else {
                level.order_queue.iter().copied().collect()
            }
        };

        for order_id in order_ids {
            let order = match self.orders.get(&order_id) {
                Some(s) => s.order,
                None => continue,
            };
            match order.order_type {
                OrderType::Stop | OrderType::TrailingStop => {
                    self.activate_stop_order(symbol_id, order_id);
                }
                OrderType::StopLimit | OrderType::TrailingStopLimit => {
                    self.activate_stop_limit_order(symbol_id, order_id);
                }
                _ => {}
            }
        }
        true
    }

    fn activate_trailing_stop_at(
        &mut self,
        symbol_id: u32,
        is_buy: bool,
        market_price: u64,
    ) -> bool {
        let order_ids: Vec<OrderId> = {
            let ob = match self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
            {
                Some(ob) => ob,
                None => return false,
            };
            let level = if is_buy {
                ob.best_trailing_buy_stop()
            } else {
                ob.best_trailing_sell_stop()
            };
            let level = match level {
                Some(l) => l,
                None => return false,
            };
            let arbitrage = if level.level.is_bid() {
                market_price >= level.level.price
            } else {
                market_price <= level.level.price
            };
            if !arbitrage {
                return false;
            }
            if level.has_tombstones() {
                level.order_ids().collect()
            } else {
                level.order_queue.iter().copied().collect()
            }
        };

        for order_id in order_ids {
            let order = match self.orders.get(&order_id) {
                Some(s) => s.order,
                None => continue,
            };
            match order.order_type {
                OrderType::Stop | OrderType::TrailingStop => {
                    self.activate_stop_order(symbol_id, order_id);
                }
                OrderType::StopLimit | OrderType::TrailingStopLimit => {
                    self.activate_stop_limit_order(symbol_id, order_id);
                }
                _ => {}
            }
        }
        true
    }

    fn activate_stop_order(&mut self, symbol_id: u32, id: OrderId) {
        let mut order = match self.orders.remove(&id) {
            Some(s) => s.order,
            None => return,
        };

        // Remove from stop book
        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            if order.is_trailing_stop() || order.is_trailing_stop_limit() {
                ob.delete_trailing_stop_order(&order);
            } else {
                ob.delete_stop_order(&order);
            }
        }

        // Convert to market
        order.order_type = OrderType::Market;
        order.price = 0;
        order.stop_price = 0;
        order.time_in_force = if order.is_fok() {
            crate::matching::types::OrderTimeInForce::Fok
        } else {
            crate::matching::types::OrderTimeInForce::Ioc
        };

        self.on_update_order(&order);
        self.match_market(symbol_id, &mut order);
        self.on_delete_order(&order);
    }

    fn activate_stop_limit_order(&mut self, symbol_id: u32, id: OrderId) {
        let mut order = match self.orders.remove(&id) {
            Some(s) => s.order,
            None => return,
        };

        if let Some(ob) = self
            .order_books
            .get_mut(symbol_id as usize)
            .and_then(|o| o.as_mut())
        {
            if order.is_trailing_stop() || order.is_trailing_stop_limit() {
                ob.delete_trailing_stop_order(&order);
            } else {
                ob.delete_stop_order(&order);
            }
        }

        // Convert to limit
        order.order_type = OrderType::Limit;
        order.stop_price = 0;

        self.on_update_order(&order);
        self.match_limit(symbol_id, &mut order);

        if order.leaves_quantity > 0 && !order.is_ioc() && !order.is_fok() {
            let oc = order;
            self.orders.insert(order.id, OrderSlot { order });
            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                ob.add_order(&oc);
            }
        } else {
            self.on_delete_order(&order);
        }
    }

    // -- Matching chain calculation --------------------------------------------

    fn accumulate_level_volume(
        &self,
        level: &crate::matching::order_book::LevelData,
        available: &mut u64,
        volume: u64,
    ) -> bool {
        if level.has_tombstones() {
            for order_id in level.order_ids() {
                if *available >= volume {
                    return true;
                }
                let qty = self
                    .orders
                    .get(&order_id)
                    .map_or(0, |s| s.order.leaves_quantity);
                *available += qty;
            }
        } else {
            for &order_id in &level.order_queue {
                if *available >= volume {
                    return true;
                }
                let qty = self
                    .orders
                    .get(&order_id)
                    .map_or(0, |s| s.order.leaves_quantity);
                *available += qty;
            }
        }

        *available >= volume
    }

    fn calculate_matching_chain_volume(
        &self,
        symbol_id: u32,
        level_price: u64,
        volume: u64,
        is_buy: bool,
    ) -> u64 {
        let ob = match self
            .order_books
            .get(symbol_id as usize)
            .and_then(|o| o.as_ref())
        {
            Some(ob) => ob,
            None => return 0,
        };

        let mut available: u64 = 0;

        if is_buy {
            for (_price, level) in ob.asks().range(..=level_price) {
                if self.accumulate_level_volume(level, &mut available, volume) {
                    return available;
                }
            }
        } else {
            for (_price, level) in ob.bids().range(level_price..).rev() {
                if self.accumulate_level_volume(level, &mut available, volume) {
                    return available;
                }
            }
        }

        if available >= volume { available } else { 0 }
    }

    fn execute_matching_chain(&mut self, symbol_id: u32, price: u64, volume: u64) {
        let mut remaining = volume;

        // Execute bid side
        while remaining > 0 {
            let order_id = self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
                .and_then(|ob| ob.best_bid())
                .and_then(|l| l.front_order_id());

            let order_id = match order_id {
                Some(id) => id,
                None => break,
            };

            let qty = self
                .orders
                .get(&order_id)
                .map_or(0, |s| s.order.leaves_quantity.min(remaining));
            if qty == 0 {
                break;
            }

            let order_for_callback = self.orders[&order_id].order;
            self.on_execute_order(&order_for_callback, price, qty);
            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                ob.update_last_price(&order_for_callback, price);
                ob.update_matching_price(&order_for_callback, price);
            }

            let hidden_before = self.orders[&order_id].order.hidden_quantity();
            let visible_before = self.orders[&order_id].order.visible_quantity();

            let fully_consumed = {
                let slot = self.orders.get_mut(&order_id).unwrap();
                slot.order.executed_quantity += qty;
                slot.order.leaves_quantity -= qty;
                slot.order.leaves_quantity == 0
            };

            let hidden_after = self.orders[&order_id].order.hidden_quantity();
            let visible_after = self.orders[&order_id].order.visible_quantity();
            let hidden_delta = hidden_before - hidden_after;
            let visible_delta = visible_before - visible_after;

            if fully_consumed {
                let order = self.orders.remove(&order_id).unwrap().order;
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    ob.reduce_order(&order, qty, hidden_delta, visible_delta);
                }
                self.on_delete_order(&order);
            } else {
                let order = self.orders[&order_id].order;
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    ob.reduce_order(&order, qty, hidden_delta, visible_delta);
                }
            }
            remaining -= qty;
        }

        remaining = volume;

        // Execute ask side
        while remaining > 0 {
            let order_id = self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
                .and_then(|ob| ob.best_ask())
                .and_then(|l| l.front_order_id());

            let order_id = match order_id {
                Some(id) => id,
                None => break,
            };

            let qty = self
                .orders
                .get(&order_id)
                .map_or(0, |s| s.order.leaves_quantity.min(remaining));
            if qty == 0 {
                break;
            }

            let order_for_callback = self.orders[&order_id].order;
            self.on_execute_order(&order_for_callback, price, qty);
            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                ob.update_last_price(&order_for_callback, price);
                ob.update_matching_price(&order_for_callback, price);
            }

            let hidden_before = self.orders[&order_id].order.hidden_quantity();
            let visible_before = self.orders[&order_id].order.visible_quantity();

            let fully_consumed = {
                let slot = self.orders.get_mut(&order_id).unwrap();
                slot.order.executed_quantity += qty;
                slot.order.leaves_quantity -= qty;
                slot.order.leaves_quantity == 0
            };

            let hidden_after = self.orders[&order_id].order.hidden_quantity();
            let visible_after = self.orders[&order_id].order.visible_quantity();
            let hidden_delta = hidden_before - hidden_after;
            let visible_delta = visible_before - visible_after;

            if fully_consumed {
                let order = self.orders.remove(&order_id).unwrap().order;
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    ob.reduce_order(&order, qty, hidden_delta, visible_delta);
                }
                self.on_delete_order(&order);
            } else {
                let order = self.orders[&order_id].order;
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    ob.reduce_order(&order, qty, hidden_delta, visible_delta);
                }
            }
            remaining -= qty;
        }
    }

    fn execute_matching_chain_at(
        &mut self,
        symbol_id: u32,
        limit_price: u64,
        is_buy: bool,
        incoming: &mut Order,
    ) {
        let mut remaining = incoming.leaves_quantity;

        while remaining > 0 {
            let level_info =
                self.order_books
                    .get(symbol_id as usize)
                    .and_then(|o| o.as_ref())
                    .and_then(|ob| {
                        if is_buy {
                            ob.asks()
                                .range(..=limit_price)
                                .next()
                                .and_then(|(&price, level)| {
                                    level.front_order_id().map(|id| (price, id))
                                })
                        } else {
                            ob.bids().range(limit_price..).next_back().and_then(
                                |(&price, level)| level.front_order_id().map(|id| (price, id)),
                            )
                        }
                    });

            let (price, order_id) = match level_info {
                Some(info) => info,
                None => break,
            };

            let qty = self
                .orders
                .get(&order_id)
                .map_or(0, |s| s.order.leaves_quantity.min(remaining));
            if qty == 0 {
                break;
            }

            let order_for_callback = self.orders[&order_id].order;
            self.on_execute_order(&order_for_callback, price, qty);
            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                ob.update_last_price(&order_for_callback, price);
                ob.update_matching_price(&order_for_callback, price);
            }

            let hidden_before = self.orders[&order_id].order.hidden_quantity();
            let visible_before = self.orders[&order_id].order.visible_quantity();

            let fully_consumed = {
                let slot = self.orders.get_mut(&order_id).unwrap();
                slot.order.executed_quantity += qty;
                slot.order.leaves_quantity -= qty;
                slot.order.leaves_quantity == 0
            };

            let hidden_after = self.orders[&order_id].order.hidden_quantity();
            let visible_after = self.orders[&order_id].order.visible_quantity();
            let hidden_delta = hidden_before - hidden_after;
            let visible_delta = visible_before - visible_after;

            if fully_consumed {
                let order = self.orders.remove(&order_id).unwrap().order;
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    ob.reduce_order(&order, qty, hidden_delta, visible_delta);
                }
                self.on_delete_order(&order);
            } else {
                let order = self.orders[&order_id].order;
                if let Some(ob) = self
                    .order_books
                    .get_mut(symbol_id as usize)
                    .and_then(|o| o.as_mut())
                {
                    ob.reduce_order(&order, qty, hidden_delta, visible_delta);
                }
            }

            self.on_execute_order(incoming, price, qty);
            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                ob.update_last_price(incoming, price);
                ob.update_matching_price(incoming, price);
            }
            incoming.executed_quantity += qty;
            incoming.leaves_quantity -= qty;
            remaining -= qty;
        }
    }

    fn recalculate_trailing_stop_price(&mut self, symbol_id: u32, level_type: LevelType) {
        // Collect trailing stop orders that need recalculation
        let orders_to_update: Vec<(OrderId, u64)> = {
            let ob = match self
                .order_books
                .get(symbol_id as usize)
                .and_then(|o| o.as_ref())
            {
                Some(ob) => ob,
                None => return,
            };
            let levels = match level_type {
                LevelType::Ask => ob.trailing_buy_stop(),
                LevelType::Bid => ob.trailing_sell_stop(),
            };
            let mut updates = Vec::new();
            for level in levels.values() {
                for order_id in level.order_ids() {
                    if let Some(slot) = self.orders.get(&order_id) {
                        let new_price = ob.calculate_trailing_stop_price(&slot.order);
                        if new_price != slot.order.stop_price {
                            updates.push((order_id, new_price));
                        }
                    }
                }
            }
            updates
        };

        for (order_id, new_price) in orders_to_update {
            let Some(old_order) = self.orders.get(&order_id).map(|slot| slot.order) else {
                continue;
            };

            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                ob.delete_trailing_stop_order(&old_order);
            }

            let Some(order) = self.orders.get_mut(&order_id).map(|slot| {
                match slot.order.order_type {
                    OrderType::TrailingStop => {
                        slot.order.stop_price = new_price;
                    }
                    OrderType::TrailingStopLimit => {
                        let diff = slot.order.price as i64 - slot.order.stop_price as i64;
                        slot.order.stop_price = new_price;
                        slot.order.price = (new_price as i64 + diff) as u64;
                    }
                    _ => {}
                }
                slot.order
            }) else {
                continue;
            };

            self.on_update_order(&order);

            if let Some(ob) = self
                .order_books
                .get_mut(symbol_id as usize)
                .and_then(|o| o.as_mut())
            {
                ob.add_trailing_stop_order(&order);
            }
        }
    }

    // -- Level update notification ---------------------------------------------

    fn update_level(&mut self, symbol_id: u32, update: &LevelUpdate) {
        let ob = match self
            .order_books
            .get(symbol_id as usize)
            .and_then(|o| o.as_ref())
        {
            Some(ob) => ob,
            None => return,
        };
        match update.update_type {
            UpdateType::Add => self.handler.on_add_level(ob, &update.level, update.top),
            UpdateType::Update => self.handler.on_update_level(ob, &update.level, update.top),
            UpdateType::Delete => self.handler.on_delete_level(ob, &update.level, update.top),
            UpdateType::None => {}
        }
        self.handler.on_update_order_book(ob, update.top);
    }
}
