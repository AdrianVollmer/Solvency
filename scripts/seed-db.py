#!/usr/bin/env python3
"""
Seed the MoneyMapper database with realistic test data.

Usage:
    python scripts/seed-db.py [database_path]

If no path is provided, defaults to 'moneymapper.db' in the current directory.

This script generates 3 years of demo data including:
- Multiple accounts (Cash and Securities)
- Expenses with realistic categories
- Monthly salary with yearly raises and variations
- Trading activities (buys, sells, dividends)
"""

import argparse
import random
import sqlite3
from datetime import datetime, timedelta
from pathlib import Path

# Realistic expense templates by category
EXPENSE_TEMPLATES: dict[str, list[tuple[str, int, int]]] = {
    # (description, min_cents, max_cents)
    "Groceries": [
        ("Whole Foods Market", 4500, 15000),
        ("Trader Joe's", 3000, 8000),
        ("Safeway", 2500, 12000),
        ("Costco", 8000, 25000),
        ("Target - Groceries", 2000, 6000),
        ("Walmart Grocery", 3000, 10000),
        ("Kroger", 2500, 9000),
        ("Aldi", 2000, 5000),
        ("Sprouts Farmers Market", 3500, 8000),
        ("Local Farmers Market", 1500, 4000),
    ],
    "Restaurants": [
        ("Chipotle Mexican Grill", 1200, 2500),
        ("Olive Garden", 2500, 6000),
        ("Chili's", 2000, 5000),
        ("Panera Bread", 1000, 2000),
        ("Subway", 800, 1500),
        ("McDonald's", 600, 1500),
        ("Thai Palace Restaurant", 1500, 3500),
        ("Sushi House", 2000, 5000),
        ("Italian Bistro", 3000, 7000),
        ("Local Diner", 1200, 2500),
        ("Pizza Hut", 1500, 3500),
        ("Taco Bell", 500, 1200),
        ("Five Guys", 1200, 2000),
        ("Cheesecake Factory", 3000, 7000),
    ],
    "Coffee & Snacks": [
        ("Starbucks", 450, 800),
        ("Dunkin'", 300, 600),
        ("Peet's Coffee", 400, 700),
        ("Local Coffee Shop", 350, 650),
        ("7-Eleven", 200, 800),
        ("Vending Machine", 150, 300),
    ],
    "Gas": [
        ("Shell Gas Station", 3500, 7000),
        ("Chevron", 3000, 6500),
        ("Exxon", 3200, 6800),
        ("BP Gas Station", 3000, 6000),
        ("Costco Gas", 2800, 5500),
        ("76 Gas Station", 3100, 6200),
    ],
    "Public Transit": [
        ("Metro Card Reload", 2000, 10000),
        ("Bus Fare", 250, 500),
        ("Uber", 800, 3500),
        ("Lyft", 750, 3200),
        ("Train Ticket", 500, 2500),
        ("Airport Shuttle", 1500, 3000),
    ],
    "Parking": [
        ("Downtown Parking Garage", 1000, 3000),
        ("Airport Parking", 2000, 8000),
        ("Street Parking Meter", 200, 800),
        ("Event Parking", 1500, 4000),
        ("Monthly Parking Pass", 10000, 25000),
    ],
    "Rent/Mortgage": [
        ("Monthly Rent Payment", 120000, 250000),
        ("Mortgage Payment", 150000, 350000),
    ],
    "Maintenance": [
        ("Plumber - Leak Repair", 15000, 40000),
        ("Electrician Service", 10000, 30000),
        ("HVAC Maintenance", 8000, 20000),
        ("Lawn Care Service", 5000, 15000),
        ("House Cleaning Service", 8000, 20000),
        ("Handyman Services", 5000, 15000),
        ("Pest Control", 10000, 25000),
    ],
    "Insurance": [
        ("Renters Insurance", 2000, 5000),
        ("Home Insurance Premium", 8000, 20000),
    ],
    "Utilities": [
        ("Electric Bill - Power Co", 8000, 20000),
        ("Gas Bill - Utility Co", 4000, 12000),
        ("Water & Sewer Bill", 3000, 8000),
        ("Internet - Comcast", 5000, 10000),
        ("Phone Bill - Verizon", 4000, 12000),
        ("Trash Collection", 2000, 5000),
    ],
    "Entertainment": [
        ("Netflix Subscription", 1599, 2299),
        ("Spotify Premium", 999, 1599),
        ("Movie Theater", 1200, 3500),
        ("Concert Tickets", 5000, 20000),
        ("Bowling Alley", 2000, 5000),
        ("Mini Golf", 1500, 3000),
        ("Escape Room", 2500, 4000),
        ("Museum Admission", 1500, 3000),
        ("Disney+ Subscription", 799, 1399),
        ("HBO Max", 1599, 1599),
        ("Video Game Purchase", 2000, 7000),
        ("Steam Game Sale", 500, 3000),
        ("Book Purchase", 1000, 2500),
    ],
    "Shopping": [
        ("Amazon.com", 1500, 15000),
        ("Target", 2000, 10000),
        ("Walmart", 1500, 8000),
        ("Best Buy - Electronics", 5000, 50000),
        ("IKEA", 5000, 30000),
        ("Home Depot", 3000, 20000),
        ("Macy's", 4000, 15000),
        ("Nordstrom", 5000, 25000),
        ("Old Navy", 2000, 8000),
        ("Nike Store", 5000, 15000),
        ("Apple Store", 10000, 150000),
        ("Bed Bath & Beyond", 3000, 10000),
        ("Etsy", 2000, 8000),
    ],
    "Healthcare": [
        ("CVS Pharmacy", 1000, 5000),
        ("Walgreens", 800, 4000),
        ("Doctor Visit Copay", 2000, 5000),
        ("Dentist - Checkup", 5000, 15000),
        ("Eye Exam", 5000, 15000),
        ("Prescription Medication", 1000, 10000),
        ("Urgent Care Visit", 5000, 15000),
        ("Lab Work", 2000, 10000),
        ("Physical Therapy", 3000, 10000),
    ],
    "Other": [
        ("ATM Withdrawal", 2000, 20000),
        ("Bank Fee", 500, 3500),
        ("Gift - Birthday", 2000, 10000),
        ("Charity Donation", 2000, 20000),
        ("Pet Supplies - PetSmart", 2000, 8000),
        ("Vet Visit", 5000, 30000),
        ("Dry Cleaning", 1500, 4000),
        ("Haircut", 2000, 6000),
        ("Gym Membership", 2500, 6000),
        ("Office Supplies", 1000, 5000),
    ],
}

# Tags to create
TAGS: list[tuple[str, str, str]] = [
    # (name, color, style)
    ("recurring", "#8b5cf6", "solid"),
    ("essential", "#ef4444", "solid"),
    ("discretionary", "#10b981", "outline"),
    ("tax-deductible", "#3b82f6", "solid"),
    ("reimbursable", "#f59e0b", "outline"),
    ("subscription", "#ec4899", "striped"),
    ("one-time", "#6b7280", "outline"),
    ("emergency", "#dc2626", "solid"),
    ("planned", "#059669", "solid"),
    ("impulse", "#f97316", "striped"),
    ("gift", "#d946ef", "solid"),
    ("work-related", "#0891b2", "solid"),
]

# Rules for automatic categorization
RULES: list[tuple[str, str, str, str]] = [
    # (name, pattern, action_type, action_value - will be replaced with actual IDs)
    ("Starbucks to Coffee", "(?i)starbucks", "assign_category", "Coffee & Snacks"),
    ("Gas Stations", "(?i)(shell|chevron|exxon|bp|76).*gas", "assign_category", "Gas"),
    ("Uber/Lyft Rides", "(?i)(uber|lyft)", "assign_category", "Public Transit"),
    (
        "Streaming Services",
        "(?i)(netflix|spotify|disney|hbo|hulu)",
        "assign_tag",
        "subscription",
    ),
    ("Amazon Orders", "(?i)amazon", "assign_category", "Shopping"),
    (
        "Grocery Stores",
        "(?i)(whole foods|trader joe|safeway|kroger|aldi)",
        "assign_category",
        "Groceries",
    ),
    ("Pharmacy", "(?i)(cvs|walgreens|rite aid)", "assign_category", "Healthcare"),
    (
        "Fast Food",
        "(?i)(mcdonald|taco bell|wendy|burger king)",
        "assign_category",
        "Restaurants",
    ),
]

# Accounts to create
ACCOUNTS: list[tuple[str, str]] = [
    # (name, account_type)
    ("Primary Checking", "Cash"),
    ("Savings Account", "Cash"),
    ("Credit Card", "Cash"),
    ("Brokerage Account", "Securities"),
    ("Roth IRA", "Securities"),
]

# Trading symbols and their characteristics
TRADING_SYMBOLS: list[tuple[str, str, int, int]] = [
    # (symbol, name, typical_price_cents, volatility_percent)
    ("AAPL", "Apple Inc.", 17500, 15),
    ("MSFT", "Microsoft Corporation", 38000, 12),
    ("GOOGL", "Alphabet Inc.", 14000, 18),
    ("AMZN", "Amazon.com Inc.", 18500, 20),
    ("NVDA", "NVIDIA Corporation", 50000, 30),
    ("VTI", "Vanguard Total Stock Market ETF", 24000, 10),
    ("VOO", "Vanguard S&P 500 ETF", 45000, 10),
    ("BND", "Vanguard Total Bond Market ETF", 7500, 3),
    ("SCHD", "Schwab US Dividend Equity ETF", 7800, 8),
    ("QQQ", "Invesco QQQ Trust", 40000, 15),
]

# Salary configuration
SALARY_CONFIG = {
    "base_annual_salary": 85000_00,  # $85,000 in cents
    "yearly_raise_percent": (2, 5),  # 2-5% yearly raise
    "bonus_months": [3, 12],  # March and December bonuses
    "bonus_percent": (5, 20),  # 5-20% of monthly salary
    "deduction_chance": 0.1,  # 10% chance of deduction
    "deduction_percent": (1, 5),  # 1-5% deduction when it occurs
}


def create_connection(db_path: str) -> sqlite3.Connection:
    """Create a database connection."""
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    return conn


def get_category_map(conn: sqlite3.Connection) -> dict[str, int]:
    """Get mapping of category names to IDs."""
    cursor = conn.execute("SELECT id, name FROM categories")
    return {row["name"]: row["id"] for row in cursor.fetchall()}


def get_tag_map(conn: sqlite3.Connection) -> dict[str, int]:
    """Get mapping of tag names to IDs."""
    cursor = conn.execute("SELECT id, name FROM tags")
    return {row["name"]: row["id"] for row in cursor.fetchall()}


def get_account_map(conn: sqlite3.Connection) -> dict[str, int]:
    """Get mapping of account names to IDs."""
    cursor = conn.execute("SELECT id, name FROM accounts")
    return {row["name"]: row["id"] for row in cursor.fetchall()}


def clear_existing_data(conn: sqlite3.Connection) -> None:
    """Clear existing generated data but keep schema and default categories."""
    print("Clearing existing data...")
    conn.execute("DELETE FROM expense_tags")
    conn.execute("DELETE FROM expenses")
    conn.execute("DELETE FROM trading_activities")
    conn.execute("DELETE FROM market_data")
    conn.execute("DELETE FROM accounts")
    conn.execute("DELETE FROM rules")
    conn.execute("DELETE FROM tags")
    conn.commit()


def seed_tags(conn: sqlite3.Connection) -> None:
    """Insert tags."""
    print("Seeding tags...")
    for name, color, style in TAGS:
        conn.execute(
            "INSERT OR IGNORE INTO tags (name, color, style) VALUES (?, ?, ?)",
            (name, color, style),
        )
    conn.commit()


def seed_rules(conn: sqlite3.Connection) -> None:
    """Insert categorization rules."""
    print("Seeding rules...")
    category_map = get_category_map(conn)
    tag_map = get_tag_map(conn)

    for name, pattern, action_type, action_value in RULES:
        # Resolve action_value to actual ID
        if action_type == "assign_category":
            resolved_value = str(category_map.get(action_value, ""))
        else:  # assign_tag
            resolved_value = str(tag_map.get(action_value, ""))

        if resolved_value:
            conn.execute(
                """INSERT INTO rules (name, pattern, action_type, action_value)
                   VALUES (?, ?, ?, ?)""",
                (name, pattern, action_type, resolved_value),
            )
    conn.commit()


def seed_accounts(conn: sqlite3.Connection) -> None:
    """Insert accounts."""
    print("Seeding accounts...")
    for name, account_type in ACCOUNTS:
        conn.execute(
            "INSERT OR IGNORE INTO accounts (name, account_type) VALUES (?, ?)",
            (name, account_type),
        )
    conn.commit()


def generate_salary_schedule(
    start_date: datetime,
    end_date: datetime,
) -> list[tuple[datetime, int, str]]:
    """Generate monthly salary deposits with yearly raises and variations.

    Returns list of (date, amount_cents, description) tuples.
    """
    salaries: list[tuple[datetime, int, str]] = []
    base_salary = SALARY_CONFIG["base_annual_salary"]
    monthly_salary = base_salary // 12

    current_date = start_date.replace(day=15)  # Salary on 15th of month
    current_year = start_date.year
    current_monthly = monthly_salary

    while current_date <= end_date:
        # Apply yearly raise at the start of each new year
        if current_date.year > current_year:
            raise_pct = random.uniform(
                SALARY_CONFIG["yearly_raise_percent"][0],
                SALARY_CONFIG["yearly_raise_percent"][1],
            )
            current_monthly = int(current_monthly * (1 + raise_pct / 100))
            current_year = current_date.year

        amount = current_monthly
        description = "Salary Deposit"

        # Apply random deduction
        if random.random() < SALARY_CONFIG["deduction_chance"]:
            deduction_pct = random.uniform(
                SALARY_CONFIG["deduction_percent"][0],
                SALARY_CONFIG["deduction_percent"][1],
            )
            amount = int(amount * (1 - deduction_pct / 100))
            description = "Salary Deposit (after deductions)"

        # Add bonus in bonus months
        bonus = 0
        if current_date.month in SALARY_CONFIG["bonus_months"]:
            bonus_pct = random.uniform(
                SALARY_CONFIG["bonus_percent"][0],
                SALARY_CONFIG["bonus_percent"][1],
            )
            bonus = int(current_monthly * bonus_pct / 100)

        salaries.append((current_date, amount, description))

        if bonus > 0:
            bonus_date = current_date + timedelta(days=random.randint(1, 5))
            bonus_desc = (
                "Performance Bonus" if current_date.month == 3 else "Year-End Bonus"
            )
            salaries.append((bonus_date, bonus, bonus_desc))

        # Move to next month
        if current_date.month == 12:
            current_date = current_date.replace(year=current_date.year + 1, month=1)
        else:
            current_date = current_date.replace(month=current_date.month + 1)

    return salaries


def seed_expenses(
    conn: sqlite3.Connection,
    num_expenses: int = 1500,
    days_back: int = 1095,
) -> None:
    """Generate realistic expenses over 3 years."""
    print(f"Seeding {num_expenses} expenses over {days_back} days...")

    category_map = get_category_map(conn)
    tag_map = get_tag_map(conn)
    account_map = get_account_map(conn)

    today = datetime.now().date()
    start_date = today - timedelta(days=days_back)

    # Get account IDs
    checking_id = account_map.get("Primary Checking")
    savings_id = account_map.get("Savings Account")
    credit_card_id = account_map.get("Credit Card")

    # Weight categories by typical spending frequency
    category_weights = {
        "Groceries": 15,
        "Restaurants": 12,
        "Coffee & Snacks": 20,
        "Gas": 8,
        "Public Transit": 10,
        "Parking": 5,
        "Rent/Mortgage": 1,  # Once per month
        "Maintenance": 2,
        "Insurance": 1,
        "Utilities": 2,  # Monthly bills
        "Entertainment": 8,
        "Shopping": 10,
        "Healthcare": 3,
        "Other": 5,
    }

    # Build weighted category list
    weighted_categories: list[str] = []
    for cat, weight in category_weights.items():
        weighted_categories.extend([cat] * weight)

    # Tag assignment probabilities
    tag_probabilities = {
        "recurring": ["Rent/Mortgage", "Insurance", "Utilities"],
        "essential": ["Groceries", "Gas", "Healthcare", "Rent/Mortgage", "Utilities"],
        "discretionary": [
            "Entertainment",
            "Shopping",
            "Restaurants",
            "Coffee & Snacks",
        ],
        "subscription": [],  # Assigned via pattern matching
    }

    expenses_data: list[tuple] = []
    expense_tags_data: list[tuple[int, int]] = []

    for _ in range(num_expenses):
        # Pick random category
        category_name = random.choice(weighted_categories)

        # Get expense template
        if category_name not in EXPENSE_TEMPLATES:
            continue

        templates = EXPENSE_TEMPLATES[category_name]
        description, min_cents, max_cents = random.choice(templates)

        # Generate amount with some variation
        amount_cents = random.randint(min_cents, max_cents)

        # Generate date (weighted toward recent dates)
        days_ago = int(random.paretovariate(1.5) * 30) % days_back
        expense_date = today - timedelta(days=days_ago)

        # Get category ID
        category_id = category_map.get(category_name)

        # Assign account based on expense type
        # Rent/mortgage from checking, small purchases on credit card
        if category_name in ["Rent/Mortgage", "Utilities", "Insurance"]:
            account_id = checking_id
        elif amount_cents > 50000:  # Large purchases from checking
            account_id = checking_id if random.random() < 0.7 else credit_card_id
        else:  # Most purchases on credit card
            account_id = credit_card_id if random.random() < 0.8 else checking_id

        # Add some notes occasionally
        notes = None
        if random.random() < 0.15:
            note_templates = [
                "Paid with credit card",
                "Split with roommate",
                "Business expense - need to submit",
                "Birthday celebration",
                "Weekly shopping",
                "Emergency purchase",
                "Sale item",
                "Used coupon",
            ]
            notes = random.choice(note_templates)

        # Generate payee (the recipient of the payment - i.e. the vendor)
        payee = description.split(" - ")[0]  # Use vendor name as payee

        expenses_data.append(
            (
                expense_date.isoformat(),
                -amount_cents,  # Negative for expenses
                "USD",
                description,
                category_id,
                account_id,
                notes,
                None,  # payer (not set for expenses - we are the payer)
                payee,  # payee (the vendor receiving money)
            )
        )

    # Generate salary with yearly raises
    print("Generating salary schedule with yearly raises...")
    salary_schedule = generate_salary_schedule(
        datetime.combine(start_date, datetime.min.time()),
        datetime.combine(today, datetime.min.time()),
    )
    for salary_date, amount, desc in salary_schedule:
        expenses_data.append(
            (
                salary_date.date().isoformat(),
                amount,  # Positive for income
                "USD",
                desc,
                None,  # No category for income
                checking_id,  # Salary goes to checking
                None,  # No notes
                "Employer Inc.",  # payer
                None,  # payee
            )
        )

    # Add other income entries (refunds, gifts, etc.)
    other_income = [
        ("Tax Refund", 30000, 150000, "IRS"),
        ("Cashback Reward", 1000, 5000, "Credit Card Co"),
        ("Reimbursement", 2000, 10000, "Company ABC"),
        ("Gift Received", 2500, 10000, "Family Member"),
        ("Sold Item", 1500, 8000, "eBay Buyer"),
        ("Freelance Payment", 20000, 80000, "Client LLC"),
    ]

    # Spread other income throughout the period
    num_other_income = days_back // 60  # About 1 other income every 2 months
    for _ in range(num_other_income):
        desc, min_c, max_c, payer = random.choice(other_income)
        amount_cents = random.randint(min_c, max_c)
        days_ago = random.randint(0, days_back)
        expense_date = today - timedelta(days=days_ago)

        # Tax refund goes to savings, others to checking
        income_account = savings_id if desc == "Tax Refund" else checking_id

        expenses_data.append(
            (
                expense_date.isoformat(),
                amount_cents,  # Positive for income
                "USD",
                desc,
                None,  # No category for income
                income_account,
                None,  # No notes
                payer,  # payer (the one sending money to us)
                None,  # payee (not set for income - we are the payee)
            )
        )

    # Insert expenses
    conn.executemany(
        """INSERT INTO expenses
           (date, amount_cents, currency, description, category_id, account_id, notes, payer, payee)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)""",
        expenses_data,
    )
    conn.commit()

    # Get expense IDs and assign tags
    print("Assigning tags to expenses...")
    cursor = conn.execute(
        "SELECT id, description, category_id FROM expenses ORDER BY id"
    )
    expenses = cursor.fetchall()

    # Reverse lookup for category names
    id_to_category = {v: k for k, v in category_map.items()}

    for expense in expenses:
        expense_id = expense["id"]
        description = expense["description"]
        category_id = expense["category_id"]
        category_name = id_to_category.get(category_id, "")

        tags_to_add: set[str] = set()

        # Add tags based on category
        for tag_name, categories in tag_probabilities.items():
            if category_name in categories and random.random() < 0.7:
                tags_to_add.add(tag_name)

        # Pattern-based tags
        desc_lower = description.lower()
        if any(
            s in desc_lower
            for s in ["netflix", "spotify", "disney", "hbo", "subscription"]
        ):
            tags_to_add.add("subscription")
            tags_to_add.add("recurring")

        if "gift" in desc_lower:
            tags_to_add.add("gift")

        # Random additional tags
        if random.random() < 0.1:
            tags_to_add.add("tax-deductible")
        if random.random() < 0.05:
            tags_to_add.add("reimbursable")
        if random.random() < 0.08:
            tags_to_add.add("impulse")

        # Insert expense-tag relationships
        for tag_name in tags_to_add:
            tag_id = tag_map.get(tag_name)
            if tag_id:
                expense_tags_data.append((expense_id, tag_id))

    conn.executemany(
        "INSERT OR IGNORE INTO expense_tags (expense_id, tag_id) VALUES (?, ?)",
        expense_tags_data,
    )
    conn.commit()


def seed_trading_activities(
    conn: sqlite3.Connection,
    days_back: int = 1095,
) -> None:
    """Generate realistic trading activities over 3 years."""
    print(f"Seeding trading activities over {days_back} days...")

    account_map = get_account_map(conn)
    brokerage_id = account_map.get("Brokerage Account")
    ira_id = account_map.get("Roth IRA")

    today = datetime.now().date()
    start_date = today - timedelta(days=days_back)

    activities: list[tuple] = []

    # Track positions for each account to generate realistic sells
    positions: dict[tuple[int, str], int] = {}  # (account_id, symbol) -> quantity

    # Initial deposits to fund the accounts
    initial_deposits = [
        (start_date, brokerage_id, 5000_00, "Initial deposit"),
        (start_date, ira_id, 6000_00, "Annual IRA contribution"),
    ]

    for deposit_date, account_id, amount, notes in initial_deposits:
        activities.append(
            (
                deposit_date.isoformat(),
                "$CASH-USD",
                amount / 100,  # quantity (shares/dollars)
                "DEPOSIT",
                100,  # unit price 1.00 for cash
                "USD",
                0,  # no fee
                account_id,
                notes,
            )
        )

    # Generate monthly contributions
    current_date = start_date
    while current_date <= today:
        # Monthly brokerage contribution (varies)
        contrib_amount = random.randint(500_00, 1500_00)
        activities.append(
            (
                current_date.isoformat(),
                "$CASH-USD",
                contrib_amount / 100,
                "DEPOSIT",
                100,
                "USD",
                0,
                brokerage_id,
                "Monthly investment contribution",
            )
        )

        # Annual IRA contribution (in January)
        if current_date.month == 1 and current_date != start_date:
            ira_contrib = random.randint(6000_00, 6500_00)  # Max IRA contribution
            activities.append(
                (
                    current_date.isoformat(),
                    "$CASH-USD",
                    ira_contrib / 100,
                    "DEPOSIT",
                    100,
                    "USD",
                    0,
                    ira_id,
                    "Annual IRA contribution",
                )
            )

        # Move to next month
        if current_date.month == 12:
            current_date = current_date.replace(year=current_date.year + 1, month=1)
        else:
            current_date = current_date.replace(month=current_date.month + 1)

    # Generate buy/sell transactions spread throughout the period
    # More buys than sells for a growing portfolio
    num_trades = days_back // 10  # About 1 trade every 10 days

    for _ in range(num_trades):
        days_ago = random.randint(0, days_back)
        trade_date = today - timedelta(days=days_ago)

        # Pick account (80% brokerage, 20% IRA)
        account_id = brokerage_id if random.random() < 0.8 else ira_id

        # Pick symbol (weight toward ETFs for IRA)
        if account_id == ira_id:
            # IRA focuses on ETFs
            symbol_pool = [
                s
                for s in TRADING_SYMBOLS
                if s[0] in ["VTI", "VOO", "BND", "SCHD", "QQQ"]
            ]
        else:
            symbol_pool = TRADING_SYMBOLS

        symbol_info = random.choice(symbol_pool)
        symbol, name, base_price, volatility = symbol_info

        # Calculate price with time-based trend and volatility
        years_ago = days_ago / 365
        # Stocks generally trend up over time
        trend_factor = 1 + (years_ago * random.uniform(0.05, 0.15))
        # Add random volatility
        volatility_factor = 1 + random.uniform(-volatility / 100, volatility / 100)
        price_cents = int(base_price / trend_factor * volatility_factor)

        # Determine if buy or sell
        position_key = (account_id, symbol)
        current_qty = positions.get(position_key, 0)

        # 80% chance of buy if no position or small position
        # 30% chance of sell if we have a position
        is_buy = current_qty < 5 or random.random() < 0.7

        if is_buy:
            # Buy 1-10 shares
            quantity = random.randint(1, 10)
            fee_cents = random.choice([0, 0, 0, 100, 495])  # Most trades are free
            positions[position_key] = current_qty + quantity

            activities.append(
                (
                    trade_date.isoformat(),
                    symbol,
                    quantity,
                    "BUY",
                    price_cents,
                    "USD",
                    fee_cents,
                    account_id,
                    f"Buy {quantity} shares of {symbol}",
                )
            )
        elif current_qty > 0:
            # Sell some or all of position
            sell_qty = random.randint(1, min(current_qty, 5))
            fee_cents = random.choice([0, 0, 0, 100, 495])
            positions[position_key] = current_qty - sell_qty

            activities.append(
                (
                    trade_date.isoformat(),
                    symbol,
                    sell_qty,
                    "SELL",
                    price_cents,
                    "USD",
                    fee_cents,
                    account_id,
                    f"Sell {sell_qty} shares of {symbol}",
                )
            )

    # Generate dividends for dividend-paying stocks
    dividend_symbols = ["AAPL", "MSFT", "SCHD", "VTI", "VOO"]

    # Quarterly dividends
    current_date = start_date
    while current_date <= today:
        # Dividends in March, June, September, December
        if current_date.month in [3, 6, 9, 12]:
            for account_id in [brokerage_id, ira_id]:
                for symbol in dividend_symbols:
                    position_key = (account_id, symbol)
                    qty = positions.get(position_key, 0)

                    if qty > 0:
                        # Dividend per share (varies by stock)
                        div_per_share = random.randint(20, 150)  # $0.20 - $1.50
                        total_dividend = qty * div_per_share

                        if total_dividend > 0:
                            activities.append(
                                (
                                    current_date.isoformat(),
                                    symbol,
                                    qty,
                                    "DIVIDEND",
                                    div_per_share,
                                    "USD",
                                    0,
                                    account_id,
                                    f"Quarterly dividend: {qty} shares × ${div_per_share / 100:.2f}",
                                )
                            )

        # Move to next month
        if current_date.month == 12:
            current_date = current_date.replace(year=current_date.year + 1, month=1)
        else:
            current_date = current_date.replace(month=current_date.month + 1)

    # Insert all trading activities
    conn.executemany(
        """INSERT INTO trading_activities
           (date, symbol, quantity, activity_type, unit_price_cents, currency, fee_cents, account_id, notes)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)""",
        activities,
    )
    conn.commit()

    print(f"  Created {len(activities)} trading activities")


def print_summary(conn: sqlite3.Connection) -> None:
    """Print summary of seeded data."""
    print("\n" + "=" * 50)
    print("Database seeded successfully!")
    print("=" * 50)

    cursor = conn.execute("SELECT COUNT(*) as count FROM accounts")
    print(f"Accounts: {cursor.fetchone()['count']}")

    cursor = conn.execute("SELECT COUNT(*) as count FROM categories")
    print(f"Categories: {cursor.fetchone()['count']}")

    cursor = conn.execute("SELECT COUNT(*) as count FROM tags")
    print(f"Tags: {cursor.fetchone()['count']}")

    cursor = conn.execute("SELECT COUNT(*) as count FROM expenses")
    print(f"Expenses: {cursor.fetchone()['count']}")

    cursor = conn.execute("SELECT COUNT(*) as count FROM expense_tags")
    print(f"Expense-Tag relations: {cursor.fetchone()['count']}")

    cursor = conn.execute("SELECT COUNT(*) as count FROM rules")
    print(f"Rules: {cursor.fetchone()['count']}")

    cursor = conn.execute("SELECT COUNT(*) as count FROM trading_activities")
    print(f"Trading activities: {cursor.fetchone()['count']}")

    # Expense summary
    cursor = conn.execute(
        "SELECT SUM(amount_cents) / 100.0 as total FROM expenses WHERE amount_cents < 0"
    )
    total_expenses = cursor.fetchone()["total"] or 0
    print(f"Total spending: ${abs(total_expenses):,.2f}")

    cursor = conn.execute(
        "SELECT SUM(amount_cents) / 100.0 as total FROM expenses WHERE amount_cents > 0"
    )
    total_income = cursor.fetchone()["total"] or 0
    print(f"Total income: ${total_income:,.2f}")

    cursor = conn.execute(
        """SELECT MIN(date) as min_date, MAX(date) as max_date FROM expenses"""
    )
    row = cursor.fetchone()
    print(f"Expense date range: {row['min_date']} to {row['max_date']}")

    # Trading summary
    cursor = conn.execute(
        """SELECT MIN(date) as min_date, MAX(date) as max_date FROM trading_activities"""
    )
    row = cursor.fetchone()
    if row["min_date"]:
        print(f"Trading date range: {row['min_date']} to {row['max_date']}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Seed the MoneyMapper database with realistic demo data."
    )
    parser.add_argument(
        "database",
        nargs="?",
        default="demo.db",
        help="Path to the SQLite database file (default: demo.db)",
    )
    parser.add_argument(
        "--expenses",
        type=int,
        default=1500,
        help="Number of expenses to generate (default: 1500)",
    )
    parser.add_argument(
        "--days",
        type=int,
        default=1095,
        help="Number of days back to generate data (default: 1095, i.e. 3 years)",
    )
    parser.add_argument(
        "--clear",
        action="store_true",
        help="Clear existing data before seeding",
    )

    args = parser.parse_args()

    db_path = Path(args.database)
    if not db_path.exists():
        print(f"Error: Database file not found: {db_path}")
        print("Run the application first to create the database with migrations.")
        return

    print(f"Seeding database: {db_path}")

    conn = create_connection(str(db_path))

    try:
        if args.clear:
            clear_existing_data(conn)

        seed_accounts(conn)
        seed_tags(conn)
        seed_rules(conn)
        seed_expenses(conn, num_expenses=args.expenses, days_back=args.days)
        seed_trading_activities(conn, days_back=args.days)
        print_summary(conn)

    finally:
        conn.close()


if __name__ == "__main__":
    main()
