// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{config::Config, monitor_dispatcher::MonitorDispatcher};
use anyhow::{anyhow, Result};
use starcoin_rpc_api::types::{
    BlockTransactionsView, BlockView, ModuleIdView, SignedUserTransactionView,
    TransactionEventView, TransactionPayloadView,
};
use starcoin_rpc_client::RpcClient;
use starcoin_types::{
    account_address::AccountAddress,
    account_config::{genesis_address, WithdrawEvent},
    block::BlockNumber,
    identifier::Identifier,
    language_storage::ModuleId,
    language_storage::{StructTag, TypeTag},
};
use std::{str::FromStr, sync::Arc, thread::JoinHandle};
use teloxide::{prelude::*, types::Message, Bot};
use tracing::{error, info};

fn parse_txn_p2p_amount(txn_view: SignedUserTransactionView) -> Result<Option<u128>> {
    let txn_payload_view = txn_view
        .raw_txn
        .decoded_payload
        .ok_or(anyhow!("should decode txn"))?;
    let amount = match txn_payload_view {
        TransactionPayloadView::ScriptFunction(function_view) => {
            info!(
                "script function: {:?}::{:?}",
                function_view.module, function_view.function
            );
            if function_view.module
                == ModuleIdView::from(ModuleId::new(
                    AccountAddress::ONE,
                    Identifier::new("TransferScripts")?,
                ))
                && function_view.function == Identifier::new("peer_to_peer_v2")?
            {
                function_view.args[1].0.as_u64().map(|n| n as u128)
            } else {
                None
            }
        }
        _ => None,
    };

    Ok(amount)
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

        match self.rpc_client.chain_get_blocks_by_number(
            Some(start_block),
            start_block - end_block,
            None,
        ) {
            Ok(blocks) => {
                if blocks.is_empty() {
                    self.send_message_to_chat(
                        chat_id,
                        &format!(
                            " No matched transactions found in blocks {} to {}",
                            start_block, end_block
                        ),
                    )
                    .await?;
                    return Ok(());
                }

                let mut matched_txn = Vec::new();
                for block in blocks {
                    match block.body {
                        BlockTransactionsView::Full(txs) => {
                            for tx in txs {
                                let amount = parse_txn_p2p_amount(tx.clone())?;
                                if self.config.min_transaction_amount < amount.unwrap_or(0) {
                                    matched_txn.push((tx.transaction_hash.clone(), amount));
                                }
                            }
                        }
                        _ => continue,
                    }
                }

                if matched_txn.is_empty() {
                    self.send_message_to_chat(
                        chat_id,
                        &format!(
                            " No matched transactions found in blocks {} to {}",
                            start_block, end_block
                        ),
                    )
                    .await?;
                    return Ok(());
                }

                let total_amount = matched_txn
                    .iter()
                    .map(|pair| pair.1.unwrap_or(0))
                    .sum::<u128>();
                self.send_message_to_chat(
                    chat_id,
                    format!(
                        "Transaction Total Amount: {}, txn list: {:?}",
                        total_amount, matched_txn
                    )
                    .as_str(),
                )
                .await?;
            }
            Err(e) => {
                error!("Error fetching transactions: {}", e);
                let message = "❌ Error fetching transactions from database";
                self.send_message_to_chat(chat_id, message).await?;
            }
        }

        Ok(())
    }

    fn get_help_message(&self) -> String {
        r#"
🤖 **Starcoin Monitor Bot Commands**

📊 **Query Commands:**
• `/transactions <start_block> <end_block>` - Get large transactions in block range

📝 **Examples:**
• `/transactions 1000 1100` - Get of large peer to peer transactions from block 1000 to 1100

💡 **Tips:**
• Large transactions are automatically monitored and alerts are sent
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

fn get_withdraw_amount(txn_event_view: &TransactionEventView) -> Result<Option<u128>> {
    let struct_type_tag = match txn_event_view.type_tag.0.clone() {
        TypeTag::Struct(struct_tag) => struct_tag,
        _ => return Ok(None),
    };
    let withdraw_event_tag = StructTag {
        address: genesis_address(),
        module: Identifier::from_str("Account")?,
        name: Identifier::from_str("WithdrawEvent")?,
        type_params: vec![],
    };

    if *struct_type_tag != withdraw_event_tag {
        return Ok(None);
    };

    let withdraw_event = WithdrawEvent::try_from_bytes(txn_event_view.data.0.as_slice())?;
    Ok(Some(withdraw_event.amount()))
}

#[async_trait::async_trait]
impl MonitorDispatcher for TelegramBot {
    async fn dispatch_event(&self, event: &TransactionEventView) -> Result<()> {
        let withdraw_amount = get_withdraw_amount(event)?;
        if withdraw_amount.is_none()
            || withdraw_amount.unwrap() < self.config.min_transaction_amount
        {
            return Ok(());
        };

        let type_tag = match event.type_tag.0.clone() {
            TypeTag::Struct(struct_tag) => struct_tag,
            _ => return Ok(()),
        };

        let withdraw_amount = withdraw_amount.unwrap();
        let msg = format!(
            "🚨[Transfer Over-Limit]: There has an over-limit transaction event being executed here. block number: {:?}, txn_hash: {}, event type: {:?}, withdraw_amount: {:.9}",
            event.block_number.unwrap().0, event.block_hash.unwrap().to_hex_literal(), type_tag.to_canonical_string(), withdraw_amount as f64 / 1e9
        );
        self.send_message(msg.as_str()).await
    }

    async fn dispatch_block(&self, _block: &BlockView) -> Result<()> {
        //self.send_message(format!("block: {:?}", block).as_str())
        //    .await
        Ok(())
    }

    async fn dispatch_stcscan_index_exception(
        &self,
        curr_number: BlockNumber,
        cached_number: BlockNumber,
    ) -> Result<()> {
        let msg = format!(
            "🚨[STCScan Index Exception]: Current OnChain block number: {}, ES Cached index number: {}, Interval: {}",
            curr_number,
            cached_number,
            curr_number - cached_number
        );
        self.send_message(msg.as_str()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use starcoin_crypto::HashValue;
    use starcoin_rpc_api::chain::GetTransactionOption;

    #[test]
    pub fn test_parse_p2p_txn_amount() -> Result<()> {
        let rpc_client = RpcClient::connect_websocket("ws://main.seed.starcoin.org:9870")?;
        let txn_view = rpc_client
            .chain_get_transaction(
                HashValue::from_hex_literal(
                    "0x8fb476816c3d59bb68376f4bf69ac36669d5ab48d03c3d2a4889b81b93e37e3c",
                )?,
                Some(GetTransactionOption { decode: true }),
            )?
            .expect("not have any txn");

        let amount = parse_txn_p2p_amount(txn_view.user_transaction.unwrap())?.unwrap();
        assert_eq!(amount, 10000000000000);

        Ok(())
    }
}
