#!/usr/bin/env bash
# setup_database.sh — Create the database and run migrations.
#
# Usage:
#   bash scripts/setup_database.sh
#
# Requires:
#   - PostgreSQL running locally
#   - DATABASE_URL set in .env (or exported in shell)
#   - sqlx-cli: cargo install sqlx-cli --no-default-features --features rustls,postgres

set -euo pipefail

# Load .env if it exists
if [ -f .env ]; then
  export $(grep -v '^#' .env | xargs)
fi

if [ -z "${DATABASE_URL:-}" ]; then
  echo "ERROR: DATABASE_URL is not set. Check your .env file."
  exit 1
fi

echo "DATABASE_URL: ${DATABASE_URL%%@*}@***"

# Create the database if it doesn't exist
echo "Creating database (if not exists)..."
sqlx database create || echo "Database may already exist, continuing..."

# Run all pending migrations
echo "Running migrations..."
sqlx migrate run

echo "Done! Database is ready."
