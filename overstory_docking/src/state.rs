// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Reusable docking state and interaction execution.
//!
//! This module owns the docking state machine; it explicitly does not own the
//! public outcome vocabulary or Understory's semantic tiling model.

use alloc::vec::Vec;

use crate::{DockInteractionKind, DockOutcome};

use understory_tiling::{
    Axis, DockPolicy, FrameProjectionInput, HitKind, InteractionOptions, InteractionState,
    InteractionUpdate, LayoutFrame, LayoutInput, OverlayFrame, PaneId, Placement, Point,
    TabGeometryError, TabGroupGeometry, TileError, TileOp, TileTree, begin_interaction,
    commit_proposal, hit_test, update_interaction, validate_interaction_update,
};

/// Reusable docking state for an Overstory workbench region.
#[derive(Clone, Debug)]
pub struct DockState {
    tree: TileTree,
    layout_input: LayoutInput,
    interaction_options: InteractionOptions,
    policy: DockPolicy,
    frame: LayoutFrame,
    frame_epoch: u64,
    resolved_tab_geometry: Vec<TabGroupGeometry>,
    preview_frame: Option<LayoutFrame>,
    overlay: OverlayFrame,
    interaction: InteractionState,
    selected: PaneId,
    zoomed: Option<PaneId>,
}

impl DockState {
    /// Creates docking state from a tile tree and layout input.
    #[must_use]
    pub fn new(tree: TileTree, layout_input: LayoutInput, selected: PaneId) -> Self {
        let interaction_options = InteractionOptions::from_layout_input(layout_input);
        Self::with_options(
            tree,
            layout_input,
            interaction_options,
            DockPolicy::default(),
            selected,
        )
    }

    /// Creates docking state with explicit interaction options and policy.
    #[must_use]
    pub fn with_options(
        tree: TileTree,
        layout_input: LayoutInput,
        interaction_options: InteractionOptions,
        policy: DockPolicy,
        selected: PaneId,
    ) -> Self {
        let frame = tree.layout(layout_input);
        Self {
            tree,
            layout_input,
            interaction_options,
            policy,
            frame,
            frame_epoch: 1,
            resolved_tab_geometry: Vec::new(),
            preview_frame: None,
            overlay: OverlayFrame::default(),
            interaction: InteractionState::None,
            selected,
            zoomed: None,
        }
    }

    /// Returns the committed tile tree.
    #[must_use]
    pub const fn tree(&self) -> &TileTree {
        &self.tree
    }

    pub(crate) fn pane_ids(&self) -> Vec<PaneId> {
        self.tree.panes().collect()
    }

    /// Returns the current committed layout frame.
    #[must_use]
    pub const fn frame(&self) -> &LayoutFrame {
        &self.frame
    }

    pub(crate) const fn frame_epoch(&self) -> u64 {
        self.frame_epoch
    }

    /// Returns the preview frame when an interaction has one.
    #[must_use]
    pub const fn preview_frame(&self) -> Option<&LayoutFrame> {
        self.preview_frame.as_ref()
    }

    /// Returns the frame that should be rendered now.
    #[must_use]
    pub fn display_frame(&self) -> &LayoutFrame {
        self.preview_frame.as_ref().unwrap_or(&self.frame)
    }

    /// Returns the current interaction overlay.
    #[must_use]
    pub const fn overlay(&self) -> &OverlayFrame {
        &self.overlay
    }

    /// Returns the current interaction state.
    #[must_use]
    pub const fn interaction(&self) -> &InteractionState {
        &self.interaction
    }

    /// Returns the selected pane.
    #[must_use]
    pub const fn selected(&self) -> PaneId {
        self.selected
    }

    /// Sets the selected pane.
    pub const fn set_selected(&mut self, pane: PaneId) {
        self.selected = pane;
    }

    /// Returns the layout input used for layout solves.
    #[must_use]
    pub const fn layout_input(&self) -> LayoutInput {
        self.layout_input
    }

    /// Replaces the layout input and recomputes the current frame.
    pub fn set_layout_input(&mut self, input: LayoutInput) {
        self.layout_input = input;
        self.sync_interaction_layout_input();
        self.cancel_interaction();
        self.relayout();
    }

    /// Returns the pane currently projected to fill the docking bounds.
    #[must_use]
    pub fn zoomed_pane(&self) -> Option<PaneId> {
        self.zoomed
    }

    /// Zooms the selected visible pane without mutating the tile tree.
    ///
    /// Returns the pane selected for the presentation projection.
    pub fn zoom_selected(&mut self) -> PaneId {
        let pane = self.selected_visible_pane();
        self.set_zoom(Some(pane));
        pane
    }

    /// Restores the normal tiled presentation without mutating the tile tree.
    ///
    /// Returns the pane that had been zoomed, if the current frame was zoomed.
    pub fn unzoom(&mut self) -> Option<PaneId> {
        let pane = self.zoomed_pane();
        self.set_zoom(None);
        pane
    }

    /// Zooms the selected pane, or restores the tiled presentation when zoomed.
    ///
    /// Returns the newly zoomed pane, or `None` after restoring the tiled view.
    pub fn toggle_zoom_selected(&mut self) -> Option<PaneId> {
        if self.zoomed_pane().is_some() {
            self.unzoom();
            None
        } else {
            Some(self.zoom_selected())
        }
    }

    /// Returns the current interaction options.
    #[must_use]
    pub const fn interaction_options(&self) -> InteractionOptions {
        self.interaction_options
    }

    /// Replaces the current interaction options.
    pub const fn set_interaction_options(&mut self, options: InteractionOptions) {
        self.interaction_options = options;
    }

    /// Returns current policy data.
    #[must_use]
    pub const fn policy(&self) -> &DockPolicy {
        &self.policy
    }

    /// Replaces current policy data.
    pub const fn set_policy(&mut self, policy: DockPolicy) {
        self.policy = policy;
    }

    /// Returns whether layout mutations are policy-locked.
    #[must_use]
    pub const fn is_policy_locked(&self) -> bool {
        self.policy.locked_layout
    }

    /// Sets the policy lock.
    pub const fn set_policy_locked(&mut self, locked: bool) {
        self.policy.locked_layout = locked;
    }

    /// Toggles the policy lock and returns the new state.
    pub const fn toggle_policy_lock(&mut self) -> bool {
        self.policy.locked_layout = !self.policy.locked_layout;
        self.policy.locked_layout
    }

    /// Recomputes the committed layout frame and clears preview geometry.
    pub fn relayout(&mut self) {
        self.frame = self.tree.layout(self.layout_input);
        if let Some(pane) = self.zoomed
            && self.apply_zoom_projection(pane).is_err()
        {
            self.zoomed = None;
        }
        self.frame_epoch = self.frame_epoch.saturating_add(1);
        self.resolved_tab_geometry.clear();
        self.preview_frame = None;
    }

    /// Replaces provisional tab rectangles with presentation-resolved geometry.
    ///
    /// Call this after the retained tab controls have been arranged and before
    /// starting a pointer interaction. Applying geometry cancels any active
    /// interaction because its hit-test snapshot is no longer current.
    pub fn apply_tab_group_geometry(
        &mut self,
        groups: &[TabGroupGeometry],
    ) -> Result<(), TabGeometryError> {
        if groups == self.resolved_tab_geometry {
            return Ok(());
        }
        self.frame.apply_tab_group_geometry(groups)?;
        self.resolved_tab_geometry = groups.to_vec();
        self.cancel_interaction();
        Ok(())
    }

    /// Activates `pane` through the semantic tiling operation path.
    pub fn activate_pane(&mut self, pane: PaneId) -> Result<TileOp, TileError> {
        let op = self.apply_op(TileOp::ActivatePane { pane })?;
        self.selected = pane;
        Ok(op)
    }

    /// Returns the selected pane if visible, otherwise the first visible pane.
    #[must_use]
    pub fn selected_visible_pane(&self) -> PaneId {
        if self
            .frame
            .panes()
            .iter()
            .any(|pane| pane.pane == self.selected)
        {
            return self.selected;
        }
        self.frame
            .panes()
            .first()
            .map(|pane| pane.pane)
            .unwrap_or(self.selected)
    }

    /// Applies a semantic tile operation.
    pub fn apply_op(&mut self, op: TileOp) -> Result<TileOp, TileError> {
        self.tree.apply(op.clone())?;
        self.cancel_interaction();
        self.relayout();
        Ok(op)
    }

    /// Splits the selected visible pane.
    pub fn split_selected(
        &mut self,
        axis: Axis,
        new_pane: PaneId,
        share: f64,
    ) -> Result<TileOp, TileError> {
        let pane = self.selected_visible_pane();
        let op = TileOp::SplitPane {
            pane,
            axis,
            new_pane,
            placement: Placement::After,
            share,
        };
        let op = self.apply_op(op)?;
        self.selected = new_pane;
        Ok(op)
    }

    /// Closes the selected visible pane.
    pub fn close_selected(&mut self) -> Result<TileOp, TileError> {
        let pane = self.selected_visible_pane();
        let op = self.apply_op(TileOp::ClosePane { pane })?;
        self.selected = self.selected_visible_pane();
        Ok(op)
    }

    /// Starts pointer interaction from a local point.
    pub fn begin_pointer(&mut self, point: Point) -> DockOutcome {
        if let Some(pane) = pane_from_hit(hit_test(&self.frame, point)) {
            self.selected = pane;
        }
        self.interaction = begin_interaction(&self.frame, point, &self.interaction_options);
        self.overlay = OverlayFrame::default();
        self.preview_frame = None;
        match interaction_kind(&self.interaction) {
            Some(kind) => DockOutcome::InteractionStarted(kind),
            None => DockOutcome::NoHit,
        }
    }

    /// Updates the active pointer interaction.
    pub fn update_pointer(&mut self, point: Point) -> DockOutcome {
        let update = update_interaction(
            &self.tree,
            &self.frame,
            &mut self.interaction,
            point,
            &self.interaction_options,
        );
        let outcome = if update.proposal.is_some() {
            match self.validate_update(&update) {
                Ok(validated) => DockOutcome::ProposalValid(validated.operation().clone()),
                Err(error) => DockOutcome::ProposalRejected(error),
            }
        } else {
            interaction_kind(&self.interaction).map_or(DockOutcome::None, DockOutcome::NoProposal)
        };
        self.overlay = update.overlay;
        self.preview_frame = self.finalize_preview(update.preview);
        if matches!(self.interaction, InteractionState::None) {
            self.overlay = OverlayFrame::default();
        }
        outcome
    }

    /// Ends the active pointer interaction.
    pub fn end_pointer(&mut self, point: Point) -> DockOutcome {
        let mut interaction = core::mem::replace(&mut self.interaction, InteractionState::None);
        match &interaction {
            InteractionState::None => return DockOutcome::None,
            InteractionState::PendingDrag(pending) if !pending.is_ready(point) => {
                return self.finish_click(point);
            }
            _ => {}
        }

        let update = update_interaction(
            &self.tree,
            &self.frame,
            &mut interaction,
            point,
            &self.interaction_options,
        );
        let result = update.proposal.is_some().then(|| {
            self.validate_update(&update)
                .and_then(|validated| commit_proposal(&mut self.tree, validated))
        });

        self.overlay = update.overlay;
        self.preview_frame = self.finalize_preview(update.preview);

        if let Some(result) = result {
            match result {
                Ok(op) => {
                    self.cancel_interaction();
                    self.relayout();
                    DockOutcome::Committed(op)
                }
                Err(error) => {
                    self.cancel_interaction();
                    DockOutcome::Rejected(error)
                }
            }
        } else {
            let outcome =
                interaction_kind(&interaction).map_or(DockOutcome::None, DockOutcome::NoProposal);
            self.cancel_interaction();
            outcome
        }
    }

    /// Cancels the active pointer interaction.
    pub fn cancel_pointer(&mut self) -> DockOutcome {
        self.cancel_interaction();
        DockOutcome::Canceled
    }

    fn finish_click(&mut self, point: Point) -> DockOutcome {
        self.overlay = OverlayFrame::default();
        self.preview_frame = None;
        match hit_test(&self.frame, point) {
            Some(HitKind::Tab { pane, .. }) => match self.activate_pane(pane) {
                Ok(op) => DockOutcome::Committed(op),
                Err(error) => DockOutcome::Rejected(error),
            },
            Some(HitKind::Pane { pane }) => {
                self.selected = pane;
                DockOutcome::PaneSelected(pane)
            }
            hit => DockOutcome::ClickMiss(hit),
        }
    }

    fn validate_update(
        &self,
        update: &InteractionUpdate,
    ) -> Result<understory_tiling::ValidatedProposal, TileError> {
        validate_interaction_update(
            &self.tree,
            &self.frame,
            update,
            &self.policy,
            &self.interaction_options,
        )
    }

    fn sync_interaction_layout_input(&mut self) {
        self.interaction_options.preview_layout = Some(self.layout_input);
        self.interaction_options.resize.min_pane_size = self.layout_input.min_pane_size;
    }

    fn set_zoom(&mut self, pane: Option<PaneId>) {
        self.cancel_interaction();
        match pane {
            Some(pane) => {
                self.apply_zoom_projection(pane)
                    .expect("selected visible pane and layout bounds must form a valid projection");
                self.zoomed = Some(pane);
                self.frame_epoch = self.frame_epoch.saturating_add(1);
            }
            None => {
                self.zoomed = None;
                self.relayout();
            }
        }
    }

    fn apply_zoom_projection(
        &mut self,
        pane: PaneId,
    ) -> Result<(), understory_tiling::FrameProjectionError> {
        self.frame.apply_projection(FrameProjectionInput::Zoom {
            focus: pane,
            projected_rect: self.layout_input.bounds.abs(),
        })
    }

    fn finalize_preview(&self, preview: Option<LayoutFrame>) -> Option<LayoutFrame> {
        preview.map(|mut frame| {
            if !self.resolved_tab_geometry.is_empty() {
                let _ = frame.apply_tab_group_geometry(&self.resolved_tab_geometry);
            }
            if let Some(pane) = self.zoomed {
                let _ = frame.apply_projection(FrameProjectionInput::Zoom {
                    focus: pane,
                    projected_rect: self.layout_input.bounds.abs(),
                });
            }
            frame
        })
    }

    fn cancel_interaction(&mut self) {
        self.overlay = OverlayFrame::default();
        self.preview_frame = None;
        self.interaction = InteractionState::None;
    }
}

fn pane_from_hit(hit: Option<HitKind>) -> Option<PaneId> {
    match hit {
        Some(HitKind::Pane { pane }) | Some(HitKind::Tab { pane, .. }) => Some(pane),
        _ => None,
    }
}

fn interaction_kind(interaction: &InteractionState) -> Option<DockInteractionKind> {
    match interaction {
        InteractionState::None => None,
        InteractionState::PendingDrag(_) => Some(DockInteractionKind::PendingDrag),
        InteractionState::Drag(_) => Some(DockInteractionKind::Drag),
        InteractionState::Resize(_) => Some(DockInteractionKind::Resize),
    }
}

#[cfg(test)]
mod tests;
