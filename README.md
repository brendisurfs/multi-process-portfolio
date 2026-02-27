# Multi-Process Portfolio Demo

A market-pair portfolio trading system based on the work I did for **Trades by Cortex**, the automated trading firm I co-founded.
The actual production system is proprietary; this is a stripped-down demonstration of the concurrency architecture and design principles that underpin it.

--- 

## Problem 

Trading systems have the fundamental challenge of being CPU-bound by strategy evaluation, while order exectuion and data ingestion is I/O bound. 
Naive systems serialize these stages or place it all within a single async runtime, both of which are wrong. 
This codebase demonstrates the architecture we built to keep those steps within a trading loop decoupled.

## Design Decisions 

### 1. Rayon for traders, not Tokio

Traders evaluate strategies on each tick. This is purely CPU work, no I/O. 
Putting this on a Tokio worker thread wastes an async slot and adds a scheduling overhead. 
Rayon's work-stealing pool is the right choice for bounded CPU parallelism with backpressure. 

The threadpool is explicitly sized (`num_threads(2)`). 
In production, this was expanded to the max number of cores available. 

### 2. `flume` as the sync/async bridge. 

### 3. Lock discipline

In `trader.rs`, the portfolio lock is acquired only to read the current position, then explicitely dropped before the strategy runs.

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

### 4. `parking_lot::Mutex` over `std::sync::Mutex`

`parking_lot`'s mutex is not poisonable (no unwrap required), and has the `deadlock_detection` feature, enabling us to catch lock ordering bugs during development.


### 5. Direct market data dispatch: O(1), not fan-out

The `TradingEngineHandle` is a `HashMap<String, Sender<MarketEvent>>`. 
The market data generator routes directly to the relevant trader's channel by key.
This means there is no broadcast, no filtering per receiver, and no wasted sends. 
In a larger system handling thousands of markets simultaniously, eliminating fan-out is meaningful.


### 6. Pluggable strategies via trait objects


## Difference Relative to Production System


