use anyhow::{anyhow, Result};
use binance::{
    config::Config,
    account::Account,
    userstream::UserStream,
    websockets::WebSockets,
    model::{OrderSide, OrderType, TimeInForce},
};
use serde::{Deserialize, Serialize};
use std::env;

pub struct RealBinanceClient {
    account: Account,
    api_key: String,
    api_secret: String,
}

impl RealBinanceClient {
    pub fn new() -> Result<Self> {
        let api_key = env::var("BINANCE_API_KEY")
            .map_err(|_| anyhow!("BINANCE_API_KEY not set"))?;
        let api_secret = env::var("BINANCE_API_SECRET")
            .map_err(|_| anyhow!("BINANCE_API_SECRET not set"))?;
        
        let config = Config::default()
            .set_api_key(api_key.clone())
            .set_secret_key(api_secret.clone());
        
        let account = Account::new(config);
        
        Ok(Self {
            account,
            api_key,
            api_secret,
        })
    }
    
    // Get real account balance
    pub async fn get_balance(&self, asset: &str) -> Result<f64> {
        let account_info = self.account.get_account()
            .await
            .map_err(|e| anyhow!("Failed to get account: {}", e))?;
        
        // Find the asset balance
        for balance in account_info.balances {
            if balance.asset == asset {
                let free: f64 = balance.free.parse().unwrap_or(0.0);
                return Ok(free);
            }
        }
        
        Ok(0.0)
    }
    
    // Get real USDT balance
    pub async fn get_usdt_balance(&self) -> Result<f64> {
        self.get_balance("USDT").await
    }
    
    // Get BTC balance
    pub async fn get_btc_balance(&self) -> Result<f64> {
        self.get_balance("BTC").await
    }
    
    // Place a real market buy order
    pub async fn place_market_buy(&self, symbol: &str, quantity: f64) -> Result<String> {
        let order = self.account
            .order()
            .market()
            .buy(symbol, quantity)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to place market buy: {}", e))?;
        
        Ok(order.order_id.to_string())
    }
    
    // Place a real market sell order
    pub async fn place_market_sell(&self, symbol: &str, quantity: f64) -> Result<String> {
        let order = self.account
            .order()
            .market()
            .sell(symbol, quantity)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to place market sell: {}", e))?;
        
        Ok(order.order_id.to_string())
    }
}