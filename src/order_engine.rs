use std::{collections::hash_map::Entry, sync::Arc, time::Duration};

use parking_lot::Mutex;
use rand::Rng;
use tracing::info;

use crate::{
    portfolio::{Portfolio, Position},
    MarketPair,
};

pub enum OrderEvent {
    Reverse(MarketPair),
    Long(MarketPair, Arc<Mutex<Portfolio>>),
    Short(MarketPair, Arc<Mutex<Portfolio>>),
    Close(MarketPair, Arc<Mutex<Portfolio>>),
}

pub struct OrderEngine;

impl OrderEngine {
    pub fn start(self, order_rx: flume::Receiver<OrderEvent>) {
        tracing::trace!("Building order runtime");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build runtime");

        tracing::trace!("Starting order handle thread");
        let engine_handle = std::thread::spawn(move || {
            info!("Starting order handler");

            rt.block_on(async move {
                // we receive an order request.
                // We place that order, wait for it to fill in its own thread.
                while let Ok(event) = order_rx.recv_async().await {
                    match event {
                        OrderEvent::Close(pair, portfolio) => {
                            tracing::trace!(pair = pair.asset, "Received close");
                            let mut ptf = portfolio.lock();
                            ptf.close_position(&pair);
                            drop(ptf);
                        }

                        OrderEvent::Short(market_pair, portfolio) => {
                            info!(pair = market_pair.asset, "Spawning new SHORT handler");
                            // spawn a new task to handle selling and logic.
                            tokio::spawn(async move {
                                // Simulate the broker fill interaction.
                                tokio::time::sleep(Duration::from_millis(300)).await;

                                let mut price_generator = rand::thread_rng();
                                let price = price_generator.gen_range(10.0..=20.0);

                                // LOCK AFTER.
                                let mut ptf = portfolio.lock();
                                if let Entry::Vacant(e) = ptf.positions.entry(market_pair) {
                                    e.insert(Position { price, size: -1 });
                                    info!("SHORT position");
                                }
                                drop(ptf);
                            });
                        }

                        OrderEvent::Long(market_pair, portfolio) => {
                            info!(pair = market_pair.asset, "Spawning new LONG handler");
                            tokio::spawn(async move {
                                tokio::time::sleep(Duration::from_millis(300)).await;

                                let mut price_generator = rand::thread_rng();
                                let price = price_generator.gen_range(10.0..=20.0);

                                let mut ptf = portfolio.lock();
                                if let Entry::Vacant(e) = ptf.positions.entry(market_pair) {
                                    e.insert(Position { price, size: 1 });
                                    info!("Bought position");
                                }
                                drop(ptf);
                            });
                        }
                        OrderEvent::Reverse(mp) => {
                            println!("Spawning new reverse handler")
                        }
                    }
                }
            });
        });
    }
}
