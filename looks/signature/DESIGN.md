---
name: Signature
description: A token-driven, gradient-forward look for Overstory.
colors:
  accent: { light: "#5b5ef0", dark: "#6366f1" }
  surface: { light: "#eef0f7", dark: "#181a22" }
  panel: { light: "#ffffff", dark: "#1c1e27" }
  card: { light: "#ffffff", dark: "#222530" }
  control: { light: "#f6f7fb", dark: "#1f2230" }
  control-border: { light: "#c9cedd", dark: "#3a3e4d" }
  track: { light: "#dfe3ee", dark: "#2a2d3a" }
  thumb: { light: "#ffffff", dark: "#f6f7fb" }
  on-accent: "#ffffff"
  on-surface: { light: "#1b1d27", dark: "#eceef6" }
  muted: { light: "#5d637a", dark: "#a3a8bb" }
  heading: { light: "#14161f", dark: "#f5f6fc" }
  body: { light: "#3a3f51", dark: "#cdd0dc" }
  focus: { light: "#4b51e6", dark: "#aab2ff" }
rounded:
  sm: 6px
  md: 12px
  lg: 18px
  pill: 999px
motion:
  fast: 90
  normal: 140
  emphasized: 220
  glow: 420
---

# Signature

A token-driven, gradient-forward look for Overstory. This file follows the
[Google Labs DESIGN.md format](https://github.com/google-labs-code/design.md):
the YAML frontmatter above is the machine-readable source of truth, and the
sections below are human-readable rationale.

Two deviations from the base spec, both deliberate:

- **Per-scheme colors.** Each color is a `{ light, dark }` map rather than a
  single value, because a look carries both schemes. A bare scalar (see
  `on-accent`) applies to every scheme.
- **Motion.** The `motion` block (milliseconds) is an extension; the base spec
  has no motion tokens.

The Rust defaults in `palette.rs` mirror this frontmatter exactly; a crate test
fails if the two drift.

## Overview

Soft, lit-from-above surfaces. A small set of seed colors derives the whole
palette: subtle multi-stop gradients on surfaces and accents, hover/pressed
shades, a focus glow, and a hairline highlight are all computed from these
seeds rather than hand-listed.

## Colors

`accent` drives the action gradient and focus glow. `surface`, `panel`, and
`card` are increasingly raised backdrops. `control` is the input surface;
`track` and `thumb` back sliders and toggles. The `on-*` and text roles
(`on-surface`, `muted`, `heading`, `body`) are foregrounds.

## Layout

`rounded` is the corner scale (`sm` chips, `md` controls, `lg` containers,
`pill` fully round). Spacing is currently authored in code.

## Motion

`fast` / `normal` / `emphasized` size interaction transitions; `glow` is the
slower emphasized duration. Easing curves are authored in code.
