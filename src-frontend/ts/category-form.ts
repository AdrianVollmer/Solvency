// Category form: icon picker with autocomplete + live preview
// Fetches all icons from /api/icons/all in a single request (browser-cached).

const MAX_DROPDOWN_ITEMS = 50;

async function initCategoryForm(): Promise<void> {
  const iconInput = document.getElementById("category-icon") as HTMLInputElement;
  const colorSelect = document.getElementById("category-color") as HTMLSelectElement;
  const preview = document.getElementById("icon-preview");
  const dropdown = document.getElementById("icon-dropdown");

  if (!iconInput || !colorSelect || !preview || !dropdown) return;

  // Fetch all icon names + SVGs in one request
  let icons: string[];
  let svgMap: Record<string, string>;
  try {
    const resp = await fetch("/api/icons/all");
    svgMap = await resp.json();
    icons = Object.keys(svgMap).sort();
  } catch {
    return; // Degrade gracefully: plain text input
  }

  const iconSet = new Set(icons);
  let highlightIndex = -1;
  let filteredIcons: string[] = [];
  let isOpen = false;

  // Inject an SVG into a container with given CSS classes, applying color via style
  function svgHtml(svg: string, classes: string, color?: string): string {
    if (!svg) return "";
    const colorAttr = color ? ` style="color: ${color};"` : "";
    return svg.replace("<svg", `<svg class="${classes}"${colorAttr}`);
  }

  function updatePreview(): void {
    const name = iconInput.value.trim();
    const color = colorSelect.value;

    if (name && iconSet.has(name)) {
      const icon = svgHtml(svgMap[name], "w-5 h-5 lucide-icon");
      preview.innerHTML =
        `<span class="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0" style="background-color: ${color}20; color: ${color};">` +
        icon +
        `</span>`;
      iconInput.classList.remove("border-red-400", "dark:border-red-500");
    } else if (name) {
      preview.innerHTML =
        `<span class="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 bg-red-50 dark:bg-red-900/20 text-red-400">` +
        `<svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>` +
        `</span>`;
      iconInput.classList.add("border-red-400", "dark:border-red-500");
    } else {
      preview.innerHTML =
        `<span class="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 bg-neutral-100 dark:bg-neutral-700 text-neutral-400">` +
        `<svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>` +
        `</span>`;
      iconInput.classList.remove("border-red-400", "dark:border-red-500");
    }
  }

  function renderDropdown(items: string[]): void {
    const capped = items.slice(0, MAX_DROPDOWN_ITEMS);
    filteredIcons = capped;
    highlightIndex = -1;

    if (items.length === 0) {
      dropdown.innerHTML =
        `<div class="px-3 py-2 text-sm text-neutral-500 dark:text-neutral-400">No matching icons</div>`;
      openDropdown();
      return;
    }

    const color = colorSelect.value;
    let html = "";
    for (let i = 0; i < capped.length; i++) {
      const name = capped[i];
      const iconHtml = svgHtml(svgMap[name] || "", "w-4 h-4 lucide-icon flex-shrink-0", color);
      html +=
        `<button type="button" data-icon-index="${i}" data-icon-name="${name}" class="icon-option flex items-center gap-2 w-full px-3 py-1.5 text-sm text-left rounded-md hover:bg-neutral-100 dark:hover:bg-neutral-700 transition-colors">` +
        `<span class="w-4 h-4 flex-shrink-0" style="color: ${color};">${iconHtml}</span>` +
        `<span class="truncate">${name}</span>` +
        `</button>`;
    }
    if (items.length > MAX_DROPDOWN_ITEMS) {
      html += `<div class="px-3 py-1.5 text-xs text-neutral-400 dark:text-neutral-500">${items.length - MAX_DROPDOWN_ITEMS} more — keep typing to narrow</div>`;
    }
    dropdown.innerHTML = html;
    openDropdown();
  }

  function openDropdown(): void {
    dropdown.classList.remove("hidden");
    isOpen = true;
  }

  function closeDropdown(): void {
    dropdown.classList.add("hidden");
    isOpen = false;
    highlightIndex = -1;
  }

  function selectIcon(name: string): void {
    iconInput.value = name;
    updatePreview();
    closeDropdown();
  }

  function setHighlight(index: number): void {
    const items = dropdown.querySelectorAll<HTMLElement>(".icon-option");
    for (const item of items) {
      item.classList.remove("bg-neutral-100", "dark:bg-neutral-700");
    }
    if (index >= 0 && index < items.length) {
      highlightIndex = index;
      items[index].classList.add("bg-neutral-100", "dark:bg-neutral-700");
      items[index].scrollIntoView({ block: "nearest" });
    }
  }

  // --- Event listeners ---

  iconInput.addEventListener("input", () => {
    const query = iconInput.value.trim().toLowerCase();
    updatePreview();
    if (query === "") {
      renderDropdown(icons);
    } else {
      const matches = icons.filter((name) => name.includes(query));
      renderDropdown(matches);
    }
  });

  iconInput.addEventListener("focus", () => {
    const query = iconInput.value.trim().toLowerCase();
    if (query === "") {
      renderDropdown(icons);
    } else {
      const matches = icons.filter((name) => name.includes(query));
      renderDropdown(matches);
    }
  });

  iconInput.addEventListener("keydown", (e: KeyboardEvent) => {
    if (!isOpen) return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      const next = highlightIndex + 1;
      if (next < filteredIcons.length) {
        setHighlight(next);
      }
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      const prev = highlightIndex - 1;
      if (prev >= 0) {
        setHighlight(prev);
      }
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (highlightIndex >= 0 && highlightIndex < filteredIcons.length) {
        selectIcon(filteredIcons[highlightIndex]);
      }
    } else if (e.key === "Escape") {
      closeDropdown();
    }
  });

  dropdown.addEventListener("click", (e: Event) => {
    const target = (e.target as HTMLElement).closest<HTMLElement>(".icon-option");
    if (!target) return;
    const idx = parseInt(target.dataset.iconIndex || "0", 10);
    if (idx >= 0 && idx < filteredIcons.length) {
      selectIcon(filteredIcons[idx]);
    }
  });

  colorSelect.addEventListener("change", () => {
    updatePreview();
    if (isOpen) {
      const query = iconInput.value.trim().toLowerCase();
      const matches = query === "" ? icons : icons.filter((name) => name.includes(query));
      renderDropdown(matches);
    }
  });

  document.addEventListener("click", (e: Event) => {
    const target = e.target as HTMLElement;
    if (!target.closest("#icon-picker-wrapper")) {
      closeDropdown();
    }
  });

  // Initial preview
  updatePreview();
}

document.addEventListener("DOMContentLoaded", initCategoryForm);
