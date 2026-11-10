// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Complete conversion from retained tab layout into tiling geometry.
//!
//! This module owns geometry completeness, coordinate conversion, and frame
//! freshness checks; it explicitly does not own layout or tiling mutation.

use alloc::vec::Vec;
use core::fmt;

use overstory::{TabKey, Ui, tab_bar_layout_snapshot};
use understory_tiling::{
    InteractionState, PaneId, Rect, TabGeometry, TabGeometryError, TabGroupGeometry, TileId,
};

use super::DockSurfaceInner;
use crate::DockState;

/// Failure to capture or apply complete retained tab geometry.
#[derive(Clone, Debug, PartialEq)]
pub enum DockGeometryError {
    /// The surface has not been synchronized with the state's current frame.
    StaleFrame {
        /// Frame epoch most recently synchronized into the surface.
        synchronized: Option<u64>,
        /// Current state frame epoch.
        current: u64,
    },
    /// UI layout or arrangement has not been drained yet.
    PendingLayout,
    /// Geometry cannot be replaced while an interaction is in progress.
    ActiveInteraction,
    /// A semantic tab bar has no retained realization.
    MissingTabBar {
        /// Missing tab-group identity.
        group: TileId,
    },
    /// A retained tab bar has no arranged layout snapshot.
    MissingLayout {
        /// Tab group whose layout is unavailable.
        group: TileId,
    },
    /// A retained viewport does not overlap its semantic reserved rectangle.
    DisjointViewport {
        /// Invalid tab group.
        group: TileId,
    },
    /// A retained tab key cannot represent an Understory pane id.
    InvalidPaneKey {
        /// Owning tab group.
        group: TileId,
        /// Unconvertible key.
        key: TabKey,
    },
    /// A retained tab has no matching semantic pane.
    UnexpectedPane {
        /// Owning tab group.
        group: TileId,
        /// Unexpected pane.
        pane: PaneId,
    },
    /// A retained tab snapshot repeats one semantic pane.
    DuplicatePane {
        /// Owning tab group.
        group: TileId,
        /// Repeated pane.
        pane: PaneId,
    },
    /// A semantic pane has no retained tab geometry.
    MissingPane {
        /// Owning tab group.
        group: TileId,
        /// Missing pane.
        pane: PaneId,
    },
    /// Understory rejected otherwise complete resolved geometry.
    Rejected(TabGeometryError),
}

impl fmt::Display for DockGeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleFrame {
                synchronized,
                current,
            } => write!(
                f,
                "dock surface frame epoch {synchronized:?} does not match state epoch {current}"
            ),
            Self::ActiveInteraction => {
                f.write_str("tab geometry cannot be replaced during an active interaction")
            }
            Self::PendingLayout => f.write_str("tab geometry requires current arranged UI layout"),
            Self::MissingTabBar { group } => {
                write!(f, "tab group {} has no retained tab bar", group.0)
            }
            Self::MissingLayout { group } => {
                write!(f, "tab group {} has no arranged tab layout", group.0)
            }
            Self::DisjointViewport { group } => write!(
                f,
                "tab group {} retained viewport is outside its reserved rectangle",
                group.0
            ),
            Self::InvalidPaneKey { group, key } => write!(
                f,
                "tab group {} key {} cannot represent a pane id",
                group.0,
                key.value()
            ),
            Self::UnexpectedPane { group, pane } => write!(
                f,
                "tab group {} retained unexpected pane {}",
                group.0, pane.0
            ),
            Self::DuplicatePane { group, pane } => write!(
                f,
                "tab group {} retained pane {} more than once",
                group.0, pane.0
            ),
            Self::MissingPane { group, pane } => write!(
                f,
                "tab group {} is missing retained pane {}",
                group.0, pane.0
            ),
            Self::Rejected(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for DockGeometryError {}

impl From<TabGeometryError> for DockGeometryError {
    fn from(value: TabGeometryError) -> Self {
        Self::Rejected(value)
    }
}

pub(super) fn capture(
    ui: &Ui,
    state: &DockState,
    inner: &DockSurfaceInner,
) -> Result<Vec<TabGroupGeometry>, DockGeometryError> {
    let current = state.frame_epoch();
    if inner.synchronized_frame_epoch != Some(current) {
        return Err(DockGeometryError::StaleFrame {
            synchronized: inner.synchronized_frame_epoch,
            current,
        });
    }
    if ui.has_pending_layout() {
        return Err(DockGeometryError::PendingLayout);
    }
    if !matches!(state.interaction(), InteractionState::None) || state.preview_frame().is_some() {
        return Err(DockGeometryError::ActiveInteraction);
    }

    let frame = state.frame();
    let frame_space = ui.effective_bounds(inner.chrome_layer);
    let mut groups = Vec::with_capacity(frame.tab_bars().len());
    for semantic_bar in frame.tab_bars() {
        let view = inner
            .tab_bars
            .iter()
            .find(|view| view.group == semantic_bar.group)
            .ok_or(DockGeometryError::MissingTabBar {
                group: semantic_bar.group,
            })?;
        let snapshot =
            tab_bar_layout_snapshot(ui, view.body).ok_or(DockGeometryError::MissingLayout {
                group: semantic_bar.group,
            })?;
        let viewport = intersect_rect(
            local_rect(snapshot.viewport, frame_space),
            semantic_bar.rect,
        )
        .ok_or(DockGeometryError::DisjointViewport {
            group: semantic_bar.group,
        })?;
        let semantic_panes: Vec<_> = frame
            .tabs()
            .iter()
            .filter(|tab| tab.group == semantic_bar.group)
            .map(|tab| tab.pane)
            .collect();
        let mut seen = Vec::with_capacity(snapshot.items.len());
        let mut tabs = Vec::with_capacity(snapshot.items.len());
        for item in snapshot.items {
            let pane = pane_id(semantic_bar.group, item.key)?;
            if !semantic_panes.contains(&pane) {
                return Err(DockGeometryError::UnexpectedPane {
                    group: semantic_bar.group,
                    pane,
                });
            }
            if seen.contains(&pane) {
                return Err(DockGeometryError::DuplicatePane {
                    group: semantic_bar.group,
                    pane,
                });
            }
            seen.push(pane);
            tabs.push(TabGeometry {
                pane,
                rect: local_rect(item.bounds, frame_space),
                interaction_regions: item
                    .interaction_regions
                    .into_iter()
                    .filter_map(|region| intersect_rect(local_rect(region, frame_space), viewport))
                    .collect(),
            });
        }
        if let Some(pane) = semantic_panes.into_iter().find(|pane| !seen.contains(pane)) {
            return Err(DockGeometryError::MissingPane {
                group: semantic_bar.group,
                pane,
            });
        }
        groups.push(TabGroupGeometry {
            group: semantic_bar.group,
            viewport,
            tabs,
            bar_drag_regions: snapshot
                .background_regions
                .into_iter()
                .filter_map(|region| {
                    intersect_rect(local_rect(region, frame_space), semantic_bar.rect)
                })
                .collect(),
        });
    }
    Ok(groups)
}

fn pane_id(group: TileId, key: TabKey) -> Result<PaneId, DockGeometryError> {
    u32::try_from(key.value())
        .map(PaneId)
        .map_err(|_| DockGeometryError::InvalidPaneKey { group, key })
}

fn local_rect(rect: Rect, origin: Rect) -> Rect {
    Rect::new(
        rect.x0 - origin.x0,
        rect.y0 - origin.y0,
        rect.x1 - origin.x0,
        rect.y1 - origin.y0,
    )
}

fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let rect = Rect::new(
        a.x0.max(b.x0),
        a.y0.max(b.y0),
        a.x1.min(b.x1),
        a.y1.min(b.y1),
    );
    (rect.x1 >= rect.x0 && rect.y1 >= rect.y0).then_some(rect)
}
