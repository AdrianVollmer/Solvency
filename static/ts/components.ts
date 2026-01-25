// Web Components for MoneyMapper
// Uses light DOM for Tailwind CSS compatibility

/**
 * Base class for all custom components.
 * Provides common attribute handling and lifecycle utilities.
 */
abstract class BaseComponent extends HTMLElement {
  /** Attributes to observe for changes */
  static get observedAttributes(): string[] {
    return [];
  }

  /** Get a string attribute with optional default */
  protected getAttr(name: string, defaultValue = ""): string {
    return this.getAttribute(name) ?? defaultValue;
  }

  /** Get a boolean attribute (presence = true) */
  protected getBoolAttr(name: string): boolean {
    return this.hasAttribute(name);
  }

  /** Get a numeric attribute with optional default */
  protected getNumAttr(name: string, defaultValue = 0): number {
    const val = this.getAttribute(name);
    if (val === null) return defaultValue;
    const num = parseFloat(val);
    return isNaN(num) ? defaultValue : num;
  }

  /** Called when the element is added to the DOM */
  connectedCallback(): void {
    this.render();
  }

  /** Called when an observed attribute changes */
  attributeChangedCallback(
    _name: string,
    oldValue: string | null,
    newValue: string | null
  ): void {
    if (oldValue !== newValue) {
      this.render();
    }
  }

  /** Render the component - must be implemented by subclasses */
  protected abstract render(): void;
}

/**
 * Base class for badge-style components (tags, status indicators, etc.)
 * Handles common color and styling patterns.
 */
abstract class BadgeComponent extends BaseComponent {
  /** Convert hex color to CSS with optional opacity */
  protected colorWithOpacity(color: string, opacity: number): string {
    // Handle hex colors like #6366f1
    if (color.startsWith("#") && color.length === 7) {
      const hex = Math.round(opacity * 255)
        .toString(16)
        .padStart(2, "0");
      return color + hex;
    }
    return color;
  }

  /** Base badge classes shared across badge types */
  protected get baseBadgeClasses(): string {
    return "inline-flex items-center text-xs font-medium";
  }
}

// ============================================================================
// mm-badge - Generic colored badge for tags and categories
// ============================================================================

/**
 * A colored badge component supporting solid, outline, and striped styles.
 *
 * @attr color - The badge color (hex, e.g., "#6366f1")
 * @attr style-type - Badge style: "solid", "outline", or "striped" (default: "solid")
 * @attr size - Badge size: "sm" or "md" (default: "sm")
 *
 * @example
 * <mm-badge color="#6366f1" style-type="solid">Groceries</mm-badge>
 * <mm-badge color="#f59e0b" style-type="outline">Travel</mm-badge>
 */
class BadgeElement extends BadgeComponent {
  static override get observedAttributes(): string[] {
    return ["color", "style-type", "size"];
  }

  protected render(): void {
    const color = this.getAttr("color", "#6b7280");
    const styleType = this.getAttr("style-type", "solid");
    const size = this.getAttr("size", "sm");

    const sizeClasses =
      size === "md" ? "px-2.5 py-1 rounded-full" : "px-2 py-0.5 rounded";

    let style = "";

    switch (styleType) {
      case "solid":
        // Full opaque background with white text
        style = `background-color: ${color}; color: white;`;
        break;
      case "outline":
        style = `background-color: transparent; color: ${color}; border: 1.5px solid ${color};`;
        break;
      case "striped":
        style = `background: repeating-linear-gradient(135deg, ${this.colorWithOpacity(color, 0.13)}, ${this.colorWithOpacity(color, 0.13)} 4px, ${this.colorWithOpacity(color, 0.25)} 4px, ${this.colorWithOpacity(color, 0.25)} 8px); color: ${color}; border: 1px solid ${this.colorWithOpacity(color, 0.25)};`;
        break;
      default: // "light" - transparent background with colored text (default)
        style = `background-color: ${this.colorWithOpacity(color, 0.08)}; color: ${color};`;
    }

    this.className = `${this.baseBadgeClasses} ${sizeClasses}`.trim();
    this.style.cssText = style;
  }
}

// ============================================================================
// mm-status-badge - Status indicator with preset type-to-color mapping
// ============================================================================

type StatusType =
  | "buy"
  | "sell"
  | "dividend"
  | "interest"
  | "deposit"
  | "withdrawal"
  | "transfer_in"
  | "transfer_out"
  | "fee"
  | "tax"
  | "split"
  | "add_holding"
  | "remove_holding"
  | "success"
  | "warning"
  | "error"
  | "info";

/**
 * A status badge with preset color mappings for common status types.
 * Uses Tailwind's color classes for consistency with light/dark modes.
 *
 * @attr type - Status type (buy, sell, dividend, fee, success, error, etc.)
 *
 * @example
 * <mm-status-badge type="buy">Buy</mm-status-badge>
 * <mm-status-badge type="dividend">Dividend</mm-status-badge>
 */
class StatusBadgeElement extends BadgeComponent {
  static override get observedAttributes(): string[] {
    return ["type"];
  }

  private static readonly typeStyles: Record<
    StatusType,
    { light: string; dark: string }
  > = {
    // Trading activity types
    buy: {
      light: "bg-green-100 text-green-800",
      dark: "dark:bg-green-900/30 dark:text-green-300",
    },
    add_holding: {
      light: "bg-green-100 text-green-800",
      dark: "dark:bg-green-900/30 dark:text-green-300",
    },
    sell: {
      light: "bg-red-100 text-red-800",
      dark: "dark:bg-red-900/30 dark:text-red-300",
    },
    remove_holding: {
      light: "bg-red-100 text-red-800",
      dark: "dark:bg-red-900/30 dark:text-red-300",
    },
    dividend: {
      light: "bg-blue-100 text-blue-800",
      dark: "dark:bg-blue-900/30 dark:text-blue-300",
    },
    interest: {
      light: "bg-blue-100 text-blue-800",
      dark: "dark:bg-blue-900/30 dark:text-blue-300",
    },
    deposit: {
      light: "bg-emerald-100 text-emerald-800",
      dark: "dark:bg-emerald-900/30 dark:text-emerald-300",
    },
    transfer_in: {
      light: "bg-emerald-100 text-emerald-800",
      dark: "dark:bg-emerald-900/30 dark:text-emerald-300",
    },
    withdrawal: {
      light: "bg-orange-100 text-orange-800",
      dark: "dark:bg-orange-900/30 dark:text-orange-300",
    },
    transfer_out: {
      light: "bg-orange-100 text-orange-800",
      dark: "dark:bg-orange-900/30 dark:text-orange-300",
    },
    fee: {
      light: "bg-yellow-100 text-yellow-800",
      dark: "dark:bg-yellow-900/30 dark:text-yellow-300",
    },
    tax: {
      light: "bg-yellow-100 text-yellow-800",
      dark: "dark:bg-yellow-900/30 dark:text-yellow-300",
    },
    split: {
      light: "bg-purple-100 text-purple-800",
      dark: "dark:bg-purple-900/30 dark:text-purple-300",
    },
    // Generic status types
    success: {
      light: "bg-green-100 text-green-800",
      dark: "dark:bg-green-900/30 dark:text-green-300",
    },
    warning: {
      light: "bg-yellow-100 text-yellow-800",
      dark: "dark:bg-yellow-900/30 dark:text-yellow-300",
    },
    error: {
      light: "bg-red-100 text-red-800",
      dark: "dark:bg-red-900/30 dark:text-red-300",
    },
    info: {
      light: "bg-blue-100 text-blue-800",
      dark: "dark:bg-blue-900/30 dark:text-blue-300",
    },
  };

  protected render(): void {
    const type = this.getAttr("type", "info").toLowerCase() as StatusType;
    const styles = StatusBadgeElement.typeStyles[type] ??
      StatusBadgeElement.typeStyles.info;

    this.className =
      `${this.baseBadgeClasses} px-2.5 py-0.5 rounded-full ${styles.light} ${styles.dark}`;
  }
}

// ============================================================================
// mm-delete-button - HTMX-integrated delete button with confirmation
// ============================================================================

/**
 * A delete button that integrates with HTMX for deletions with confirmation.
 *
 * @attr target - The ID of the element to delete (without #)
 * @attr endpoint - The DELETE endpoint URL
 * @attr confirm - Confirmation message (default: "Are you sure?")
 * @attr label - Accessible label (default: "Delete")
 * @attr swap - HTMX swap mode (default: "outerHTML")
 *
 * @example
 * <mm-delete-button target="expense-123" endpoint="/expenses/123"></mm-delete-button>
 */
class DeleteButtonElement extends BaseComponent {
  static override get observedAttributes(): string[] {
    return ["target", "endpoint", "confirm", "label", "swap"];
  }

  protected render(): void {
    const target = this.getAttr("target");
    const endpoint = this.getAttr("endpoint");
    const confirmMsg = this.getAttr(
      "confirm",
      "Are you sure you want to delete this?"
    );
    const label = this.getAttr("label", "Delete");
    const swap = this.getAttr("swap", "outerHTML");

    this.innerHTML = `
      <button
        hx-delete="${endpoint}"
        hx-target="#${target}"
        hx-swap="${swap}"
        hx-confirm="${confirmMsg}"
        class="p-2 text-neutral-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
        aria-label="${label}">
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
        </svg>
      </button>
    `;

    // Re-process HTMX attributes on the new button
    const htmx = (window as unknown as { htmx?: { process: (el: Element) => void } }).htmx;
    if (htmx) {
      htmx.process(this);
    }
  }
}

// ============================================================================
// mm-money - Formatted money display with automatic coloring
// ============================================================================

/**
 * A money display component that formats amounts and applies appropriate colors.
 *
 * @attr value - The amount in cents (integer)
 * @attr currency - Currency code (default: from data attribute or "USD")
 * @attr show-sign - Whether to show +/- sign (default: false)
 * @attr neutral - Don't color based on sign (default: false)
 *
 * @example
 * <mm-money value="-5000"></mm-money>  <!-- Shows as red -->
 * <mm-money value="5000" show-sign></mm-money>  <!-- Shows +$50.00 in green -->
 * <mm-money value="-5000" neutral></mm-money>  <!-- Shows -$50.00 without color -->
 */
class MoneyElement extends BaseComponent {
  static override get observedAttributes(): string[] {
    return ["value", "currency", "show-sign", "neutral"];
  }

  protected render(): void {
    const cents = this.getNumAttr("value", 0);
    const currency = this.getAttr("currency", "USD");
    const showSign = this.getBoolAttr("show-sign");
    const neutral = this.getBoolAttr("neutral");

    // Format the amount
    const absValue = Math.abs(cents) / 100;
    const formatted = new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: currency,
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }).format(absValue);

    let displayValue = formatted;
    if (cents < 0) {
      displayValue = "-" + formatted;
    } else if (showSign && cents > 0) {
      displayValue = "+" + formatted;
    }

    // Apply coloring
    let colorClasses = "";
    if (!neutral) {
      if (cents < 0) {
        colorClasses = "text-red-600 dark:text-red-400";
      } else if (cents > 0) {
        colorClasses = "text-accent-600 dark:text-accent-400";
      }
    }

    this.className = `font-semibold tabular-nums ${colorClasses}`.trim();
    this.textContent = displayValue;
  }
}

// ============================================================================
// mm-color-dot - Small colored indicator dot
// ============================================================================

/**
 * A small colored dot indicator, commonly used next to category names.
 *
 * @attr color - The dot color (hex)
 * @attr size - Size: "sm", "md", "lg" (default: "md")
 *
 * @example
 * <mm-color-dot color="#6366f1"></mm-color-dot>
 */
class ColorDotElement extends BaseComponent {
  static override get observedAttributes(): string[] {
    return ["color", "size"];
  }

  protected render(): void {
    const color = this.getAttr("color", "#6b7280");
    const size = this.getAttr("size", "md");

    const sizeClasses: Record<string, string> = {
      sm: "w-2 h-2",
      md: "w-3 h-3",
      lg: "w-4 h-4",
    };

    this.className = `inline-block rounded-full ${sizeClasses[size] ?? sizeClasses.md}`;
    this.style.backgroundColor = color;
  }
}

// ============================================================================
// mm-category-badge - Category display with color dot and name
// ============================================================================

/**
 * A category badge showing a colored dot and the category name.
 *
 * @attr color - The category color (hex)
 * @attr name - The category name
 *
 * @example
 * <mm-category-badge color="#6366f1" name="Groceries"></mm-category-badge>
 */
class CategoryBadgeElement extends BadgeComponent {
  static override get observedAttributes(): string[] {
    return ["color", "name"];
  }

  protected render(): void {
    const color = this.getAttr("color", "#6b7280");
    const name = this.getAttr("name", "");

    this.className = `${this.baseBadgeClasses} gap-1.5 px-2.5 py-0.5 rounded-full`;
    this.style.cssText = `background-color: ${this.colorWithOpacity(color, 0.08)}; color: ${color};`;
    this.innerHTML = `<mm-color-dot color="${color}" size="sm"></mm-color-dot><span>${name}</span>`;
  }
}

// ============================================================================
// mm-tag - Tag with optional inline delete button (X icon)
// ============================================================================

/**
 * A tag component with optional inline delete functionality.
 * Similar to mm-badge but includes an optional delete button with X icon.
 *
 * @attr color - The tag color (hex)
 * @attr style-type - Badge style: "solid", "outline", "striped", or "light" (default: "solid")
 * @attr tag-id - ID of the tag (used for delete target)
 * @attr deletable - Include delete button if present
 * @attr endpoint - Delete endpoint URL (required if deletable)
 *
 * @example
 * <mm-tag color="#6366f1" style-type="solid" tag-id="123" deletable endpoint="/tags/123">Travel</mm-tag>
 */
class TagElement extends BadgeComponent {
  static override get observedAttributes(): string[] {
    return ["color", "style-type", "tag-id", "deletable", "endpoint"];
  }

  protected render(): void {
    const color = this.getAttr("color", "#6b7280");
    const styleType = this.getAttr("style-type", "solid");
    const tagId = this.getAttr("tag-id");
    const deletable = this.getBoolAttr("deletable");
    const endpoint = this.getAttr("endpoint");

    // Get the text content (tag name) before we modify innerHTML
    const tagName = this.textContent?.trim() || "";

    let style = "";
    switch (styleType) {
      case "solid":
        style = `background-color: ${color}; color: white;`;
        break;
      case "outline":
        style = `background-color: transparent; color: ${color}; border: 1.5px solid ${color};`;
        break;
      case "striped":
        style = `background: repeating-linear-gradient(135deg, ${this.colorWithOpacity(color, 0.13)}, ${this.colorWithOpacity(color, 0.13)} 4px, ${this.colorWithOpacity(color, 0.25)} 4px, ${this.colorWithOpacity(color, 0.25)} 8px); color: ${color}; border: 1px solid ${this.colorWithOpacity(color, 0.25)};`;
        break;
      default: // "light"
        style = `background-color: ${this.colorWithOpacity(color, 0.08)}; color: ${color};`;
    }

    this.className = `${this.baseBadgeClasses} gap-1 px-2 py-1 rounded-full text-sm`;
    this.style.cssText = style;

    if (tagId) {
      this.id = `tag-${tagId}`;
    }

    if (deletable && endpoint) {
      this.innerHTML = `
        ${tagName}
        <button
          hx-delete="${endpoint}"
          hx-target="#tag-${tagId}"
          hx-swap="outerHTML"
          hx-confirm="Are you sure you want to delete this tag?"
          class="hover:opacity-75">
          <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
          </svg>
        </button>
      `;
      // Re-process HTMX attributes
      const htmx = (window as unknown as { htmx?: { process: (el: Element) => void } }).htmx;
      if (htmx) {
        htmx.process(this);
      }
    } else {
      this.textContent = tagName;
    }
  }
}

// ============================================================================
// mm-empty-state - Empty state display with icon, message, and optional action
// ============================================================================

/**
 * An empty state component for when there's no data to display.
 *
 * @attr icon - Icon type: "folder", "inbox", "file", "search", "chart" (default: "folder")
 * @attr title - Main message text
 * @attr description - Secondary description text (optional)
 * @attr action-url - URL for the action button (optional)
 * @attr action-label - Label for the action button (optional)
 *
 * @example
 * <mm-empty-state
 *     icon="folder"
 *     title="No expenses found"
 *     description="Add your first expense to get started"
 *     action-url="/expenses/new"
 *     action-label="Add Expense">
 * </mm-empty-state>
 */
class EmptyStateElement extends BaseComponent {
  static override get observedAttributes(): string[] {
    return ["icon", "title", "description", "action-url", "action-label"];
  }

  private static readonly icons: Record<string, string> = {
    folder: `<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>`,
    inbox: `<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4"/>`,
    file: `<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>`,
    search: `<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"/>`,
    chart: `<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"/>`,
    activity: `<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6"/>`,
  };

  protected render(): void {
    const iconType = this.getAttr("icon", "folder");
    const title = this.getAttr("title", "No items found");
    const description = this.getAttr("description");
    const actionUrl = this.getAttr("action-url");
    const actionLabel = this.getAttr("action-label");

    const iconPath = EmptyStateElement.icons[iconType] ?? EmptyStateElement.icons.folder;

    let actionHtml = "";
    if (actionUrl && actionLabel) {
      actionHtml = `
        <a href="${actionUrl}" class="mt-4 inline-block btn btn-primary">
          ${actionLabel}
        </a>
      `;
    }

    let descriptionHtml = "";
    if (description) {
      descriptionHtml = `<p class="text-sm text-neutral-500 dark:text-neutral-400 mt-1">${description}</p>`;
    }

    this.className = "block bg-white dark:bg-neutral-800 rounded-xl border border-neutral-200 dark:border-neutral-700 p-8 text-center";
    this.innerHTML = `
      <svg class="w-12 h-12 mx-auto mb-4 text-neutral-300 dark:text-neutral-600" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        ${iconPath}
      </svg>
      <p class="font-medium text-neutral-900 dark:text-white">${title}</p>
      ${descriptionHtml}
      ${actionHtml}
    `;
  }
}

// ============================================================================
// mm-stat-card - Metric display card with label and value
// ============================================================================

/**
 * A stat card component for displaying metrics with label and value.
 *
 * @attr label - The metric label
 * @attr value - The metric value (displayed large)
 * @attr secondary - Secondary value like percentage (optional)
 * @attr trend - Trend indicator: "up", "down", or "neutral" for coloring secondary (optional)
 *
 * @example
 * <mm-stat-card label="Total Value" value="$12,345.67"></mm-stat-card>
 * <mm-stat-card label="Gain/Loss" value="$1,234.56" secondary="+5.2%" trend="up"></mm-stat-card>
 */
class StatCardElement extends BaseComponent {
  static override get observedAttributes(): string[] {
    return ["label", "value", "secondary", "trend"];
  }

  protected render(): void {
    const label = this.getAttr("label", "");
    const value = this.getAttr("value", "-");
    const secondary = this.getAttr("secondary");
    const trend = this.getAttr("trend");

    let trendClasses = "text-neutral-600 dark:text-neutral-400";
    if (trend === "up") {
      trendClasses = "text-green-600 dark:text-green-400";
    } else if (trend === "down") {
      trendClasses = "text-red-600 dark:text-red-400";
    }

    let secondaryHtml = "";
    if (secondary) {
      secondaryHtml = `<p class="text-sm ${trendClasses}">${secondary}</p>`;
    }

    this.className = "block bg-white dark:bg-neutral-800 rounded-lg border border-neutral-200 dark:border-neutral-700 px-4 py-3";
    this.innerHTML = `
      <p class="text-xs text-neutral-500 dark:text-neutral-400">${label}</p>
      <p class="text-xl font-semibold text-neutral-900 dark:text-white">${value}</p>
      ${secondaryHtml}
    `;
  }
}

// ============================================================================
// Register all components
// ============================================================================

function registerComponents(): void {
  customElements.define("mm-badge", BadgeElement);
  customElements.define("mm-status-badge", StatusBadgeElement);
  customElements.define("mm-delete-button", DeleteButtonElement);
  customElements.define("mm-money", MoneyElement);
  customElements.define("mm-color-dot", ColorDotElement);
  customElements.define("mm-category-badge", CategoryBadgeElement);
  customElements.define("mm-tag", TagElement);
  customElements.define("mm-empty-state", EmptyStateElement);
  customElements.define("mm-stat-card", StatCardElement);
}

// Register on DOM ready or immediately if already loaded
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", registerComponents);
} else {
  registerComponents();
}
