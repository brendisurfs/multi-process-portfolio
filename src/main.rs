#![allow(unused)]
mod config;
mod indicators;
mod order_engine;
mod portfolio;
mod strategies;
mod trader;

use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
    time::Duration,
};

use bon::Builder;

use crossbeam::channel::tick;
use order_engine::{OrderEngine, OrderEvent};
use parking_lot::Mutex;
use portfolio::{Portfolio, Position};
use rand::{seq::SliceRandom, Rng};
use rayon::{ThreadPool, ThreadPoolBuilder};
use serde::Deserialize;
use strategies::{Rsi, SimpleStrat};
use time::{serde::timestamp, OffsetDateTime};
use tracing::{info, instrument, Level};
use tracing_subscriber::util::SubscriberInitExt;
use trader::{Candle, MarketData, Trader, TradingEngine};
use uuid::Uuid;

enum MarketEvent {
    Ohlc(Candle),
}

enum TimeEvent {
    Midnight,
    MarketOpen,
    MarketClosed,
}

#[derive(PartialEq, Deserialize, Debug, Hash, Eq, Clone)]
struct MarketPair {
    asset: String,
    base: String,
}

impl MarketPair {
    fn new(asset: &str, base: &str) -> MarketPair {
        MarketPair {
            asset: asset.into(),
            base: base.into(),
        }
    }
}

enum Command {
    ForceExit,
    PortfolioStatus,
    CloseAllPositions,
    AddPortfolioPosition(Position),
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_line_number(true)
        .init();

    let engine_id = Uuid::new_v4();
    let sui_market = MarketPair::new("SUI", "USD");
    let sol_market = MarketPair::new("SOL", "USD");
    let btc_market = MarketPair::new("BTC", "USD");
    let nq_market = MarketPair::new("NQ", "");

    // we store our markets to use in the market data generator.
    let markets = [
        sui_market.clone(),
        sol_market.clone(),
        btc_market.clone(),
        nq_market.clone(),
    ];

    let (cmd_tx, cmd_rx) = flume::bounded::<Command>(128);
    let (order_tx, order_rx) = flume::bounded::<OrderEvent>(128);
    let (market_event_tx, market_event_rx) = flume::bounded::<MarketEvent>(128);

    let portfolio = Arc::new(Mutex::new(
        Portfolio::builder()
            .engine_id(engine_id)
            .positions(HashMap::new())
            .markets(vec![sui_market.clone(), sol_market.clone()])
            .build(),
    ));

    let sui_trader = Trader::builder()
        .engine_id(engine_id)
        .market_pair(sui_market)
        .market_data(MarketData::new())
        .command_recv(cmd_rx.clone())
        .portfolio(Arc::clone(&portfolio))
        .order_sender(order_tx.clone())
        .strategy(Box::new(Rsi { period: 14 }))
        .tick_rate(Duration::from_secs(5))
        .build();

    let btc_trader = Trader::builder()
        .engine_id(engine_id)
        .market_pair(btc_market)
        .market_data(MarketData::new())
        .command_recv(cmd_rx.clone())
        .portfolio(Arc::clone(&portfolio))
        .order_sender(order_tx.clone())
        .strategy(Box::new(Rsi { period: 14 }))
        .tick_rate(Duration::from_secs(15))
        .build();

    let nq_trader = Trader::builder()
        .engine_id(engine_id)
        .market_pair(nq_market)
        .market_data(MarketData::new())
        .command_recv(cmd_rx.clone())
        .portfolio(Arc::clone(&portfolio))
        .order_sender(order_tx.clone())
        .strategy(Box::new(SimpleStrat {}))
        .tick_rate(Duration::from_secs(30))
        .build();

    let sol_trader = Trader::builder()
        .engine_id(engine_id)
        .market_pair(sol_market)
        .market_data(MarketData::new())
        .command_recv(cmd_rx.clone())
        .portfolio(Arc::clone(&portfolio))
        .strategy(Box::new(SimpleStrat {}))
        .order_sender(order_tx.clone())
        .tick_rate(Duration::from_secs(2))
        .build();

    let traders = vec![sui_trader, sol_trader, btc_trader, nq_trader];

    let trading_engine_handle = TradingEngine::builder()
        .engine_id(engine_id)
        .traders(traders)
        .build()
        .start();

    OrderEngine::default().start(order_rx);

    // imitation market data generator.
    std::thread::spawn(move || {
        let tick = tick(Duration::from_millis(100));
        let mut price_generator = rand::thread_rng();

        tracing::info!("Starting market data generator");
        while tick.recv().is_ok() {
            let Some(market_target) = markets.choose(&mut price_generator) else {
                tracing::warn!("no chosen");
                continue;
            };

            let open: f32 = price_generator.gen_range(10.0..=20.0);
            let high: f32 = price_generator.gen_range(10.0..=20.0);
            let low: f32 = price_generator.gen_range(10.0..=20.0);
            let close: f32 = price_generator.gen_range(10.0..=20.0);
            let volume: i64 = price_generator.gen_range(1000..=2000);
            let timestamp = OffsetDateTime::now_utc().unix_timestamp();
            if let Some(trader) = trading_engine_handle.traders.get(&market_target.asset) {
                trader
                    .send(MarketEvent::Ohlc(Candle {
                        open,
                        high,
                        low,
                        close,
                        volume,
                        timestamp,
                    }))
                    .inspect_err(|why| tracing::error!("Failed to send market event: {why}"));
            }
        }
    });
    std::thread::park();
}
