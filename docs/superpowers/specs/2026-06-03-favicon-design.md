# Favicon & Browser Branding — Design Spec

**Bead:** fewd-z0r · **Date:** 2026-06-03 · **Plan ref:** IMPLEMENTATION_PLAN.md Phase 15

## Goal

Replace the placeholder 🍽️ emoji data-URI favicon with a custom-designed
icon set that matches the app's palette and reads clearly at browser-tab sizes
and as an iOS home-screen icon.

## The Mark

A **place setting**: crossed fork & knife over a plate ring, on a full-bleed
rounded-square ("squircle") background. Chosen direction "A2" from
brainstorming (selected by the family; squircle preferred over a floating
circle because it fills the tab and reads larger at 16px).

| Element        | Value                | Palette source              |
| -------------- | -------------------- | --------------------------- |
| Background     | sage `#4a7a4a`       | `--color-primary-600`       |
| Plate ring     | gold `#deb321` stroke| `--color-accent-400`        |
| Cutlery        | cream `#fdfaf6`      | `--color-surface`           |
| Corner radius  | ~22% of viewBox      | iOS-app-icon feel           |

Approved composition (`favicon.svg`, the canonical source — `viewBox="0 0 100 100"`):

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="0" y="0" width="100" height="100" rx="22" fill="#4a7a4a"/>
  <circle cx="50" cy="50" r="32" fill="none" stroke="#deb321" stroke-width="2.5" opacity="0.85"/>
  <!-- fork, tilted left -->
  <g transform="translate(40,52) rotate(-18)" fill="#fdfaf6">
    <rect x="-1.8" y="-8" width="3.6" height="42" rx="1.8"/>
    <rect x="-9" y="-26" width="2.6" height="15" rx="1.3"/>
    <rect x="-1.3" y="-27" width="2.6" height="16" rx="1.3"/>
    <rect x="6.4" y="-26" width="2.6" height="15" rx="1.3"/>
    <rect x="-7" y="-13" width="14" height="3.4" rx="1.7"/>
  </g>
  <!-- knife, tilted right -->
  <g transform="translate(61,52) rotate(18)" fill="#fdfaf6">
    <rect x="-1.8" y="-2" width="3.6" height="36" rx="1.8"/>
    <path d="M -2 -27 Q 6 -24 4.5 -2 L -2 -2 Z"/>
  </g>
</svg>
```

The **apple-touch source** is the same artwork with `rx="0"` (square corners),
so iOS can apply its own mask without double-rounding (see Deliverables §3).

### Color is literal, not tokenized

Favicons render outside the page's CSS context, so the `@theme` CSS variables
and `currentColor` are unavailable. Every color in the SVG must be a literal
hex value matching the table above. If the palette tokens change later, the
favicon must be regenerated manually — it does not track them automatically.

## Deliverables

All assets land in `public/` (served at site root, embedded into the release
binary via `rust-embed`'s `#[folder = "../dist"]` after `bun run build`).

1. **`public/favicon.svg`** — the squircle source above. Scalable; primary icon
   for modern browsers.
2. **`public/favicon.ico`** — 16×16 + 32×32 frames packed into one `.ico`.
   Legacy/Windows fallback.
3. **`public/apple-touch-icon.png`** — 180×180 PNG for iOS "Add to Home Screen".
   **Full-bleed sage SQUARE — NOT pre-rounded.** iOS applies its own rounded
   mask; a pre-rounded squircle with transparent corners double-rounds and shows
   dark corners on older iOS. So this asset fills the 180×180 box edge-to-edge
   with the sage background (corners filled), no transparency.
4. **`index.html`** — replace the existing emoji data-URI `<link rel="icon">`
   with:
   ```html
   <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
   <link rel="icon" type="image/x-icon" href="/favicon.ico" />
   <link rel="apple-touch-icon" href="/apple-touch-icon.png" />
   ```

### Two SVG sources

Because the apple-touch PNG must be a square (not a squircle), there are two
SVG inputs:

- `favicon.svg` — the rounded squircle (also rasterized to the `.ico` frames,
  which keep the visible rounding).
- An **apple-touch source** — same artwork but `rx=0` and the sage background
  extended to fill the full square. This can be a transient SVG used only
  during generation (not necessarily committed), or a documented inline variant
  in the generation script.

## Asset Generation

No SVG rasterizer is installed system-wide (no ImageMagick/rsvg/inkscape).
Generate via `bunx` — no permanent dependencies added, identical output quality
to installing `sharp`/`png-to-ico` locally (same libraries). Capture the exact
commands in a committed **`scripts/gen-favicon.sh`** so the assets are
reproducible if the design changes.

Pipeline (sketch — finalize flags in the plan):

1. `favicon.svg` → `favicon-16.png`, `favicon-32.png` via `bunx sharp-cli`
2. `favicon-16.png` + `favicon-32.png` → `favicon.ico` via `bunx png-to-ico`
3. apple-touch source SVG → `apple-touch-icon.png` (180×180) via `bunx sharp-cli`
4. Clean up intermediate PNGs

`favicon.svg` is committed as-authored (not generated). The script regenerates
`.ico` and `.png` from the SVG source(s).

## Verification

From IMPLEMENTATION_PLAN.md Phase 15, plus build integration:

- [ ] Browser tab shows the custom favicon (light and dark tab bars)
- [ ] SVG favicon colors match the app palette (sage / gold / cream)
- [ ] `.ico` is legible at 16px and 32px
- [ ] iOS "Add to Home Screen" shows the icon with correctly rounded corners
      (no dark corners, no double-rounding)
- [ ] `bun run build` embeds the assets (they appear in `dist/`); the served
      app at root returns them
- [ ] Emoji data-URI `<link>` is gone from `index.html`

## Out of Scope

- `site.webmanifest` / PWA install metadata — not requested; the three standard
  link tags cover browser + iOS. (Could be a follow-up bead if PWA install is
  ever wanted.)
- Maskable Android adaptive-icon variant — same; defer unless needed.
- Re-theming the in-app logo/header — this bead is browser branding only.
