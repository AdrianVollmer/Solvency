// API Log Polling for toast notifications

interface ApiErrorSummary {
  id: number;
  symbol: string | null;
  action: string;
  error_message: string;
}

interface PollResponse {
  new_errors: ApiErrorSummary[];
  latest_id: number;
}

interface ShowToastFn {
  (message: string, options?: { type?: string; duration?: number }): void;
}

class ApiLogPoller {
  private lastSeenId: number;
  private pollInterval: number;
  private timer: number | null = null;

  constructor(initialId: number, pollInterval: number = 3000) {
    this.lastSeenId = initialId;
    this.pollInterval = pollInterval;
  }

  start(): void {
    // Don't poll immediately, wait for first interval
    this.timer = window.setInterval(() => this.poll(), this.pollInterval);
  }

  stop(): void {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  private async poll(): Promise<void> {
    try {
      const response = await fetch(
        `/api/api-logs/poll?since_id=${this.lastSeenId}`
      );
      if (!response.ok) return;

      const data: PollResponse = await response.json();

      const showToast = (window as unknown as { showToast?: ShowToastFn })
        .showToast;
      if (showToast) {
        for (const error of data.new_errors) {
          const symbol = error.symbol ? ` for ${error.symbol}` : "";
          const message = `API Error${symbol}: ${error.error_message} <a href="/trading/api-logs/${error.id}" class="underline ml-2">View details</a>`;
          showToast(message, { type: "error", duration: 10000 });
        }
      }

      this.lastSeenId = data.latest_id;
    } catch (e) {
      console.error("Failed to poll for API errors:", e);
    }
  }
}

// Export for use in pages
(window as unknown as { ApiLogPoller?: typeof ApiLogPoller }).ApiLogPoller =
  ApiLogPoller;
