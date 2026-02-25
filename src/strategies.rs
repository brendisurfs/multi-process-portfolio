use std::{sync::Arc, time::Duration};

use parking_lot::Mutex;

use crate::{
    portfolio::{Portfolio, Position},
    trader::MarketData,
    MarketPair,
};

pub enum TradeSignal {
    Long,
    Short,
    Close,
}

/// Systems context for data, etc.
pub struct SystemCtx {
    pub position: Position,
    pub market_pair: MarketPair,
    pub market_data: MarketData,
}

pub trait SignalGenerator {
    fn generate_signal(&mut self, ctx: SystemCtx) -> Option<TradeSignal>;
}

pub struct StrategyParams {
    pub duration: Duration,
}

pub struct SimpleStrat {}

impl SignalGenerator for SimpleStrat {
    fn generate_signal(&mut self, ctx: SystemCtx) -> Option<TradeSignal> {
        if ctx.market_data.candles.is_empty() {
            tracing::warn!("Empty candles");
            return None;
        }

        if ctx.position.size == 0 {
            tracing::info!("Sending Long signal");
            return Some(TradeSignal::Long);
        }
        None
    }
}

pub struct Rsi {
    pub period: usize,
}

impl SignalGenerator for Rsi {
    fn generate_signal(&mut self, ctx: SystemCtx) -> Option<TradeSignal> {
        let candles = &ctx.market_data.candles;
        if candles.is_empty() {
            tracing::warn!("Empty candles");
            return None;
        }

        match ctx.position.size {
            _ => {
                tracing::trace!("Has position");
                let Some(first_candle) = ctx.market_data.candles.front() else {
                    return None;
                };

                if ctx.position.price > first_candle.close {
                    return Some(TradeSignal::Close);
                }
            }
            0 => {
                tracing::info!("Sending Short signal");
                return Some(TradeSignal::Short);
            }
        }

        None
    }
}
