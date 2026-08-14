# 🤖 Manguins - Cryptocurrency Trading Bot

A Telegram-based trading bot that automatically analyzes cryptocurrency prices and helps you make smarter trading decisions on Binance.

---

## **What Does Manguins Do?**

Manguins is a bot that:

- 📊 **Analyzes price patterns** - Watches cryptocurrency prices and identifies buying/selling opportunities
- 💬 **Uses Telegram** - Control everything from your phone via Telegram messages
- 🎯 **Suggests trades** - Tells you when to BUY, SELL, or HOLD
- 💰 **Places real orders** - Can automatically buy/sell on your Binance account
- 🛡️ **Protects your money** - Automatically sets stop losses and profit targets to limit losses

---

## **Quick Start**

### **1. Prerequisites**
You need:
- A Binance account (with API keys)
- A Telegram account (to receive bot messages)
- A computer/server to run the bot 24/7

### **2. Setup Instructions**

**Create a `.env` file** with your credentials:
```
BINANCE_API_KEY=your_api_key_here
BINANCE_API_SECRET=your_api_secret_here
TELEGRAM_BOT_TOKEN=your_bot_token_here
TELEGRAM_USER_ID=your_telegram_id_here
TRADING_SYMBOL=BTCUSDT
POSITION_SIZE=0.01
```

**Build and run the bot:**
```bash
cargo build --release
cargo run --release
```

---

## **How to Use (Telegram Commands)**

Send these messages to your bot on Telegram:

| Command | What it does |
|---------|------------|
| `/start` | Shows welcome message and instructions |
| `/balance` | Shows your current wallet balance |
| `/analyze` | Analyzes current price and suggests action (BUY/SELL/HOLD) |
| `/buy 100` | Places a buy order for $100 worth |
| `/sell 50` | Sells $50 worth of your holdings |
| `/positions` | Shows all your open trades |
| `/status` | Shows if the bot is running properly |
| `/help` | Shows all available commands |

---

## **What's Inside the Bot?**

### **Smart Analysis (Technical Indicators)**

The bot uses professional trading indicators to make decisions:

- **RSI (Relative Strength Index)** - Detects overbought/oversold conditions
- **MACD** - Identifies trend changes
- **EMA (Exponential Moving Averages)** - Tracks price momentum
- **Bollinger Bands** - Shows support/resistance levels
- **Volume Analysis** - Confirms price moves with trading volume
- **ATR (Average True Range)** - Calculates safe stop losses

### **Smart Risk Management**

- Automatically sets **stop losses** (cuts losses if price drops too much)
- Automatically sets **take profit levels** (locks in gains)
- **Rate limiting** - Prevents accidental spam trades
- **Authorization** - Only you can control your trades

---

## **Key Features**

✅ **Real Binance Integration** - Actually places trades on the exchange  
✅ **Multi-Indicator Analysis** - Uses 5+ indicators for accurate signals  
✅ **Confidence Scoring** - Shows how confident the bot is about a trade  
✅ **24/7 Monitoring** - Runs continuously, watches for opportunities  
✅ **Mobile-Friendly** - Control everything from Telegram on your phone  
✅ **Error Handling** - Logs all errors and keeps running smoothly  

---

## **Important Safety Notes ⚠️**

🚨 **This bot places REAL trades with REAL money**

- Start with small position sizes
- Never give full withdrawal permissions to API keys
- Test thoroughly before using with large amounts
- Keep your API keys secret
- Monitor the bot regularly
- Cryptocurrency trading has risks - you can lose money

---

## **Supported Cryptocurrencies**

Works with any trading pair on Binance, such as:
- BTC/USDT (Bitcoin)
- ETH/USDT (Ethereum)
- SOL/USDT (Solana)
- BNB/USDT (Binance Coin)
- Any other pair available on Binance

---

## **Troubleshooting**

**Bot won't start?**
- Check your `.env` file has all required fields
- Verify API keys are correct on Binance
- Ensure Telegram bot token is valid

**No analysis results?**
- Need at least 30 candles of price data
- May need to wait 1-2 hours for bot to gather data
- Check logs for error messages

**Trades not executing?**
- Verify you have enough balance
- Check API key has trading permissions on Binance
- Confirm correct trading pair is set

---

## **Support & Contributions**

Have issues or improvements? 
- Check error logs in the console
- Verify all `.env` settings are correct
- Review Binance API status

---

## **Disclaimer**

This software is provided "as-is". The creators are not responsible for:
- Financial losses from trades
- Bot malfunctions
- Cryptocurrency market crashes
- API changes from Binance

**Trade at your own risk.**

---

**Version:** 0.1.2  
**Built with:** Rust, Tokio, Binance API, Telegram Bot API
