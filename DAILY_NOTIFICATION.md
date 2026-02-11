# 每日通知功能

## 概述

每日通知功能是一个简化的定时任务系统，用于每日23:59自动查询Elasticsearch获取当日交易数据，并发送汇总通知到Telegram。

## 功能特性

- **简单定时**: 每日23:59自动执行查询和通知
- **智能过滤**: 根据配置的最小交易金额过滤大额交易
- **汇总通知**: 发送当日大额交易汇总到Telegram
- **轻量实现**: 使用简单的sleep循环，无需复杂的调度器

## 配置要求

确保在`.env`文件中配置以下环境变量：

```bash
# Elasticsearch配置
ES_URL=http://127.0.0.1:9200
ES_USER_NAME=elastic
ES_PASSWORD=changeme

# Telegram配置
TELEGRAM_BOT_TOKEN=your_bot_token
TELEGRAM_CHAT_ID=your_chat_id

# 最小交易金额（nano单位，默认1 STC）
MIN_TRANSACTION_AMOUNT=1000000000
```

## 工作原理

### 定时执行逻辑

系统使用简单的sleep循环实现定时：

1. **计算下次执行时间**: 计算距离下一个23:59的时间
2. **智能调度**: 如果当前时间已过23:59，则调度到明天
3. **睡眠等待**: 使用`tokio::time::sleep`等待到目标时间
4. **执行任务**: 在23:59执行ES查询和发送通知
5. **循环继续**: 添加60秒延迟后继续下一轮循环

### 执行流程

```
启动服务 → 计算下次23:59时间 → 睡眠等待 → 执行查询 → 发送通知 → 延迟60秒 → 重复
```

## 消息格式

通知消息包含以下信息：

```
📊 每日交易汇总

📅 日期: 2024-01-15
🔢 大额交易总数: 25
💰 交易总额: 150.500000000 STC
```

## 技术实现

### 核心组件

- `DailyNotificationService`: 主服务类，管理定时任务
- `query_daily_transfers`: ES查询函数
- `send_daily_summary`: 通知发送函数
- 简单的时间计算和sleep机制

### 依赖项

```toml
tokio = { version = "1.47.0", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
reqwest = { version = "0.11", features = ["json", "rustls-tls"] }
base64 = "0.21"
```

### 错误处理

- ES连接失败时记录错误日志
- 查询失败时记录详细错误信息
- 通知发送失败时记录错误但不中断服务
- 使用`tracing::error!`记录所有错误

## 监控和日志

系统使用结构化日志记录所有操作：

- `info`: 服务启动、任务执行、查询结果等
- `error`: 查询失败、通知发送失败等错误

日志示例：
```
INFO  Starting Daily Notification Service...
INFO  Next daily notification scheduled for: 2024-01-15 23:59:00 (sleeping for 8h 30m 15s)
INFO  Executing daily notification tasks...
INFO  Starting daily ES query for transfers...
INFO  Retrieved 150 transfer documents
INFO  Sending daily summary notification...
INFO  Daily summary notification sent successfully
```

## 扩展性

### 修改执行时间

要修改执行时间，只需更改`run`方法中的时间计算：

```rust
// 例如改为每天12:00执行
let target_time = now
    .date_naive()
    .and_hms_opt(12, 0, 0)  // 改为12:00
    .unwrap();
```

### 添加更多任务

在循环中添加更多任务：

```rust
// Execute daily tasks
info!("Executing daily notification tasks...");

// 任务1: 查询转账
match query_daily_transfers(config.clone()).await {
    // ... 处理逻辑
}

// 任务2: 其他操作
// ... 其他任务逻辑

// 任务3: 清理操作
// ... 清理逻辑
```

## 故障排除

### 常见问题

1. **服务不执行任务**
   - 检查日志中的时间计算
   - 确认系统时间是否正确
   - 查看是否有错误日志

2. **ES查询失败**
   - 检查ES服务是否运行
   - 验证用户名密码
   - 确认网络连接

3. **通知发送失败**
   - 检查Telegram Bot Token
   - 验证Chat ID
   - 确认网络代理设置

### 调试模式

启用详细日志：
```bash
RUST_LOG=debug cargo run
```

## 性能考虑

- 使用轻量级的sleep机制，资源占用低
- 查询结果限制为1000条记录
- 异步处理避免阻塞
- 可配置的最小金额阈值减少数据处理量

## 安全注意事项

- ES认证信息存储在环境变量中
- 使用HTTPS连接ES（如果配置）
- 限制查询范围避免数据泄露
- 定期轮换认证凭据

## 优势

相比复杂的调度器实现，这种简化方案具有以下优势：

1. **简单可靠**: 没有复杂的依赖和配置
2. **易于调试**: 逻辑清晰，问题容易定位
3. **资源占用低**: 使用简单的sleep，无需额外的调度线程
4. **易于维护**: 代码简洁，修改方便
5. **稳定运行**: 避免了调度器的复杂性和潜在问题
