# Build stage for frontend assets (runs first so rust-builder can use lucide icons)
FROM node:22-slim AS frontend-builder

WORKDIR /build

# Copy package files and install dependencies
COPY package.json package-lock.json ./
RUN npm ci

# Copy frontend source files
COPY scripts/build-ts.js scripts/generate-icons.js ./scripts/
COPY static ./static
COPY tailwind.config.js ./

# Build CSS and JS
RUN npm run build:css && npm run build:ts

# Build stage for Rust binary
FROM rust:1.92-slim-bookworm AS rust-builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first for caching
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./

# Create dummy src to build dependencies (build.rs needs the icons dir
# to exist but an empty dir is fine for the dependency-caching step)
RUN mkdir -p src node_modules/lucide-static/icons \
    && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy Lucide icons from frontend-builder so build.rs can generate icons.rs
COPY --from=frontend-builder /build/node_modules/lucide-static/icons ./node_modules/lucide-static/icons

# Copy actual source and rebuild
COPY src ./src
COPY migrations ./migrations
COPY templates ./templates
RUN touch src/main.rs && cargo build --release

# Build stage for demo database
FROM python:3.12-slim AS demo-builder

WORKDIR /build

# Copy binary and assets needed to initialize DB
COPY --from=rust-builder /build/target/release/moneymapper /build/moneymapper
COPY --from=frontend-builder /build/static /build/static
COPY migrations ./migrations
COPY scripts/seed-db.py /build/seed-db.py

# Initialize database (run app briefly to apply migrations) and seed it
ENV DATABASE_URL=sqlite:///build/demo.db
ENV HOST=127.0.0.1
ENV PORT=9999
RUN timeout 5 /build/moneymapper || true
RUN python3 /build/seed-db.py --clear /build/demo.db

# Final runtime stage
FROM debian:bookworm-slim

ARG DEMO=false

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from rust builder
COPY --from=rust-builder /build/target/release/moneymapper /app/moneymapper

# Copy static assets from frontend builder
COPY --from=frontend-builder /build/static /app/static

# Create data directory
RUN mkdir -p /app/data

# Copy demo database if DEMO build arg is set
# Using a shell trick: copy to real path or /dev/null based on DEMO arg
COPY --from=demo-builder /build/demo.db /tmp/demo.db
RUN if [ "$DEMO" = "true" ]; then \
        mv /tmp/demo.db /app/data/moneymapper.db; \
    else \
        rm /tmp/demo.db; \
    fi

# Set environment defaults
ENV DATABASE_URL=sqlite:///app/data/moneymapper.db
ENV PORT=7070
ENV HOST=0.0.0.0
ENV RUST_LOG=info

EXPOSE 7070

CMD ["/app/moneymapper"]
