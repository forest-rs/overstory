---
name: Chicago
description: A flat, beveled, square-cornered retro look for Overstory.
colors:
  face: { light: "#c0c0c0", dark: "#3a3a3a" }
  window: { light: "#ffffff", dark: "#1e1e1e" }
  bevel-light: { light: "#ffffff", dark: "#6a6a6a" }
  bevel-dark: { light: "#808080", dark: "#000000" }
  accent: { light: "#000080", dark: "#5a7fe6" }
  text: { light: "#000000", dark: "#f0f0f0" }
  muted: { light: "#808080", dark: "#a0a0a0" }
  on-accent: "#ffffff"
rounded:
  none: 0px
---

# Chicago

A flat, beveled, square-cornered homage to the mid-90s desktop. Follows the
[Google Labs DESIGN.md format](https://github.com/google-labs-code/design.md):
the YAML frontmatter is the machine-readable source of truth.

Per the per-scheme extension, each color is a `{ light, dark }` map (`light` is
the classic gray chrome; `dark` is a darker variant). There is no motion block —
period UI did not animate. The Rust defaults in `palette.rs` mirror this
frontmatter; a test fails if they drift.

## Overview

Extruded 3D bevels from a fixed system palette: a `#c0c0c0` face, white
highlight, gray shadow, navy selection. Nothing is derived — the period look is
hand-set system colors painted flat.

## Colors

`face` is the control/dialog fill; `window` is the editable (text field) fill.
`bevel-light` and `bevel-dark` are the two 3D edges (light top/left, dark
bottom/right; inverted when a button is pressed). `accent` is the selection and
"on" color; `text`/`muted` are foregrounds.

## Layout

Square corners throughout (`rounded.none`).
