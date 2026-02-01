# Contributing to Solvency

Contributions are welcome. If you have a bug fix, feature idea, or
improvement, feel free to open a pull request. For larger changes,
consider opening an issue first to discuss the approach.

Please also read [AGENTS.md](AGENTS.md) -- despite its name, it
contains the project conventions, development workflow, and version
management guidelines that apply to all contributors, human or
otherwise.

## Prerequisites

- Rust (1.92+)
- Node.js (for Tailwind CSS)
- pkg-config and libssl-dev (for OpenSSL)

## Building

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

## Running

``` bash
./target/release/solvency
```

The server will start on <http://localhost:7070>. See the
[README](README.md) for the required environment variables.

## Database

Migrations are embedded in the binary and run automatically on startup.

To run migrations manually:

``` bash
sqlx migrate run
```
