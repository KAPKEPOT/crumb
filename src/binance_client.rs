use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBalance {
    pub asset: String,
    pub free: String,
    pub locked: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub balances: Vec<AccountBalance>,
    pub can_trade: bool,
    pub can_deposit: bool,
    pub can_withdraw: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    pub symbol: String,
    pub order_id: u64,
    pub client_order_id: String,
    pub transact_time: u64,
    pub price: String,
    pub orig_qty: String,
    pub executed_qty: String,
    pub cum_quote_asset_transacted_qty: String,
    pub status: String,
    pub time_in_force: String,
    pub side: String,
}

pub struct RealBinanceClient {
    client: Client,
    api_key: String,
    api_secret: String,
    base_url: String,
}

impl RealBinanceClient {
    pub fn new() -> Result<Self> {
        let api_key = env::var("BINANCE_API_KEY")
            .map_err(|_| anyhow!("BINANCE_API_KEY not set"))?;
        let api_secret = env::var("BINANCE_API_SECRET")
            .map_err(|_| anyhow!("BINANCE_API_SECRET not set"))?;
        
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| anyhow!("Failed to build HTTP client: {}", e))?;

        Ok(Self {
            client,
            api_key,
            api_secret,
            base_url: "https://api.binance.com".to_string(),
        })
    }
    
    /// Generate HMAC-SHA256 signature for authenticated requests
    fn generate_signature(&self, params: &[(&str, String)]) -> Result<String> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        
        // Build query string from params
        let query_string = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        
        // Create HMAC-SHA256 signature
        let mut mac = Hmac::<Sha256>::new_from_slice(self.api_secret.as_bytes())
            .map_err(|e| anyhow!("Failed to create HMAC: {}", e))?;
        mac.update(query_string.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }
    
    /// Get account information (balances, trading status)
    pub async fn get_account(&self) -> Result<AccountInfo> {
        let endpoint = format!("{}/api/v3/account", self.base_url);
        
        // Build parameters
        let timestamp = chrono::Local::now().timestamp_millis().to_string();
        let params = vec![
            ("timestamp", timestamp),
        ];
        
        // Generate signature
        let signature = self.generate_signature(&params)?;
        
        // Build URL with all params
        let mut url = format!("{}?", endpoint);
        for (k, v) in &params {
            url.push_str(&format!("{}={}&", k, urlencoding::encode(v)));
        }
        url.push_str(&format!("signature={}", signature));
        
        let response = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch account: {}", e))?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "Binance API error [{}]: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }
        
        let account_info = response
            .json::<AccountInfo>()
            .await
            .map_err(|e| anyhow!("Failed to parse account response: {}", e))?;
        info!("Account fetched successfully");
        Ok(account_info)
    }
    
    /// Get balance for a specific asset
    pub async fn get_balance(&self, asset: &str) -> Result<f64> {
        let account = self.get_account().await?;
        
        for balance in account.balances {
            if balance.asset == asset {
                let free: f64 = balance.free.parse()
                    .map_err(|_| anyhow!("Failed to parse {} balance", asset))?;
                info!("💰 {} balance: {:.8}", asset, free);
                return Ok(free);
            }
        }
        
        Ok(0.0)
    }
    
    /// Get USDT balance
    pub async fn get_usdt_balance(&self) -> Result<f64> {
        self.get_balance("USDT").await
    }
    
    /// Get BTC balance
    pub async fn get_btc_balance(&self) -> Result<f64> {
        self.get_balance("BTC").await
    }
    
    /// Place a market buy order
    pub async fn place_market_buy(&self, symbol: &str, quantity: f64) -> Result<String> {
        self.place_market_order(symbol, quantity, "BUY").await
    }
    
    /// Place a market sell order
    #[allow(dead_code)]
    pub async fn place_market_sell(&self, symbol: &str, quantity: f64) -> Result<String> {
        self.place_market_order(symbol, quantity, "SELL").await
    }
    
    /// Internal: Place market order (BUY or SELL)
    async fn place_market_order(&self, symbol: &str, quantity: f64, side: &str) -> Result<String> {
        let endpoint = format!("{}/api/v3/order", self.base_url);
        
        // Build query params
        let timestamp = chrono::Local::now().timestamp_millis().to_string();
        let quantity_str = quantity.to_string();
        let params = vec![
            ("symbol", symbol.to_string()),
            ("side", side.to_string()),
            ("type", "MARKET".to_string()),
            ("quantity", quantity_str),
            ("timestamp", timestamp),
        ];
        
        // Generate signature
        let signature = self.generate_signature(&params)?;
        
        // Build request body with signature
        let mut body_params = params.clone();
        body_params.push(("signature", signature));

        let response = self
            .client
            .post(&endpoint)
            .header("X-MBX-APIKEY", &self.api_key)
            .form(&body_params)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to place {} order: {}", side, e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Binance order API error: {}",
                error_text
            ));
        }

        let order = response
            .json::<OrderResponse>()
            .await
            .map_err(|e| anyhow!("Failed to parse order response: {}", e))?;

        info!("📈 {} order placed - Symbol: {}, Qty: {:.8}, Order ID: {}", 
              side, symbol, quantity, order.order_id);
        
        Ok(order.order_id.to_string())
    }
}