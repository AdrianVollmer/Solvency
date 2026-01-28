# Solvency

A modern, resource-efficient app for analyzing your personal finances
built with Rust.

> [!IMPORTANT]
> **Disclaimer**: This was entirely vibe-coded as part of some personal
> experimentation with this new technology. I know neither Rust nor more
> than the most basic part of CSS. If that is a deal breaker for you, I
> understand.

## Features

The primary use case is running this program as a self-hosted Docker
instance accessed locally or via VPN. Your data stays yours! There is currently
no support for multiple users.

- **Transaction tracking** with categories, tags, and multi-currency support
- **Spending analytics** with interactive charts (Sankey diagrams, category breakdowns, time series)
- **Investment portfolio** tracking with positions, realized/unrealized gains, and market data from Yahoo Finance
- **Net worth** calculation and historical trends
- **Automatic categorization** via pattern-matching rules
- **Bulk import/export** of transactions and trading activities from CSV
- **Dark mode** and customizable settings

## Demo

Try it out with Docker (or Podman):

``` bash
docker run --rm --init -p 7070:7070 \
    -e SOLVENCY_PASSWORD_HASH=DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS \
    ghcr.io/adrianvollmer/solvency:latest
```

To spin up an instance with some demo data:

``` bash
docker run --rm --init -p 7070:7070 \
    -e SOLVENCY_PASSWORD_HASH=DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS \
    ghcr.io/adrianvollmer/solvency-demo:latest
```

## Tech Stack

- **Web Framework:** Axum 0.7
- **Templates:** Askama (compile-time checked)
- **Interactivity:** HTMX
- **Database:** SQLite with SQLx
- **RSS Parser:** feed-rs
- **UI:** Tailwind CSS
- **Background Jobs:** tokio-cron-scheduler

## Development

### Prerequisites

- Rust (1.92+)
- Node.js (for Tailwind CSS)
- pkg-config and libssl-dev (for OpenSSL)

### Building

Use the unified build script:

``` bash
# Build everything (frontend + Rust)
./scripts/build.sh

# Or build specific targets:
./scripts/build.sh frontend      # CSS, JS, and icons
./scripts/build.sh css           # Tailwind CSS only
./scripts/build.sh js            # TypeScript only
./scripts/build.sh icons         # PNG icons from SVG
./scripts/build.sh rust          # Rust debug build
./scripts/build.sh rust-release  # Rust release build
./scripts/build.sh clean         # Remove build artifacts
```

Or manually:

``` bash
npm install
npm run build        # Build CSS and JS
cargo build --release
```

Run the application:

``` bash
./target/release/solvency
```

The server will start on <http://localhost:7070>.

### Database

Migrations are embedded in the binary and run automatically on startup.

To manually run migrations:

``` bash
sqlx migrate run
```

### Environment Variables

All environment variables are prefixed with `SOLVENCY_`. Create a `.env` file:

``` bash
SOLVENCY_DATABASE_URL=sqlite://solvency.db
SOLVENCY_PORT=7070
SOLVENCY_HOST=0.0.0.0
SOLVENCY_PASSWORD_HASH=<argon2-hash>
RUST_LOG=info
```

#### Password Authentication

The `SOLVENCY_PASSWORD_HASH` environment variable is **required**. Set it to an
Argon2 hash of your password:

``` bash
# Using argon2 CLI tool
echo -n "your-password" | argon2 $(openssl rand -base64 16) -id -e
```

To explicitly allow unauthenticated access (e.g., for local-only deployments),
set:

``` bash
SOLVENCY_PASSWORD_HASH=DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS
```

The app will refuse to start if `SOLVENCY_PASSWORD_HASH` is unset, empty, or
invalid.

## Docker Deployment

The easiest way to run Solvency is with Docker.

### Using Docker Compose (Recommended)

``` bash
docker compose up -d
```

The app will be available at <http://localhost:7070> with persistent
storage.

### Using Docker directly

There is a publicly available Docker image:
`ghcr.io/adrianvollmer/solvency:latest`

To build the image:

``` bash
docker build -t solvency .
```

Run the container:

``` bash
docker run --rm -d \
  -p 7070:7070 \
  -v solvency-data:/app/data \
  -e SOLVENCY_DATABASE_URL=sqlite:///app/data/solvency.db \
  -e SOLVENCY_PASSWORD_HASH='$argon2id$...' \
  -e SOLVENCY_PORT=7070 \
  -e SOLVENCY_HOST=0.0.0.0 \
  --name solvency \
  solvency
```

### Environment Variables for Docker

- `SOLVENCY_DATABASE_URL`: Path to SQLite database (default:
  `sqlite:///app/data/solvency.db`)
- `SOLVENCY_PORT`: Port to listen on (default: `7070`)
- `SOLVENCY_HOST`: IP address to bind to (default: `0.0.0.0`)
- `SOLVENCY_PASSWORD_HASH`: **Required.** Argon2 hash for authentication, or
  `DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS` to disable auth
- `RUST_LOG`: Log level (default: `info`)

## License

MIT

## Contributors

See CONTRIBUTING.md for development guidelines.
