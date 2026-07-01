//! [`TuiCollapsible`]: a disclosure section — a clickable text header with a
//! chevron over a body that shows only when expanded.
//!
//! # Construction
//! Build with [`TuiCollapsible::new`], passing the current `collapsed` state, a
//! header label, and the body element. Register a toggle handler with
//! [`on_toggle`](TuiCollapsible::on_toggle); style the header line with
//! [`with_header_style`](TuiCollapsible::with_header_style); override the
//! disclosure glyphs with
//! [`with_chevron_glyphs`](TuiCollapsible::with_chevron_glyphs).
//!
//! # Composition
//! The element is a plain composition of existing primitives: a [`TuiColumn`]
//! whose first child is a [`TuiHoverable`]-wrapped [`TuiText`] header (label
//! followed by a chevron reflecting the state) and whose second child — present
//! only when expanded — is the body. State is owned by the caller: `collapsed`
//! is read each frame and `on_toggle` fires on a header click, leaving the
//! caller to flip its own state and re-render.

use super::{
    TuiBuffer, TuiColumn, TuiConstraint, TuiElement, TuiEvent, TuiEventContext, TuiHoverable,
    TuiLayoutContext, TuiParentElement, TuiPresentationContext, TuiRect, TuiSize, TuiStyle,
    TuiText,
};
use crate::AppContext;

/// Default disclosure glyph shown when the section is collapsed.
const DEFAULT_CHEVRON_COLLAPSED: &str = "▸";
/// Default disclosure glyph shown when the section is expanded.
const DEFAULT_CHEVRON_EXPANDED: &str = "▾";

type ToggleCallback = Box<dyn FnMut(&mut TuiEventContext, &AppContext)>;

pub struct TuiCollapsible {
    collapsed: bool,
    header_label: String,
    header_style: TuiStyle,
    chevron_collapsed: String,
    chevron_expanded: String,
    body: Option<Box<dyn TuiElement>>,
    on_toggle: Option<ToggleCallback>,
    /// The composed inner tree, built lazily on first layout/dispatch so the
    /// moved-in `body` and `on_toggle` are consumed exactly once.
    built: Option<Box<dyn TuiElement>>,
}

impl TuiCollapsible {
    /// Creates a collapsible with header `label` over `body`, disclosed when
    /// `collapsed` is `false`.
    pub fn new(collapsed: bool, label: impl Into<String>, body: impl TuiElement + 'static) -> Self {
        Self {
            collapsed,
            header_label: label.into(),
            header_style: TuiStyle::default(),
            chevron_collapsed: DEFAULT_CHEVRON_COLLAPSED.to_owned(),
            chevron_expanded: DEFAULT_CHEVRON_EXPANDED.to_owned(),
            body: Some(Box::new(body)),
            on_toggle: None,
            built: None,
        }
    }

    /// Registers `callback` to run when the header is clicked.
    pub fn on_toggle(
        mut self,
        callback: impl FnMut(&mut TuiEventContext, &AppContext) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(callback));
        self
    }

    /// Sets the style applied to the header line (label and chevron).
    pub fn with_header_style(mut self, style: TuiStyle) -> Self {
        self.header_style = style;
        self
    }

    /// Overrides the disclosure glyphs used for the collapsed and expanded
    /// states.
    pub fn with_chevron_glyphs(
        mut self,
        collapsed: impl Into<String>,
        expanded: impl Into<String>,
    ) -> Self {
        self.chevron_collapsed = collapsed.into();
        self.chevron_expanded = expanded.into();
        self
    }

    /// The chevron glyph for the current state.
    fn chevron(&self) -> &str {
        if self.collapsed {
            &self.chevron_collapsed
        } else {
            &self.chevron_expanded
        }
    }

    /// Composes the header + body into the inner element tree once, consuming
    /// the moved-in body and toggle handler.
    fn ensure_built(&mut self) {
        if self.built.is_some() {
            return;
        }

        let header_line = format!("{} {}", self.header_label, self.chevron());
        let mut header = TuiHoverable::new(
            TuiText::new(header_line)
                .with_style(self.header_style)
                .truncate(),
        );
        if let Some(on_toggle) = self.on_toggle.take() {
            header = header.on_click(on_toggle);
        }

        let mut column = TuiColumn::new().child(header);
        if !self.collapsed {
            if let Some(body) = self.body.take() {
                column = column.with_child(body);
            }
        }
        self.built = Some(column.finish());
    }
}

impl TuiElement for TuiCollapsible {
    fn layout(
        &mut self,
        constraint: TuiConstraint,
        ctx: &mut TuiLayoutContext,
        app: &AppContext,
    ) -> TuiSize {
        self.ensure_built();
        self.built
            .as_mut()
            .map(|built| built.layout(constraint, ctx, app))
            .unwrap_or(TuiSize::ZERO)
    }

    fn render(&self, area: TuiRect, buffer: &mut TuiBuffer, ctx: &mut TuiLayoutContext) {
        if let Some(built) = self.built.as_ref() {
            built.render(area, buffer, ctx);
        }
    }

    fn cursor_position(&self, area: TuiRect, ctx: &mut TuiLayoutContext) -> Option<(u16, u16)> {
        self.built.as_ref()?.cursor_position(area, ctx)
    }

    fn present(&mut self, ctx: &mut TuiPresentationContext<'_>) {
        if let Some(built) = self.built.as_mut() {
            built.present(ctx);
        }
    }

    fn dispatch_event(
        &mut self,
        event: &TuiEvent,
        area: TuiRect,
        event_ctx: &mut TuiEventContext,
        ctx: &mut TuiLayoutContext,
        app: &AppContext,
    ) -> bool {
        self.ensure_built();
        self.built
            .as_mut()
            .map(|built| built.dispatch_event(event, area, event_ctx, ctx, app))
            .unwrap_or(false)
    }
}

#[cfg(test)]
#[path = "collapsible_tests.rs"]
mod tests;
