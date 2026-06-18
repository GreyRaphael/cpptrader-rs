//! Interactive command-line matching engine powered by clap.
//!
//! # Usage
//!
//! ```sh
//! cargo run --example matching_engine
//! ```
//!
//! Then type commands at the `>` prompt. Type `help` or `quit` to exit.
//!
//! # Example session
//!
//! ```text
//! > add-symbol 1 AAPL
//! > add-book 1
//! > add 1 1 buy limit 15000 100
//! > add 2 1 sell limit 15100 200
//! > book 1
//! > enable
//! > add 3 1 buy limit 15100 50
//! > book 1
//! > quit
//! ```

use std::io::{self, BufRead, Write};

use clap::{Parser, Subcommand};
use cpptrader::matching::market_handler::MarketHandler;
use cpptrader::matching::market_manager::MarketManager;
use cpptrader::matching::order::Order;
use cpptrader::matching::order_book::OrderBook;
use cpptrader::matching::symbol::Symbol;
use cpptrader::matching::types::*;

// ---------------------------------------------------------------------------
//  CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "matching-engine",
    about = "Interactive CppTrader matching engine"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new symbol
    AddSymbol {
        /// Symbol ID
        id: u32,
        /// Symbol name (up to 8 chars)
        name: String,
    },
    /// Add an order book for a symbol
    AddBook {
        /// Symbol ID
        symbol_id: u32,
    },
    /// Add a new order
    Add {
        /// Order ID
        id: u64,
        /// Symbol ID
        symbol_id: u32,
        /// Side: buy / sell
        side: SideArg,
        /// Order type: market / limit / stop / stop-limit / trailing-stop / trailing-stop-limit
        #[arg(value_enum)]
        order_type: OrderTypeArg,
        /// Price (or stop price for stop orders)
        price: u64,
        /// Quantity
        quantity: u64,
        /// Time-in-force: gtc / ioc / fok / aon (default: gtc)
        #[arg(default_value = "gtc")]
        tif: TifArg,
        /// Extra parameter: max_visible for limit/stop-limit, slippage for market/stop
        #[arg(default_value = "max")]
        extra: ExtraArg,
    },
    /// Delete an order
    Delete {
        /// Order ID
        id: u64,
    },
    /// Reduce an order quantity
    Reduce {
        /// Order ID
        id: u64,
        /// Quantity to reduce
        quantity: u64,
    },
    /// Modify an order (price + quantity)
    Modify {
        /// Order ID
        id: u64,
        /// New price
        price: u64,
        /// New quantity
        quantity: u64,
    },
    /// Mitigate an order (in-flight mitigation)
    Mitigate {
        /// Order ID
        id: u64,
        /// New price
        price: u64,
        /// New quantity
        quantity: u64,
    },
    /// Replace an order with new ID / price / quantity
    Replace {
        /// Old order ID
        id: u64,
        /// New order ID
        new_id: u64,
        /// New price
        price: u64,
        /// New quantity
        quantity: u64,
    },
    /// Execute an order manually
    Execute {
        /// Order ID
        id: u64,
        /// Quantity to execute
        quantity: u64,
    },
    /// Trigger manual matching across all order books
    Match,
    /// Enable automatic matching
    Enable,
    /// Disable automatic matching
    Disable,
    /// Print the order book for a symbol
    Book {
        /// Symbol ID
        symbol_id: u32,
    },
    /// Print all active orders
    Orders,
    /// Exit
    Quit,
}

// ---------------------------------------------------------------------------
//  Argument parsers
// ---------------------------------------------------------------------------

#[derive(Clone, clap::ValueEnum)]
enum SideArg {
    Buy,
    Sell,
}

impl From<SideArg> for OrderSide {
    fn from(s: SideArg) -> Self {
        match s {
            SideArg::Buy => OrderSide::Buy,
            SideArg::Sell => OrderSide::Sell,
        }
    }
}

#[derive(Clone, clap::ValueEnum)]
enum OrderTypeArg {
    Market,
    Limit,
    Stop,
    StopLimit,
    TrailingStop,
    TrailingStopLimit,
}

#[derive(Clone, clap::ValueEnum)]
enum TifArg {
    Gtc,
    Ioc,
    Fok,
    Aon,
}

impl From<TifArg> for OrderTimeInForce {
    fn from(t: TifArg) -> Self {
        match t {
            TifArg::Gtc => OrderTimeInForce::Gtc,
            TifArg::Ioc => OrderTimeInForce::Ioc,
            TifArg::Fok => OrderTimeInForce::Fok,
            TifArg::Aon => OrderTimeInForce::Aon,
        }
    }
}

/// Sentinel-aware extra parameter: "max" → u64::MAX, otherwise parsed as u64.
#[derive(Clone)]
enum ExtraArg {
    Max,
    Value(u64),
}

impl std::str::FromStr for ExtraArg {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("max") {
            Ok(ExtraArg::Max)
        } else {
            Ok(ExtraArg::Value(s.parse()?))
        }
    }
}

impl ExtraArg {
    fn value(self) -> u64 {
        match self {
            ExtraArg::Max => u64::MAX,
            ExtraArg::Value(v) => v,
        }
    }
}

// ---------------------------------------------------------------------------
//  Event handler
// ---------------------------------------------------------------------------

struct PrintHandler;

impl MarketHandler for PrintHandler {
    fn on_add_symbol(&mut self, s: &Symbol) {
        println!("  [event] symbol added: {}", s);
    }
    fn on_delete_symbol(&mut self, s: &Symbol) {
        println!("  [event] symbol deleted: {}", s);
    }
    fn on_add_order_book(&mut self, ob: &OrderBook) {
        println!("  [event] order book added: {}", ob);
    }
    fn on_delete_order_book(&mut self, ob: &OrderBook) {
        println!("  [event] order book deleted: {}", ob);
    }
    fn on_add_order(&mut self, o: &Order) {
        println!("  [event] order added: {}", o);
    }
    fn on_update_order(&mut self, o: &Order) {
        println!("  [event] order updated: {}", o);
    }
    fn on_delete_order(&mut self, o: &Order) {
        println!("  [event] order deleted: {}", o);
    }
    fn on_execute_order(&mut self, o: &Order, price: u64, qty: u64) {
        println!("  [event] executed: order {} @ {} qty={}", o.id, price, qty);
    }
}

// ---------------------------------------------------------------------------
//  Helpers
// ---------------------------------------------------------------------------

fn print_book(ob: &OrderBook) {
    println!("  ┌─ Order Book: {} ──────────────────────", ob.symbol());
    println!("  │ Bids:");
    if ob.bids().is_empty() {
        println!("  │   (empty)");
    }
    for (price, level) in ob.bids().iter().rev() {
        println!(
            "  │   {:>8}  vol={:<8} orders={}",
            price, level.level.total_volume, level.level.orders
        );
    }
    println!("  │ Asks:");
    if ob.asks().is_empty() {
        println!("  │   (empty)");
    }
    for (price, level) in ob.asks() {
        println!(
            "  │   {:>8}  vol={:<8} orders={}",
            price, level.level.total_volume, level.level.orders
        );
    }
    let bid = ob
        .best_bid()
        .map_or("-".to_string(), |l| l.level.price.to_string());
    let ask = ob
        .best_ask()
        .map_or("-".to_string(), |l| l.level.price.to_string());
    println!("  │ Best: bid={}  ask={}", bid, ask);
    println!("  └──────────────────────────────────────");
}

#[allow(clippy::too_many_arguments)]
fn make_order(
    id: u64,
    symbol_id: u32,
    side: OrderSide,
    order_type: OrderTypeArg,
    price: u64,
    quantity: u64,
    tif: OrderTimeInForce,
    extra: u64,
) -> Order {
    match order_type {
        OrderTypeArg::Market => Order::market(id, symbol_id, side, quantity, extra),
        OrderTypeArg::Limit => Order::limit(id, symbol_id, side, price, quantity, tif, extra),
        OrderTypeArg::Stop => Order::stop(id, symbol_id, side, price, quantity, tif, extra),
        OrderTypeArg::StopLimit => {
            Order::stop_limit(id, symbol_id, side, price, price, quantity, tif, extra)
        }
        OrderTypeArg::TrailingStop => Order::trailing_stop(
            id,
            symbol_id,
            side,
            price,
            quantity,
            extra as i64,
            0,
            tif,
            u64::MAX,
        ),
        OrderTypeArg::TrailingStopLimit => Order::trailing_stop_limit(
            id,
            symbol_id,
            side,
            price,
            price,
            quantity,
            extra as i64,
            0,
            tif,
            u64::MAX,
        ),
    }
}

// ---------------------------------------------------------------------------
//  Main
// ---------------------------------------------------------------------------

fn main() {
    println!("CppTrader Matching Engine — type `help` for usage, `quit` to exit\n");

    let mut mm = MarketManager::new(Box::new(PrintHandler));
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    print!("> ");
    stdout.flush().unwrap();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            print!("> ");
            stdout.flush().unwrap();
            continue;
        }

        // Prepend the program name for clap parsing
        let args = std::iter::once("matching-engine".to_string())
            .chain(line.split_whitespace().map(|s| s.to_string()))
            .collect::<Vec<_>>();

        match Cli::try_parse_from(&args) {
            Ok(cli) => {
                run_command(&mut mm, cli.command);
            }
            Err(e) => {
                // Print a compact error instead of the full clap help
                let msg = e.to_string();
                if msg.contains("help") || msg.is_empty() {
                    print_help();
                } else {
                    // Show just the error line, not the full usage
                    for line in msg.lines().take(3) {
                        println!("  {}", line);
                    }
                }
            }
        }

        print!("> ");
        stdout.flush().unwrap();
    }
}

fn print_help() {
    println!("Commands:");
    println!("  add-symbol <id> <name>                              Add a symbol");
    println!("  add-book <symbol_id>                                Add an order book");
    println!("  add <id> <symbol> <side> <type> <price> <qty> [tif] [extra]");
    println!("      type: market, limit, stop, stop-limit, trailing-stop, trailing-stop-limit");
    println!("      tif:  gtc (default), ioc, fok, aon");
    println!("      extra: max (default), or numeric value");
    println!("  delete <id>                                         Delete an order");
    println!("  reduce <id> <qty>                                   Reduce order quantity");
    println!("  modify <id> <price> <qty>                           Modify order");
    println!("  mitigate <id> <price> <qty>                         Mitigate order (IFM)");
    println!("  replace <id> <new_id> <price> <qty>                 Replace order");
    println!("  execute <id> <qty>                                  Execute order manually");
    println!("  match                                               Trigger manual matching");
    println!("  enable                                              Enable auto-matching");
    println!("  disable                                             Disable auto-matching");
    println!("  book <symbol_id>                                    Print order book");
    println!("  orders                                              Print all active orders");
    println!("  quit                                                Exit");
}

fn run_command(mm: &mut MarketManager, cmd: Commands) {
    match cmd {
        Commands::AddSymbol { id, name } => {
            let mut buf = [0u8; 8];
            let bytes = name.as_bytes();
            let len = bytes.len().min(8);
            buf[..len].copy_from_slice(&bytes[..len]);
            match mm.add_symbol(Symbol::new(id, &buf)) {
                Ok(()) => println!("  OK: symbol {} added", id),
                Err(e) => println!("  Error: {}", e),
            }
        }
        Commands::AddBook { symbol_id } => {
            let sym = match mm.get_symbol(symbol_id) {
                Some(s) => *s,
                None => {
                    println!("  Error: symbol {} not found", symbol_id);
                    return;
                }
            };
            match mm.add_order_book(&sym) {
                Ok(()) => println!("  OK: order book for symbol {} added", symbol_id),
                Err(e) => println!("  Error: {}", e),
            }
        }
        Commands::Add {
            id,
            symbol_id,
            side,
            order_type,
            price,
            quantity,
            tif,
            extra,
        } => {
            let order = make_order(
                id,
                symbol_id,
                side.into(),
                order_type,
                price,
                quantity,
                tif.into(),
                extra.value(),
            );
            match mm.add_order(order) {
                Ok(()) => println!("  OK: order {} added", id),
                Err(e) => println!("  Error: {}", e),
            }
        }
        Commands::Delete { id } => match mm.delete_order(id) {
            Ok(()) => println!("  OK: order {} deleted", id),
            Err(e) => println!("  Error: {}", e),
        },
        Commands::Reduce { id, quantity } => match mm.reduce_order(id, quantity) {
            Ok(()) => println!("  OK: order {} reduced by {}", id, quantity),
            Err(e) => println!("  Error: {}", e),
        },
        Commands::Modify {
            id,
            price,
            quantity,
        } => match mm.modify_order(id, price, quantity) {
            Ok(()) => println!("  OK: order {} modified", id),
            Err(e) => println!("  Error: {}", e),
        },
        Commands::Mitigate {
            id,
            price,
            quantity,
        } => match mm.mitigate_order(id, price, quantity) {
            Ok(()) => println!("  OK: order {} mitigated", id),
            Err(e) => println!("  Error: {}", e),
        },
        Commands::Replace {
            id,
            new_id,
            price,
            quantity,
        } => match mm.replace_order(id, new_id, price, quantity) {
            Ok(()) => println!("  OK: order {} → {}", id, new_id),
            Err(e) => println!("  Error: {}", e),
        },
        Commands::Execute { id, quantity } => match mm.execute_order(id, quantity) {
            Ok(()) => println!("  OK: order {} executed {} qty", id, quantity),
            Err(e) => println!("  Error: {}", e),
        },
        Commands::Match => {
            mm.match_all();
            println!("  OK: matching complete");
        }
        Commands::Enable => {
            mm.enable_matching();
            println!("  OK: automatic matching enabled");
        }
        Commands::Disable => {
            mm.disable_matching();
            println!("  OK: automatic matching disabled");
        }
        Commands::Book { symbol_id } => match mm.get_order_book(symbol_id) {
            Some(ob) => print_book(ob),
            None => println!("  Error: order book for symbol {} not found", symbol_id),
        },
        Commands::Orders => {
            let orders: Vec<_> = mm.iter_orders().collect();
            if orders.is_empty() {
                println!("  (no active orders)");
            } else {
                println!("  Active orders ({}):", orders.len());
                for (id, order) in &orders {
                    println!(
                        "    id={} {} {} {} @ {} qty={}/{}",
                        id,
                        order.side,
                        order.order_type,
                        order.time_in_force,
                        order.price,
                        order.leaves_quantity,
                        order.quantity
                    );
                }
            }
        }
        Commands::Quit => {
            println!("  Bye!");
            std::process::exit(0);
        }
    }
}
