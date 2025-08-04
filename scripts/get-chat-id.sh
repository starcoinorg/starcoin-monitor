#!/bin/bash

# Telegram Chat ID Retrieval Script
# Usage: ./get-chat-id.sh <bot_token>

set -e

# Check parameters
if [ $# -ne 1 ]; then
    echo "Usage: $0 <bot_token>"
    echo ""
    echo "Parameters:"
    echo "  bot_token: Your Telegram Bot Token"
    echo ""
    echo "Example:"
    echo "  $0 1234567890:ABCdefGHIjklMNOpqrsTUVwxyz"
    echo ""
    echo "Steps:"
    echo "  1. Add your Bot to the target group"
    echo "  2. Send any message in the group"
    echo "  3. Run this script"
    exit 1
fi

BOT_TOKEN=$1

# Validate Bot Token format
if [[ ! $BOT_TOKEN =~ ^[0-9]+:[A-Za-z0-9_-]+$ ]]; then
    echo "Error: Invalid Bot Token format"
    echo "Correct format: <number>:<string>"
    echo "Example: 1234567890:ABCdefGHIjklMNOpqrsTUVwxyz"
    exit 1
fi

echo "🔍 Retrieving Chat ID..."
echo "Please ensure:"
echo "  1. Bot is added to the target group"
echo "  2. A message has been sent in the group"
echo ""

# Call Telegram Bot API
RESPONSE=$(curl -s "https://api.telegram.org/bot$BOT_TOKEN/getUpdates")

# Check API response
if [[ $RESPONSE == *"\"ok\":true"* ]]; then
    echo "✅ API call successful"
    echo ""
    
    # Extract Chat ID
    CHAT_IDS=$(echo "$RESPONSE" | grep -o '"chat":{"id":[^,]*' | grep -o '"id":[^,]*' | cut -d':' -f2 | sort -u)
    
    if [ -z "$CHAT_IDS" ]; then
        echo "❌ No Chat ID found"
        echo ""
        echo "Possible reasons:"
        echo "  1. Bot not added to group"
        echo "  2. No message sent in group"
        echo "  3. Incorrect Bot Token"
        echo ""
        echo "Please try:"
        echo "  1. Add Bot to the group"
        echo "  2. Send /start in the group"
        echo "  3. Run this script again"
    else
        echo "📋 Found Chat IDs:"
        echo "$CHAT_IDS" | while read -r chat_id; do
            if [[ $chat_id -lt 0 ]]; then
                echo "  🏠 Group: $chat_id"
            else
                echo "  👤 User: $chat_id"
            fi
        done
        echo ""
        echo "💡 Tips:"
        echo "  - Negative Chat ID indicates a group"
        echo "  - Positive Chat ID indicates a user"
        echo "  - Choose the Chat ID you need"
    fi
else
    echo "❌ API call failed"
    echo ""
    echo "Error message:"
    echo "$RESPONSE"
    echo ""
    echo "Possible reasons:"
    echo "  1. Incorrect Bot Token"
    echo "  2. Network connection issue"
    echo "  3. Bot is disabled"
fi 