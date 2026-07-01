# TUI Thinking Blocks — Tech Spec
Implements the behavior in [`PRODUCT.md`](./PRODUCT.md). References are pinned to commit `375878c2c331e8912a9a458249da0ae3ca35e45d`.
## Context
The TUI transcript renders agent exchanges but drops reasoning. We add reasoning rendering plus two generic, reusable TUI elements (a click handler and a collapsible) so the thinking block is a composition of primitives rather than a bespoke element.
- [`crates/warp_tui/src/transcript_view.rs`](https://github.com/warpdotdev/warp/blob/375878c2c331e8912a9a458249da0ae3ca35e45d/crates/warp_tui/src/transcript_view.rs) — owns a `TuiViewportedList` over `TuiBlockListViewportSource`; each agent exchange is a `TuiAIBlock` view in a registry.
- [`crates/warp_tui/src/agent_block.rs (24-145)`](https://github.com/warpdotdev/warp/blob/375878c2c331e8912a9a458249da0ae3ca35e45d/crates/warp_tui/src/agent_block.rs#L24-L145) — `TuiAIBlockSection` (`Input`/`PlainText`), `sections()` (uses `text_from_agent_output()`, so reasoning is dropped), `render_element()`, and `desired_height()`.
- [`crates/warp_tui/src/tui_block_list_viewport_source.rs (74-140)`](https://github.com/warpdotdev/warp/blob/375878c2c331e8912a9a458249da0ae3ca35e45d/crates/warp_tui/src/tui_block_list_viewport_source.rs#L74-L140) — re-measures every on-screen agent block each frame (viewport + overhang), so a height change from collapse/expand reflows on the next redraw with only a `notify()`.
- [`crates/warp_tui/src/tui_block_list_viewport_source.rs:337`](https://github.com/warpdotdev/warp/blob/375878c2c331e8912a9a458249da0ae3ca35e45d/crates/warp_tui/src/tui_block_list_viewport_source.rs#L337) — agent blocks render inline via `view.as_ref(app).render(app)` (not as `TuiChildView`), so events/`notify()` inside a block attribute to the transcript (root) view.
- [`app/src/ai/agent/mod.rs:1717`](https://github.com/warpdotdev/warp/blob/375878c2c331e8912a9a458249da0ae3ca35e45d/app/src/ai/agent/mod.rs#L1717) — `AIAgentOutputMessageType::Reasoning { text, finished_duration: Option<Duration> }`; `finished_duration` is `None` while streaming, `Some` when done. `AIAgentOutput.messages` is public and ordered.
- [`app/src/ai/blocklist/block/view_impl/common.rs:690`](https://github.com/warpdotdev/warp/blob/375878c2c331e8912a9a458249da0ae3ca35e45d/app/src/ai/blocklist/block/view_impl/common.rs#L690) — GUI `format_elapsed_seconds` (pluralization reference; reimplemented locally in `agent_block.rs` since the module is `pub(super)`).
- [`app/src/ai/blocklist/block.rs (720-858)`](https://github.com/warpdotdev/warp/blob/375878c2c331e8912a9a458249da0ae3ca35e45d/app/src/ai/blocklist/block.rs#L720-L858) — GUI `CollapsibleElementState`: the collapse/auto-collapse/manual-toggle semantics to mirror (minus the display-mode gating, deferred here).
- [`crates/warpui_core/src/elements/tui/mod.rs (40-58)`](https://github.com/warpdotdev/warp/blob/375878c2c331e8912a9a458249da0ae3ca35e45d/crates/warpui_core/src/elements/tui/mod.rs#L40-L58) — element exports.
- [`crates/warpui_core/src/elements/tui/event_handler.rs`](https://github.com/warpdotdev/warp/blob/375878c2c331e8912a9a458249da0ae3ca35e45d/crates/warpui_core/src/elements/tui/event_handler.rs) — `TuiEventHandler`, the structural precedent for a wrap-one-child, offer-to-child-first, callback element (key-only today).
- [`crates/warpui_core/src/elements/gui/hoverable.rs`](https://github.com/warpdotdev/warp/blob/375878c2c331e8912a9a458249da0ae3ca35e45d/crates/warpui_core/src/elements/gui/hoverable.rs) — GUI `Hoverable`, the naming/shape precedent for the new TUI element.
- [`app/src/tui_export.rs`](https://github.com/warpdotdev/warp/blob/375878c2c331e8912a9a458249da0ae3ca35e45d/app/src/tui_export.rs) — public app surface for `warp_tui`; already exports `AIAgentOutput{,Message,MessageType}`, `AIAgentText`, `AIAgentTextSection`, `MessageId`.
## Proposed changes
### 1. `TuiHoverable` element (new)
`crates/warpui_core/src/elements/tui/hoverable.rs`, exported from `elements/tui/mod.rs`. Named for parity with the GUI `Hoverable` but scoped to click handling only. Structurally mirrors `TuiEventHandler`: wraps one child; `layout`/`render`/`cursor_position`/`present` delegate to the child; `dispatch_event` offers the event to the child first, and if unhandled and the event is `TuiEvent::LeftMouseDown` with `position` inside `area`, runs an `.on_click(FnMut(&mut TuiEventContext, &AppContext))` callback and returns handled. No hover-state tracking, cursor, delays, or other click variants yet (no TUI `MouseStateHandle` exists); these can be added later.
### 2. `TuiCollapsible` element (new)
`crates/warpui_core/src/elements/tui/collapsible.rs`, exported from `elements/tui/mod.rs`. Presentational disclosure element; owns no persistent state (caller owns `collapsed`). Built as a `TuiColumn`:
- Header row: caller's header content element + a space + a chevron `TuiText` (default glyphs: right-triangle collapsed, down-triangle expanded), wrapped in a `TuiHoverable` whose `on_click` invokes the caller's `on_toggle`.
- Body: the caller's body element, added as a second child only when `!collapsed`.
Builders: `new(collapsed, header, body)`, `.on_toggle(FnMut(&mut TuiEventContext, &AppContext))`, `.with_chevron_style(TuiStyle)`.
### 3. Reasoning extraction (`agent_block.rs`)
Add a `Thinking { message_id: MessageId, finished_duration: Option<Duration>, body: String }` variant to `TuiAIBlockSection`. Rewrite `sections()` to iterate `output.messages` in order instead of `text_from_agent_output()`:
- `Text` → `PlainText` sections (unchanged behavior).
- `Reasoning` → a `Thinking` section whose `body` joins the reasoning's `PlainText` sections with newlines (other section kinds skipped, matching current plaintext-only handling). A reasoning message with an empty body still yields a `Thinking` section (Behavior 15). Reasoning is always shown in this version — the display-mode gating is deferred.
### 4. Per-reasoning collapse state (`agent_block.rs`)
Add `thinking_states: Rc<RefCell<HashMap<MessageId, ThinkingUiState>>>` to `TuiAIBlock` (empty in `new`). `ThinkingUiState { collapsed: bool, last_known_finished: bool, user_toggled: bool }` mirrors the relevant `CollapsibleElementState` semantics:
- Default `collapsed = false` (expanded while streaming).
- Finish transition, applied during `sections()`/render via interior mutability (mutation during layout is permitted by the `TuiElement::layout` contract): when `finished_duration.is_some() && !last_known_finished`, set `collapsed = true` unless `user_toggled`; always update `last_known_finished`.
- Toggle flips `collapsed` and sets `user_toggled = true`.
### 5. Render thinking via `TuiCollapsible` (`agent_block.rs`)
Render `Thinking` (routed from `TuiAIBlock::render_element`, not the stateless `TuiAIBlockSection::render_element`) as a `TuiCollapsible`:
- Header text `Thinking...` when `finished_duration` is `None`, else `format!("Thought for {}", format_elapsed_seconds(dur))` using the locally reimplemented helper.
- `collapsed` read from the per-message `ThinkingUiState`; body = reasoning text lines indented four spaces, wrapped, bright-black.
- Color: `theme.terminal_colors().bright.black` via `Fill::from(..).into()` (as in `agent_block.rs`).
- `on_toggle`: capture a clone of `thinking_states` + the `MessageId`; flip `collapsed`, set `user_toggled`, then `event_ctx.notify()`. On-screen re-measure handles the height change; no dirty-marking needed.
Preserve the existing single input→output top-gap when the first output section is a thinking block.
## Testing and validation
Unit tests, run via `cargo nextest run -p warpui_core` and `cargo nextest run -p warp_tui`.
- `crates/warpui_core/src/elements/tui/hoverable_tests.rs`: `LeftMouseDown` inside `area` runs `on_click`; outside does not; a child that handles the event pre-empts `on_click`. (Behavior 9)
- `crates/warpui_core/src/elements/tui/collapsible_tests.rs`: header always rendered; body rendered only when expanded; chevron glyph matches state; clicking the header invokes `on_toggle`. (Behavior 2, 5, 9)
- Extend `crates/warp_tui/src/agent_block_tests.rs`:
  - Streaming reasoning yields a `Thinking` section with header `Thinking...` and the body present/expanded. (Behavior 3, 6, 15)
  - Finished reasoning yields header `Thought for N seconds` with correct pluralization. (Behavior 4)
  - Auto-collapse on finish; body hidden. (Behavior 8, 13)
  - Manual toggle before finish prevents auto-collapse. (Behavior 10)
  - Reasoning interleaves with plain-text sections in message order. (Behavior 1); multiple independent reasoning blocks. (Behavior 14)
Manual verification via the TUI dev binary (`script/run-tui`): observe `Thinking...` with expanded body while streaming, auto-collapse to `Thought for N seconds` on finish, and click-to-toggle in both states. (Behavior 6–9, 12). Run `./script/format` and `cargo clippy` (per `AGENTS.md`) before a PR.
## Parallelization
Not beneficial. The two generic elements, reasoning extraction, and rendering are tightly coupled and all edited within `agent_block.rs` plus two small new sibling files in one crate; splitting across agents would create merge contention on `elements/tui/mod.rs` and `agent_block.rs` for no wall-clock gain. Implement sequentially: elements (1–2) → extraction/state/render (3–5) → tests.
