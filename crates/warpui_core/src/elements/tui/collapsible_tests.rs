use std::cell::Cell;
use std::rc::Rc;

use super::TuiCollapsible;
use crate::elements::tui::{
    TuiBuffer, TuiBufferExt, TuiConstraint, TuiElement, TuiEvent, TuiEventContext,
    TuiLayoutContext, TuiPoint, TuiRect, TuiSize, TuiText,
};
use crate::event::ModifiersState;
use crate::{App, AppContext, EntityIdMap};

/// Lays out (so the inner tree is built) then renders `element` to one string
/// per row.
fn layout_and_render(element: &mut dyn TuiElement, size: TuiSize, app: &AppContext) -> Vec<String> {
    let mut rendered_views = EntityIdMap::default();
    let mut ctx = TuiLayoutContext {
        rendered_views: &mut rendered_views,
    };
    let area = TuiRect::new(0, 0, size.width, size.height);
    element.layout(TuiConstraint::loose(size), &mut ctx, app);
    let mut buffer = TuiBuffer::empty(area);
    element.render(area, &mut buffer, &mut ctx);
    buffer.to_lines()
}

fn left_mouse_down(x: u16, y: u16) -> TuiEvent {
    TuiEvent::LeftMouseDown {
        position: TuiPoint::new(x, y),
        modifiers: ModifiersState::default(),
        click_count: 1,
        is_first_mouse: false,
    }
}

#[test]
fn expanded_renders_header_with_down_chevron_and_body() {
    App::test((), |app| async move {
        app.read(|app_ctx| {
            let mut collapsible =
                TuiCollapsible::new(false, "Thinking...", TuiText::new("reasoning"));
            let lines = layout_and_render(&mut collapsible, TuiSize::new(20, 4), app_ctx);
            assert_eq!(lines[0].trim_end(), "Thinking... ▾");
            assert_eq!(lines[1].trim_end(), "reasoning");
        });
    });
}

#[test]
fn collapsed_renders_only_header_with_right_chevron() {
    App::test((), |app| async move {
        app.read(|app_ctx| {
            let mut collapsible =
                TuiCollapsible::new(true, "Thinking...", TuiText::new("reasoning"));
            let lines = layout_and_render(&mut collapsible, TuiSize::new(20, 4), app_ctx);
            assert_eq!(lines[0].trim_end(), "Thinking... ▸");
            // The body is not rendered when collapsed.
            assert!(lines[1..].iter().all(|line| line.trim().is_empty()));
        });
    });
}

#[test]
fn header_click_invokes_on_toggle() {
    App::test((), |app| async move {
        app.read(|app_ctx| {
            let hits = Rc::new(Cell::new(0u32));
            let counter = hits.clone();
            let mut collapsible =
                TuiCollapsible::new(false, "Thinking...", TuiText::new("reasoning"))
                    .on_toggle(move |_ctx, _app| counter.set(counter.get() + 1));

            let size = TuiSize::new(20, 4);
            let mut rendered_views = EntityIdMap::default();
            let mut ctx = TuiLayoutContext {
                rendered_views: &mut rendered_views,
            };
            let area = TuiRect::new(0, 0, size.width, size.height);
            collapsible.layout(TuiConstraint::loose(size), &mut ctx, app_ctx);

            let mut event_ctx = TuiEventContext::default();
            // Click on the header row (row 0).
            let handled = collapsible.dispatch_event(
                &left_mouse_down(2, 0),
                area,
                &mut event_ctx,
                &mut ctx,
                app_ctx,
            );
            assert!(handled);
            assert_eq!(hits.get(), 1);
        });
    });
}
