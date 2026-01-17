#!/usr/bin/env bash
set -e

cd "$(dirname "$0")/.."
DB_PATH="data/app.db"

sqlite3 "$DB_PATH" "INSERT OR IGNORE INTO feeds (url) VALUES ('https://steipete.me/rss.xml');"
echo "Feed added successfully"
