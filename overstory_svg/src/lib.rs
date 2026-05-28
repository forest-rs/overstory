// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! SVG import for Overstory.
//!
//! This crate lowers static SVG documents parsed by `usvg` into ordinary
//! Overstory elements. The first slice imports visible path nodes as
//! [`overstory::VectorPath`] children under a fixed-size root panel.
//!
//! ## Fence
//!
//! `overstory_svg` owns SVG parsing and import policy; it explicitly does not
//! own Overstory's presentation model, renderer lowering, layout engine, or the
//! full SVG rendering feature set.

use std::error::Error;
use std::fmt;

use overstory::{
    Affine, AppendSpec, BezPath, Brush, Cap, Color, ElementId, Fill, Join, Layout, Panel, PathFill,
    PathPaintOrder, PathPrimitive, PathStroke, Rect, Stroke, Ui, VectorPath,
};
use usvg::tiny_skia_path::PathSegment;

/// Options controlling SVG import.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct SvgImportOptions {}

/// Error returned when an SVG document cannot be imported.
#[derive(Debug)]
pub enum SvgImportError {
    /// `usvg` rejected the input SVG.
    Parse(usvg::Error),

    /// The parsed SVG did not produce a non-zero document size.
    EmptySize,
}

impl fmt::Display for SvgImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "{error}"),
            Self::EmptySize => f.write_str("SVG document has an empty size"),
        }
    }
}

impl Error for SvgImportError {}

impl From<usvg::Error> for SvgImportError {
    fn from(error: usvg::Error) -> Self {
        Self::Parse(error)
    }
}

/// Imported SVG document ready to append into an Overstory [`Ui`].
#[derive(Clone, Debug)]
pub struct SvgElementTree {
    width: f32,
    height: f32,
    paths: Vec<SvgPathElement>,
}

impl SvgElementTree {
    /// Parses `bytes` with `usvg` and imports the supported static elements.
    pub fn from_bytes(bytes: &[u8], options: SvgImportOptions) -> Result<Self, SvgImportError> {
        let tree = usvg::Tree::from_data(bytes, &usvg::Options::default())?;
        Self::from_usvg_tree(&tree, options)
    }

    /// Imports an already parsed `usvg` tree.
    pub fn from_usvg_tree(
        tree: &usvg::Tree,
        _options: SvgImportOptions,
    ) -> Result<Self, SvgImportError> {
        let size = tree.size();
        let width = size.width();
        let height = size.height();
        if width <= 0.0 || height <= 0.0 {
            return Err(SvgImportError::EmptySize);
        }

        let mut paths = Vec::new();
        collect_group_paths(tree.root(), 1.0, &mut paths);
        Ok(Self {
            width,
            height,
            paths,
        })
    }

    /// Returns the SVG viewport width in logical pixels.
    #[must_use]
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Returns the SVG viewport height in logical pixels.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.height
    }

    /// Returns the number of imported path elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Returns true when the document imported no supported drawing elements.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Returns imported path elements in SVG paint order.
    #[must_use]
    pub fn paths(&self) -> &[SvgPathElement] {
        &self.paths
    }
}

impl AppendSpec for SvgElementTree {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let root = ui.append(
            parent,
            Panel::new().layout(
                Layout::new()
                    .relative()
                    .logical_size(self.width, self.height),
            ),
        );

        for path in self.paths {
            let bounds = path.bounds;
            ui.append(
                root,
                VectorPath::from_primitive(path.primitive).layout(
                    Layout::new()
                        .absolute_at(bounds.x, bounds.y)
                        .logical_size(bounds.width, bounds.height),
                ),
            );
        }

        root
    }
}

/// One imported SVG path element.
#[derive(Clone, Debug)]
pub struct SvgPathElement {
    source_id: Option<String>,
    bounds: SvgBounds,
    primitive: PathPrimitive,
}

impl SvgPathElement {
    /// Returns the source SVG id, if the element had one.
    #[must_use]
    pub fn source_id(&self) -> Option<&str> {
        self.source_id.as_deref()
    }

    /// Returns the path's canvas-space bounds including stroke.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        self.bounds.to_rect()
    }

    /// Returns the resolved path presentation primitive.
    #[must_use]
    pub fn primitive(&self) -> &PathPrimitive {
        &self.primitive
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SvgBounds {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl SvgBounds {
    fn from_usvg(rect: usvg::Rect) -> Self {
        Self {
            x: rect.left(),
            y: rect.top(),
            width: rect.width(),
            height: rect.height(),
        }
    }

    fn to_rect(self) -> Rect {
        Rect::new(
            f64::from(self.x),
            f64::from(self.y),
            f64::from(self.x + self.width),
            f64::from(self.y + self.height),
        )
    }
}

fn collect_group_paths(
    group: &usvg::Group,
    inherited_opacity: f32,
    output: &mut Vec<SvgPathElement>,
) {
    let opacity = inherited_opacity * group.opacity().get();
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_group_paths(group, opacity, output),
            usvg::Node::Path(path) => {
                if let Some(element) = import_path(path, opacity) {
                    output.push(element);
                }
            }
            usvg::Node::Image(_) | usvg::Node::Text(_) => {}
        }
    }
}

fn import_path(path: &usvg::Path, inherited_opacity: f32) -> Option<SvgPathElement> {
    if !path.is_visible() {
        return None;
    }

    let bounds = SvgBounds::from_usvg(path.abs_stroke_bounding_box());
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return None;
    }

    let transform = Affine::translate((-f64::from(bounds.x), -f64::from(bounds.y)))
        * affine_from_usvg(path.abs_transform());
    let mut primitive = PathPrimitive::new(bez_path_from_usvg(path.data()))
        .transform(transform)
        .paint_order(paint_order_from_usvg(path.paint_order()));

    if let Some(fill) = path
        .fill()
        .and_then(|fill| path_fill_from_usvg(fill, inherited_opacity))
    {
        primitive.fill = Some(fill);
    }
    if let Some(stroke) = path
        .stroke()
        .and_then(|stroke| path_stroke_from_usvg(stroke, inherited_opacity))
    {
        primitive.stroke = Some(stroke);
    }

    (!primitive.is_empty()).then(|| SvgPathElement {
        source_id: (!path.id().is_empty()).then(|| path.id().to_owned()),
        bounds,
        primitive,
    })
}

fn bez_path_from_usvg(path: &usvg::tiny_skia_path::Path) -> BezPath {
    let mut out = BezPath::new();
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(point) => out.move_to((f64::from(point.x), f64::from(point.y))),
            PathSegment::LineTo(point) => out.line_to((f64::from(point.x), f64::from(point.y))),
            PathSegment::QuadTo(control, point) => out.quad_to(
                (f64::from(control.x), f64::from(control.y)),
                (f64::from(point.x), f64::from(point.y)),
            ),
            PathSegment::CubicTo(control0, control1, point) => out.curve_to(
                (f64::from(control0.x), f64::from(control0.y)),
                (f64::from(control1.x), f64::from(control1.y)),
                (f64::from(point.x), f64::from(point.y)),
            ),
            PathSegment::Close => out.close_path(),
        }
    }
    out
}

fn path_fill_from_usvg(fill: &usvg::Fill, inherited_opacity: f32) -> Option<PathFill> {
    let brush = brush_from_paint(fill.paint(), fill.opacity().get() * inherited_opacity)?;
    Some(PathFill::new(brush).rule(fill_rule_from_usvg(fill.rule())))
}

fn path_stroke_from_usvg(stroke: &usvg::Stroke, inherited_opacity: f32) -> Option<PathStroke> {
    let brush = brush_from_paint(stroke.paint(), stroke.opacity().get() * inherited_opacity)?;
    let mut style = Stroke::new(f64::from(stroke.width().get()))
        .with_caps(line_cap_from_usvg(stroke.linecap()))
        .with_join(line_join_from_usvg(stroke.linejoin()))
        .with_miter_limit(f64::from(stroke.miterlimit().get()));
    if let Some(dashes) = stroke.dasharray() {
        style = style.with_dashes(
            f64::from(stroke.dashoffset()),
            dashes.iter().map(|dash| f64::from(*dash)),
        );
    }
    Some(PathStroke {
        brush,
        stroke: style,
    })
}

fn brush_from_paint(paint: &usvg::Paint, opacity: f32) -> Option<Brush> {
    match paint {
        usvg::Paint::Color(color) => Some(
            Brush::from(Color::from_rgb8(color.red, color.green, color.blue))
                .multiply_alpha(opacity.clamp(0.0, 1.0)),
        ),
        usvg::Paint::LinearGradient(_)
        | usvg::Paint::RadialGradient(_)
        | usvg::Paint::Pattern(_) => None,
    }
}

fn fill_rule_from_usvg(rule: usvg::FillRule) -> Fill {
    match rule {
        usvg::FillRule::NonZero => Fill::NonZero,
        usvg::FillRule::EvenOdd => Fill::EvenOdd,
    }
}

fn paint_order_from_usvg(order: usvg::PaintOrder) -> PathPaintOrder {
    match order {
        usvg::PaintOrder::FillAndStroke => PathPaintOrder::FillThenStroke,
        usvg::PaintOrder::StrokeAndFill => PathPaintOrder::StrokeThenFill,
    }
}

fn line_cap_from_usvg(cap: usvg::LineCap) -> Cap {
    match cap {
        usvg::LineCap::Butt => Cap::Butt,
        usvg::LineCap::Round => Cap::Round,
        usvg::LineCap::Square => Cap::Square,
    }
}

fn line_join_from_usvg(join: usvg::LineJoin) -> Join {
    match join {
        usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => Join::Miter,
        usvg::LineJoin::Round => Join::Round,
        usvg::LineJoin::Bevel => Join::Bevel,
    }
}

fn affine_from_usvg(transform: usvg::Transform) -> Affine {
    Affine::new([
        f64::from(transform.sx),
        f64::from(transform.ky),
        f64::from(transform.kx),
        f64::from(transform.sy),
        f64::from(transform.tx),
        f64::from(transform.ty),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use overstory::{Size, TYPE_VECTOR_PATH};

    #[test]
    fn imports_rect_as_vector_path_element() {
        let svg = br##"
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">
              <rect id="block" x="2" y="3" width="8" height="4" fill="#ff0000"/>
            </svg>
        "##;

        let tree = SvgElementTree::from_bytes(svg, SvgImportOptions::default()).unwrap();

        assert_eq!(tree.width(), 20.0);
        assert_eq!(tree.height(), 10.0);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.paths()[0].source_id(), Some("block"));

        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::new().logical_size(20.0, 10.0));
        let svg_root = ui.append(root, tree);
        ui.compute_layout(Size::new(20.0, 10.0));

        let svg_children = ui.get(svg_root).unwrap().children();
        assert_eq!(svg_children.len(), 1);
        let path = ui.get(svg_children[0]).unwrap();
        assert_eq!(path.type_tag(), Some(TYPE_VECTOR_PATH));
    }

    #[test]
    fn unsupported_gradient_paint_is_skipped() {
        let svg = br##"
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">
              <defs>
                <linearGradient id="g">
                  <stop offset="0%" stop-color="#000"/>
                  <stop offset="100%" stop-color="#fff"/>
                </linearGradient>
              </defs>
              <rect x="0" y="0" width="20" height="10" fill="url(#g)"/>
            </svg>
        "##;

        let tree = SvgElementTree::from_bytes(svg, SvgImportOptions::default()).unwrap();

        assert!(tree.is_empty());
    }
}
