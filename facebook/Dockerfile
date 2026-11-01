# Multi-stage Dockerfile for Social Listener (100% Rust + Headless Chrome)
# Base Image: Debian Bookworm Slim

# Stage 1: Build binary
FROM rust:slim-bookworm AS builder

WORKDIR /usr/src/social-listener

# Install system build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    sqlite3 \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src

RUN cargo build --release

# Stage 2: Final Runtime Image
FROM debian:bookworm-slim

# Install Google Chrome Stable & essential runtime libs
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    gnupg \
    libsqlite3-0 \
    libssl3 \
    fonts-liberation \
    xdg-utils \
    && curl -fsSL https://dl.google.com/linux/linux_signing_key.pub | gpg --dearmor -o /usr/share/keyrings/google-chrome.gpg \
    && echo "deb [arch=amd64 signed-by=/usr/share/keyrings/google-chrome.gpg] http://dl.google.com/linux/chrome/deb/ stable main" > /etc/apt/sources.list.d/google-chrome.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends google-chrome-stable \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /usr/src/social-listener/target/release/social-listener /app/social-listener
COPY bin/fb /app/bin/fb
RUN chmod +x /app/bin/fb /app/social-listener

# Copy migrations and JS scripts
COPY migrations /app/migrations
COPY js /app/js

# Create runtime profile directory
RUN mkdir -p /app/runtime/profile

EXPOSE 8000

ENV PORT=8000 \
    HOST=0.0.0.0 \
    DATABASE_URL=sqlite:///app/social_listener.db?mode=rwc \
    FB_BIN_PATH=/app/bin/fb \
    CHROME_PROFILE_DIR=/app/runtime/profile

CMD ["/app/social-listener"]
