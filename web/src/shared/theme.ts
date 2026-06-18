/**
 * Single source of design-system CSS for the OOXML Flue web app.
 *
 * Ported faithfully from the Safety Secretary design system.
 *
 * Usage: the returned string is injected INSIDE each page's existing
 * <style> element via ${themeCss()}. It must therefore contain plain CSS
 * only — NO backticks and NO ${ sequences.
 *
 * Layers (in order): token layer (colors / typography / spacing-radii-
 * shadows-controls-focus-motion / base reset / focus ring / a + code
 * defaults), then the component primitive classes (ss-btn, ss-input,
 * ss-card, ss-empty, ss-statusdot, ss-nav__link).
 *
 * Fonts: by default this emits NO live third-party webfont request — the
 * --font-sans / --font-mono stacks name Inter / JetBrains Mono with system
 * fallbacks. A commented opt-in @import is provided at the top of the CSS.
 */
export function themeCss(): string {
  return `
/* To ship the exact Inter + JetBrains Mono webfonts, uncomment: */
/* @import url("https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&family=JetBrains+Mono:wght@400;500;600&display=swap"); */

/* COLORS */
:root {
  color-scheme: light;
  --color-bg: #ffffff;
  --color-surface: #f7f7f8;
  --color-surface-elev: #ffffff;
  --color-border: #e2e2e5;
  --color-text: #1a1a1f;
  --color-muted: #6b6b76;
  --color-accent: #5e6ad2;
  --color-success: #3d9a6d;
  --color-warning: #b8852a;
  --color-danger: #d24b4b;
  --color-info: #5e6ad2;
  --color-focus-ring: var(--color-accent);
}
.dark, :root.dark, [data-theme="dark"] {
  color-scheme: dark;
  --color-bg: #0e0e10;
  --color-surface: #16161a;
  --color-surface-elev: #1c1c21;
  --color-border: #2a2a32;
  --color-text: #e4e4e8;
  --color-muted: #8e8e9a;
  --color-accent: #7b83ff;
  --color-success: #4cb782;
  --color-warning: #d6a44a;
  --color-danger: #eb5757;
  --color-info: #7b83ff;
}

/* TYPOGRAPHY */
:root {
  --font-sans: "Inter", ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  --font-mono: "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  --text-xs: 0.75rem;     /* 12px */
  --text-sm: 0.8125rem;   /* 13px */
  --text-base: 0.875rem;  /* 14px */
  --text-lg: 1rem;        /* 16px */
  --text-xl: 1.125rem;    /* 18px */
  --text-2xl: 1.5rem;     /* 24px */
  --text-3xl: 2rem;       /* 32px */
  --font-weight-normal: 400;
  --font-weight-medium: 500;
  --font-weight-semibold: 600;
  --leading-tight: 1.2;
  --leading-snug: 1.4;
  --leading-normal: 1.5;
  --leading-relaxed: 1.65;
  --tracking-tight: -0.01em;
  --tracking-wide: 0.04em;
}

/* SPACING, RADII, SHADOWS, CONTROLS, FOCUS, MOTION */
:root {
  --space-0: 0;
  --space-1: 0.25rem;
  --space-2: 0.5rem;
  --space-3: 0.75rem;
  --space-4: 1rem;
  --space-5: 1.25rem;
  --space-6: 1.5rem;
  --space-8: 2rem;
  --space-10: 2.5rem;
  --space-16: 4rem;
  --control-h-sm: 2rem;
  --control-h-md: 2.5rem;
  --control-h-lg: 2.75rem;
  --radius-sm: 0.25rem;
  --radius-md: 0.375rem;
  --radius-lg: 0.5rem;
  --radius-full: 9999px;
  --border-width: 1px;
  --border: var(--border-width) solid var(--color-border);
  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.06);
  --shadow-md: 0 4px 12px rgba(0, 0, 0, 0.12);
  --shadow-xl: 0 20px 48px rgba(0, 0, 0, 0.45);
  --ring-width: 2px;
  --ring-offset: 2px;
  --ease-standard: cubic-bezier(0.2, 0, 0, 1);
  --duration-fast: 120ms;
  --duration-normal: 180ms;
}

/* BASE reset + a single focus treatment everywhere */
*, *::before, *::after { box-sizing: border-box; }
html {
  font-family: var(--font-sans);
  color: var(--color-text);
  background-color: var(--color-bg);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  text-rendering: optimizeLegibility;
}
body {
  margin: 0;
  font-family: var(--font-sans);
  font-size: var(--text-base);
  line-height: var(--leading-normal);
  color: var(--color-text);
  background-color: var(--color-bg);
}
::selection { background-color: color-mix(in srgb, var(--color-accent) 28%, transparent); }
:where(a, button, input, textarea, select, [tabindex]):focus-visible {
  outline: none;
  box-shadow: 0 0 0 var(--ring-offset) var(--color-bg),
    0 0 0 calc(var(--ring-offset) + var(--ring-width)) var(--color-accent);
}
a { color: var(--color-accent); text-decoration: none; }
code, kbd, samp { font-family: var(--font-mono); font-size: 0.95em; }

/* Button — variants: primary|secondary|ghost|destructive, sizes sm|md|lg */
.ss-btn {
  display: inline-flex; align-items: center; justify-content: center; gap: var(--space-2);
  min-width: 0; border: var(--border-width) solid transparent; border-radius: var(--radius-md);
  font-family: var(--font-sans); font-weight: var(--font-weight-medium); line-height: 1;
  white-space: nowrap; cursor: pointer; outline: none;
  transition: background-color var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard), opacity var(--duration-fast) var(--ease-standard);
}
.ss-btn[disabled], .ss-btn[aria-disabled="true"] { cursor: not-allowed; opacity: 0.6; }
.ss-btn--sm { min-height: var(--control-h-sm); padding: 0 0.625rem; font-size: var(--text-xs); }
.ss-btn--md { min-height: var(--control-h-md); padding: 0 0.75rem; font-size: var(--text-sm); }
.ss-btn--lg { min-height: var(--control-h-lg); padding: 0 1rem; font-size: var(--text-base); }
.ss-btn--primary { background: var(--color-accent); border-color: var(--color-accent); color: var(--color-bg); }
.ss-btn--primary:hover:not([disabled]) { opacity: 0.9; }
.ss-btn--secondary { background: var(--color-surface); border-color: var(--color-border); color: var(--color-text); }
.ss-btn--secondary:hover:not([disabled]) { border-color: var(--color-accent); background: var(--color-surface-elev); }
.ss-btn--ghost { background: transparent; border-color: transparent; color: var(--color-muted); }
.ss-btn--ghost:hover:not([disabled]) { background: var(--color-surface); color: var(--color-text); }
.ss-btn--destructive { background: var(--color-surface); border-color: var(--color-border); color: var(--color-danger); }
.ss-btn--destructive:hover:not([disabled]) { border-color: var(--color-danger); background: color-mix(in srgb, var(--color-danger) 12%, var(--color-surface)); }

/* Input / field */
.ss-input {
  display: block; width: 100%;
  border: var(--border-width) solid var(--color-border); border-radius: var(--radius-md);
  background: var(--color-surface); color: var(--color-text);
  font-family: var(--font-sans); font-size: var(--text-sm);
  padding: 0 0.75rem; min-height: var(--control-h-md); box-shadow: var(--shadow-sm); outline: none;
  transition: border-color var(--duration-fast) var(--ease-standard), box-shadow var(--duration-fast) var(--ease-standard);
}
.ss-input::placeholder { color: var(--color-muted); }
.ss-input:hover:not(:disabled):not([readonly]) { border-color: var(--color-accent); }
.ss-input:focus { border-color: var(--color-accent); box-shadow: 0 0 0 var(--ring-width) color-mix(in srgb, var(--color-accent) 35%, transparent); }
.ss-input:disabled { cursor: not-allowed; opacity: 0.6; }

/* Card */
.ss-card {
  display: flex; flex-direction: column;
  border: var(--border-width) solid var(--color-border); border-radius: var(--radius-lg);
  background: var(--color-surface); overflow: hidden;
}
.ss-card--interactive { cursor: pointer; transition: border-color var(--duration-fast) var(--ease-standard); }
.ss-card--interactive:hover { border-color: var(--color-accent); }
.ss-card--selected { border-color: var(--color-accent); box-shadow: inset 0 0 0 1px var(--color-accent); }

/* EmptyState */
.ss-empty {
  display: grid; justify-items: center; text-align: center; gap: 0.5rem;
  border: var(--border-width) dashed var(--color-border); border-radius: var(--radius-lg);
  background: var(--color-surface); padding: var(--space-8) var(--space-6);
}
.ss-empty__title { margin: 0; font-size: var(--text-base); font-weight: var(--font-weight-medium); color: var(--color-text); }
.ss-empty__desc { margin: 0; max-width: 32ch; font-size: var(--text-sm); color: var(--color-muted); line-height: var(--leading-snug); }

/* Status dot — open=info, in-progress=warning, completed=success, blocked=danger */
.ss-statusdot { display: inline-block; width: 0.5rem; height: 0.5rem; flex: none; border-radius: var(--radius-full); }

/* Sidebar nav row */
.ss-nav__link {
  display: flex; align-items: center; gap: 0.5rem; min-height: 2.25rem; padding: 0 0.75rem;
  border-radius: var(--radius-md); color: var(--color-muted); font-size: var(--text-sm);
  text-decoration: none; outline: none;
  transition: background-color var(--duration-fast) var(--ease-standard), color var(--duration-fast) var(--ease-standard);
}
.ss-nav__link:hover { background: var(--color-surface-elev); color: var(--color-text); }
.ss-nav__link--active { background: var(--color-surface-elev); color: var(--color-text); font-weight: var(--font-weight-medium); box-shadow: var(--shadow-sm); }
`;
}
