# overstory_svg

`overstory_svg` imports static SVG documents into ordinary Overstory element
trees.

This crate owns the SVG parsing and lowering adapter. It does not make
`overstory` depend on SVG, and it does not try to be a complete SVG renderer in
its first slice. The current importer maps `usvg` path nodes to
`overstory::VectorPath` elements with resolved color fill/stroke paint.

```rust,ignore
let svg = overstory_svg::SvgElementTree::from_bytes(
    include_bytes!("icon.svg"),
    overstory_svg::SvgImportOptions::default(),
)?;

let root = ui.root();
let svg_root = ui.append(root, svg);
```

Unsupported SVG paint servers such as gradients and patterns are skipped for
now. Text, images, filters, masks, and richer source metadata are future work.
