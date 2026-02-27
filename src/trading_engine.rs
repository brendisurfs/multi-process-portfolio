use std::collections::HashMap;

use bon::Builder;
use uuid::Uuid;

use crate::{
    trader::{Trader, TraderConfig},
    MarketEvent,
};

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

        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(traders.len())
            .build()?;

        for trader in traders {
            let exit_recv = stop_rx.clone();
            let asset = trader.market_pair.asset.clone();
            let (market_event_send, market_event_recv) = flume::bounded::<MarketEvent>(2048);

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
