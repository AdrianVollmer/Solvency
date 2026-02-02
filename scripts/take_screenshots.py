#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "playwright",
# ]
# ///
"""
Take screenshots of the Solvency app for documentation.

This script:
1. Builds the release binary (if needed)
2. Starts the server with a temporary database
3. Seeds demo data via seed-db.py
4. Takes screenshots of key pages in multiple viewports and themes
5. Saves them to docs/screenshots/

Usage:
    uv run scripts/take_screenshots.py
    uv run scripts/take_screenshots.py --pages dashboard spending
    uv run scripts/take_screenshots.py --viewports desktop mobile
    uv run scripts/take_screenshots.py --themes light
"""

import argparse
import asyncio
import os
import signal
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
from pathlib import Path
from typing import Literal

from dataclasses import dataclass, field

from playwright.async_api import ViewportSize, async_playwright

ColorScheme = Literal["light", "dark"]

WORKSPACE = Path(__file__).resolve().parent.parent
BINARY = WORKSPACE / "target" / "release" / "solvency"
SEED_SCRIPT = WORKSPACE / "scripts" / "seed-db.py"
OUTPUT_DIR = WORKSPACE / "docs" / "screenshots"

PORT = 9877


@dataclass
class PageConfig:
    """Configuration for a page to screenshot."""

    path: str
    wait_for: list[str] = field(default_factory=list)
    """CSS selectors to wait for before capturing (e.g. chart canvases)."""


PAGES: dict[str, PageConfig] = {
    "dashboard": PageConfig("/"),
    "transactions": PageConfig("/transactions"),
    "spending-category": PageConfig(
        "/spending",
        wait_for=["#category-chart canvas"],
    ),
    "spending-over-time": PageConfig(
        "/spending?tab=time",
        wait_for=["#time-chart canvas"],
    ),
    "spending-monthly": PageConfig(
        "/spending?tab=monthly",
        wait_for=["#monthly-chart canvas"],
    ),
    "spending-flow": PageConfig(
        "/spending?tab=flow",
        wait_for=["#flow-chart canvas"],
    ),
    "balances": PageConfig("/balances"),
    "recurring-expenses": PageConfig("/recurring-expenses"),
    "positions": PageConfig("/trading/positions"),
    "closed-positions": PageConfig("/trading/positions/closed"),
    "net-worth": PageConfig("/trading/net-worth"),
    "accounts": PageConfig("/accounts"),
    "manage": PageConfig("/manage"),
}

VIEWPORTS: dict[str, dict[str, int]] = {
    "desktop": {"width": 1920, "height": 1080},
    "tablet": {"width": 768, "height": 1024},
    "mobile": {"width": 390, "height": 844},
}

THEMES: list[ColorScheme] = ["light", "dark"]


def build_binary() -> None:
    """Build the release binary if it doesn't exist."""
    if BINARY.exists():
        return
    print("Building release binary...")
    subprocess.run(
        ["cargo", "build", "--release", "-p", "solvency"],
        cwd=WORKSPACE,
        check=True,
    )


def init_and_seed_db(db_path: Path) -> None:
    """Run the server briefly to apply migrations, then seed."""
    env = os.environ.copy()
    env["SOLVENCY_DATABASE_URL"] = f"sqlite://{db_path}"
    env["SOLVENCY_HOST"] = "127.0.0.1"
    env["SOLVENCY_PORT"] = str(PORT)
    env["SOLVENCY_PASSWORD_HASH"] = "DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS"

    print("Initializing database (running migrations)...")
    proc = subprocess.Popen(
        [str(BINARY)],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    # Wait for the server to be ready, then kill it
    for _ in range(60):
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{PORT}/", timeout=1):
                break
        except (urllib.error.URLError, OSError):
            pass
        import time

        time.sleep(0.5)
    proc.terminate()
    proc.wait(timeout=5)

    print("Seeding demo data...")
    subprocess.run(
        [sys.executable, str(SEED_SCRIPT), "--clear", str(db_path)],
        check=True,
    )


class ServerProcess:
    """Context manager for the Solvency server."""

    def __init__(self, db_path: Path) -> None:
        self.db_path = db_path
        self.process: subprocess.Popen[bytes] | None = None

    def __enter__(self) -> "ServerProcess":
        env = os.environ.copy()
        env["SOLVENCY_DATABASE_URL"] = f"sqlite://{self.db_path}"
        env["SOLVENCY_HOST"] = "127.0.0.1"
        env["SOLVENCY_PORT"] = str(PORT)
        env["SOLVENCY_PASSWORD_HASH"] = "DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS"
        env["RUST_LOG"] = "warn"

        self.process = subprocess.Popen(
            [str(BINARY)],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        print(f"Started server (PID: {self.process.pid})")
        return self

    def __exit__(self, *args: object) -> None:
        if self.process:
            self.process.send_signal(signal.SIGTERM)
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
            print("Server stopped")


async def wait_for_server(max_retries: int = 60) -> None:
    """Wait for the server to be ready."""
    for _ in range(max_retries):
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{PORT}/", timeout=1):
                print("Server is ready")
                return
        except (urllib.error.URLError, OSError):
            pass
        await asyncio.sleep(0.5)
    raise RuntimeError("Server did not start in time")


async def take_screenshots(
    pages: dict[str, PageConfig],
    viewports: dict[str, dict[str, int]],
    themes: list[ColorScheme],
) -> list[Path]:
    """Take screenshots of all page/viewport/theme combinations."""
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    saved: list[Path] = []

    async with async_playwright() as p:
        browser = await p.chromium.launch()

        for viewport_name, viewport_size in viewports.items():
            for theme in themes:
                context = await browser.new_context(
                    viewport=ViewportSize(
                        width=viewport_size["width"],
                        height=viewport_size["height"],
                    ),
                    color_scheme=theme,
                )
                page = await context.new_page()

                for page_name, cfg in pages.items():
                    url = f"http://127.0.0.1:{PORT}{cfg.path}"
                    await page.goto(url)
                    await page.wait_for_load_state("networkidle")

                    for selector in cfg.wait_for:
                        await page.wait_for_selector(
                            selector, state="attached", timeout=10000
                        )

                    await asyncio.sleep(0.3)

                    filename = f"{page_name}-{viewport_name}-{theme}.png"
                    filepath = OUTPUT_DIR / filename
                    await page.screenshot(path=str(filepath), full_page=False)
                    saved.append(filepath)
                    print(f"  {filename}")

                await context.close()

        await browser.close()

    return saved


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Take Solvency screenshots for documentation."
    )
    parser.add_argument(
        "--pages",
        nargs="+",
        choices=list(PAGES.keys()),
        default=None,
        help="Pages to screenshot (default: all)",
    )
    parser.add_argument(
        "--viewports",
        nargs="+",
        choices=list(VIEWPORTS.keys()),
        default=None,
        help="Viewports to use (default: all)",
    )
    parser.add_argument(
        "--themes",
        nargs="+",
        choices=THEMES,
        default=None,
        help="Themes to use (default: all)",
    )
    return parser.parse_args()


async def main() -> None:
    args = parse_args()

    pages: dict[str, PageConfig] = (
        {k: PAGES[k] for k in args.pages} if args.pages else PAGES
    )
    viewports = (
        {k: VIEWPORTS[k] for k in args.viewports} if args.viewports else VIEWPORTS
    )
    themes = args.themes or THEMES

    total = len(pages) * len(viewports) * len(themes)
    print(
        f"Taking {total} screenshots "
        f"({len(pages)} pages x {len(viewports)} viewports x {len(themes)} themes)"
    )

    build_binary()

    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "solvency.db"

        init_and_seed_db(db_path)

        with ServerProcess(db_path):
            await wait_for_server()
            saved = await take_screenshots(pages, viewports, themes)

    print(f"\nDone! {len(saved)} screenshots saved to {OUTPUT_DIR}/")


if __name__ == "__main__":
    asyncio.run(main())
