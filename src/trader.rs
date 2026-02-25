use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    thread::sleep,
    time::Duration,
};

use bon::Builder;
use crossbeam::channel::tick;
use parking_lot::Mutex;
use tracing::{instrument, warn};
use uuid::Uuid;

use crate::{
    indicators::Ohlc,
    order_engine::OrderEvent,
    portfolio::{Portfolio, Position},
    strategies::{SignalGenerator, SystemCtx, TradeSignal},
    Command, MarketEvent, MarketPair,
};

#[derive(Debug, Clone)]
pub struct Candle {
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: i64,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct MarketData {
    pub candles: VecDeque<Candle>,
}

impl MarketData {
    pub fn new() -> MarketData {
        MarketData {
            candles: VecDeque::new(),
        }
    }
}

/// # Trader
/// A trader handles trades for a given market pair, acting on that market only.
/// It can receive commands and send out orders via flume channels.
#[derive(Builder)]
pub struct Trader {
    pub engine_id: Uuid,
    pub market_pair: MarketPair,
    pub market_data: MarketData,
    pub portfolio: Arc<Mutex<Portfolio>>,
    pub order_sender: flume::Sender<OrderEvent>,
    pub command_recv: flume::Receiver<Command>,

    // interval the trader will run over.
    tick_rate: Duration,
    // the logic for the trader to run.
    strategy: Box<dyn SignalGenerator + Send>,
}

pub struct TraderConfig {
    pub exit_recv: flume::Receiver<bool>,
    pub market_event_recv: flume::Receiver<MarketEvent>,
}

impl Trader {
    /// starts the traders event loop.
    #[instrument(skip(self, config), fields(ticker = self.market_pair.asset))]
    pub fn start(mut self, config: TraderConfig) {
        tracing::info!("Starting trader");

        let TraderConfig {
            exit_recv,
            market_event_recv,
        } = config;
        let ticker = tick(self.tick_rate);

        // NON TERMINATING
        'strategy_loop: loop {
            if let Ok(should_stop) = exit_recv.try_recv() {
                if should_stop {
                    tracing::warn!("Fully stopping trader");
                    break;
                }
            }
            if let Ok(Command::ForceExit) = self.command_recv.try_recv() {
                tracing::warn!("STOPPING");
                break;
            }

            if let Ok(MarketEvent::Ohlc(ohlc)) = market_event_recv.try_recv() {
                self.market_data.candles.push_front(ohlc);
            }

            if ticker.try_recv().is_ok() {
                let ptf = self.portfolio.lock();

                let Some(position) = ptf.positions.get(&self.market_pair) else {
                    warn!("No position found for {:?}", self.market_pair);
                    continue;
                };

                let ctx = SystemCtx {
                    position: position.clone(),
                    market_pair: self.market_pair.clone(),
                    market_data: self.market_data.clone(),
                };

                drop(ptf);

                if let Some(signal) = self.strategy.generate_signal(ctx) {
                    tracing::trace!("matching signal to order event");
                    let event = match signal {
                        TradeSignal::Close => {
                            OrderEvent::Close(self.market_pair.clone(), self.portfolio.clone())
                        }

                        TradeSignal::Long => {
                            OrderEvent::Long(self.market_pair.clone(), self.portfolio.clone())
                        }

                        TradeSignal::Short => {
                            OrderEvent::Short(self.market_pair.clone(), self.portfolio.clone())
                        }
                    };
                    let _ = self
                        .order_sender
                        .send(event)
                        .inspect_err(|why| tracing::error!("{why}"));
                };
            }
            sleep(Duration::from_millis(10));
        }
    }
}

/// handle to send market data to.
#[derive(Clone)]
pub struct TradingEngineHandle {
    pub traders: HashMap<String, flume::Sender<MarketEvent>>,
}

#[derive(Builder)]
/// Holds the main logic for traders.
/// Each trader is spawned on a new OS thread.
/// * `engine_id`:
/// * `traders`:
pub struct TradingEngine {
    pub engine_id: Uuid,
    pub traders: Vec<Trader>,
}

impl TradingEngine {
    pub fn start(&mut self) -> anyhow::Result<TradingEngineHandle> {
        let traders = std::mem::take(&mut self.traders);

        // handles to send market data.
        let mut traders_map = HashMap::with_capacity(traders.len());
        let (stop_tx, stop_rx) = flume::bounded::<bool>(1);

        let thread_pool = rayon::ThreadPoolBuilder::new().num_threads(2).build()?;

        for trader in traders {
            let exit_recv = stop_rx.clone();
            let asset = trader.market_pair.asset.clone();
            let (market_event_send, market_event_recv) = flume::bounded::<MarketEvent>(512);

            thread_pool.spawn(move || {
                trader.start(TraderConfig {
                    exit_recv,
                    market_event_recv,
                });
            });

            tracing::trace!("Inserting handles into stores");
            traders_map.insert(asset, market_event_send);
        }

        Ok(TradingEngineHandle {
            traders: traders_map,
        })
    }
}
