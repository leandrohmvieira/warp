use std::cell::Cell;
use std::rc::Rc;

use super::TuiHoverable;
use crate::elements::tui::{
    TuiElement, TuiEvent, TuiEventContext, TuiLayoutContext, TuiPoint, TuiRect,
};
use crate::event::ModifiersState;
use crate::{App, EntityIdMap};

fn left_mouse_down(x: u16, y: u16) -> TuiEvent {
    TuiEvent::LeftMouseDown {
        position: TuiPoint::new(x, y),
        modifiers: ModifiersState::default(),
        click_count: 1,
        is_first_mouse: false,
    }
}

#[test]
fn click_inside_area_runs_callback_and_reports_handled() {
    App::test((), |app| async move {
        app.read(|app_ctx| {
            let hits = Rc::new(Cell::new(0u32));
            let counter = hits.clone();
            let mut hoverable = TuiHoverable::new(()).on_click(move |_ctx, _app| {
                counter.set(counter.get() + 1);
            });

            let area = TuiRect::new(0, 0, 4, 2);
            let mut event_ctx = TuiEventContext::default();
            let mut rendered_views = EntityIdMap::default();
            let mut ctx = TuiLayoutContext {
                rendered_views: &mut rendered_views,
            };

            let handled = hoverable.dispatch_event(
                &left_mouse_down(1, 1),
                area,
                &mut event_ctx,
                &mut ctx,
                app_ctx,
            );
            assert!(handled);
            assert_eq!(hits.get(), 1);

            // A click outside the area is left unhandled and runs no callback.
            let handled = hoverable.dispatch_event(
                &left_mouse_down(10, 10),
                area,
                &mut event_ctx,
                &mut ctx,
                app_ctx,
            );
            assert!(!handled);
            assert_eq!(hits.get(), 1);
        });
    });
}

#[test]
fn child_consumes_the_event_before_the_wrapper() {
    App::test((), |app| async move {
        app.read(|app_ctx| {
            let outer_hits = Rc::new(Cell::new(0u32));
            let outer_counter = outer_hits.clone();

            // A child that always handles the event pre-empts the wrapper's click.
            let child = AlwaysHandles;
            let mut hoverable = TuiHoverable::new(child).on_click(move |_, _| {
                outer_counter.set(outer_counter.get() + 1);
            });

            let mut event_ctx = TuiEventContext::default();
            let mut rendered_views = EntityIdMap::default();
            let mut ctx = TuiLayoutContext {
                rendered_views: &mut rendered_views,
            };
            let handled = hoverable.dispatch_event(
                &left_mouse_down(0, 0),
                TuiRect::new(0, 0, 1, 1),
                &mut event_ctx,
                &mut ctx,
                app_ctx,
            );

            assert!(handled);
            assert_eq!(outer_hits.get(), 0);
        });
    });
}

/// A leaf element that reports every event as handled, used to verify the
/// wrapper defers to its child.
struct AlwaysHandles;

impl TuiElement for AlwaysHandles {
    fn layout(
        &mut self,
        constraint: crate::elements::tui::TuiConstraint,
        _ctx: &mut TuiLayoutContext,
        _app: &crate::AppContext,
    ) -> crate::elements::tui::TuiSize {
        constraint.min
    }

    fn render(
        &self,
        _area: TuiRect,
        _buffer: &mut crate::elements::tui::TuiBuffer,
        _ctx: &mut TuiLayoutContext,
    ) {
    }

    fn dispatch_event(
        &mut self,
        _event: &TuiEvent,
        _area: TuiRect,
        _event_ctx: &mut TuiEventContext,
        _ctx: &mut TuiLayoutContext,
        _app: &crate::AppContext,
    ) -> bool {
        true
    }
}
