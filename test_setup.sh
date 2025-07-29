#!/bin/bash

echo "🚀 Starcoin Monitor Setup Script"
echo "================================"

# Check if .env file exists
if [ ! -f .env ]; then
    echo "📝 Creating .env file from template..."
    cp env.example .env
    echo "✅ .env file created. Please edit it with your configuration."
else
    echo "✅ .env file already exists."
fi

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "🔧 Building project..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
    echo ""
    echo "📋 Next steps:"
    echo "1. Edit .env file with your configuration:"
    echo "   - TELEGRAM_BOT_TOKEN: Get from @BotFather"
    echo "   - TELEGRAM_CHAT_ID: Your chat ID"
    echo "   - STARCOIN_RPC_URL: Starcoin RPC endpoint"
    echo ""
    echo "2. Run the service:"
    echo "   cargo run --release"
    echo ""
    echo "3. Test the Telegram bot:"
    echo "   - Send /start to your bot"
    echo "   - Try commands like /help, /transactions 1000 1100"
    echo ""
    echo "🎉 Setup complete!"
else
    echo "❌ Build failed. Please check the error messages above."
    exit 1
fi 