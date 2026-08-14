use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

// ============ ENHANCED DATA STRUCTURES ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineData {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalIndicators {
    pub rsi: f64,
    pub macd: f64,
    pub signal_line: f64,
    pub histogram: f64,
    pub ema_short: f64,  // 9-period
    pub ema_long: f64,   // 21-period
    pub bb_upper: f64,   // Bollinger Band upper
    pub bb_lower: f64,
    pub bb_middle: f64,
    pub volume_sma: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StrategySignal {
    StrongBuy,
    Buy,
    Sell,
    StrongSell,
    Hold,
}

#[derive(Debug, Clone)]
pub struct EnhancedTradingSignal {
    pub signal: StrategySignal,
    pub confidence: f64,  // 0.0 to 1.0
    pub indicators: TechnicalIndicators,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
}

// TECHNICAL INDICATORS
pub struct StrategyAnalyzer;

impl StrategyAnalyzer {
    /// Calculate Exponential Moving Average
    pub fn calculate_ema(prices: &[f64], period: usize) -> Result<Vec<f64>> {
        if prices.len() < period {
            return Err(anyhow!("Not enough data for EMA (need {}, got {})", period, prices.len()));
        }

        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut ema_values = Vec::with_capacity(prices.len());

        // SMA for first value
        let sma: f64 = prices[..period].iter().sum::<f64>() / period as f64;
        ema_values.push(sma);

        // Calculate EMA
        for i in period..prices.len() {
            let ema = (prices[i] - ema_values[i - period]) * multiplier + ema_values[i - period];
            ema_values.push(ema);
        }

        Ok(ema_values)
    }

    /// Calculate RSI (Relative Strength Index)
    pub fn calculate_rsi(prices: &[f64], period: usize) -> Result<Vec<f64>> {
        if prices.len() < period + 1 {
            return Err(anyhow!("Not enough data for RSI (need {}, got {})", period + 1, prices.len()));
        }

        let mut rsi_values = Vec::with_capacity(prices.len() - period);
        let mut gains = Vec::new();
        let mut losses = Vec::new();

        for i in 1..prices.len() {
            let change = prices[i] - prices[i - 1];
            if change > 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(-change);
            }
        }

        let mut avg_gain: f64 = gains[..period].iter().sum::<f64>() / period as f64;
        let mut avg_loss: f64 = losses[..period].iter().sum::<f64>() / period as f64;

        for i in period..gains.len() {
            avg_gain = (avg_gain * (period - 1) as f64 + gains[i]) / period as f64;
            avg_loss = (avg_loss * (period - 1) as f64 + losses[i]) / period as f64;

            let rsi = if avg_loss == 0.0 {
                if avg_gain == 0.0 { 50.0 } else { 100.0 }
            } else {
                let rs = avg_gain / avg_loss;
                100.0 - (100.0 / (1.0 + rs))
            };

            rsi_values.push(rsi);
        }

        Ok(rsi_values)
    }

    /// Calculate MACD (Moving Average Convergence Divergence)
    pub fn calculate_macd(prices: &[f64]) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>)> {
        let ema_12 = Self::calculate_ema(prices, 12)?;
        let ema_26 = Self::calculate_ema(prices, 26)?;
        
        let padding_needed = ema_12.len() - ema_26.len();
        let mut aligned_ema_26 = vec![f64::NAN; padding_needed];
        aligned_ema_26.extend(ema_26);
        
        // MACD line = EMA12 - EMA26 (only valid where both are defined)
        let macd_line: Vec<f64> = ema_12
            .iter()
            .zip(aligned_ema_26.iter())
            .map(|(e12, e26)| {
                if e26.is_nan() {
                    f64::NAN  // Invalid MACD before EMA26 stabilizes
                } else {
                    e12 - e26
                }
            })
            .collect();
        
        // Signal line = EMA9 of MACD (only calculate from valid MACD values)
        // Find first valid (non-NaN) MACD value
        let first_valid_idx = macd_line.iter().position(|&x| !x.is_nan()).unwrap_or(0);
        let valid_macd = &macd_line[first_valid_idx..];
        let signal_line = if valid_macd.len() >= 9 {
            Self::calculate_ema(valid_macd, 9)?
        } else {
            return Err(anyhow!("Not enough valid MACD data to calculate signal line"));
        };
        
        // Pad signal line to match full MACD length
        let signal_padding = first_valid_idx + (valid_macd.len() - signal_line.len());
        let mut padded_signal = vec![f64::NAN; signal_padding];
        padded_signal.extend(signal_line);
        
        // Histogram = MACD - Signal
        let histogram: Vec<f64> = macd_line
            .iter()
            .zip(padded_signal.iter())
            .map(|(m, s)| {
                if m.is_nan() || s.is_nan() {
                    f64::NAN
                } else {
                    m - s
                }
            })
            .collect();
            
        Ok((macd_line, padded_signal, histogram))
    }

    /// Calculate Bollinger Bands
    pub fn calculate_bollinger_bands(prices: &[f64], period: usize, std_dev: f64) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>)> {
        if prices.len() < period {
            return Err(anyhow!("Not enough data for Bollinger Bands"));
        }

        let mut upper = Vec::with_capacity(prices.len());
        let mut middle = Vec::with_capacity(prices.len());
        let mut lower = Vec::with_capacity(prices.len());

        for i in period..=prices.len() {
            let window = &prices[i - period..i];
            let sma: f64 = window.iter().sum::<f64>() / period as f64;
            let variance: f64 = window
                .iter()
                .map(|&p| (p - sma).powi(2))
                .sum::<f64>() / period as f64;
            let sigma = variance.sqrt();

            middle.push(sma);
            upper.push(sma + (std_dev * sigma));
            lower.push(sma - (std_dev * sigma));
        }

        Ok((upper, middle, lower))
    }

    /// Calculate Volume SMA for volume filter
    pub fn calculate_volume_sma(volumes: &[f64], period: usize) -> Result<Vec<f64>> {
        if volumes.len() < period {
            return Err(anyhow!("Not enough volume data"));
        }

        let mut volume_sma = Vec::with_capacity(volumes.len() - period + 1);
        
        for i in period..=volumes.len() {
            let window = &volumes[i - period..i];
            let avg: f64 = window.iter().sum::<f64>() / period as f64;
            volume_sma.push(avg);
        }

        Ok(volume_sma)
    }

    /// Comprehensive strategy analysis with multi-indicator confirmation
    pub fn analyze(klines: &[KlineData], config: &StrategyConfig) -> Result<EnhancedTradingSignal> {
        if klines.len() < 30 {
            return Err(anyhow!("Minimum 30 klines required for full analysis"));
        }

        let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
        let volumes: Vec<f64> = klines.iter().map(|k| k.volume).collect();

        // Calculate all indicators
        let rsi_values = Self::calculate_rsi(&closes, config.rsi_period)?;
        let (macd_line, signal_line, histogram) = Self::calculate_macd(&closes)?;
        let ema_short = Self::calculate_ema(&closes, config.ema_short)?;
        let ema_long = Self::calculate_ema(&closes, config.ema_long)?;
        let (bb_upper, bb_middle, bb_lower) = Self::calculate_bollinger_bands(&closes, config.bb_period, 2.0)?;
        let volume_sma = Self::calculate_volume_sma(&volumes, config.volume_period)?;

        // Get current values
        let current_rsi = *rsi_values.last().ok_or_else(|| anyhow!("No RSI data"))?;
        let current_macd = *macd_line.last().ok_or_else(|| anyhow!("No MACD data"))?;
        let current_signal = *signal_line.last().ok_or_else(|| anyhow!("No Signal data"))?;
        let current_histogram = *histogram.last().ok_or_else(|| anyhow!("No Histogram data"))?;
        let current_ema_short = *ema_short.last().ok_or_else(|| anyhow!("No EMA9"))?;
        let current_ema_long = *ema_long.last().ok_or_else(|| anyhow!("No EMA21"))?;
        let current_bb_upper = *bb_upper.last().ok_or_else(|| anyhow!("No BB"))?;
        let current_bb_middle = *bb_middle.last().ok_or_else(|| anyhow!("No BB middle"))?;
        let current_bb_lower = *bb_lower.last().ok_or_else(|| anyhow!("No BB lower"))?;
        let current_volume = volumes[volumes.len() - 1];
        let current_volume_sma = *volume_sma.last().ok_or_else(|| anyhow!("No Volume SMA"))?;
        let current_price = *closes.last().ok_or_else(|| anyhow!("No price data"))?;

        // Get previous values for trend analysis
        let prev_histogram = if histogram.len() > 1 {
            histogram[histogram.len() - 2]
        } else {
            0.0
        };

        let indicators = TechnicalIndicators {
            rsi: current_rsi,
            macd: current_macd,
            signal_line: current_signal,
            histogram: current_histogram,
            ema_short: current_ema_short,
            ema_long: current_ema_long,
            bb_upper: current_bb_upper,
            bb_lower: current_bb_lower,
            bb_middle: current_bb_middle,
            volume_sma: current_volume_sma,
        };

        // Scoring system (0-5 for each condition)
        let mut buy_score: f64 = 0.0;
        let mut sell_score: f64 = 0.0;

        // ============ BUY SIGNALS ============

        // 1. EMA Trend: Price above EMA9, EMA9 above EMA21 (uptrend)
        if current_price > current_ema_short && current_ema_short > current_ema_long {
            buy_score += 1.5;
        }

        // 2. RSI Confirmation: Oversold (< 40) or rising from oversold (30-50)
        if current_rsi < 40.0 {
            buy_score += 1.5;
        } else if current_rsi < 50.0 && rsi_values.len() > 1 && rsi_values[rsi_values.len() - 2] < current_rsi {
            buy_score += 1.0;
        }

        // 3. MACD Confirmation: MACD above signal & histogram positive & rising
        if current_macd > current_signal && current_histogram > 0.0 && current_histogram > prev_histogram {
            buy_score += 1.5;
        }

        // 4. Volume Filter: Current volume > 20-period SMA
        if current_volume > current_volume_sma * 1.2 {
            buy_score += 0.5;
        }

        // 5. Bollinger Bands: Price touching lower band (oversold)
        if current_price < current_bb_lower * 1.01 {
            buy_score += 1.0;
        }

        // ============ SELL SIGNALS ============

        // 1. EMA Trend: Price below EMA9, EMA9 below EMA21 (downtrend)
        if current_price < current_ema_short && current_ema_short < current_ema_long {
            sell_score += 1.5;
        }

        // 2. RSI Confirmation: Overbought (> 60) or falling from overbought (50-70)
        if current_rsi > 60.0 {
            sell_score += 1.5;
        } else if current_rsi > 50.0 && rsi_values.len() > 1 && rsi_values[rsi_values.len() - 2] > current_rsi {
            sell_score += 1.0;
        }

        // 3. MACD Confirmation: MACD below signal & histogram negative & falling
        if current_macd < current_signal && current_histogram < 0.0 && current_histogram < prev_histogram {
            sell_score += 1.5;
        }

        // 4. Volume Filter: Current volume > 20-period SMA
        if current_volume > current_volume_sma * 1.2 {
            sell_score += 0.5;
        }

        // 5. Bollinger Bands: Price touching upper band (overbought)
        if current_price > current_bb_upper * 0.99 {
            sell_score += 1.0;
        }

        // DECISION LOGIC
        let (signal, confidence) = if buy_score > sell_score + 1.5 {
            // Clear BUY signal with strong confidence
            if buy_score >= 4.5 {
                (StrategySignal::StrongBuy, (buy_score / 7.0).min(1.0))
            } else {
                (StrategySignal::Buy, (buy_score / 7.0).min(1.0))
            }
        } else if sell_score > buy_score + 1.5 {
            // Clear SELL signal with strong confidence
            if sell_score >= 4.5 {
                (StrategySignal::StrongSell, (sell_score / 7.0).min(1.0))
            } else {
                (StrategySignal::Sell, (sell_score / 7.0).min(1.0))
            }
        } else {
            // Indecisive/conflicting signals - Hold with calculated confidence
            // Use the higher score to show which direction is slightly favored
            let hold_confidence = if buy_score > sell_score {
                (buy_score / 7.0) * 0.5  // 50% of buy confidence
            } else if sell_score > buy_score {
                (sell_score / 7.0) * 0.5  // 50% of sell confidence
            } else {
                0.25  // Equal scores = very low confidence
            };
            (StrategySignal::Hold, hold_confidence.min(1.0))
        };

        // ============ RISK MANAGEMENT ============

        let atr = Self::calculate_atr(klines, 14)?;
        let stop_loss = if signal == StrategySignal::Buy || signal == StrategySignal::StrongBuy {
            current_price - (atr * 2.0)
        } else if signal == StrategySignal::Sell || signal == StrategySignal::StrongSell {
            current_price + (atr * 2.0)
        } else {
            current_price
        };

        let take_profit = if signal == StrategySignal::Buy || signal == StrategySignal::StrongBuy {
            current_price + (atr * 3.0)
        } else if signal == StrategySignal::Sell || signal == StrategySignal::StrongSell {
            current_price - (atr * 3.0)
        } else {
            current_price
        };

        info!(
            "📊 Enhanced Strategy: {} | Confidence: {:.2}% | Price: ${:.2} | RSI: {:.2} | MACD: {:.6} | EMA9: ${:.2} | EMA21: ${:.2}",
            format!("{:?}", signal),
            confidence * 100.0,
            current_price,
            current_rsi,
            current_macd,
            current_ema_short,
            current_ema_long
        );

        Ok(EnhancedTradingSignal {
            signal,
            confidence: confidence.min(1.0),
            indicators,
            entry_price: current_price,
            stop_loss,
            take_profit,
        })
    }

    /// Calculate Average True Range (ATR) for volatility
    fn calculate_atr(klines: &[KlineData], period: usize) -> Result<f64> {
        if klines.len() < period {
            return Err(anyhow!("Not enough data for ATR"));
        }

        let mut true_ranges = Vec::new();

        for i in 1..klines.len() {
            let high = klines[i].high;
            let low = klines[i].low;
            let close_prev = klines[i - 1].close;

            let tr = (high - low)
                .max((high - close_prev).abs())
                .max((low - close_prev).abs());

            true_ranges.push(tr);
        }

        let atr: f64 = true_ranges[true_ranges.len() - period..]
            .iter()
            .sum::<f64>() / period as f64;

        Ok(atr)
    }
}

// ============ CONFIGURATION ============

#[derive(Debug, Clone)]
pub struct StrategyConfig {
    pub rsi_period: usize,
    pub ema_short: usize,
    pub ema_long: usize,
    pub bb_period: usize,
    pub volume_period: usize,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            rsi_period: 14,
            ema_short: 9,
            ema_long: 21,
            bb_period: 20,
            volume_period: 20,
        }
    }
}