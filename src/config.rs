use std::{collections::HashMap, fs};

use serde::Deserialize;

use crate::{trader::Trader, MarketPair};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum StrategyChoice {
    Rsi,
    Simple,
}

#[derive(Debug, Deserialize)]
struct StrategyField {
    pub name: StrategyChoice,
    pub options: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct ConfigBracket {
    pub asset: String,
    pub base: String,
    pub strategy: StrategyField,
}

pub type TraderMap = HashMap<MarketPair, Trader>;

pub fn load_config(path: &str) -> TraderMap {
    let config = fs::read_to_string(path).expect("failed to read config");

    let json_config: Vec<ConfigBracket> =
        serde_json::from_str(&config).expect("failed to derive from str");
    dbg!(json_config);
    TraderMap::new()
}

#[cfg(test)]
mod tests {
    use super::load_config;

    #[test]
    fn test_load_config() {
        let conf = load_config("./config.json");
    }
}
