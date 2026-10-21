# UI Context

## Theme

Dark-first with a light variant following the OS setting.
Neutral surfaces with a single orange accent carried over
from the macOS app (`Theme.accent` = sRGB 1.0, 0.36, 0.0).
Compact utility layout: one window, no marketing chrome.

## Colors

All components use these CSS custom properties; no hardcoded
hex values outside `src/App.css`.

| Role            | CSS Variable       | Dark value | Light value |
| --------------- | ------------------ | ---------- | ----------- |
| Page background | `--bg-base`        | `#16161a`  | `#f5f5f7`   |
| Surface         | `--bg-surface`     | `#1f1f25`  | `#ffffff`   |
| Primary text    | `--text-primary`   | `#f2f2f5`  | `#1c1c1e`   |
| Muted text      | `--text-muted`     | `#9a9aa5`  | `#6b6b73`   |
| Primary accent  | `--accent-primary` | `#ff5c00`  | `#ff5c00`   |
| Border          | `--border-default` | `#2e2e36`  | `#dcdce1`   |
| Error           | `--state-error`    | `#ff5a5f`  | `#d93a3f`   |
| Success         | `--state-success`  | `#34c759`  | `#248a3d`   |

## Typography

| Role      | Font                       | Variable      |
| --------- | -------------------------- | ------------- |
| UI text   | Segoe UI Variable, system  | `--font-sans` |
| Code/mono | Cascadia Mono, Consolas    | `--font-mono` |

## Border Radius

| Context           | Value  |
| ----------------- | ------ |
| Inline / small UI | `6px`  |
| Cards / panels    | `12px` (matches `Theme.cardCorner`) |
| Modals / overlays | `16px` |

## Component Library

None. Plain React components with CSS modules-free global
stylesheet `src/App.css`. Keep components small and in
`src/components/`.

## Layout Patterns

- Main window: header, drop zone, file list, target picker,
  job list stacked vertically; ~560x640 default.
- Settings: separate tab inside the same window.
- Progress: linear bar per job with status text and
  Open / Show actions when done.

## Radial Wheel (overlay window)

Separate transparent window using `src/wheel.css`; it does not
load `App.css`. Always dark regardless of the OS theme.

- Geometry ported from the macOS `RadialGeometry`: inner radius
  64, outer 150, gap 4, label radius 56% between them, hit region
  to outer + 24, disc 170. Wedge 0 is centred at the top and
  indexes run clockwise.
- Wedges: `rgba(28,28,34,0.86)` fill, 1px light edge; highlighted
  wedge fills with `#ff5c00`, scales 1.025 and nudges 4px outward.
- Centre pill shows the highlighted label, the single item's
  label, or CONVERT / TOOLS. Empty states use the macOS messages.
- Labels: category icon over a bold, letter-spaced caption
  (smaller when more than 8 items).
- Appears with a 280ms spring scale-in; reduced-motion disables it.

## Icons

Inline SVG, stroke-based, 16px inline and 20px for buttons.
