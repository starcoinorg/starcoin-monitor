// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use crate::helper;
use anyhow::Result;
use starcoin_rpc_client::RpcClient;
use starcoin_types::block::BlockNumber;
use std::{sync::Arc, thread::JoinHandle};
use teloxide::{prelude::*, types::Message, Bot};
use tracing::{error, info};

async fn do_handle_blocks(
    rpc_client: Arc<RpcClient>,
    config: Arc<Config>,
    start_num: BlockNumber,
    end_num: BlockNumber,
) -> Result<Option<String>> {
    let rpc_client1 = rpc_client.clone();
    let block_views = tokio::task::spawn_blocking(move || {
        rpc_client1.chain_get_blocks_by_number(Some(start_num), end_num - start_num, None)
    })
    .await??;

    if block_views.is_empty() {
        return Ok(Some(format!(
            "从区块 {} 到 {}， 没有找到大交易",
            start_num, end_num
        )));
    }

    let all_transactions =
        helper::extract_full_txn_from_block_view(rpc_client.clone(), block_views).await?;
    if all_transactions.is_empty() {
        return Ok(Some(format!(
            "从区块 {} 到 {}， 没有找到大交易",
            start_num, end_num
        )));
    }

    // Process all collected transactions
    let mut matched_txn = Vec::new();

    for tx in all_transactions {
        let amount = helper::parse_txn_p2p_amount(tx.clone())?;
        if config.min_transaction_amount < amount.unwrap_or(0) {
            matched_txn.push((tx.transaction_hash.clone(), amount));
        }
    }

    if matched_txn.is_empty() {
        return Ok(Some(format!(
            "从区块 {} 到 {}， 没有找到大交易",
            start_num, end_num
        )));
    }

    let total_amount = matched_txn
        .iter()
        .map(|pair| pair.1.unwrap_or(0))
        .sum::<u128>();

    Ok(Some(format!(
        "查询区块区间: https://stcscan.io/main/blocks/height/{}, https://stcscan.io/main/blocks/height/{}
         \n交易总额  {:.9} STC,
         \n交易列表: {:?}",
        start_num,
        end_num,
        (total_amount as f64) / (1e9f64),
        matched_txn
    )))
}

#[derive(Clone)]
pub struct TelegramBot {
    config: Arc<Config>,
    bot: Arc<Bot>,
    rpc_client: Arc<RpcClient>,
}

impl TelegramBot {
    pub fn new(config: Arc<Config>, rpc_client: Arc<RpcClient>) -> Self {
        let bot =
            Self::create_bot_with_proxy(&config.telegram_bot_token, config.telegram_proxy.clone());
        Self {
            config: config.clone(),
            bot: Arc::new(bot),
            rpc_client,
        }
    }

    fn create_bot_with_proxy(token: &str, proxy: Option<String>) -> Bot {
        let proxy_url =
            proxy.unwrap_or_else(|| std::env::var("TELOXIDE_PROXY").unwrap_or_default());

        // Check if proxy environment variable is set
        if proxy_url.is_empty() {
            info!("No proxy configured, using direct connection");
            return Bot::new(token);
        };

        info!("Using proxy: {}", proxy_url);

        // Create reqwest client with proxy and longer timeout
        let client = reqwest::Client::builder()
            .proxy(
                reqwest::Proxy::http(&format!("http://{}", proxy_url))
                    .expect("Failed to create HTTP proxy"),
            )
            .proxy(
                reqwest::Proxy::https(&format!("http://{}", proxy_url))
                    .expect("Failed to create HTTPS proxy"),
            )
            .timeout(std::time::Duration::from_secs(60)) // Increased timeout to 60 seconds
            .connect_timeout(std::time::Duration::from_secs(30)) // Add connect timeout
            .pool_idle_timeout(std::time::Duration::from_secs(90)) // Keep connections alive longer
            .build()
            .expect("Failed to create reqwest client");

        Bot::with_client(token, client)
    }

    pub fn run(&self) -> Result<JoinHandle<()>> {
        let bot = self.bot.clone();
        let config = self.config.clone();
        let rpc_client = self.rpc_client.clone();
        Ok(std::thread::spawn(move || {
            info!("TelegramBot::run | entered");
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .enable_io()
                .build()
                .unwrap();

            info!("TelegramBot::run | Tokio builded");

            // let db = self.db.clone();
            let handler = Update::filter_message().branch(
                dptree::filter(|msg: Message| msg.text().is_some()).endpoint(
                    move |msg: Message| {
                        let config = config.clone();
                        let rpc_client = rpc_client.clone();

                        async move {
                            let text = msg.text().unwrap();
                            let chat_id = msg.chat.id;
                            let _user_id =
                                msg.from().map(|u| u.id.0.to_string()).unwrap_or_default();

                            let telegram_bot = TelegramBot::new(config.clone(), rpc_client.clone());
                            telegram_bot.handle_command(text, chat_id, _user_id).await
                        }
                    },
                ),
            );

            rt.block_on(async {
                Dispatcher::builder(bot.clone(), handler)
                    .build()
                    .dispatch()
                    .await
            });
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
            self.send_message_to_chat(
                chat_id,
                "❌ Start block must be less than or equal to end block",
            )
            .await?;
            return Ok(());
        }

        if end_block - start_block > 2000 {
            self.send_message_to_chat(
                chat_id,
                "❌ The interval between the start and end points does not exceed 2,000 blocks.",
            )
            .await?;
            return Ok(());
        }

        let respon_message = do_handle_blocks(
            self.rpc_client.clone(),
            self.config.clone(),
            start_block,
            end_block,
        )
        .await?;
        if let Some(msg) = respon_message {
            self.send_message_to_chat(chat_id, &msg).await?;
        }
        Ok(())
    }

    fn get_help_message(&self) -> String {
        r#"
🤖 **Starcoin Monitor Bot Commands**

📊 **查询命令:**
• `/transactions <start_block> <end_block>` - 查询两个区块之间的大交易
        "#
        .trim()
        .to_string()
    }

    pub async fn send_message(&self, message: &str) -> Result<()> {
        self.send_message_to_chat(ChatId(self.config.telegram_chat_id.parse()?), message)
            .await
    }

    fn escape_markdown_v2(text: &str) -> String {
        // Characters that need to be escaped in MarkdownV2
        let special_chars = [
            '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.',
            '!',
        ];
        let mut escaped = String::new();

        for ch in text.chars() {
            if special_chars.contains(&ch) {
                escaped.push('\\');
            }
            escaped.push(ch);
        }

        escaped
    }

    async fn send_message_to_chat(&self, chat_id: ChatId, message: &str) -> Result<()> {
        // Escape the message for MarkdownV2
        let escaped_message = Self::escape_markdown_v2(message);

        // Retry mechanism for network errors
        let mut retries = 0;
        let max_retries = 3;

        loop {
            match self
                .bot
                .send_message(chat_id, &escaped_message)
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await
            {
                Ok(_) => {
                    info!("Message sent to chat {}", chat_id);
                    return Ok(());
                }
                Err(e) => {
                    error!(
                        "Failed to send message to chat {} (attempt {}/{}): {}",
                        chat_id,
                        retries + 1,
                        max_retries,
                        e
                    );

                    if retries >= max_retries {
                        // Try sending without markdown as last resort
                        match self.bot.send_message(chat_id, message).await {
                            Ok(_) => {
                                info!("Message sent to chat {} (without markdown)", chat_id);
                                return Ok(());
                            }
                            Err(e2) => {
                                error!(
                                    "Failed to send message to chat {} (without markdown): {}",
                                    chat_id, e2
                                );
                                return Err(anyhow::anyhow!(
                                    "Failed to send message after {} retries: {}",
                                    max_retries,
                                    e
                                ));
                            }
                        }
                    }

                    // Wait before retrying
                    tokio::time::sleep(tokio::time::Duration::from_secs(2 * (retries + 1))).await;
                    retries += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper;
    use starcoin_crypto::HashValue;
    use starcoin_rpc_api::chain::GetTransactionOption;

    #[test]
    pub fn test_parse_p2p_txn_amount() -> Result<()> {
        let rpc_client = RpcClient::connect_websocket("ws://main.seed.starcoin.org:9870")?;
        let txn_view = rpc_client
            .chain_get_transaction(
                HashValue::from_hex_literal(
                    "0x6ed3afdf412404f98fc16d9350b9a19d3258598be5f6b73215a6ab06247b6a53",
                )?,
                Some(GetTransactionOption { decode: true }),
            )?
            .expect("not have any txn");

        let amount = helper::parse_txn_p2p_amount(txn_view.user_transaction.unwrap())?.unwrap();
        assert_eq!(amount, 14630926741510);

        Ok(())
    }
}
