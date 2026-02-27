# Multi-Process Portfolio Demo

A market-pair portfolio trading system based on the work I did for **Trades by Cortex**, the automated trading firm I co-founded.
The actual production system is proprietary; this is a stripped-down demonstration of the concurrency architecture and design principles that underpin it.

--- 

## Problem 

Trading systems have the fundamental challenge of being CPU-bound by strategy evaluation, while order execution and data ingestion is I/O bound. 
Naive systems serialize these stages or place it all within a single async runtime, both of which are wrong. 
This codebase demonstrates the architecture we built to keep those steps within a trading loop decoupled.

## Architecture

```
                         ┌─────────────────┐
                         │   Main Thread   │
                         │   (parked)      │
                         └────────┬────────┘
                                  │ spawns
          ┌───────────────────────┼───────────────────────┐
          │                       │                       │
          ▼                       ▼                       ▼
┌──────────────────┐   ┌───────────────────┐   ┌──────────────────────┐
│  Market Data     │   │  Rayon Thread     │   │  Order Engine        │
│  Generator       │   │  Pool (2 threads) │   │  OS thread +         │
│  OS thread       │   │                   │   │  tokio single-thread │
│                  │   │  ┌─────────────┐  │   │  runtime             │
│  tick(100ms)     │   │  │  SUI Trader │  │   │                      │
│  random OHLC     │   │  │  RSI / 5s   │  │   │  recv_async()        │
│                  │   │  ├─────────────┤  │   │                      │
│                  │   │  │  SOL Trader │  │   │  tokio::spawn        │
│                  │   │  │  Simple/ 2s │  │   │  per order           │
│                  │   │  ├─────────────┤  │   │                      │
│                  │   │  │  BTC Trader │  │   │                      │
│                  │   │  │  RSI / 15s  │  │   │                      │
│                  │   │  └─────────────┘  │   │                      │
└────────┬─────────┘   └────────┬──────────┘   └──────────┬───────────┘
         │                      │                         │
         │ flume::bounded(512)  │ flume::bounded(128)     │
         │ per-asset channel    └────────── OrderEvent ──►│
         │                                                │
         │  TradingEngineHandle                           │
         │  HashMap<asset, Sender<MarketEvent>>           │
         └──────────────── MarketEvent ──────────────────►(each trader)

                     Arc<Mutex<Portfolio>>
              shared across all traders and order engine
```

**Data flows:**
- Market data generator dispatches OHLC candles directly to the target trader's channel by asset key — no broadcast, no filtering
- Traders emit `OrderEvent` to the shared order channel on each strategy signal
- The order engine acquires the portfolio lock only after simulated broker fill, minimizing contention

---

## Design Decisions 

### 1. Rayon for traders, not Tokio

Traders evaluate strategies on each tick. This is purely CPU work, no I/O. 
Putting this on a Tokio worker thread wastes an async slot and adds a scheduling overhead. 
Rayon's work-stealing pool is the right choice for bounded CPU parallelism with backpressure. 

The threadpool is explicitly sized (`num_threads(2)`). 
In production, this was expanded to the max number of cores available. 

### 2. `flume` as the sync/async bridge. 

`std::sync::mpsc` does not provide an async recv, and `tokio::sync::mpsc` requires an async context to send, which we do not have inside Rayon threads.
`flume` is the only crate that works in both sync and async contexts.

### 3.  Bounded channels 

Backpressure is explicit, not hidden. If the order engine falls behind, we expose ourselves to avoidable losses.

### 4. Lock discipline

In `trader.rs`, the portfolio lock is acquired only to read the current position, then explicitly dropped before the strategy runs.

 ```rust
let ptf = self.portfolio.lock();
let Some(position) = ptf.positions.get(&self.market_pair) else { ... };
let ctx = SystemCtx { position: position.clone(), ... };
drop(ptf);  // <-- released before any strategy work

if let Some(signal) = self.strategy.generate_signal(ctx) { ... }

```

Strategies can be arbitrarily expensive, with some strategies utilizing on-demand indicator calculation. 
Holding a lock through signal generation would serialize all traders on every tick. 
Cloning the position and dropping early keeps lock hold time constant and contention minimal. 

### 5. `parking_lot::Mutex` over `std::sync::Mutex`

`parking_lot`'s `Mutex` is faster than `std::sync::Mutex` under low-moderate contention due to its adaptive spin strategy. 
In a latency-sensitive situation, this has a legitimate impact on performance. 
It is also not poisonable (no unwrap required), and by using the `deadlock_detection` feature, we can catch lock ordering bugs during development.

### 6. Direct market data dispatch: O(1), not fan-out

The `TradingEngineHandle` is a `HashMap<String, Sender<MarketEvent>>`. 
The market data generator routes directly to the relevant trader's channel by key.
This means there is no broadcast, no filtering per receiver, and no wasted sends. 
In a larger system handling thousands of markets simultaneously, eliminating fan-out is meaningful.

### 7. Pluggable strategies via trait objects

```rust
pub trait SignalGenerator {
fn generate_signal(&mut self, ctx: SystemCtx) -> Option<TradeSignal>;
}
```

Each `Trader` holds a `Box<dyn SignalGenerator + Send>`. In production, strategies vary significantly per market (RSI, Heikin-Ashi smoothed momentum, etc). The trait boundary also lets strategy state be mutable per tick, without exposing it to the rest of the system.

## Difference Relative to Production System

- **Robust error handling** - Traders that error are not removed from the pool.
- **Live broker connectivity** - Replaced here with a simulated fill delay. Production is connected to exchange Websocket feeds and REST APIs.
- **Persistent portfolio state** - Positions are held in memory only. Production reconciled state against the broker on both startup and in intervals throughout the day, with state being written to a durable store. 
- **Risk management layer** - This would be a separate process that checks all orders against portfolio exposure limits before they reached the order engine.
 
