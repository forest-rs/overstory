---
name: System 6
description: A 1-bit black-and-white classic Mac look for Overstory.
colors:
  ink: { light: "#000000", dark: "#ffffff" }
  paper: { light: "#ffffff", dark: "#101010" }
  desktop: { light: "#a0a0a0", dark: "#303030" }
  muted: { light: "#808080", dark: "#909090" }
rounded:
  button: 9px
  none: 0px
---

# System 6

A 1-bit black-and-white classic Mac look. Follows the
[Google Labs DESIGN.md format](https://github.com/google-labs-code/design.md):
the YAML frontmatter is the machine-readable source of truth.

Per the per-scheme extension, each color is a `{ light, dark }` map (`light` is
the classic black-on-white desktop; `dark` is an inverted night variant). There
is no motion block — System 6 did not animate. The Rust defaults in `palette.rs`
mirror this frontmatter; a test fails if they drift.

## Overview

A two-color world: `ink` and `paper`. `desktop` and `muted` are pragmatic grays
standing in for things the original faked with a 50% dither (the desktop stipple
and disabled text) — a true stipple would need a pattern brush the toolkit
doesn't expose yet.

## Colors

`ink` is every line and label; `paper` is every surface. Selection and the
pressed-button inversion use ink-on-paper flipped to paper-on-ink.

## Layout

Rounded push buttons (`rounded.button`); everything else square (`rounded.none`).
