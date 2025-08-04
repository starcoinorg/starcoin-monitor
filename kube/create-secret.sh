#!/bin/bash

# Starcoin Monitor - Telegram Secret Generation Script
# Usage: ./create-secret.sh <bot_token> <chat_id>

set -e

# Check parameters
if [ $# -ne 2 ]; then
    echo "Usage: $0 <bot_token> <chat_id>"
    echo ""
    echo "Parameters:"
    echo "  bot_token: Your Telegram Bot Token (get from @BotFather)"
    echo "  chat_id: Your Telegram Chat ID (can be user ID or group ID)"
    echo ""
    echo "Example:"
    echo "  $0 1234567890:ABCdefGHIjklMNOpqrsTUVwxyz 123456789"
    exit 1
fi

BOT_TOKEN=$1
CHAT_ID=$2

# Validate Bot Token format
if [[ ! $BOT_TOKEN =~ ^[0-9]+:[A-Za-z0-9_-]+$ ]]; then
    echo "Error: Invalid Bot Token format"
    echo "Correct format: <number>:<string>"
    echo "Example: 1234567890:ABCdefGHIjklMNOpqrsTUVwxyz"
    exit 1
fi

# Validate Chat ID format
if [[ ! $CHAT_ID =~ ^-?[0-9]+$ ]]; then
    echo "Error: Invalid Chat ID format"
    echo "Chat ID should be a number (positive for users, negative for groups)"
    echo "Example: 123456789 or -987654321"
    exit 1
fi

# Generate base64 encoding
BOT_TOKEN_B64=$(echo -n "$BOT_TOKEN" | base64)
CHAT_ID_B64=$(echo -n "$CHAT_ID" | base64)

# Create Secret configuration file
cat > ./starcoin-monitor-secret.yaml << EOF
apiVersion: v1
kind: Secret
metadata:
  name: starcoin-monitor-secret
  namespace: starcoin-main
  labels:
    app: starcoin-monitor
type: Opaque
data:
  TELEGRAM_BOT_TOKEN: $BOT_TOKEN_B64
  TELEGRAM_CHAT_ID: $CHAT_ID_B64
EOF

echo "✅ Secret configuration file generated: kube/starcoin-monitor-secret.yaml"
echo ""
echo "📋 Configuration info:"
echo "  Bot Token: ${BOT_TOKEN:0:10}..."
echo "  Chat ID: $CHAT_ID"
echo ""
echo "🚀 Deployment command:"
echo "  kubectl apply -f kube/starcoin-monitor-secret.yaml"
echo ""
echo "⚠️  Security reminders:"
echo "  - Make sure Secret file is not committed to version control"
echo "  - Consider adding kube/starcoin-monitor-secret.yaml to .gitignore"
echo "  - For production environments, consider using more secure Secret management solutions" 