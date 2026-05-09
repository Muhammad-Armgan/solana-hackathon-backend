# Universal Learning Platform — Rust Backend

Axum-based REST API for the Universal Learning Platform.  
Handles Phantom wallet authentication, quiz/paper generation, file uploads, COIN token economics, and real on-chain SPL token operations on Solana Devnet.

---

## Table of Contents

1. [Tech Stack](#tech-stack)
2. [Project Structure](#project-structure)
3. [Environment Variables](#environment-variables)
4. [Local Development Setup](#local-development-setup)
5. [Database Schema](#database-schema)
6. [Solana Setup](#solana-setup)
7. [Running the Server](#running-the-server)
8. [API Reference](#api-reference)
9. [Token Economics](#token-economics)
10. [Rate Limiting](#rate-limiting)
11. [File Storage](#file-storage)
12. [AI Integration](#ai-integration)
13. [Testing](#testing)

---

## Tech Stack

| Layer       | Technology                          |
|-------------|-------------------------------------|
| Language    | Rust (edition 2021)                 |
| Framework   | Axum 0.7                            |
| Database    | PostgreSQL via SQLx 0.8             |
| Cache       | Redis (optional — rate limiting)    |
| Auth        | JWT + ed25519 Phantom wallet signature |
| Blockchain  | Solana Devnet, SPL Token            |
| AI          | Hugging Face Spaces (FastAPI RAG)   |
| Storage     | Cloudinary (prod) / local (dev)     |

---

## Project Structure

```
backend-rust/
├── Cargo.toml
├── .env                          # Local environment variables (not committed)
├── migrations/
│   ├── 001_initial_schema.sql    # Core tables
│   └── 002_indexes.sql           # Performance indexes
├── src/
│   ├── main.rs                   # Entry point — binds on 0.0.0.0:3000
│   ├── lib.rs                    # Module exports
│   ├── state.rs                  # Shared AppState (Arc<AppState>)
│   ├── config/
│   │   └── settings.rs           # All env vars with defaults
│   ├── database/
│   │   ├── postgres.rs           # PgPool creation
│   │   └── redis.rs              # Redis ConnectionManager (optional)
│   ├── models/
│   │   ├── user.rs               # User, Balance
│   │   ├── quiz.rs               # Quiz
│   │   ├── paper.rs              # Paper
│   │   ├── upload.rs             # Upload
│   │   └── transaction.rs        # Transaction, TokenTransfer
│   ├── services/
│   │   ├── auth_service.rs       # JWT issue/decode
│   │   ├── ai_client.rs          # HTTP calls to HF APIs with fallbacks
│   │   └── file_storage.rs       # Local or Cloudinary storage
│   ├── solana/
│   │   ├── client.rs             # Raw Solana JSON-RPC client
│   │   ├── token.rs              # TokenService — mint, burn, transfer, balance
│   │   ├── instructions.rs       # SPL instruction builders
│   │   ├── program.rs            # Anchor program helpers
│   │   └── mod.rs
│   ├── handlers/
│   │   ├── auth_handler.rs       # /api/auth/*
│   │   ├── quiz_handler.rs       # /api/quiz/*
│   │   ├── paper_handler.rs      # /api/paper/*
│   │   ├── upload_handler.rs     # /api/upload/*
│   │   ├── token_handler.rs      # /api/token/*
│   │   └── solana_handler.rs     # /api/solana/* + /api/token/submit-signed-tx
│   ├── middleware/
│   │   ├── auth.rs               # JWT extraction + require_auth
│   │   ├── rate_limit.rs         # Per-user daily limits via Redis (skipped if Redis unavailable)
│   │   ├── cors.rs               # CORS layer
│   │   └── logging.rs            # Tower HTTP trace layer
│   ├── routes/
│   │   └── api.rs                # Route registration
│   └── utils/
│       ├── error.rs              # AppError → HTTP status codes
│       └── signature.rs          # ed25519 verification with Phantom prefix
└── tests/
    ├── common/mod.rs
    ├── auth_tests.rs
    ├── quiz_tests.rs
    └── token_tests.rs
```

---

## Environment Variables

| Variable                      | Required | Default                         | Description                                    |
|-------------------------------|----------|---------------------------------|------------------------------------------------|
| `DATABASE_URL`                | ✅        | —                               | PostgreSQL connection string                   |
| `REDIS_URL`                   |          | `redis://localhost:6379`        | Redis URL (optional — rate limiting disabled if unavailable) |
| `JWT_SECRET`                  | ✅        | —                               | Secret for signing JWTs                        |
| `JWT_EXPIRY_HOURS`            |          | `24`                            | JWT lifetime in hours                          |
| `SOLANA_RPC_URL`              |          | `https://api.devnet.solana.com` | Solana RPC endpoint                            |
| `SOLANA_WALLET_PRIVATE_KEY`   |          | —                               | Base58 keypair for the platform wallet (mint authority) |
| `SOLANA_TOKEN_MINT_ADDRESS`   |          | —                               | COIN SPL token mint address                    |
| `SOLANA_PROGRAM_ID`           |          | —                               | Deployed Anchor program ID                     |
| `HF_QUIZ_API_URL`             |          | —                               | HuggingFace quiz generation endpoint           |
| `HF_PAPER_API_URL`            |          | —                               | HuggingFace paper generation endpoint          |
| `HF_SCORE_API_URL`            |          | —                               | HuggingFace upload scoring endpoint            |
| `HF_API_TOKEN`                |          | —                               | HuggingFace Bearer token                       |
| `FRONTEND_URL`                |          | `http://localhost:3001`         | Allowed CORS origin                            |
| `STORAGE_PATH`                |          | `./uploads`                     | Local upload directory                         |
| `CLOUDINARY_CLOUD_NAME`       |          | —                               | Enables Cloudinary storage when set            |
| `CLOUDINARY_API_KEY`          |          | —                               | Cloudinary API key                             |
| `CLOUDINARY_API_SECRET`       |          | —                               | Cloudinary API secret                          |
| `MAX_UPLOAD_SIZE`             |          | `10485760` (10 MB)              | Max upload size in bytes                       |
| `RATE_LIMIT_QUIZZES_PER_DAY`  |          | `20`                            | Max quiz generations per user per day          |
| `RATE_LIMIT_PAPERS_PER_DAY`   |          | `10`                            | Max paper generations per user per day         |
| `RATE_LIMIT_UPLOADS_PER_DAY`  |          | `5`                             | Max uploads per user per day                   |
| `QUIZ_COOLDOWN_SECONDS`       |          | `30`                            | Cooldown between quiz generations              |

---

## Local Development Setup

### Prerequisites

- Rust 1.78+ — `rustup update stable`
- PostgreSQL 15+
- Redis 7+ (optional — backend starts without it, rate limiting is disabled)
- WSL2 (if on Windows)

### Steps

```bash
# 1. Enter the backend directory
cd backend-rust

# 2. Configure environment
cp .env .env.local
# Edit .env.local — set DATABASE_URL and JWT_SECRET at minimum

# 3. Start Redis (optional)
sudo service redis-server start

# 4. Build and run (migrations run automatically on startup)
cargo run
```

The server starts on `http://0.0.0.0:3000`.  
Migrations in `./migrations/` are applied automatically via `sqlx::migrate!` on every startup.

---

## Database Schema

| Table             | Purpose                                                        |
|-------------------|----------------------------------------------------------------|
| `users`           | Registered wallets, optional email, signup bonus flag          |
| `balances`        | Cached COIN balance per user (authoritative for debits)        |
| `quizzes`         | Quiz history — subject, questions (JSONB), answers, score      |
| `papers`          | Paper history — subject, payload (JSONB), download URL         |
| `uploads`         | Upload history — filename, AI score, reward tokens             |
| `transactions`    | All COIN movements — earn, spend, send, receive, buy           |
| `token_transfers` | Peer-to-peer COIN transfer records                             |

**Balance sync rule:**  
DB is authoritative for debits (quiz/paper spends). On-chain is authoritative for credits (mints).  
`GET /api/token/balance` syncs DB upward if on-chain > DB, never downward.

---

## Solana Setup

### Deployed Contracts (Devnet)

| Item            | Value                                          |
|-----------------|------------------------------------------------|
| Network         | Devnet                                         |
| Program ID      | `HHfqXJ9sZNNRJZGonfinA8gNY7vLpJ9tyrFQ4eAiQsgK` |
| COIN Mint       | `2YQFHTscEGsNzCbyVDGDdhFDvtNGcaAvBVK97NWDCGBg` |
| Decimals        | 2 (1 COIN = 100 raw units)                     |
| Mint Authority  | `AbDvsMYhystzRi6F7nmj9ThotbcrFHNmSuN6tqEEsh6i` |

### Token Flow

- **Minting** (signup bonus, upload rewards, buy credits): server-side via platform wallet as mint authority
- **Burning** (quiz/paper spends): client-side via Phantom wallet — user signs the burn tx, backend submits to RPC
- **Transfers** (send COIN): client-side SPL transfer — Phantom signs, backend ensures recipient ATA exists first, then submits
- **Fallback**: if Phantom is unavailable, all operations fall back to DB-only deduction

---

## Running the Server

```bash
# Development
cargo run

# Development with auto-reload
cargo install cargo-watch
cargo watch -x run

# Production build
cargo build --release
./target/release/backend-rust
```

---

## API Reference

All protected endpoints require `Authorization: Bearer <jwt>`.

### Authentication

| Method | Path               | Auth | Description                                   |
|--------|--------------------|------|-----------------------------------------------|
| POST   | `/api/auth/signup` | No   | Register wallet, mint 20 COIN signup bonus    |
| POST   | `/api/auth/login`  | No   | Authenticate existing wallet, receive JWT     |
| GET    | `/api/auth/me`     | Yes  | Current user profile + COIN balance           |

**Signup / Login body:**
```json
{
  "wallet_address": "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi",
  "signed_message": "Sign this message to authenticate with Universal Learning Platform.\nWallet: 4vJ9...\nNonce: 1715000000000",
  "signature": "<base58-ed25519-signature>"
}
```

### Quiz

| Method | Path                 | Auth | Cost    | Description                                      |
|--------|----------------------|------|---------|--------------------------------------------------|
| POST   | `/api/quiz/generate` | Yes  | −5 COIN | Generate quiz + deduct COIN from DB              |
| POST   | `/api/quiz/record`   | Yes  | —       | Record quiz row after on-chain Phantom burn      |
| POST   | `/api/quiz/submit`   | Yes  | —       | Submit answers and record score                  |
| GET    | `/api/quiz/history`  | Yes  | —       | Paginated quiz history (`?limit=20&offset=0`)    |

> `generate` is used when Phantom is unavailable (DB-only fallback).  
> `record` is used after a successful on-chain burn — no balance deduction.

### Paper

| Method | Path                              | Auth | Cost    | Description                                      |
|--------|-----------------------------------|------|---------|--------------------------------------------------|
| POST   | `/api/paper/generate`             | Yes  | −5 COIN | Generate verified paper + deduct from DB         |
| POST   | `/api/paper/generate-unverified`  | Yes  | −2 COIN | Generate community paper + deduct from DB        |
| POST   | `/api/paper/record`               | Yes  | —       | Record verified paper after on-chain burn        |
| POST   | `/api/paper/record-unverified`    | Yes  | —       | Record community paper after on-chain burn       |
| GET    | `/api/paper/download/:id`         | Yes  | —       | Retrieve paper content                           |
| GET    | `/api/paper/history`              | Yes  | —       | Paginated paper history                          |

### Upload

| Method | Path                      | Auth | Reward   | Description                                      |
|--------|---------------------------|------|----------|--------------------------------------------------|
| POST   | `/api/upload/submit`      | Yes  | 0–2 COIN | Upload past paper (multipart/form-data, `file` field) |
| GET    | `/api/upload/status/:id`  | Yes  | —        | Check upload processing status                   |
| GET    | `/api/upload/history`     | Yes  | —        | Paginated upload history                         |

### Token

| Method | Path                          | Auth | Description                                      |
|--------|-------------------------------|------|--------------------------------------------------|
| GET    | `/api/token/balance`          | Yes  | COIN balance (syncs on-chain if higher than DB)  |
| POST   | `/api/token/send`             | Yes  | Custodial send — deducts DB, mints to recipient  |
| GET    | `/api/token/history`          | Yes  | Transaction + peer transfer history              |
| POST   | `/api/token/buy`              | Yes  | Buy COIN (PayPal placeholder, 5 COIN per $1)     |

**Send body:**
```json
{ "recipient_wallet": "RecipientBase58Address", "amount": 10 }
```

### Solana

| Method | Path                              | Auth | Description                                      |
|--------|-----------------------------------|------|--------------------------------------------------|
| GET    | `/api/solana/blockhash`           | Yes  | Fresh blockhash for building client-side txs     |
| POST   | `/api/solana/prepare-transfer`    | Yes  | Ensure recipient ATA exists (platform pays rent) |
| POST   | `/api/token/submit-signed-tx`     | Yes  | Submit Phantom-signed burn or transfer tx        |

**submit-signed-tx body:**
```json
{
  "signed_tx": "<base64-serialized-signed-transaction>",
  "tx_type": "burn",
  "amount": 5,
  "purpose": "quiz_spend",
  "recipient_wallet": null
}
```
`tx_type` is `"burn"` for quiz/paper spends or `"transfer"` for peer sends.

### Error Responses

```json
{ "error": "Human-readable message" }
```

| Status | Meaning                                    |
|--------|--------------------------------------------|
| 400    | Bad request — invalid input                |
| 401    | Unauthorized — missing or invalid JWT      |
| 403    | Forbidden — insufficient balance or limit  |
| 404    | Not found                                  |
| 500    | Internal server error                      |

---

## Token Economics

| Operation                  | COIN Change | On-chain         | DB          |
|----------------------------|-------------|------------------|-------------|
| Sign up (new user)         | +20         | ✅ Mint          | ✅          |
| Take verified quiz         | −5          | ✅ Burn (Phantom)| ✅          |
| Generate verified paper    | −5          | ✅ Burn (Phantom)| ✅          |
| Generate community paper   | −2          | ✅ Burn (Phantom)| ✅          |
| Upload paper (score > 0)   | +floor(score)| ✅ Mint         | ✅          |
| Send COIN                  | −amount     | ✅ SPL Transfer  | ✅          |
| Buy credits                | +5×USD      | ✅ Mint          | ✅          |

---

## Rate Limiting

Enforced per user via Redis. If Redis is unavailable, limits are skipped gracefully.

| Endpoint                    | Limit       | Window       |
|-----------------------------|-------------|--------------|
| `/api/quiz/generate`        | 20 requests | per day      |
| `/api/quiz/generate`        | 1 request   | 30s cooldown |
| `/api/paper/generate`       | 10 requests | per day      |
| `/api/upload/submit`        | 5 requests  | per day      |
| `/api/token/send`           | 50 requests | per day      |
| `/api/token/buy`            | 5 requests  | per hour     |

---

## File Storage

**Local** (default): files saved to `STORAGE_PATH` (`./uploads`).  
**Cloudinary** (production): set `CLOUDINARY_CLOUD_NAME`, `CLOUDINARY_API_KEY`, `CLOUDINARY_API_SECRET`.

---

## AI Integration

The backend proxies to a FastAPI RAG system hosted on HuggingFace Spaces.  
All calls have static fallbacks so the API stays functional when the AI service is cold-starting.

| Feature          | HF Endpoint                    | Fallback                        |
|------------------|--------------------------------|---------------------------------|
| Quiz generation  | `HF_QUIZ_API_URL`              | 2 static sample questions       |
| Paper generation | `HF_PAPER_API_URL`             | Static paper title + sections   |
| Upload scoring   | `HF_SCORE_API_URL`             | Score: 72.5, Reward: 1 COIN     |

---

## Testing

```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test auth_tests
cargo test --test quiz_tests
cargo test --test token_tests

# With output
cargo test -- --nocapture
```

Integration tests require PostgreSQL and Redis.  
Set `TEST_DATABASE_URL` and `TEST_REDIS_URL` for a dedicated test database.
