// Categories page - tree view with drag-and-drop

declare const Sortable: any;

function getXsrfToken(): string | null {
  const meta = document.querySelector('meta[name="xsrf-token"]');
  return meta ? meta.getAttribute("content") : null;
}

interface Category {
  id: number;
  name: string;
  parentId: number | null;
  color: string;
  icon: string;
  builtIn: boolean;
}

interface CategoryNode extends Category {
  children: CategoryNode[];
}

let categories: Category[] = [];
let sortableInstances: any[] = [];
let svgMap: Record<string, string> = {};

async function fetchIcons(): Promise<void> {
  try {
    const resp = await fetch("/api/icons/all");
    svgMap = await resp.json();
  } catch {
    // Icons will render empty; non-critical
  }
}

function inlineSvg(svg: string, classes: string): string {
  if (!svg) return "";
  return svg.replace("<svg", `<svg class="${classes}"`);
}

// Decode HTML entities for plain text usage (API calls, form values)
function decodeHtml(html: string): string {
  const txt = document.createElement('textarea');
  txt.innerHTML = html;
  return txt.value;
}

// Build tree structure from flat array
function buildTree(items: Category[], parentId: number | null = null): CategoryNode[] {
  return items
    .filter(item => item.parentId === parentId)
    .map(item => ({
      ...item,
      children: buildTree(items, item.id)
    }));
}

// Render the entire tree
function renderTree(): void {
  const container = document.getElementById('categories-tree');
  const emptyState = document.getElementById('empty-state');
  if (!container || !emptyState) return;

  // Destroy existing sortable instances
  for (const instance of sortableInstances) {
    instance.destroy();
  }
  sortableInstances = [];

  if (categories.length === 0) {
    container.innerHTML = '';
    emptyState.classList.remove('hidden');
    return;
  }

  emptyState.classList.add('hidden');
  const tree = buildTree(categories);
  container.innerHTML = renderNodes(tree);

  initSortable();
}

const ICON_GRIP = '<svg class="w-4 h-4 lucide-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="9" cy="12" r="1"/><circle cx="9" cy="5" r="1"/><circle cx="9" cy="19" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="15" cy="5" r="1"/><circle cx="15" cy="19" r="1"/></svg>';

// Lucide "copy" icon
const ICON_COPY = '<svg class="w-4 h-4 lucide-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>';

// Render nodes recursively
function renderNodes(nodes: CategoryNode[]): string {
  if (nodes.length === 0) return '';

  let html = '';
  for (const node of nodes) {
    const iconSvg = inlineSvg(svgMap[node.icon] || "", "w-5 h-5 lucide-icon");

    const dragHandle = node.builtIn ? '' : `
          <div class="drag-handle cursor-grab active:cursor-grabbing p-1 mr-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 touch-none">
            ${ICON_GRIP}
          </div>`;

    const nameContent = `<a href="/categories/${node.id}" class="flex items-center gap-3 flex-1 min-w-0${node.builtIn ? ' pl-1' : ''}">
            <span class="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0" style="background-color: ${node.color}20; color: ${node.color};">
              ${iconSvg}
            </span>
            <span class="font-medium text-gray-900 dark:text-gray-100">${node.name}</span>
          </a>`;

    const cloneBtn = `<a href="/categories/new?clone_from=${node.id}" title="Clone category"
          class="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 opacity-0 group-hover:opacity-100 transition-opacity">
            ${ICON_COPY}
          </a>`;

    html += `
      <div class="tree-node" data-id="${node.id}">
        <div class="category-row group">
          ${dragHandle}
          ${nameContent}
          ${cloneBtn}
        </div>
        <div class="tree-children" data-parent-id="${node.id}">
          ${renderNodes(node.children)}
        </div>
      </div>
    `;
  }
  return html;
}

// Initialize SortableJS on all containers
function initSortable(): void {
  const containers = document.querySelectorAll('#categories-tree, .tree-children');

  for (const container of containers) {
    const instance = new Sortable(container, {
      group: 'categories',
      handle: '.drag-handle',
      animation: 150,
      fallbackOnBody: true,
      swapThreshold: 0.65,
      ghostClass: 'sortable-ghost',
      dragClass: 'sortable-drag',
      delay: 150,
      delayOnTouchOnly: true,
      touchStartThreshold: 5,
      onEnd: handleDragEnd
    });
    sortableInstances.push(instance);
  }
}

// Handle drag end - update parent
async function handleDragEnd(evt: any): Promise<void> {
  const itemEl = evt.item as HTMLElement;
  const id = parseInt(itemEl.dataset.id || '0');
  const newContainer = evt.to as HTMLElement;

  // Determine new parent
  let newParentId: number | null = null;
  if (newContainer.classList.contains('tree-children')) {
    newParentId = parseInt(newContainer.dataset.parentId || '0');
  }

  // Find category and check if parent changed
  const cat = categories.find(c => c.id === id);
  if (!cat) return;

  // Built-in categories cannot be reparented
  if (cat.builtIn) {
    renderTree();
    return;
  }

  const oldParentId = cat.parentId;
  if (oldParentId !== newParentId) {
    // Update local state optimistically
    cat.parentId = newParentId;

    // Persist to server
    const success = await updateCategoryParent(id, newParentId);
    if (!success) {
      // Revert local state on failure
      cat.parentId = oldParentId;
      // Re-render to show correct state
      renderTree();
    }
  }
}

async function updateCategoryParent(id: number, parentId: number | null): Promise<boolean> {
  const cat = categories.find(c => c.id === id);
  if (!cat) return false;

  const params = new URLSearchParams();
  params.append('name', decodeHtml(cat.name));
  if (parentId !== null) params.append('parent_id', String(parentId));
  params.append('color', cat.color);
  params.append('icon', cat.icon);

  try {
    const headers: Record<string, string> = {
      'Content-Type': 'application/x-www-form-urlencoded',
    };
    const token = getXsrfToken();
    if (token) headers['X-XSRF-Token'] = token;

    const response = await fetch(`/categories/${id}`, {
      method: 'PUT',
      headers,
      body: params,
    });
    if (!response.ok) {
      console.error('Failed to update category:', response.status);
      return false;
    }
    return true;
  } catch (e) {
    console.error('Error updating category:', e);
    return false;
  }
}

// Initialize
async function init(initialCategories: Category[]): Promise<void> {
  categories = initialCategories;
  await fetchIcons();
  renderTree();
}

// Export functions for use in HTML
declare global {
  interface Window {
    categoriesPage: {
      init: (categories: Category[]) => void;
    };
  }
}

window.categoriesPage = {
  init,
};
