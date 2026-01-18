We need support for market data. Use the crate `yahoo_finance_api` for
that.

It should pull market data for all known symbols, each for all relevant
date ranges. Relevant date ranges are those where the position of that
symbols was greater than zero.

The maximum resolution is one day, so we only need to fetch closing
prices.

Make sure not to hit the API too hard, because we don't want to get
blocked.

Add a menu item called "Market Data" where the user can see for which
date ranges and which symbols we have data, and more importantly, which
are missing.

As a first feature, change the "Positions" table such that it shows the
current value of the position (quantity times current price).
