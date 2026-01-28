# Solvency

A modern, resource-efficient app for analyzing your personal finances
built with Rust.

<p align="center">
  <a href="docs/articles-desktop-collage.png"><img src="docs/articles-desktop-collage.png" width="45%"></a>
  <a href="docs/articles-desktop-list-collage.png"><img src="docs/articles-desktop-list-collage.png" width="45%"></a>
</p>
<p align="center">
  <a href="docs/articles-desktop-fullscreen-collage.png"><img src="docs/articles-desktop-fullscreen-collage.png" width="60%"></a>
</p>
<p align="center">
  <a href="docs/articles-mobile-collage.png"><img src="docs/articles-mobile-collage.png" width="20%"></a>
  <a href="docs/articles-mobile-list-collage.png"><img src="docs/articles-mobile-list-collage.png" width="20%"></a>
</p>

> [!IMPORTANT]
> **Disclaimer**: This was entirely vibe-coded as part of some personal
> experimentation with this new technology. I know neither Rust nor more
> than the most basic part of CSS. If that is a deal breaker for you, I
> understand.

## Features

The primary use case is running this program as a self-hosted Docker
instance accessed locally or via VPN. There is currently no support for
multiple users.

Market data is pulled from Yahoo Finances.

## Demo

Try it out with Docker (or Podman):

``` bash
docker run --rm --init -p 7070:7070 \
    -e PASSWORD_HASH=DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS \
    ghcr.io/adrianvollmer/solvency:latest
```

To spin up an instance with some demo data:

``` bash
docker run --rm --init -p 7070:7070 \
    -e PASSWORD_HASH=DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS \
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

Copy `.env.example` to `.env` and customize:

``` bash
DATABASE_URL=sqlite://solvency.db
PORT=7070
HOST=0.0.0.0
RUST_LOG=info
PASSWORD_HASH=<argon2-hash>
```

#### Password Authentication

The `PASSWORD_HASH` environment variable is **required**. Set it to an Argon2
hash of your password:

``` bash
# Using argon2 CLI tool
echo -n "your-password" | argon2 $(openssl rand -base64 16) -id -e
```

To explicitly allow unauthenticated access (e.g., for local-only deployments),
set:

``` bash
PASSWORD_HASH=DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS
```

The app will refuse to start if `PASSWORD_HASH` is unset, empty, or invalid.

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
  -e DATABASE_URL=sqlite:///app/data/solvency.db \
  -e PASSWORD_HASH='$argon2id$...' \
  -e PORT=7070 \
  -e HOST=0.0.0.0 \
  --name solvency \
  solvency
```

### Environment Variables for Docker

- `DATABASE_URL`: Path to SQLite database (default:
  `sqlite:///app/data/solvency.db`)
- `PORT`: Port to listen on (default: `7070`)
- `HOST`: IP address to bind to (default: `0.0.0.0`)
- `RUST_LOG`: Log level (default: `info`)
- `PASSWORD_HASH`: **Required.** Argon2 hash for authentication, or
  `DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS` to disable auth

## License

MIT

## Contributors

See CONTRIBUTING.md for development guidelines.
