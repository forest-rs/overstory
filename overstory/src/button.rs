// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Button`: clickable / keyboard-activatable templated control.
//!
//! Application code:
//!
//! ```ignore
//! ui.append(
//!     parent,
//!     Button::new("Click me")
//!         .font_size(22.0)
//!         .on_click(|ui, event| { … }),
//! );
//! ```

use alloc::boxed::Box;
use alloc::string::String;

use peniko::Brush;
use understory_property_binding::PropertyEndpoint;
use understory_style::TypeTag;

use crate::Phase;
use crate::element::ElementId;
use crate::template::Template;
use crate::text_behavior::Text;
use crate::ui::{ClickEvent, KeyEvent, Ui};
use crate::widget::{AppendSpec, Widget};

/// Selector identity for `Button`.
pub const TYPE_BUTTON: TypeTag = TypeTag(2);

/// Builder + spec for a `Button`. Use [`Button::new`] to start a
/// chain, configure with the chaining methods, and pass to
/// [`Ui::append`].
pub struct Button {
    content: String,
    font_size: f64,
    foreground: Option<Brush>,
    focusable: bool,
    on_click: Option<BoxedClickHandler>,
    on_key: Option<BoxedKeyHandler>,
}

type BoxedClickHandler = Box<dyn FnMut(&mut Ui, ClickEvent) + 'static>;
type BoxedKeyHandler = Box<dyn FnMut(&mut Ui, KeyEvent<'_>) + 'static>;

impl core::fmt::Debug for Button {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Button")
            .field("content", &self.content)
            .field("font_size", &self.font_size)
            .finish_non_exhaustive()
    }
}

impl Button {
    /// Starts a new `Button` builder with the given label.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            font_size: 22.0,
            foreground: None,
            focusable: true,
            on_click: None,
            on_key: None,
        }
    }

    /// Overrides the button's font size (logical pixels). Default
    /// is 22.0.
    #[must_use]
    pub fn font_size(mut self, font_size: f64) -> Self {
        self.font_size = font_size;
        self
    }

    /// Overrides the button's foreground brush. Default leaves it
    /// to the cascade (typically the theme's `ON_ACCENT_FG` token).
    #[must_use]
    pub fn foreground(mut self, brush: impl Into<Brush>) -> Self {
        self.foreground = Some(brush.into());
        self
    }

    /// Sets whether the button participates in focus traversal.
    /// Default `true`.
    #[must_use]
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    /// Registers a click handler. Fires on pointer click and on
    /// keyboard activation (Space / Enter while focused). The
    /// closure receives the standard [`ClickEvent`] payload —
    /// dispatch fires on either `Phase::Target` (button itself
    /// hit) or `Phase::Bubble` (a template part hit).
    #[must_use]
    pub fn on_click<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, ClickEvent) + 'static,
    {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// Registers a custom key handler. The button already handles
    /// Space / Enter to synthesize a click; use this for richer
    /// keyboard interactions.
    #[must_use]
    pub fn on_key<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Ui, KeyEvent<'_>) + 'static,
    {
        self.on_key = Some(Box::new(handler));
        self
    }
}

impl AppendSpec for Button {
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(ButtonWidget));
        let content_prop = ui.properties().content;
        let font_size_prop = ui.properties().font_size;
        let fg_prop = ui.properties().foreground;
        ui.set_local(id, content_prop, self.content);
        ui.set_local(id, font_size_prop, self.font_size);
        if let Some(brush) = self.foreground {
            ui.set_local(id, fg_prop, brush);
        }
        if self.focusable {
            ui.set_focusable(id, true);
        }
        if let Some(handler) = self.on_click {
            ui.on_click(id, handler);
        } else {
            // No custom handler — but Space / Enter should still
            // emit a *click* for handlers that subscribe via
            // post-construction `Ui::on_click`. We install a
            // default key-to-click bridge below.
        }
        if let Some(handler) = self.on_key {
            ui.on_key(id, handler);
        } else {
            // Default: Space / Enter while focused dispatches a
            // synthetic click. Apps that registered their own
            // `on_key` are responsible for activation themselves.
            install_default_activation_key(ui, id);
        }
        id
    }
}

/// Per-element widget for `Button`. Carries no per-instance state —
/// the button's runtime configuration lives in properties + the
/// registered handlers. The unit struct is enough to attach
/// behavior via `Widget`.
#[derive(Debug, Default)]
pub struct ButtonWidget;

impl Widget for ButtonWidget {
    fn type_tag(&self) -> TypeTag {
        TYPE_BUTTON
    }

    fn default_template(&self) -> Option<&'static dyn Template> {
        Some(&BUTTON_TEMPLATE)
    }
}

static BUTTON_TEMPLATE: ButtonTemplate = ButtonTemplate;

#[derive(Debug, Default)]
struct ButtonTemplate;

impl Template for ButtonTemplate {
    fn instantiate(&self, ui: &mut Ui, owner: ElementId) {
        let content_prop = ui.properties().content;
        let foreground_prop = ui.properties().foreground;
        let font_size_prop = ui.properties().font_size;

        // The button's label is a `Text` widget bound to the button's
        // Content / Foreground / FontSize.
        let label = ui.append(owner, Text::new(""));

        let _ = ui.bind(
            PropertyEndpoint::new(owner, content_prop),
            PropertyEndpoint::new(label, content_prop),
        );
        let _ = ui.bind(
            PropertyEndpoint::new(owner, foreground_prop),
            PropertyEndpoint::new(label, foreground_prop),
        );
        let _ = ui.bind(
            PropertyEndpoint::new(owner, font_size_prop),
            PropertyEndpoint::new(label, font_size_prop),
        );
    }
}

/// Default Space / Enter → click bridge for `Button`. Installed by
/// `AppendSpec::append_to` when the app didn't supply its own
/// `on_key`.
fn install_default_activation_key(ui: &mut Ui, button: ElementId) {
    ui.on_key(button, move |ui, event| {
        use ui_events::keyboard::{Key, KeyState, NamedKey};
        if !matches!(event.phase, Phase::Target) {
            return;
        }
        if event.key.state != KeyState::Down {
            return;
        }
        let activate = match &event.key.key {
            Key::Named(NamedKey::Enter) => true,
            Key::Character(s) if s == " " => true,
            _ => false,
        };
        if activate {
            // Synthesize a click. We use the focused element's world-
            // bounds origin so click handlers that care about position
            // get a sensible value.
            let pos = ui.effective_bounds(button).origin();
            ui.synthesize_click(button, pos);
        }
    });
}
