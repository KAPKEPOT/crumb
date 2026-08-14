use crate::strategy::{StrategySignal, TechnicalIndicators};
use serde::{Deserialize, Serialize};

/// Unified trading signal with all analysis data
/// Single source of truth for signal data across the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSignal {
    pub symbol: String,
    pub signal: StrategySignal,
    pub confidence: f64,                    // 0.0 to 1.0
    pub indicators: TechnicalIndicators,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub timestamp: i64,
}

impl TradingSignal {
    /// Create a new trading signal
    pub fn new(
        symbol: String,
        signal: StrategySignal,
        confidence: f64,
        indicators: TechnicalIndicators,
        entry_price: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Self {
        Self {
            symbol,
            signal,
            confidence,
            indicators,
            entry_price,
            stop_loss,
            take_profit,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}