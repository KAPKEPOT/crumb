use anyhow::{anyhow, Result};
use dotenvy::dotenv;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use tracing::{error, info, warn};

mod binance_client;
mod strategy;
mod types;
use crate::binance_client::RealBinanceClient;

// CONFIGURATION 
#[derive(Debug, Clone)]
struct Config {
    rsi_period: usize,
    kline_limit: usize,
    request_timeout_secs: u64,
    rate_limit_secs: u64,
}

impl Config {
    fn from_env() -> Result<Self> {
        Ok(Self {
            rsi_period: std::env::var("RSI_PERIOD")
                .unwrap_or_else(|_| "14".to_string())
                .parse()
                .unwrap_or(14),
            kline_limit: std::env::var("KLINE_LIMIT")
                .unwrap_or_else(|_| "50".to_string())
                .parse()
                .unwrap_or(50),
            request_timeout_secs: std::env::var("REQUEST_TIMEOUT_SECS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            rate_limit_secs: std::env::var("RATE_LIMIT_SECS")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .unwrap_or(2),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BinanceKline {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Position {
    symbol: String,
    amount: f64,
    entry_price: f64,
    timestamp: i64,
}

// TELEGRAM COMMANDS
#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    Start,
    Balance,
    Positions,
    Analyze,
    Buy { amount: f64 },
    Sell { amount: f64 },
    Status,
    Help,
}

// RATE LIMITER
#[derive(Clone)]
struct RateLimiter {
    limits: Arc<tokio::sync::Mutex<HashMap<u64, SystemTime>>>,
    cooldown: Duration,
}

impl RateLimiter {
    fn new(cooldown_secs: u64) -> Self {
        Self {
            limits: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            cooldown: Duration::from_secs(cooldown_secs),
        }
    }

    async fn check(&self, user_id: u64) -> Result<()> {
        let mut limits = self.limits.lock().await;
        
        if let Some(last_request) = limits.get(&user_id) {
            if last_request.elapsed().unwrap_or(Duration::from_secs(0)) < self.cooldown {
                return Err(anyhow!(
                    "Rate limit exceeded. Please wait {} seconds.",
                    self.cooldown.as_secs()
                ));
            }
        }
        
        limits.insert(user_id, SystemTime::now());
        Ok(())
    }
}

// ============ MAIN BOT STRUCTURE ============

struct TradingBot {
    binance_client: Client,
    real_binance: RealBinanceClient,
    telegram_bot: Bot,
    user_id: u64,
    symbol: String,
    position_size: f64,
    config: Config,
    rate_limiter: RateLimiter,
    positions: Arc<tokio::sync::Mutex<Vec<Position>>>,
}

impl TradingBot {
    async fn new(bot: Bot) -> Result<Self> {
        dotenv().ok();

        let config = Config::from_env()?;
        let user_id = std::env::var("TELEGRAM_USER_ID")?
            .parse::<u64>()
            .map_err(|_| anyhow!("Invalid TELEGRAM_USER_ID format"))?;
        let symbol = std::env::var("TRADING_SYMBOL")?;
        let position_size = std::env::var("POSITION_SIZE")?
            .parse::<f64>()
            .map_err(|_| anyhow!("Invalid POSITION_SIZE format"))?;

        if position_size <= 0.0 {
            return Err(anyhow!("POSITION_SIZE must be greater than 0"));
        }
        
        // Create real Binance client
        let real_binance = RealBinanceClient::new()?;

        Ok(Self {
            binance_client: Client::builder()
                .timeout(Duration::from_secs(config.request_timeout_secs))
                .build()?,
            real_binance,
            telegram_bot: bot,
            user_id,
            symbol,
            position_size,
            config: config.clone(),
            rate_limiter: RateLimiter::new(config.rate_limit_secs),
            positions: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        })
    }

    // ============ BINANCE API METHODS ============

    async fn get_klines(&self, symbol: &str) -> Result<Vec<BinanceKline>> {
        let url = format!(
            "https://api.binance.com/api/v3/klines?symbol={}&interval=1h&limit={}",
            urlencoding::encode(symbol),
            self.config.kline_limit
        );

        let response = self
            .binance_client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to fetch klines: {}", e);
                anyhow!("Failed to fetch market data: {}", e)
            })?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Binance API error: {}",
                response.status()
            ));
        }

        let data: Vec<Vec<serde_json::Value>> = response
            .json()
            .await
            .map_err(|e| {
                error!("Failed to parse klines response: {}", e);
                anyhow!("Failed to parse market data")
            })?;

        let klines: Vec<BinanceKline> = data
            .iter()
            .filter_map(|k| {
                if k.len() < 5 {
                    warn!("Invalid kline data: insufficient fields");
                    return None;
                }

                let parse_value = |v: &serde_json::Value| -> Option<f64> {
                    v.as_str().and_then(|s| s.parse().ok())
                };

                Some(BinanceKline {
                    open: parse_value(&k[1])?,
                    high: parse_value(&k[2])?,
                    low: parse_value(&k[3])?,
                    close: parse_value(&k[4])?,
                    volume: parse_value(&k[7])?,
                })
            })
            .collect();

        if klines.is_empty() {
            return Err(anyhow!("No valid kline data received"));
        }

        Ok(klines)
    }
    
    async fn get_current_btc_price(&self) -> Result<f64> {
        let klines = self.get_klines("BTCUSDT").await?;
        if let Some(last_kline) = klines.last() {
            Ok(last_kline.close)
        } else {
            Err(anyhow!("No kline data available for BTC price"))
        }
    }
    
    async fn analyze_strategy(&self) -> Result<types::TradingSignal> {
        let klines = self.get_klines(&self.symbol).await?;
        
        // Convert to strategy module format
        let kline_data: Vec<strategy::KlineData> = klines
            .iter()
            .map(|k| strategy::KlineData {
                open: k.open,
                high: k.high,
                low: k.low,
                close: k.close,
                volume: k.volume,
            })
            .collect();
            
        let config = strategy::StrategyConfig::default();
        let trading_signal = strategy::StrategyAnalyzer::analyze(&self.symbol, &kline_data, &config)?;
        
        Ok(trading_signal)
    }

    // POSITION MANAGEMENT
    async fn add_position(&self, symbol: String, amount: f64, price: f64) -> Result<()> {
        let mut positions = self.positions.lock().await;
        positions.push(Position {
            symbol,
            amount,
            entry_price: price,
            timestamp: chrono::Utc::now().timestamp(),
        });
        Ok(())
    }

    async fn get_positions(&self) -> Result<Vec<Position>> {
        let positions = self.positions.lock().await;
        Ok(positions.clone())
    }

    // ============ HELPER METHODS ============

    fn escape_markdown(text: &str) -> String {
        text.replace('\\', "\\\\")
            .replace('_', "\\_")
            .replace('*', "\\*")
            .replace('[', "\\[")
            .replace(']', "\\]")
            .replace('(', "\\(")
            .replace(')', "\\)")
            .replace('~', "\\~")
            .replace('`', "\\`")
            .replace('!', "\\!")
            .replace('.', "\\.")
            .replace('>', "\\>")
            .replace('#', "\\#")
            .replace('+', "\\+")
            .replace('-', "\\-")
            .replace('=', "\\=")
            .replace('|', "\\|")
            .replace('{', "\\{")
            .replace('}', "\\}")
    }

    // TELEGRAM COMMAND HANDLERS
    async fn handle_start(&self, msg: &Message) -> Result<()> {
        let text = "🤖 *Trading Bot Started\\!*\n\nCommands:\n/balance \\- Check your balance\n/positions \\- View open positions\n/analyze \\- Get current market analysis\n/buy [amount] \\- Buy crypto (e.g., /buy 100)\n/sell [amount] \\- Sell crypto (e.g., /sell 50)\n/status \\- Bot status\n/help \\- Show this message";

        self.telegram_bot
            .send_message(msg.chat.id, Self::escape_markdown(text))
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .await?;

        Ok(())
    }

    async fn handle_balance(&self, msg: &Message) -> Result<()> {
        let usdt = self.real_binance.get_usdt_balance().await?;
        let btc = self.real_binance.get_btc_balance().await?;
        
        let text = format!(
            r#"💰 *Real Account Balance*

USDT: {:.2}
BTC: {:.8}
Total USDT Value: ~${:.2}"#,
            usdt,
            btc,
            usdt + (btc * self.get_current_btc_price().await?)
        );

        self.telegram_bot
            .send_message(msg.chat.id, Self::escape_markdown(&text))
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .await?;

        Ok(())
    }

    async fn handle_analyze(&self, msg: &Message) -> Result<()> {
        let signal = self.analyze_strategy().await?;
        let action_str = match signal.signal {
            strategy::StrategySignal::StrongBuy => "🟢 STRONG BUY",
            strategy::StrategySignal::Buy => "🟢 BUY",
            strategy::StrategySignal::Sell => "🔴 SELL",
            strategy::StrategySignal::StrongSell => "🔴 STRONG SELL",
            strategy::StrategySignal::Hold => "⏸️ HOLD",
        };
        
        let text = format!(
            "📊 *Market Analysis*\n\nSymbol: {}\nPrice: ${:.2}\nRSI: {:.2}\nMACD: {:.2}\nSignal: {}\nConfidence: {:.0}%\nStop Loss: ${:.2}\nTake Profit: ${:.2}",
            self.symbol,
            signal.entry_price,
            signal.indicators.rsi,
            signal.indicators.macd,
            action_str,
            signal.confidence * 100.0,
            signal.stop_loss,
            signal.take_profit
        );
        
        self.telegram_bot
            .send_message(msg.chat.id, Self::escape_markdown(&text))
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .await?;
        
        Ok(())
    }

    async fn handle_buy(&self, msg: &Message, amount: f64) -> Result<()> {
        // Validate amount
        if amount <= 0.0 || amount.is_nan() || amount.is_infinite() {
            return Err(anyhow!("Invalid amount: must be positive"));
        }
        
        // Get current BTC price
        let signal = self.analyze_strategy().await?;
        let btc_amount = amount / signal.entry_price;
        
        // Check if you have enough USDT
        let usdt_balance = self.real_binance.get_usdt_balance().await?;
        
        if amount > usdt_balance {
            return Err(anyhow!(
                "Insufficient USDT balance. You have {:.2}, trying to spend {:.2}",
                usdt_balance, amount
            ));
        }
        
        // Place REAL order!
        let order_id = self.real_binance.place_market_buy(&self.symbol, btc_amount).await?;
        
        // Add to positions
        self.add_position(self.symbol.clone(), btc_amount, signal.entry_price).await?;
        
        info!(" REAL BUY executed! Order ID: {}", order_id);
        
        let text = format!(
            r#"✅ *Real Buy Order Filled!*
Symbol: {}
Amount: {:.8} BTC
Spent: {:.2} USDT
Price: ${:.2}
Order ID: {}
⚠️ This is a REAL trade on the exchange!"#,
            self.symbol,
            btc_amount,
            amount,
            signal.entry_price,
            order_id
        );

        self.telegram_bot
            .send_message(msg.chat.id, Self::escape_markdown(&text))
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .await?;

        Ok(())
    }

    async fn handle_sell(&self, msg: &Message, amount: f64) -> Result<()> {
        // Validate amount
        if amount <= 0.0 || amount.is_nan() || amount.is_infinite() {
            return Err(anyhow!("Invalid amount: must be positive"));
        }

        let signal = self.analyze_strategy().await?;

        info!("SELL order placed - Symbol: {}, Amount: {:.2}, Price: ${:.2}", 
              self.symbol, amount, signal.entry_price);
        let text = format!(
            "✅ *Sell Order*\nAmount: {:.2} USDT\nSymbol: {}\nPrice: ${:.2}",
            amount, self.symbol, signal.entry_price
        );

        self.telegram_bot
            .send_message(msg.chat.id, Self::escape_markdown(&text))
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .await?;

        Ok(())
    }

    async fn handle_status(&self, msg: &Message) -> Result<()> {
        let text = format!(
            "🤖 *Bot Status*\n✅ Running\n📊 Symbol: {}\n📈 Strategy: RSI ({})\n⏱️ Uptime: Online",
            self.symbol, self.config.rsi_period
        );

        self.telegram_bot
            .send_message(msg.chat.id, Self::escape_markdown(&text))
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .await?;

        Ok(())
    }

    async fn handle_positions(&self, msg: &Message) -> Result<()> {
        let positions = self.get_positions().await?;

        let text = if positions.is_empty() {
            "📊 *Positions*\nNo open positions".to_string()
        } else {
            let mut response = "📊 *Positions*\n\n".to_string();
            for pos in positions {
                response.push_str(&format!(
                    "Symbol: {}\nAmount: {:.2}\nEntry Price: ${:.2}\n\n",
                    pos.symbol, pos.amount, pos.entry_price
                ));
            }
            response
        };

        self.telegram_bot
            .send_message(msg.chat.id, Self::escape_markdown(&text))
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .await?;

        Ok(())
    }

    async fn handle_help(&self, msg: &Message) -> Result<()> {
        self.handle_start(msg).await
    }

    // ============ MAIN COMMAND HANDLER ============

    async fn handle_message(&self, msg: &Message, cmd: Command) -> Result<()> {
        // Security: Only allow authorized user
        if msg.from().map(|u| u.id.0) != Some(self.user_id) {
            warn!("Unauthorized access attempt from user: {:?}", msg.from().map(|u| u.id.0));
            self.telegram_bot
                .send_message(msg.chat.id, "⛔ Unauthorized access")
                .await?;
            return Ok(());
        }

        // Rate limiting
        if let Err(e) = self.rate_limiter.check(self.user_id).await {
            self.telegram_bot
                .send_message(msg.chat.id, format!("⏱️ {}", e))
                .await?;
            return Ok(());
        }

        match cmd {
            Command::Start => self.handle_start(msg).await?,
            Command::Balance => self.handle_balance(msg).await?,
            Command::Analyze => self.handle_analyze(msg).await?,
            Command::Buy { amount } => self.handle_buy(msg, amount).await?,
            Command::Sell { amount } => self.handle_sell(msg, amount).await?,
            Command::Status => self.handle_status(msg).await?,
            Command::Positions => self.handle_positions(msg).await?,
            Command::Help => self.handle_help(msg).await?,
        };

        Ok(())
    }
}

// ============ MAIN ============

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // Initialize tracing
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    info!("🤖 Starting Trading Bot...");

    let telegram_bot = Bot::new(
        std::env::var("TELEGRAM_BOT_TOKEN")
            .expect("TELEGRAM_BOT_TOKEN env var not set"),
    );

    // Create shared bot instance
    let trading_bot = Arc::new(TradingBot::new(telegram_bot.clone()).await?);

    info!(
        "🤖 Bot is running! Symbol: {}, User ID: {}",
        trading_bot.symbol, trading_bot.user_id
    );

    // Command handler with shared bot instance
    let handler = {
        let bot = Arc::clone(&trading_bot);
        move |_: Bot, msg: Message, cmd: Command| {
            let bot = Arc::clone(&bot);
            async move {
                if let Err(e) = bot.handle_message(&msg, cmd).await {
                    error!("Error handling message: {}", e);
                    let _ = bot
                        .telegram_bot
                        .send_message(
                            msg.chat.id,
                            format!("❌ Error: {}", e),
                        )
                        .await;
                }
                Ok(())
            }
        }
    };

    Command::repl(trading_bot.telegram_bot.clone(), handler).await;

    Ok(())
}

// ============ AUTO TRADING LOOP (Optional) ============

#[allow(dead_code)]
async fn auto_trading_loop(bot: Arc<TradingBot>, chat_id: ChatId) -> Result<()> {
    loop {
        match bot.analyze_strategy().await {
            Ok(signal) => {
                match signal.signal {
                    strategy::StrategySignal::StrongBuy | strategy::StrategySignal::Buy => {
                        info!(
                            "🔵 BUY signal detected - Symbol: {}, Price: ${:.2}, RSI: {:.2}",
                            signal.symbol, signal.entry_price, signal.indicators.rsi
                        );
                        
                        let text = format!(
                            "🚀 *AUTO BUY*\nSymbol: {}\nPrice: ${:.2}\nRSI: {:.2}",
                            signal.symbol, signal.entry_price, signal.indicators.rsi
                        );
                        
                        if let Err(e) = bot
                            .telegram_bot
                            .send_message(chat_id, TradingBot::escape_markdown(&text))
                            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                            .await
                        {
                            error!("Failed to send buy signal: {}", e);
                        }
                        
                        if let Err(e) = bot.add_position(signal.symbol.clone(), bot.position_size, signal.entry_price).await {
                            error!("Failed to add position: {}", e);
                        }
                    }
                    
                    strategy::StrategySignal::Sell | strategy::StrategySignal::StrongSell => {
                        info!(
                            "🔴 SELL signal detected - Symbol: {}, Price: ${:.2}, RSI: {:.2}",
                            signal.symbol, signal.entry_price, signal.indicators.rsi
                        );
                        
                        let text = format!(
                            "📉 *AUTO SELL*\nSymbol: {}\nPrice: ${:.2}\nRSI: {:.2}",
                            signal.symbol, signal.entry_price, signal.indicators.rsi
                        );
                        
                        let _ = bot
                            .telegram_bot
                            .send_message(chat_id, TradingBot::escape_markdown(&text))
                            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                            .await;
                    }
                    
                    strategy::StrategySignal::Hold => {
                        info!(
                            "⏸️ HOLD - RSI: {:.2} is within range",
                            signal.indicators.rsi
                        );
                    }
                }
            }
            Err(e) => {
                error!("Failed to analyze strategy: {}", e);
            }
        }

        // Wait for next check (configurable, default 1 hour)
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
