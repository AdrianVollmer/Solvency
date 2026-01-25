// XSRF token handling
const XSRF_HEADER = "X-XSRF-Token";
const XSRF_FORM_FIELD = "_xsrf_token";

function getXsrfToken(): string | null {
  const meta = document.querySelector('meta[name="xsrf-token"]');
  return meta ? meta.getAttribute("content") : null;
}

function injectXsrfToken(form: HTMLFormElement): void {
  const token = getXsrfToken();
  if (!token) return;

  // Check if already has the field
  if (form.querySelector(`input[name="${XSRF_FORM_FIELD}"]`)) return;

  const input = document.createElement("input");
  input.type = "hidden";
  input.name = XSRF_FORM_FIELD;
  input.value = token;
  form.appendChild(input);
}

function injectXsrfTokenToAllForms(): void {
  const forms = document.querySelectorAll("form");
  for (const form of forms) {
    injectXsrfToken(form as HTMLFormElement);
  }
}

function initXsrfObserver(): void {
  // Watch for dynamically added forms
  const observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      for (const node of mutation.addedNodes) {
        if (node instanceof HTMLFormElement) {
          injectXsrfToken(node);
        } else if (node instanceof HTMLElement) {
          const forms = node.querySelectorAll("form");
          for (const form of forms) {
            injectXsrfToken(form as HTMLFormElement);
          }
        }
      }
    }
  });

  observer.observe(document.body, {
    childList: true,
    subtree: true,
  });
}

// Toast notification system
interface ToastOptions {
  type?: "success" | "error" | "info" | "warning";
  duration?: number;
}

function showToast(message: string, options: ToastOptions = {}): void {
  const { type = "info", duration = 5000 } = options;

  const container = document.getElementById("toast-container");
  if (!container) return;

  const toast = document.createElement("div");

  const baseClasses =
    "p-4 rounded-lg shadow-lg transform transition-all duration-300 ease-out translate-y-2 opacity-0 max-w-sm";
  const typeClasses: Record<string, string> = {
    error: "bg-red-600 text-white",
    success: "bg-green-600 text-white",
    warning: "bg-yellow-500 text-white",
    info: "bg-neutral-800 text-white dark:bg-neutral-700",
  };

  toast.className = `${baseClasses} ${typeClasses[type] || typeClasses.info}`;

  toast.innerHTML = `
    <div class="flex items-start gap-3">
      <div class="flex-1 text-sm">${message}</div>
      <button class="text-white/80 hover:text-white flex-shrink-0" onclick="this.closest('.toast-item')?.remove(); this.parentElement?.parentElement?.remove();">
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
        </svg>
      </button>
    </div>
  `;

  container.appendChild(toast);

  // Animate in
  requestAnimationFrame(() => {
    toast.classList.remove("translate-y-2", "opacity-0");
  });

  // Auto-remove
  if (duration > 0) {
    setTimeout(() => {
      toast.classList.add("translate-y-2", "opacity-0");
      setTimeout(() => toast.remove(), 300);
    }, duration);
  }
}

// Make available globally
(window as unknown as Record<string, unknown>).showToast = showToast;

function initTheme(): void {
  const stored = localStorage.getItem("theme");
  const systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;

  if (stored === "dark" || (!stored && systemDark)) {
    document.documentElement.classList.add("dark");
  } else if (stored === "light") {
    document.documentElement.classList.remove("dark");
  }
}

function toggleTheme(): void {
  const html = document.documentElement;
  const isDark = html.classList.toggle("dark");
  const newTheme = isDark ? "dark" : "light";
  localStorage.setItem("theme", newTheme);

  const token = getXsrfToken();
  const headers: Record<string, string> = {
    "Content-Type": "application/x-www-form-urlencoded",
  };
  if (token) {
    headers[XSRF_HEADER] = token;
  }

  fetch("/settings/theme", {
    method: "POST",
    headers,
    body: `theme=${newTheme}`,
  }).catch(() => {});
}

function initSidebar(): void {
  const toggle = document.getElementById("sidebar-toggle");
  const sidebar = document.getElementById("sidebar");
  const backdrop = document.getElementById("sidebar-backdrop");

  if (!toggle || !sidebar || !backdrop) return;

  const closeSidebar = () => {
    sidebar.classList.add("-translate-x-full");
    backdrop.classList.add("hidden");
  };

  const openSidebar = () => {
    sidebar.classList.remove("-translate-x-full");
    backdrop.classList.remove("hidden");
  };

  toggle.addEventListener("click", () => {
    if (sidebar.classList.contains("-translate-x-full")) {
      openSidebar();
    } else {
      closeSidebar();
    }
  });

  backdrop.addEventListener("click", closeSidebar);
}

function initHtmx(): void {
  // Configure HTMX to include XSRF token in all requests
  document.body.addEventListener("htmx:configRequest", (event: Event) => {
    const token = getXsrfToken();
    if (token) {
      const detail = (event as CustomEvent).detail;
      detail.headers[XSRF_HEADER] = token;
    }
  });

  document.body.addEventListener("htmx:beforeRequest", () => {
    document.body.classList.add("htmx-request");
  });

  document.body.addEventListener("htmx:afterRequest", () => {
    document.body.classList.remove("htmx-request");
  });

  document.body.addEventListener("htmx:responseError", (event: Event) => {
    const detail = (event as CustomEvent).detail;
    console.error("HTMX error:", detail);
  });
}

document.addEventListener("DOMContentLoaded", () => {
  initTheme();
  initSidebar();
  initHtmx();

  // Initialize XSRF protection
  injectXsrfTokenToAllForms();
  initXsrfObserver();

  const themeToggle = document.getElementById("theme-toggle");
  if (themeToggle) {
    themeToggle.addEventListener("click", toggleTheme);
  }
});

window
  .matchMedia("(prefers-color-scheme: dark)")
  .addEventListener("change", (e) => {
    const stored = localStorage.getItem("theme");
    if (!stored || stored === "system") {
      if (e.matches) {
        document.documentElement.classList.add("dark");
      } else {
        document.documentElement.classList.remove("dark");
      }
    }
  });
