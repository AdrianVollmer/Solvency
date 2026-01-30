// Tag form: color/style dropdowns + live badge preview

function initTagForm(): void {
  const form = document.getElementById("tag-form");
  if (!form) return;

  const nameInput = document.getElementById("tag-name") as HTMLInputElement;
  const colorSelect = document.getElementById("tag-color") as HTMLSelectElement;
  const styleSelect = document.getElementById("tag-style") as HTMLSelectElement;
  const preview = document.getElementById("tag-preview") as HTMLElement;

  if (!nameInput || !colorSelect || !styleSelect || !preview) return;

  function updatePreview(): void {
    const name = nameInput.value || "Tag Name";
    const color = colorSelect.value;
    const style = styleSelect.value;

    let inlineStyle = "";
    if (style === "solid") {
      inlineStyle = `background-color: ${color}; color: ${textColorForSolid(color)};`;
    } else if (style === "striped") {
      inlineStyle =
        `background: repeating-linear-gradient(135deg, ${color}25 0px, ${color}25 2px, ${color}0a 2px, ${color}0a 6px); ` +
        `color: ${ghostTextColor(color)};`;
    } else {
      inlineStyle = `background-color: ${color}1a; color: ${ghostTextColor(color)};`;
    }

    preview.innerHTML =
      `<span class="inline-flex items-center gap-1 text-sm font-medium px-2 py-1 rounded-full" style="${inlineStyle}">${escapeHtml(name)}</span>`;
  }

  // Bind events
  nameInput.addEventListener("input", updatePreview);
  colorSelect.addEventListener("change", updatePreview);
  styleSelect.addEventListener("change", updatePreview);

  // Initial preview
  updatePreview();
}

// --- Colour contrast helpers (mirrors Rust logic in src/models/tag.rs) ---

function parseHex(hex: string): [number, number, number] | null {
  const h = hex.replace(/^#/, "");
  if (h.length < 6) return null;
  const r = parseInt(h.slice(0, 2), 16);
  const g = parseInt(h.slice(2, 4), 16);
  const b = parseInt(h.slice(4, 6), 16);
  if (isNaN(r) || isNaN(g) || isNaN(b)) return null;
  return [r, g, b];
}

function linearize(c: number): number {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

function relativeLuminance(r: number, g: number, b: number): number {
  return 0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b);
}

function contrastRatio(l1: number, l2: number): number {
  const [lighter, darker] = l1 > l2 ? [l1, l2] : [l2, l1];
  return (lighter + 0.05) / (darker + 0.05);
}

function textColorForSolid(hex: string): string {
  const rgb = parseHex(hex);
  if (!rgb) return "white";
  const lum = relativeLuminance(rgb[0], rgb[1], rgb[2]);
  return lum > 0.179 ? "#1e293b" : "white";
}

function ghostTextColor(hex: string): string {
  const rgb = parseHex(hex);
  if (!rgb) return hex;
  let [r, g, b] = rgb;
  for (let i = 0; i < 40; i++) {
    const lum = relativeLuminance(r, g, b);
    if (contrastRatio(1.0, lum) >= 4.5) {
      return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
    }
    r = Math.floor(r * 0.9);
    g = Math.floor(g * 0.9);
    b = Math.floor(b * 0.9);
  }
  return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
}

function escapeHtml(s: string): string {
  const el = document.createElement("span");
  el.textContent = s;
  return el.innerHTML;
}

document.addEventListener("DOMContentLoaded", initTagForm);
