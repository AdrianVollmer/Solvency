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

  fetch("/settings/theme", {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
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
