# Starcoin Monitor Usage Guide

## Quick Start

### 1. Prerequisites
- Rust 1.70+ installed
- Telegram Bot Token (from @BotFather)
- Telegram Chat ID

### 2. Setup
```bash
# Clone and setup
git clone <repository-url>
cd starcoin-monitor

# Run setup script
./test_setup.sh

# Edit configuration
nano .env
```

### 3. Configuration
Edit `.env` file with your settings:

```env
# Starcoin RPC URL
STARCOIN_RPC_URL=ws://main.seed.starcoin.org:9870

# Telegram Bot Configuration
TELEGRAM_BOT_TOKEN=your_bot_token_here
TELEGRAM_CHAT_ID=your_chat_id_here

# Database Configuration
DATABASE_URL=sqlite:starcoin_monitor.db

# Monitoring Configuration
MIN_TRANSACTION_AMOUNT=1000000000  # 1 STC in nano units
BLOCK_SUBSCRIPTION_INTERVAL=1000   # milliseconds
```

### 4. Run the Service
```bash
# Development
cargo run

# Production
cargo run --release
```

## Telegram Bot Commands

### Basic Commands
- `/start` - Show welcome message and help
- `/help` - Show detailed help with all commands

### Transaction Queries
- `/transactions <start_block> <end_block>` - Get large transactions in block range
  - Example: `/transactions 1000 1100`
  - Returns: List of transactions with amounts, addresses, and block info

### Summary Queries
- `/summary <start_block> <end_block>` - Get transaction summary for block range
  - Example: `/summary 1000 1100`
  - Returns: Total transactions, total amount, and averages

### Balance Queries
- `/balance <address> [token]` - Get account balance
  - Example: `/balance 0x1234567890abcdef`
  - Example: `/balance 0x1234567890abcdef STC`
  - Returns: Current balance and last updated time

## Docker Deployment

### Using Docker Compose
```bash
# Build and run
docker-compose up -d

# View logs
docker-compose logs -f

# Stop service
docker-compose down
```

### Using Docker directly
```bash
# Build image
docker build -t starcoin-monitor .

# Run container
docker run -d \
  --name starcoin-monitor \
  -e TELEGRAM_BOT_TOKEN=your_token \
  -e TELEGRAM_CHAT_ID=your_chat_id \
  -v $(pwd)/data:/app/data \
  starcoin-monitor
```

## Monitoring Features

### Large Transaction Detection
- Automatically monitors all blocks for large transactions
- Configurable threshold via `MIN_TRANSACTION_AMOUNT`
- Sends alerts to Telegram when threshold is exceeded

### Data Storage
- SQLite database for local storage
- Stores transaction history, account balances, and alert status
- Persistent data across service restarts

### Real-time Alerts
- Instant notifications for large transactions
- Detailed transaction information in alerts
- Prevents duplicate alerts for same transaction

## Troubleshooting

### Common Issues

1. **Telegram Bot Not Responding**
   - Check bot token is correct
   - Ensure bot is started with @BotFather
   - Verify chat ID is correct

2. **No Transaction Alerts**
   - Check RPC URL is accessible
   - Verify minimum amount threshold
   - Check logs for connection errors

3. **Database Errors**
   - Ensure write permissions to database file
   - Check disk space
   - Verify SQLite is available

### Log Levels
```bash
# Set log level via environment
export RUST_LOG=debug
cargo run

# Or via command line
cargo run -- --log-level debug
```

### Health Checks
```bash
# Check if service is running
ps aux | grep starcoin-monitor

# Check database
sqlite3 starcoin_monitor.db ".tables"

# Check logs
tail -f logs/starcoin-monitor.log
```

## Advanced Configuration

### Custom RPC Endpoints
```env
# Main network
STARCOIN_RPC_URL=ws://main.seed.starcoin.org:9870

# Test network
STARCOIN_RPC_URL=ws://barnard.seed.starcoin.org:9870

# Custom node
STARCOIN_RPC_URL=ws://your-node:9870
```

### Performance Tuning
```env
# Polling interval (milliseconds)
BLOCK_SUBSCRIPTION_INTERVAL=1000

# Minimum transaction amount (nano STC)
MIN_TRANSACTION_AMOUNT=1000000000
```

### Database Configuration
```env
# SQLite (default)
DATABASE_URL=sqlite:starcoin_monitor.db

# PostgreSQL (if supported)
DATABASE_URL=postgresql://user:pass@localhost/starcoin_monitor
```

## Security Considerations

1. **Bot Token Security**
   - Keep bot token secret
   - Use environment variables
   - Don't commit tokens to version control

2. **Database Security**
   - Use file permissions to protect database
   - Regular backups
   - Consider encryption for sensitive data

3. **Network Security**
   - Use HTTPS for RPC connections when available
   - Firewall rules for production deployment
   - Monitor for suspicious activity

## Development

### Adding New Commands
1. Add command handler in `src/telegram.rs`
2. Update help message
3. Add tests if applicable

### Extending Monitoring
1. Modify `src/monitor.rs` for new detection logic
2. Update database schema if needed
3. Add new alert types

### Testing
```bash
# Run tests
cargo test

# Run with specific test
cargo test test_name

# Run integration tests
cargo test --test integration
```

## Support

For issues and questions:
- Create GitHub issue
- Check existing documentation
- Review logs for error details
- Contact maintainers

## License

MIT License - see LICENSE file for details. 