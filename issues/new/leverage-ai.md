Leverage AI via OpenAI-compatible API to automatically categorize
transactions.

Local models can be very slow, so this must be a long-term background
process. These should be monitorable from a new page. Especially with
thousands uncategorized transactions this might take several hours, so
let's be ready for that.

We should probably do one transaction at time (or batch it? let's find
the right approach), give the model existing categories and force it to
output JSON.

The settings page will need a new section with URL, model, API key, etc to a
model. Let's support OpenAPI, Ollama (this is a hard requirement), Anthropic, etc.
