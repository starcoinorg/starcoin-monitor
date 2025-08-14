# Starcoin Monitor

A comprehensive monitoring service for the Starcoin blockchain that tracks large transactions and provides real-time alerts via Telegram.

## Features

- 🔍 **Real-time Monitoring**: Continuously monitors Starcoin blockchain for large transactions
- 📱 **Telegram Integration**: Sends alerts to Telegram when large transactions are detected
- 🤖 **Interactive Bot**: Telegram bot with commands to query transaction data
- 📊 **Query Capabilities**: Query transactions by block range, get summaries, and check balances
- 🚀 **PubSub Support**: Real-time event-driven monitoring using WebSocket subscriptions

```bash
cargo run --release
```

## Telegram Bot Commands

- `/start` or `/help` - Show help message with available commands
- `/transactions <start_block> <end_block>` - Get large transactions in block range

## Installation

1. **Clone the repository**
   ```bash
   git clone <repository-url>
   cd starcoin-monitor
   ```

2. **Install Rust dependencies**
   ```bash
   cargo build --release
   ```

3. **Set up environment variables**
   ```bash
   cp env.example .env
   # Edit .env with your configuration
   ```

4. **Configure your Telegram Bot**
   - Create a bot via [@BotFather](https://t.me/botfather)
   - Get your bot token and chat ID
   - Update the `.env` file with your credentials

## Configuration

Edit the `.env` file with your settings:

```env
# Starcoin RPC URL
STARCOIN_RPC_URL=ws://main.seed.starcoin.org:9870

# Telegram Bot Configuration
TELEGRAM_BOT_TOKEN=your_telegram_bot_token_here
TELEGRAM_CHAT_ID=your_chat_id_here

# Database Configuration
DATABASE_URL=sqlite:starcoin_monitor.db

# Monitoring Configuration
MIN_TRANSACTION_AMOUNT=1000000000  # 1 STC in nano units
BLOCK_SUBSCRIPTION_INTERVAL=1000   # milliseconds (polling mode only)
```

## Usage

### Polling Mode
```bash
# Start with default polling mode
cargo run --release

# With custom log level
cargo run --release -- --log-level debug
```

### PubSub Mode
```bash
# Start with PubSub mode for real-time monitoring
cargo run --release -- --pubsub

# With debug logging
cargo run --release -- --pubsub --log-level debug
```

### Using the Telegram bot
- Send `/start` to get help
- Use commands to query transaction data
- Receive automatic alerts for large transactions

## Architecture

- **Monitor Service**: Continuously polls Starcoin RPC for new blocks and transactions
- **PubSub Service**: Real-time event-driven monitoring using WebSocket subscriptions
- **Database Layer**: SQLite database for storing transaction data and alerts
- **Telegram Bot**: Interactive bot for querying data and receiving alerts
- **Configuration**: Environment-based configuration management

## Development

### Prerequisites
- Rust 1.70+
- SQLite
- Telegram Bot Token

### Building
```bash
cargo build
cargo test
cargo run --release
```

### Logging
The service uses structured logging with different levels:
- `info`: General operational information
- `warn`: Warning messages
- `error`: Error messages

## Documentation

- [Usage Guide](USAGE.md) - Detailed usage instructions
- [PubSub Guide](PUBSUB_USAGE.md) - PubSub mode documentation

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

MIT License - see LICENSE file for details.

## Support

For issues and questions:
- Create an issue on GitHub
- Contact the maintainers
- Check the documentation

## Roadmap

- [x] WebSocket subscription for real-time updates
- [ ] Support for multiple tokens
- [ ] Advanced filtering options
- [ ] Web dashboard
- [ ] Email notifications
- [ ] Slack integration
