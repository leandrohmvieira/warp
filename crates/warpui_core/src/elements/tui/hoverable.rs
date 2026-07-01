//! [`TuiHoverable`]: wraps a child element and runs a callback when it is
//! clicked.
//!
//! # Construction
//! Wrap a child with [`TuiHoverable::new`] and register a click handler with
//! [`on_click`](TuiHoverable::on_click). Layout, render, height, and cursor are
//! transparent — they delegate to the wrapped child.
//!
//! # Dispatch policy
//! On [`dispatch_event`](TuiElement::dispatch_event) the event is offered to the
//! child first. If the child consumes it, dispatch stops. Otherwise a
//! [`LeftMouseDown`](TuiEvent::LeftMouseDown) whose position falls within this
//! element's area invokes the click handler (with the [`TuiEventContext`] and
//! the [`AppContext`]) and is reported handled. Other events are left unhandled
//! so ancestors can react.
//!
//! # Scope
//! This is the TUI counterpart of the GUI's
//! [`Hoverable`](crate::elements::gui::Hoverable), but intentionally minimal: it
//! only handles clicks. Hover-state tracking, cursor changes, hover delays, and
//! the other click variants the GUI element offers can be added here as needs
//! arise.

use super::{
    TuiBuffer, TuiConstraint, TuiElement, TuiEvent, TuiEventContext, TuiLayoutContext,
    TuiPresentationContext, TuiRect, TuiRectExt, TuiSize,
};
use crate::AppContext;

type ClickCallback = Box<dyn FnMut(&mut TuiEventContext, &AppContext)>;

pub struct TuiHoverable {
    child: Box<dyn TuiElement>,
    on_click: Option<ClickCallback>,
}

impl TuiHoverable {
    pub fn new(child: impl TuiElement + 'static) -> Self {
        Self {
            child: Box::new(child),
            on_click: None,
        }
    }

    /// Registers `callback` to run when a `LeftMouseDown` within this element's
    /// area reaches it unhandled by the child.
    pub fn on_click(
        mut self,
        callback: impl FnMut(&mut TuiEventContext, &AppContext) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(callback));
        self
    }
}

impl TuiElement for TuiHoverable {
    fn layout(
        &mut self,
        constraint: TuiConstraint,
        ctx: &mut TuiLayoutContext,
        app: &AppContext,
    ) -> TuiSize {
        self.child.layout(constraint, ctx, app)
    }

    fn render(&self, area: TuiRect, buffer: &mut TuiBuffer, ctx: &mut TuiLayoutContext) {
        self.child.render(area, buffer, ctx);
    }

    fn cursor_position(&self, area: TuiRect, ctx: &mut TuiLayoutContext) -> Option<(u16, u16)> {
        self.child.cursor_position(area, ctx)
    }

    fn present(&mut self, ctx: &mut TuiPresentationContext<'_>) {
        self.child.present(ctx);
    }

    fn dispatch_event(
        &mut self,
        event: &TuiEvent,
        area: TuiRect,
        event_ctx: &mut TuiEventContext,
        ctx: &mut TuiLayoutContext,
        app: &AppContext,
    ) -> bool {
        if self.child.dispatch_event(event, area, event_ctx, ctx, app) {
            return true;
        }

        if let (TuiEvent::LeftMouseDown { position, .. }, Some(on_click)) =
            (event, self.on_click.as_mut())
        {
            if area.contains_point(*position) {
                on_click(event_ctx, app);
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
#[path = "hoverable_tests.rs"]
mod tests;
