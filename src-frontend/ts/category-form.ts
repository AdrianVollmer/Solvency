// Category form: icon picker with autocomplete + live preview

function initCategoryForm(): void {
  const iconInput = document.getElementById("category-icon") as HTMLInputElement;
  const colorSelect = document.getElementById("category-color") as HTMLSelectElement;
  const preview = document.getElementById("icon-preview");
  const dropdown = document.getElementById("icon-dropdown");
  const dataEl = document.getElementById("icon-list-data");

  if (!iconInput || !colorSelect || !preview || !dropdown || !dataEl) return;

  const icons: string[] = JSON.parse(dataEl.textContent || "[]");
  const iconSet = new Set(icons);
  let highlightIndex = -1;
  let filteredIcons: string[] = [];
  let isOpen = false;

  function updatePreview(): void {
    const name = iconInput.value.trim();
    const color = colorSelect.value;

    if (name && iconSet.has(name)) {
      preview.innerHTML =
        `<span class="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0" style="background-color: ${color}20; color: ${color};">` +
        `<svg class="w-5 h-5 lucide-icon" viewBox="0 0 24 24"><use href="#${name}"/></svg>` +
        `</span>`;
      iconInput.classList.remove("border-red-400", "dark:border-red-500");
    } else if (name) {
      preview.innerHTML =
        `<span class="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 bg-red-50 dark:bg-red-900/20 text-red-400">` +
        `<svg class="w-5 h-5 lucide-icon" viewBox="0 0 24 24"><use href="#x"/></svg>` +
        `</span>`;
      iconInput.classList.add("border-red-400", "dark:border-red-500");
    } else {
      preview.innerHTML =
        `<span class="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 bg-neutral-100 dark:bg-neutral-700 text-neutral-400">` +
        `<svg class="w-5 h-5 lucide-icon" viewBox="0 0 24 24"><use href="#folder"/></svg>` +
        `</span>`;
      iconInput.classList.remove("border-red-400", "dark:border-red-500");
    }
  }

  function renderDropdown(items: string[]): void {
    filteredIcons = items;
    highlightIndex = -1;

    if (items.length === 0) {
      dropdown.innerHTML =
        `<div class="px-3 py-2 text-sm text-neutral-500 dark:text-neutral-400">No matching icons</div>`;
      openDropdown();
      return;
    }

    const color = colorSelect.value;
    let html = "";
    for (let i = 0; i < items.length; i++) {
      const name = items[i];
      html +=
        `<button type="button" data-icon-index="${i}" class="icon-option flex items-center gap-2 w-full px-3 py-1.5 text-sm text-left rounded-md hover:bg-neutral-100 dark:hover:bg-neutral-700 transition-colors">` +
        `<svg class="w-4 h-4 lucide-icon flex-shrink-0" viewBox="0 0 24 24" style="color: ${color};"><use href="#${name}"/></svg>` +
        `<span class="truncate">${name}</span>` +
        `</button>`;
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
    // Re-render dropdown icon colors if open
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
