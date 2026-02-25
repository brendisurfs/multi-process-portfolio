use std::collections::HashMap;

use bon::Builder;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Command, MarketPair};

enum DataKind {
    Candle(String),
}

struct MarketEvent {
    time: OffsetDateTime,
    kind: DataKind,
}

trait MarketUpdater {
    fn update_from_market(&mut self, market_event: &MarketEvent);
}

#[derive(PartialEq, Debug, Clone)]
pub struct Position {
    pub size: i32,
    pub price: f32,
}

#[derive(Builder)]
pub struct Portfolio {
    pub engine_id: Uuid,
    pub markets: Vec<MarketPair>,
    pub positions: HashMap<MarketPair, Position>,
}

impl Portfolio {
    /// closes a position.
    pub fn close_position(&mut self, market_pair: &MarketPair) {
        tracing::trace!("Closing position");
        self.positions.remove(market_pair);
    }
}
