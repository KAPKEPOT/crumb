use anyhow::Result;

/// Application configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct Config {
    pub rsi_period: usize,
    pub kline_limit: usize,
    pub request_timeout_secs: u64,
    pub rate_limit_secs: u64,
}

impl Config {
    /// Load configuration from environment variables with sensible defaults
    pub fn from_env() -> Result<Self> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = Config::from_env();
        assert!(config.is_ok());
        let cfg = config.unwrap();
        assert_eq!(cfg.rsi_period, 14);
        assert_eq!(cfg.kline_limit, 50);
    }
}