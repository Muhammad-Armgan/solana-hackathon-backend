# ============================================================
# Stage 1 — Builder
# Uses the official Rust image (Debian Bookworm) so glibc is
# available for solana-sdk and other C-linked crates.
# ============================================================
FROM rust:1.82-bookworm AS builder

WORKDIR /app

# Install system build dependencies:
#   pkg-config + libssl-dev  → openssl-sys (needed by some transitive deps)
#   clang + llvm             → solana-sdk / BPF toolchain
#   libudev-dev              → solana hardware wallet support
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    clang \
    llvm \
    libudev-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests and lock file first so Cargo can fetch dependencies.
# This layer is cached as long as Cargo.toml / Cargo.lock don't change.
COPY Cargo.toml Cargo.lock ./

# Pre-fetch all dependencies into the Cargo cache
RUN cargo fetch

# Now copy the full source tree and migrations
COPY src ./src
COPY migrations ./migrations

# Build the release binary
RUN cargo build --release --bin backend-rust

# ============================================================
# Stage 2 — Runtime
# Minimal Debian image — only the compiled binary + migrations
# ============================================================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Runtime dependencies:
#   ca-certificates  → HTTPS to Solana RPC, HuggingFace, Cloudinary
#   libssl3          → TLS support
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/backend-rust /app/backend-rust

# Copy migrations so sqlx::migrate! can find them at runtime
# The binary embeds migration SQL at compile time via sqlx::migrate!("./migrations")
# but we keep them here as a reference and for any runtime checks
COPY --from=builder /app/migrations /app/migrations

# Railway injects PORT at runtime — the app reads std::env::var("PORT")
EXPOSE 3000

CMD ["/app/backend-rust"]
