# Build stage for frontend assets (runs first so rust-builder can use lucide icons)
FROM node:22-slim AS frontend-builder

WORKDIR /build

# Copy package files and install dependencies
COPY package.json package-lock.json ./
RUN npm ci

# Copy frontend source files and build scripts
COPY scripts ./scripts
COPY src-frontend ./src-frontend
COPY tailwind.config.js ./
COPY templates ./templates

# Copy static assets and build CSS and JS
RUN npm run build:static && npm run generate:icons && npm run build:css && npm run build:ts

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

# Remove src-tauri workspace member (desktop app not needed in Docker)
RUN sed -i 's/members = \[".", "src-tauri"\]/members = ["."]/' Cargo.toml

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
# Touch build.rs to force icon regeneration (icons were empty during dep caching)
RUN touch build.rs src/main.rs && cargo build --release

# Build stage for demo database
FROM python:3.12-slim AS demo-builder

WORKDIR /build

# Copy binary and assets needed to initialize DB
COPY --from=rust-builder /build/target/release/solvency /build/solvency
COPY --from=frontend-builder /build/static /build/static
COPY migrations ./migrations
COPY scripts/seed-db.py /build/seed-db.py

# Initialize database (run app briefly to apply migrations) and seed it
ENV SOLVENCY_DATABASE_URL=sqlite:///build/demo.db
ENV SOLVENCY_HOST=127.0.0.1
ENV SOLVENCY_PORT=9999
ENV SOLVENCY_PASSWORD_HASH=DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS
RUN timeout 5 /build/solvency || true
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
COPY --from=rust-builder /build/target/release/solvency /app/solvency

# Copy static assets from frontend builder
COPY --from=frontend-builder /build/static /app/static

# Copy migrations for runtime execution
COPY --from=rust-builder /build/migrations /app/migrations

# Create data directory
RUN mkdir -p /app/data

# Copy demo database if DEMO build arg is set
# Using a shell trick: copy to real path or /dev/null based on DEMO arg
COPY --from=demo-builder /build/demo.db /tmp/demo.db
RUN if [ "$DEMO" = "true" ]; then \
        mv /tmp/demo.db /app/data/solvency.db; \
    else \
        rm /tmp/demo.db; \
    fi

# Set environment defaults
ENV SOLVENCY_DATABASE_URL=sqlite:///app/data/solvency.db
ENV SOLVENCY_PORT=7070
ENV SOLVENCY_HOST=0.0.0.0
ENV RUST_LOG=info
# Note: SOLVENCY_PASSWORD_HASH must be set at runtime

EXPOSE 7070

CMD ["/app/solvency"]
