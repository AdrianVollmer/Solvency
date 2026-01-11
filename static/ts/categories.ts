// Categories page - tree view with drag-and-drop

declare const Sortable: any;

interface Category {
  id: number;
  name: string;
  parentId: number | null;
  color: string;
  icon: string;
}

interface CategoryNode extends Category {
  children: CategoryNode[];
}

const CATEGORY_ICONS: string[] = [
  'folder', 'house', 'car', 'utensils', 'shopping-cart', 'shopping-bag',
  'heart', 'zap', 'film', 'music', 'gamepad-2', 'plane', 'train-front', 'bus',
  'fuel', 'parking-meter', 'gift', 'coffee', 'pizza', 'apple', 'beer',
  'wine', 'pill', 'stethoscope', 'graduation-cap', 'book', 'briefcase',
  'building', 'landmark', 'banknote', 'credit-card', 'wallet', 'piggy-bank',
  'receipt', 'calculator', 'percent', 'tag', 'tags', 'scissors', 'shirt',
  'smartphone', 'tv', 'monitor', 'laptop', 'headphones', 'camera',
  'wrench', 'hammer', 'paintbrush', 'brush', 'baby', 'dog', 'cat',
  'tree-deciduous', 'flower', 'sun', 'cloud', 'umbrella', 'sparkles', 'star',
  'ellipsis', 'circle', 'square', 'triangle', 'hexagon'
];

let categories: Category[] = [];
let editingId: number | null = null;
let sortableInstances: any[] = [];

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
  updateParentSelect();
}

// Render nodes recursively
function renderNodes(nodes: CategoryNode[]): string {
  if (nodes.length === 0) return '';

  let html = '';
  for (const node of nodes) {
    // Note: node.name is already HTML-escaped by Askama, safe for innerHTML
    // But for JS string context (onclick), we need to escape quotes
    const nameForJs = node.name.replace(/'/g, "\\'");
    html += `
      <div class="tree-node" data-id="${node.id}">
        <div class="category-row group">
          <div class="drag-handle cursor-grab active:cursor-grabbing p-1 mr-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 touch-none">
            <svg class="w-4 h-4 lucide-icon" viewBox="0 0 24 24"><use href="#grip-vertical"/></svg>
          </div>
          <div class="flex items-center gap-3 flex-1 min-w-0 cursor-pointer" onclick="window.categoriesPage.openEditModal(${node.id})">
            <span class="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0" style="background-color: ${node.color}20; color: ${node.color};">
              <svg class="w-5 h-5 lucide-icon" viewBox="0 0 24 24"><use href="#${node.icon}"/></svg>
            </span>
            <span class="font-medium text-gray-900 dark:text-gray-100">${node.name}</span>
          </div>
          <button onclick="event.stopPropagation(); window.categoriesPage.deleteCategory(${node.id}, '${nameForJs}')"
            class="p-2 text-gray-400 hover:text-red-600 dark:hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity">
            <svg class="w-5 h-5 lucide-icon" viewBox="0 0 24 24"><use href="#trash-2"/></svg>
          </button>
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

// Update parent select dropdown
function updateParentSelect(): void {
  const select = document.getElementById('category-parent') as HTMLSelectElement | null;
  if (!select) return;

  const currentId = editingId;

  // Build options with indentation showing hierarchy
  let options = '<option value="">None (Root Level)</option>';

  function addOptions(nodes: CategoryNode[], depth: number = 0): void {
    for (const node of nodes) {
      // Skip the category being edited and its descendants
      if (node.id === currentId) continue;

      const indent = '\u00A0\u00A0'.repeat(depth);
      const prefix = depth > 0 ? '└ ' : '';
      // node.name is already HTML-escaped by Askama
      options += `<option value="${node.id}">${indent}${prefix}${node.name}</option>`;
      addOptions(node.children, depth + 1);
    }
  }

  const tree = buildTree(categories);
  addOptions(tree);
  select.innerHTML = options;
}

// Icon picker
function initIconPicker(): void {
  const grid = document.getElementById('icon-grid');
  if (!grid) return;

  for (const icon of CATEGORY_ICONS) {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.dataset.icon = icon;
    btn.className = 'icon-btn w-9 h-9 flex items-center justify-center hover:bg-gray-100 dark:hover:bg-gray-700 rounded transition-colors';
    btn.title = icon;
    btn.innerHTML = `<svg class="w-5 h-5 lucide-icon" viewBox="0 0 24 24"><use href="#${icon}"/></svg>`;
    btn.onclick = () => selectIcon(icon);
    grid.appendChild(btn);
  }
}

function toggleIconPicker(): void {
  const picker = document.getElementById('icon-picker');
  if (!picker) return;

  const isHidden = picker.classList.toggle('hidden');
  if (!isHidden) {
    // Reset search when opening
    const search = document.getElementById('icon-search') as HTMLInputElement | null;
    if (search) {
      search.value = '';
      filterIcons('');
      search.focus();
    }
  }
}

function filterIcons(query: string): void {
  const q = query.toLowerCase().trim();
  const buttons = document.querySelectorAll('#icon-grid .icon-btn') as NodeListOf<HTMLElement>;
  let visibleCount = 0;

  for (const btn of buttons) {
    const icon = btn.dataset.icon || '';
    const matches = !q || icon.includes(q);
    btn.classList.toggle('hidden', !matches);
    if (matches) visibleCount++;
  }

  const noIconsMsg = document.getElementById('no-icons-msg');
  if (noIconsMsg) {
    noIconsMsg.classList.toggle('hidden', visibleCount > 0);
  }
}

function selectIcon(icon: string): void {
  const iconInput = document.getElementById('category-icon') as HTMLInputElement | null;
  const selectedIcon = document.getElementById('selected-icon');
  const selectedIconName = document.getElementById('selected-icon-name');
  const picker = document.getElementById('icon-picker');

  if (iconInput) iconInput.value = icon;
  if (selectedIcon) selectedIcon.innerHTML = `<use href="#${icon}"/>`;
  if (selectedIconName) selectedIconName.textContent = icon;
  if (picker) picker.classList.add('hidden');
}

// Modal functions
function openAddModal(): void {
  editingId = null;

  const modalTitle = document.getElementById('modal-title');
  const form = document.getElementById('category-form') as HTMLFormElement | null;
  const categoryId = document.getElementById('category-id') as HTMLInputElement | null;
  const categoryColor = document.getElementById('category-color') as HTMLInputElement | null;
  const categoryColorText = document.getElementById('category-color-text') as HTMLInputElement | null;
  const modal = document.getElementById('category-modal');

  if (modalTitle) modalTitle.textContent = 'Add Category';
  if (form) form.reset();
  if (categoryId) categoryId.value = '';
  if (categoryColor) categoryColor.value = '#6b7280';
  if (categoryColorText) categoryColorText.value = '#6b7280';

  selectIcon('folder');
  updateParentSelect();

  if (modal) modal.classList.remove('hidden');
}

function openEditModal(id: number): void {
  const cat = categories.find(c => c.id === id);
  if (!cat) return;

  editingId = id;

  const modalTitle = document.getElementById('modal-title');
  const categoryId = document.getElementById('category-id') as HTMLInputElement | null;
  const categoryName = document.getElementById('category-name') as HTMLInputElement | null;
  const categoryColor = document.getElementById('category-color') as HTMLInputElement | null;
  const categoryColorText = document.getElementById('category-color-text') as HTMLInputElement | null;
  const categoryParent = document.getElementById('category-parent') as HTMLSelectElement | null;
  const modal = document.getElementById('category-modal');

  if (modalTitle) modalTitle.textContent = 'Edit Category';
  if (categoryId) categoryId.value = String(id);
  if (categoryName) categoryName.value = decodeHtml(cat.name);
  if (categoryColor) categoryColor.value = cat.color;
  if (categoryColorText) categoryColorText.value = cat.color;

  selectIcon(cat.icon);
  updateParentSelect();

  if (categoryParent) categoryParent.value = cat.parentId ? String(cat.parentId) : '';
  if (modal) modal.classList.remove('hidden');
}

function closeModal(): void {
  const modal = document.getElementById('category-modal');
  const picker = document.getElementById('icon-picker');

  if (modal) modal.classList.add('hidden');
  if (picker) picker.classList.add('hidden');
}

// API functions
async function saveCategory(): Promise<void> {
  const nameInput = document.getElementById('category-name') as HTMLInputElement | null;
  const parentInput = document.getElementById('category-parent') as HTMLSelectElement | null;
  const colorInput = document.getElementById('category-color') as HTMLInputElement | null;
  const iconInput = document.getElementById('category-icon') as HTMLInputElement | null;

  const name = nameInput?.value.trim() || '';
  if (!name) return;

  const parentId = parentInput?.value || null;
  const color = colorInput?.value || '#6b7280';
  const icon = iconInput?.value || 'folder';

  const params = new URLSearchParams();
  params.append('name', name);
  if (parentId) params.append('parent_id', parentId);
  params.append('color', color);
  params.append('icon', icon);

  try {
    let response: Response;
    if (editingId) {
      response = await fetch(`/categories/${editingId}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: params
      });
    } else {
      response = await fetch('/categories/create', {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: params
      });
    }

    if (response.ok) {
      // Reload to get updated data
      window.location.reload();
    } else {
      alert('Failed to save category');
    }
  } catch (e) {
    alert('Error saving category: ' + (e as Error).message);
  }
}

async function deleteCategory(id: number, name: string): Promise<void> {
  if (!confirm(`Delete "${name}"? This will also delete all subcategories.`)) return;

  try {
    const response = await fetch(`/categories/${id}`, {
      method: 'DELETE'
    });

    if (response.ok) {
      // Remove from local data and re-render
      removeCategory(id);
      renderTree();
    } else {
      alert('Failed to delete category');
    }
  } catch (e) {
    alert('Error deleting category: ' + (e as Error).message);
  }
}

function removeCategory(id: number): void {
  // Remove this category and all descendants
  const toRemove = new Set<number>([id]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const cat of categories) {
      if (cat.parentId !== null && toRemove.has(cat.parentId) && !toRemove.has(cat.id)) {
        toRemove.add(cat.id);
        changed = true;
      }
    }
  }
  categories = categories.filter(c => !toRemove.has(c.id));
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
    const response = await fetch(`/categories/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: params
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
function init(initialCategories: Category[]): void {
  categories = initialCategories;

  renderTree();
  initIconPicker();

  const colorInput = document.getElementById('category-color') as HTMLInputElement | null;
  const colorTextInput = document.getElementById('category-color-text') as HTMLInputElement | null;

  if (colorInput && colorTextInput) {
    colorInput.addEventListener('input', () => {
      colorTextInput.value = colorInput.value;
    });
  }
}

// Export functions for use in HTML onclick handlers
declare global {
  interface Window {
    categoriesPage: {
      init: (categories: Category[]) => void;
      openAddModal: () => void;
      openEditModal: (id: number) => void;
      closeModal: () => void;
      saveCategory: () => Promise<void>;
      deleteCategory: (id: number, name: string) => Promise<void>;
      toggleIconPicker: () => void;
      filterIcons: (query: string) => void;
    };
  }
}

window.categoriesPage = {
  init,
  openAddModal,
  openEditModal,
  closeModal,
  saveCategory,
  deleteCategory,
  toggleIconPicker,
  filterIcons
};
