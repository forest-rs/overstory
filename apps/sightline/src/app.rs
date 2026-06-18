// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Sightline application bootstrap.
//!
//! This module owns the app-level state and Overstory `Ui` construction. It
//! explicitly does not own native host selection, telemetry transport, or
//! instrument-specific data models.

use alloc::rc::Rc;
use core::cell::Cell;

use overstory::{Layout, Ui};
use overstory_app::WindowOptions;

use crate::shell;
use crate::style::{SightlineStyleTokens, SightlineThemeMode, sightline_cascade, sightline_theme};

const INITIAL_WIDTH: u32 = 1440;
const INITIAL_HEIGHT: u32 = 900;

pub(crate) fn run() -> overstory_app::HostResult<()> {
    overstory_app::run(
        WindowOptions::new("Sightline").initial_size(INITIAL_WIDTH, INITIAL_HEIGHT),
        build_ui,
    )
}

#[derive(Debug)]
pub(crate) struct AppState {
    theme_mode: Cell<SightlineThemeMode>,
}

impl AppState {
    pub(crate) fn new() -> Self {
        Self {
            theme_mode: Cell::new(SightlineThemeMode::Graphite),
        }
    }

    pub(crate) fn theme_mode(&self) -> SightlineThemeMode {
        self.theme_mode.get()
    }

    pub(crate) fn toggle_theme(&self) -> SightlineThemeMode {
        let next = self.theme_mode.get().toggled();
        self.theme_mode.set(next);
        next
    }
}

fn build_ui() -> Ui {
    let mut ui = Ui::new();
    let tokens = ui.style_tokens::<SightlineStyleTokens>();
    let state = Rc::new(AppState::new());

    ui.set_theme(sightline_theme(state.theme_mode(), tokens));
    ui.set_cascade(sightline_cascade(ui.properties(), tokens));

    let root = ui.root();
    ui.add_class(root, tokens.shell);
    ui.set_layout(root, Layout::column(0.0).fill_size());

    let shell = shell::build(&mut ui, root, tokens, Rc::clone(&state));
    shell::wire(&mut ui, shell, tokens, state);
    ui
}
