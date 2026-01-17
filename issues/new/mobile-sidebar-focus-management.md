# Mobile sidebar focus management

When the mobile sidebar opens, focus is not trapped within it, and pressing Escape does not
close it. This creates accessibility issues for keyboard and screen reader users.

## Current behavior

- Hamburger button opens sidebar via class toggle
- Focus remains on hamburger button
- Tab key moves focus to elements behind the backdrop
- No Escape key handler

## Required behavior

1. When sidebar opens, move focus to first focusable element in sidebar
2. Trap focus within sidebar while open (Tab cycles within sidebar)
3. Escape key closes sidebar
4. When sidebar closes, return focus to hamburger button
5. Backdrop click already closes sidebar (working)

## Implementation

```javascript
// On open:
sidebar.querySelector('a').focus();

// Focus trap: on Tab at last element, move to first
// On Shift+Tab at first element, move to last

// On Escape:
document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && sidebarOpen) closeSidebar();
});
```

## Relevant files

- `templates/components/navbar.html` (hamburger button)
- `templates/components/sidebar.html`
- `static/js/src/main.js`
