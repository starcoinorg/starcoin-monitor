// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use starcoin_rpc_api::types::{BlockView, TransactionEventView};
use std::sync::Arc;
use std::thread::JoinHandle;
use teloxide::{prelude::*, types::Message, Bot};
use tracing::{error, info};

use crate::{
    config::Config,
    monitor_dispatcher::MonitorDispatcher,
    types::{AccountBalance, Transaction, TransactionSummary},
};

#[derive(Clone)]
pub struct TelegramBot {
    config: Arc<Config>,
    bot: Arc<Bot>,
}

impl TelegramBot {
    pub fn new(config: Arc<Config>) -> Self {
        let bot = Bot::new(&config.telegram_bot_token);
        Self {
            config: config.clone(),
            bot: Arc::new(bot),
        }
    }

    pub fn run(&self) -> Result<JoinHandle<()>> {
        info!("TelegramBot::run | entered");

        let bot = self.bot.clone();
        let config = self.config.clone();
        Ok(std::thread::spawn(move || {
            // let db = self.db.clone();
            let handler = Update::filter_message().branch(
                dptree::filter(|msg: Message| msg.text().is_some()).endpoint(
                    move |msg: Message| {
                        let config = config.clone();
                        // let db = db.clone();

                        async move {
                            let text = msg.text().unwrap();
                            let chat_id = msg.chat.id;
                            let _user_id =
                                msg.from().map(|u| u.id.0.to_string()).unwrap_or_default();

                            let telegram_bot = TelegramBot::new(config.clone());
                            telegram_bot.handle_command(text, chat_id, _user_id).await
                        }
                    },
                ),
            );

            futures::executor::block_on(
                Dispatcher::builder(bot.clone(), handler).build().dispatch(),
            );
            info!("TelegramBot::run | Exited");
        }))
    }

    async fn handle_command(&self, text: &str, chat_id: ChatId, _user_id: String) -> Result<()> {
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        let command = parts[0].to_lowercase();
        let args = parts[1..].to_vec();

        match command.as_str() {
            "/start" => {
                let message = self.get_help_message();
                self.send_message_to_chat(chat_id, &message).await?;
            }
            "/help" => {
                let message = self.get_help_message();
                self.send_message_to_chat(chat_id, &message).await?;
            }
            "/transactions" => {
                self.handle_transactions_command(chat_id, args).await?;
            }
            "/summary" => {
                self.handle_summary_command(chat_id, args).await?;
            }
            "/balance" => {
                self.handle_balance_command(chat_id, args).await?;
            }
            _ => {
                let message = "❓ Unknown command. Use /help to see available commands.";
                self.send_message_to_chat(chat_id, message).await?;
            }
        }

        Ok(())
    }

    async fn handle_transactions_command(&self, chat_id: ChatId, args: Vec<&str>) -> Result<()> {
        if args.len() < 2 {
            let message = "❌ Usage: /transactions <start_block> <end_block>\nExample: /transactions 1000 1100";
            self.send_message_to_chat(chat_id, message).await?;
            return Ok(());
        }

        let start_block = match args[0].parse::<u64>() {
            Ok(n) => n,
            Err(_) => {
                let message = "❌ Invalid start block number";
                self.send_message_to_chat(chat_id, message).await?;
                return Ok(());
            }
        };

        let end_block = match args[1].parse::<u64>() {
            Ok(n) => n,
            Err(_) => {
                let message = "❌ Invalid end block number";
                self.send_message_to_chat(chat_id, message).await?;
                return Ok(());
            }
        };

        if start_block > end_block {
            let message = "❌ Start block must be less than or equal to end block";
            self.send_message_to_chat(chat_id, message).await?;
            return Ok(());
        }

        // match self
        //     .db
        //     .get_transactions_by_block_range(start_block, end_block)
        //     .await
        // {
        //     Ok(transactions) => {
        //         if transactions.is_empty() {
        //             let message = format!(
        //                 "📭 No transactions found in blocks {} to {}",
        //                 start_block, end_block
        //             );
        //             self.send_message_to_chat(chat_id, &message).await?;
        //         } else {
        //             let message =
        //                 self.format_transactions_list(&transactions, start_block, end_block);
        //             self.send_message_to_chat(chat_id, &message).await?;
        //         }
        //     }
        //     Err(e) => {
        //         error!("Error fetching transactions: {}", e);
        //         let message = "❌ Error fetching transactions from database";
        //         self.send_message_to_chat(chat_id, message).await?;
        //     }
        // }

        Ok(())
    }

    async fn handle_summary_command(&self, chat_id: ChatId, args: Vec<&str>) -> Result<()> {
        if args.len() < 2 {
            let message =
                "❌ Usage: /summary <start_block> <end_block>\nExample: /summary 1000 1100";
            self.send_message_to_chat(chat_id, message).await?;
            return Ok(());
        }

        // let start_block = match args[0].parse::<u64>() {
        //     Ok(n) => n,
        //     Err(_) => {
        //         let message = "❌ Invalid start block number";
        //         self.send_message_to_chat(chat_id, message).await?;
        //         return Ok(());
        //     }
        // };
        //
        // let end_block = match args[1].parse::<u64>() {
        //     Ok(n) => n,
        //     Err(_) => {
        //         let message = "❌ Invalid end block number";
        //         self.send_message_to_chat(chat_id, message).await?;
        //         return Ok(());
        //     }
        // };

        // match self
        //     .db
        //     .get_transaction_summary(start_block, end_block)
        //     .await
        // {
        //     Ok(summary) => {
        //         let message = self.format_transaction_summary(&summary);
        //         self.send_message_to_chat(chat_id, &message).await?;
        //     }
        //     Err(e) => {
        //         error!("Error fetching summary: {}", e);
        //         let message = "❌ Error fetching transaction summary";
        //         self.send_message_to_chat(chat_id, message).await?;
        //     }
        // }

        Ok(())
    }

    async fn handle_balance_command(&self, chat_id: ChatId, args: Vec<&str>) -> Result<()> {
        if args.is_empty() {
            let message = "❌ Usage: /balance <address> [token]\nExample: /balance 0x123... STC";
            self.send_message_to_chat(chat_id, message).await?;
            return Ok(());
        }

        // let address = args[0];
        // let token = args.get(1).unwrap_or(&"STC");
        //
        // match self.db.get_account_balance(address, token).await {
        //     Ok(Some(balance)) => {
        //         let message = self.format_account_balance(&balance);
        //         self.send_message_to_chat(chat_id, &message).await?;
        //     }
        //     Ok(None) => {
        //         let message = format!(
        //             "📭 No balance found for address {} and token {}",
        //             address, token
        //         );
        //         self.send_message_to_chat(chat_id, &message).await?;
        //     }
        //     Err(e) => {
        //         error!("Error fetching balance: {}", e);
        //         let message = "❌ Error fetching account balance";
        //         self.send_message_to_chat(chat_id, message).await?;
        //     }
        // }

        Ok(())
    }

    fn get_help_message(&self) -> String {
        r#"
🤖 **Starcoin Monitor Bot Commands**

📊 **Query Commands:**
• `/transactions <start_block> <end_block>` - Get large transactions in block range
• `/summary <start_block> <end_block>` - Get transaction summary for block range  
• `/balance <address> [token]` - Get account balance (default: STC)

📝 **Examples:**
• `/transactions 1000 1100` - Get transactions from block 1000 to 1100
• `/summary 1000 1100` - Get summary for blocks 1000-1100
• `/balance 0x1234567890abcdef` - Get STC balance
• `/balance 0x1234567890abcdef STC` - Get specific token balance

💡 **Tips:**
• Large transactions are automatically monitored and alerts are sent
• All data is stored locally in the database
• Use block numbers to query specific ranges

Need help? Contact the administrator.
        "#
        .trim()
        .to_string()
    }

    fn format_transactions_list(
        &self,
        transactions: &[Transaction],
        start_block: u64,
        end_block: u64,
    ) -> String {
        let mut message = format!(
            "📋 **Large Transactions (Blocks {} to {})**\n\n",
            start_block, end_block
        );

        for (i, tx) in transactions.iter().enumerate().take(10) {
            // Limit to 10 transactions
            let amount_stc = tx.amount as f64 / 1_000_000_000.0;
            message.push_str(&format!(
                "{}. **{:.2} STC**\n   From: `{}`\n   To: `{}`\n   Block: {}\n   Hash: `{}`\n\n",
                i + 1,
                amount_stc,
                tx.from_address,
                tx.to_address,
                tx.block_number,
                tx.hash
            ));
        }

        if transactions.len() > 10 {
            message.push_str(&format!(
                "... and {} more transactions",
                transactions.len() - 10
            ));
        }

        message
    }

    fn format_transaction_summary(&self, summary: &TransactionSummary) -> String {
        let total_amount_stc = summary.total_amount as f64 / 1_000_000_000.0;

        format!(
            "📊 **Transaction Summary**\n\n\
            **Period:** {}\n\
            **Total Transactions:** {}\n\
            **Total Amount:** {:.2} STC\n\
            **Average per Transaction:** {:.2} STC",
            summary.period,
            summary.total_transactions,
            total_amount_stc,
            if summary.total_transactions > 0 {
                total_amount_stc / summary.total_transactions as f64
            } else {
                0.0
            }
        )
    }

    fn format_account_balance(&self, balance: &AccountBalance) -> String {
        let balance_stc = balance.balance as f64 / 1_000_000_000.0;

        format!(
            "💰 **Account Balance**\n\n\
            **Address:** `{}`\n\
            **Token:** {}\n\
            **Balance:** {:.2} STC\n\
            **Last Updated:** {}",
            balance.address,
            balance.token,
            balance_stc,
            balance.last_updated.format("%Y-%m-%d %H:%M:%S UTC")
        )
    }

    pub async fn send_message(&self, message: &str) -> Result<()> {
        self.send_message_to_chat(ChatId(self.config.telegram_chat_id.parse()?), message)
            .await
    }

    async fn send_message_to_chat(&self, chat_id: ChatId, message: &str) -> Result<()> {
        match self
            .bot
            .send_message(chat_id, message)
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .await
        {
            Ok(_) => {
                info!("Message sent to chat {}", chat_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to send message to chat {}: {}", chat_id, e);
                Err(anyhow::anyhow!("Failed to send message: {}", e))
            }
        }
    }
}

impl MonitorDispatcher for TelegramBot {
    fn dispatch_event(&self, _event: &TransactionEventView) -> Result<()> {
        todo!()
    }

    fn dispatch_block(&self, _block: BlockView) -> Result<()> {
        todo!()
    }
}
