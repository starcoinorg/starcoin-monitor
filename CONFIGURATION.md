# Configuration Guide

## Environment Variables

Create a `.env` file in the project root with the following variables:

### Starcoin RPC Configuration
```bash
STARCOIN_RPC_URL=ws://main.seed.starcoin.org:9870
```

### Telegram Bot Configuration
```bash
TELEGRAM_BOT_TOKEN=your_bot_token_here
TELEGRAM_CHAT_ID=your_chat_id_here
TELOXIDE_PROXY=  # Optional proxy URL
```

### Transaction Monitoring Configuration
```bash
MIN_TRANSACTION_AMOUNT=1000000000  # 1 STC in nano units
```

### Elasticsearch Configuration
```bash
ES_URL=http://127.0.0.1:9200
ES_USER_NAME=elastic
ES_PASSWORD=changeme
```

### PostgreSQL Configuration
```bash
POSTGRES_HOST=localhost
POSTGRES_PORT=5432
POSTGRES_DATABASE=starcoin_monitor
POSTGRES_USERNAME=postgres
POSTGRES_PASSWORD=changeme
```

### Chain Network Configuration
```bash
CHAIN_NET=mainnet
```

## Database Setup

Before running the application, you need to create the required database tables:

```sql
-- Create schema
CREATE SCHEMA IF NOT EXISTS monitor;

-- Create chain_info table
CREATE TABLE IF NOT EXISTS monitor.chain_info
(
    chain_net    character varying(256) COLLATE pg_catalog."default" NOT NULL,
    head_block_number  numeric  NOT NULL,
    head_block_hash    numeric  NOT NULL,
    current_index_height  numeric  NOT NULL,
    current_index_state integer NOT NULL default 0,
    update_time DATE NOT NULL DEFAULT now(),
    CONSTRAINT chain_info_pkey PRIMARY KEY (chain_net)
);

-- Create big_transfer table
CREATE TABLE IF NOT EXISTS monitor.big_transfer
(
    id   BIGSERIAL NOT NULL,
    sender     character varying(66) COLLATE pg_catalog."default" NOT NULL,
    token       character varying(256) COLLATE pg_catalog."default" NOT NULL,
    receiver    character varying(66) COLLATE pg_catalog."default" NOT NULL,
    block_number numeric  NOT NULL,
    transaction_hash character varying(256) COLLATE pg_catalog."default" NOT NULL,
    amount      numeric  NOT NULL,
    update_time DATE NOT NULL DEFAULT now(),
    CONSTRAINT address_holder_pkey PRIMARY KEY (id),
    CONSTRAINT address_holder_unq UNIQUE (transaction_hash)
);
```

## Running the Application

1. Set up your environment variables in `.env` file
2. Ensure PostgreSQL is running and accessible
3. Create the database tables as shown above
4. Run the application:

```bash
cargo run
```

The application will:
1. Connect to PostgreSQL database
2. Start the indexer to sync blockchain data
3. Monitor for new blocks and transactions
4. Send Telegram notifications for large transactions
5. Store large transactions in the database
