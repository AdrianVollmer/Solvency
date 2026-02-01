There are a few places which does a "preview" of transactions, for
example when selecting a range in the "Net Worth" chart. It's a way to
drill into the data.

A similar thing happens when clicking on a bar in the "Monthly Spending"
chart, but it looks completely different.

Let's de-duplicate the code as much as possible, which will lead to a
more maintainable code and a more unified user experience.

Ideally, a table with limited amount of transactions appears, with a
link to the full Transactions page with a proper filter applied. Even
better if that preview table is sortable (JS only should suffice there).

Something similar might be useful for trading activities (perhaps for future
features), so keep that in mind.
