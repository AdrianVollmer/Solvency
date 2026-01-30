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
        <svg class="w-4 h-4 lucide-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
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

function initDropdowns(): void {
  document.addEventListener("click", (event: MouseEvent) => {
    const target = event.target as HTMLElement;
    const toggle = target.closest("[data-dropdown-toggle]");

    if (toggle) {
      const dropdown = toggle.closest("[data-dropdown]");
      const menu = dropdown?.querySelector(".dropdown-menu");
      if (menu) {
        const isOpen = !menu.classList.contains("hidden");
        closeAllDropdowns();
        if (!isOpen) {
          menu.classList.remove("hidden");
          toggle.setAttribute("aria-expanded", "true");
        }
      }
      return;
    }

    // Click outside any dropdown: close all
    if (!target.closest("[data-dropdown]")) {
      closeAllDropdowns();
    }
  });

  // Close on Escape key
  document.addEventListener("keydown", (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      closeAllDropdowns();
    }
  });
}

function closeAllDropdowns(): void {
  for (const menu of document.querySelectorAll(
    "[data-dropdown] .dropdown-menu",
  )) {
    menu.classList.add("hidden");
  }
  for (const toggle of document.querySelectorAll("[data-dropdown-toggle]")) {
    toggle.setAttribute("aria-expanded", "false");
  }
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

function registerServiceWorker(): void {
  if ("serviceWorker" in navigator) {
    navigator.serviceWorker
      .register("/static/service-worker.js")
      .catch((error) => {
        console.error("Service Worker registration failed:", error);
      });
  }
}

// Alert modal for error/info messages (single OK button)
function initAlertModal(): void {
  const modal = document.getElementById("alert-modal");
  const titleEl = document.getElementById("alert-modal-title");
  const messageEl = document.getElementById("alert-modal-message");
  const okBtn = document.getElementById("alert-ok-btn");
  const backdrop = modal?.querySelector("[data-alert-backdrop]");

  if (!modal || !titleEl || !messageEl || !okBtn) return;

  function closeModal(): void {
    modal!.classList.add("hidden");
  }

  okBtn.addEventListener("click", closeModal);
  backdrop?.addEventListener("click", closeModal);

  document.addEventListener("keydown", (e: KeyboardEvent) => {
    if (e.key === "Escape" && !modal.classList.contains("hidden")) {
      closeModal();
    }
  });

  function showAlertModal(message: string, title = "Error"): void {
    titleEl!.textContent = title;
    messageEl!.textContent = message;
    modal!.classList.remove("hidden");
    okBtn!.focus();
  }

  (window as unknown as Record<string, unknown>).showAlertModal =
    showAlertModal;
}

// Custom confirm modal for destructive actions
function initConfirmModal(): void {
  const modal = document.getElementById("confirm-modal");
  const messageEl = document.getElementById("confirm-modal-message");
  const cancelBtn = document.getElementById("confirm-cancel-btn");
  const actionBtn = document.getElementById("confirm-action-btn");
  const backdrop = modal?.querySelector("[data-confirm-backdrop]");

  if (!modal || !messageEl || !cancelBtn || !actionBtn) return;

  let pendingCallback: (() => void) | null = null;

  function openModal(message: string, onConfirm: () => void): void {
    messageEl!.textContent = message;
    pendingCallback = onConfirm;
    modal!.classList.remove("hidden");
    cancelBtn!.focus();
  }

  function closeModal(): void {
    modal!.classList.add("hidden");
    pendingCallback = null;
  }

  cancelBtn.addEventListener("click", closeModal);
  backdrop?.addEventListener("click", closeModal);

  actionBtn.addEventListener("click", () => {
    const cb = pendingCallback;
    closeModal();
    if (cb) cb();
  });

  document.addEventListener("keydown", (e: KeyboardEvent) => {
    if (e.key === "Escape" && !modal.classList.contains("hidden")) {
      closeModal();
    }
  });

  // Intercept HTMX requests on elements with data-confirm-modal
  document.body.addEventListener("htmx:confirm", (event: Event) => {
    const el = event.target as HTMLElement;
    if (!el.hasAttribute("data-confirm-modal")) return;

    event.preventDefault();
    const message = el.getAttribute("data-confirm-modal") || "Are you sure?";
    const detail = (event as CustomEvent).detail;
    openModal(message, () => {
      detail.issueRequest(true);
    });
  });
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

// Keyboard shortcuts
function initKeyboardShortcuts(): void {
  let pendingKey: string | null = null;
  let pendingTimer: ReturnType<typeof setTimeout> | null = null;

  const navShortcuts: Record<string, string> = {
    d: "/",
    e: "/transactions",
    a: "/spending",
    i: "/import",
    s: "/settings",
  };

  document.addEventListener("keydown", (e: KeyboardEvent) => {
    // Don't trigger in inputs, textareas, or selects
    const tag = (e.target as HTMLElement).tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

    // Don't trigger when modifier keys are held (except Shift for ?)
    if (e.ctrlKey || e.altKey || e.metaKey) return;

    const helpModal = document.getElementById("keyboard-shortcuts-modal");

    // Escape: close help modal or sidebar
    if (e.key === "Escape") {
      if (helpModal && !helpModal.classList.contains("hidden")) {
        helpModal.classList.add("hidden");
        return;
      }
      return;
    }

    // ? : toggle help modal
    if (e.key === "?") {
      if (helpModal) {
        helpModal.classList.toggle("hidden");
      }
      return;
    }

    // If help modal is open, don't process other shortcuts
    if (helpModal && !helpModal.classList.contains("hidden")) return;

    // Two-key "g" sequences for navigation
    if (pendingKey === "g") {
      pendingKey = null;
      if (pendingTimer !== null) {
        clearTimeout(pendingTimer);
        pendingTimer = null;
      }
      const dest = navShortcuts[e.key];
      if (dest) {
        window.location.href = dest;
      }
      return;
    }

    if (e.key === "g") {
      pendingKey = "g";
      pendingTimer = setTimeout(() => {
        pendingKey = null;
        pendingTimer = null;
      }, 1000);
      return;
    }

    // n: new expense (navigate to new transaction page)
    if (e.key === "n") {
      window.location.href = "/transactions/new";
      return;
    }

    // /: focus search input
    if (e.key === "/") {
      const searchInput = document.getElementById(
        "search",
      ) as HTMLInputElement | null;
      if (searchInput) {
        e.preventDefault();
        searchInput.focus();
      }
      return;
    }
  });
}

function initScrollHints(): void {
  function updateHint(el: HTMLElement): void {
    const hasOverflow = el.scrollWidth > el.clientWidth + 1;
    if (!hasOverflow) {
      el.classList.remove(
        "scroll-hint-right",
        "scroll-hint-left",
        "scroll-hint-both",
      );
      return;
    }
    const atLeft = el.scrollLeft <= 1;
    const atRight = el.scrollLeft + el.clientWidth >= el.scrollWidth - 1;

    el.classList.toggle("scroll-hint-right", atLeft && !atRight);
    el.classList.toggle("scroll-hint-left", atRight && !atLeft);
    el.classList.toggle("scroll-hint-both", !atLeft && !atRight);
  }

  const containers = document.querySelectorAll<HTMLElement>(".overflow-x-auto");
  for (const el of containers) {
    updateHint(el);
    el.addEventListener("scroll", () => updateHint(el), { passive: true });
  }

  window.addEventListener("resize", () => {
    for (const el of containers) {
      updateHint(el);
    }
  });
}

// Decorate <select data-color-select> elements with a colored dot
function initColorSelects(): void {
  for (const select of document.querySelectorAll<HTMLSelectElement>(
    "select[data-color-select]",
  )) {
    decorateColorSelect(select);
  }
}

function decorateColorSelect(select: HTMLSelectElement): void {
  function applyDot(): void {
    const color = select.value;
    const dot = encodeURIComponent(
      `<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><circle cx="5" cy="5" r="5" fill="${color}"/></svg>`,
    );
    select.style.backgroundImage = `url("data:image/svg+xml,${dot}")`;
    select.style.backgroundRepeat = "no-repeat";
    select.style.backgroundPosition = "8px center";
    select.style.paddingLeft = "28px";
  }
  select.addEventListener("change", applyDot);
  applyDot();
}

document.addEventListener("DOMContentLoaded", () => {
  initTheme();
  initSidebar();
  initDropdowns();
  initHtmx();
  initAlertModal();
  initConfirmModal();
  initKeyboardShortcuts();
  initScrollHints();
  initColorSelects();
  registerServiceWorker();

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
