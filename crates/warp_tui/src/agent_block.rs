//! An agent block in the TUI transcript: one exchange rendered as the user's
//! submitted input followed by the agent's response.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use warp::tui_export::{
    AIAgentAction, AIAgentExchangeId, AIAgentOutputMessageType, AIAgentText, AIAgentTextSection,
    AIBlockModel, AIConversationId, Appearance, MessageId,
};
use warp_core::ui::color::blend::Blend;
// `ThemeFill` is the theme-layer color (it supports blend/opacity); `Fill` below
// is the element-layer color it converts into on its way to a terminal cell.
use warp_core::ui::theme::Fill as ThemeFill;
use warpui::SingletonEntity;
use warpui_core::elements::tui::{
    Modifier, TuiCollapsible, TuiColumn, TuiConstraint, TuiContainer, TuiElement, TuiLayoutContext,
    TuiParentElement, TuiSize, TuiStyle, TuiText,
};
use warpui_core::elements::Fill;
use warpui_core::{AppContext, Entity, EntityIdMap, TuiView};

const INPUT_PREFIX: &str = "≫ ";
/// Left indent (cells) applied to a thinking block's reasoning body so every
/// wrapped line aligns beneath the header.
const THINKING_BODY_INDENT: u16 = 4;
/// Bottom padding (rows) applied beneath every rendered block, giving uniform
/// spacing between blocks and after the last one.
const BLOCK_BOTTOM_PADDING: u16 = 1;

/// Renderable pieces of an agent block; this will grow as we render richer sections.
#[derive(Clone, Debug, Eq, PartialEq)]
enum TuiAIBlockSection {
    Input(String),
    PlainText(String),
    /// A lightweight status row standing in for an agent tool call.
    ToolCall(Box<AIAgentAction>),
    /// A reasoning ("thinking") segment, rendered as a collapsible block.
    Thinking {
        message_id: MessageId,
        finished_duration: Option<Duration>,
        body: String,
    },
}

/// Per-reasoning-message UI state backing a thinking block's collapse behavior.
#[derive(Default)]
struct ThinkingUiState {
    collapsed: bool,
    last_known_finished: bool,
    user_toggled: bool,
}

/// A thin TUI rich-content view adapter backed by one agent exchange.
///
/// The rendering logic is mostly section extraction, but the shared block list
/// stores rich content by view id, so this remains a registered view.
pub(super) struct TuiAIBlock {
    conversation_id: AIConversationId,
    exchange_id: AIAgentExchangeId,
    model: Rc<dyn AIBlockModel<View = Self>>,
    /// Collapse state per reasoning message, shared into header click handlers.
    thinking_states: Rc<RefCell<HashMap<MessageId, ThinkingUiState>>>,
}

/// Extracts model state into renderable agent block sections.
impl TuiAIBlock {
    /// Creates a simple exchange-backed agent block.
    pub(super) fn new(
        conversation_id: AIConversationId,
        exchange_id: AIAgentExchangeId,
        model: Rc<dyn AIBlockModel<View = Self>>,
    ) -> Self {
        Self {
            conversation_id,
            exchange_id,
            model,
            thinking_states: Default::default(),
        }
    }

    /// Replaces the backing model when the same exchange is reassigned.
    pub(super) fn replace_model(
        &mut self,
        conversation_id: AIConversationId,
        model: Rc<dyn AIBlockModel<View = Self>>,
    ) {
        self.conversation_id = conversation_id;
        self.model = model;
    }

    /// Returns the conversation that currently owns this agent block.
    pub(super) fn conversation_id(&self) -> AIConversationId {
        self.conversation_id
    }

    /// Returns the exchange rendered by this agent block.
    pub(super) fn exchange_id(&self) -> AIAgentExchangeId {
        self.exchange_id
    }

    /// Returns this block's wrapped height at the given width.
    pub(super) fn desired_height(&self, width: u16, app: &AppContext) -> usize {
        let mut rendered_views = EntityIdMap::default();
        let mut ctx = TuiLayoutContext {
            rendered_views: &mut rendered_views,
        };
        let mut element = self.render_element(app);
        usize::from(
            element
                .layout(
                    TuiConstraint::loose(TuiSize::new(width, u16::MAX)),
                    &mut ctx,
                    app,
                )
                .height,
        )
    }

    /// Extracts this exchange's visible input/output into logical render sections,
    /// preserving message order so reasoning interleaves with plain-text output.
    fn sections(&self, app: &AppContext) -> Vec<TuiAIBlockSection> {
        let mut sections = Vec::new();
        let input = self
            .model
            .inputs_to_render(app)
            .iter()
            .filter_map(|input| input.display_query())
            .collect::<Vec<_>>()
            .join("\n");
        if !input.is_empty() {
            sections.push(TuiAIBlockSection::Input(input));
        }

        // Walk output messages in order so tool-call rows interleave with text.
        if let Some(output) = self.model.status(app).output_to_render() {
            let output = output.get();
            for message in &output.messages {
                match &message.message {
                    AIAgentOutputMessageType::Text(text) => {
                        sections.extend(text.sections.iter().filter_map(|section| {
                            match section {
                                AIAgentTextSection::PlainText { text } => (!text.text().is_empty())
                                    .then(|| TuiAIBlockSection::PlainText(text.text().to_owned())),
                                // Add item variants here as the TUI learns to render richer sections.
                                AIAgentTextSection::Code { .. }
                                | AIAgentTextSection::Table { .. }
                                | AIAgentTextSection::Image { .. }
                                | AIAgentTextSection::MermaidDiagram { .. } => None,
                            }
                        }));
                    }
                    AIAgentOutputMessageType::Action(action) => {
                        sections.push(TuiAIBlockSection::ToolCall(Box::new(action.clone())));
                    }
                    AIAgentOutputMessageType::Reasoning {
                        text,
                        finished_duration,
                    } => {
                        self.sync_thinking_state(&message.id, finished_duration.is_some());
                        sections.push(TuiAIBlockSection::Thinking {
                            message_id: message.id.clone(),
                            finished_duration: *finished_duration,
                            body: reasoning_body(text),
                        });
                    }
                    // Other message kinds are not rendered by the TUI transcript yet.
                    AIAgentOutputMessageType::Summarization { .. }
                    | AIAgentOutputMessageType::Subagent(_)
                    | AIAgentOutputMessageType::TodoOperation(_)
                    | AIAgentOutputMessageType::WebSearch(_)
                    | AIAgentOutputMessageType::WebFetch(_)
                    | AIAgentOutputMessageType::CommentsAddressed { .. }
                    | AIAgentOutputMessageType::DebugOutput { .. }
                    | AIAgentOutputMessageType::ArtifactCreated(_)
                    | AIAgentOutputMessageType::SkillInvoked(_)
                    | AIAgentOutputMessageType::MessagesReceivedFromAgents { .. }
                    | AIAgentOutputMessageType::EventsFromAgents { .. } => {}
                }
            }
        }

        sections
    }

    /// Applies the finish transition for a reasoning message's collapse state:
    /// on the first finish it auto-collapses, unless the user has manually
    /// toggled the block. Interior mutability lets this run during layout-time
    /// section extraction.
    fn sync_thinking_state(&self, message_id: &MessageId, finished: bool) {
        let mut states = self.thinking_states.borrow_mut();
        let state = states.entry(message_id.clone()).or_default();
        if finished && !state.last_known_finished && !state.user_toggled {
            state.collapsed = true;
        }
        state.last_known_finished = finished;
    }

    /// Whether the thinking block for `message_id` is currently collapsed.
    fn is_thinking_collapsed(&self, message_id: &MessageId) -> bool {
        self.thinking_states
            .borrow()
            .get(message_id)
            .is_some_and(|state| state.collapsed)
    }

    /// Renders a reasoning message as a collapsible thinking block.
    fn render_thinking(
        &self,
        message_id: &MessageId,
        finished_duration: Option<Duration>,
        body: &str,
        app: &AppContext,
    ) -> Box<dyn TuiElement> {
        let theme = Appearance::as_ref(app).theme();
        let text_color = Fill::from(ThemeFill::from(theme.terminal_colors().bright.black)).into();
        let style = TuiStyle::default().fg(text_color);

        let header = match finished_duration {
            Some(duration) => format!("Thought for {}", format_elapsed_seconds(duration)),
            None => "Thinking...".to_owned(),
        };

        // Indent the whole reasoning body beneath the header via left padding so
        // every wrapped line aligns, not just the first. An empty body renders
        // nothing, so a just-started block shows only the header.
        let body_element = TuiContainer::new(TuiText::new(body.to_owned()).with_style(style))
            .with_padding_left(THINKING_BODY_INDENT);

        let collapsed = self.is_thinking_collapsed(message_id);
        let thinking_states = self.thinking_states.clone();
        let toggle_message_id = message_id.clone();
        let collapsible = TuiCollapsible::new(collapsed, header, body_element)
            .with_header_style(style)
            .on_toggle(move |event_ctx, _app| {
                let mut states = thinking_states.borrow_mut();
                let state = states.entry(toggle_message_id.clone()).or_default();
                state.collapsed = !state.collapsed;
                state.user_toggled = true;
                event_ctx.notify();
            });
        TuiContainer::new(collapsible)
            .with_padding_bottom(BLOCK_BOTTOM_PADDING)
            .finish()
    }

    /// Builds this block's generic TUI element tree.
    fn render_element(&self, app: &AppContext) -> Box<dyn TuiElement> {
        let sections = self.sections(app);

        // Every section renders with its own bottom padding (see the section
        // renderers), so blocks are uniformly spaced without special-casing the
        // input→output boundary.
        let mut column = TuiColumn::new();
        for section in &sections {
            let element = match section {
                TuiAIBlockSection::Thinking {
                    message_id,
                    finished_duration,
                    body,
                } => self.render_thinking(message_id, *finished_duration, body, app),
                TuiAIBlockSection::Input(_)
                | TuiAIBlockSection::PlainText(_)
                | TuiAIBlockSection::ToolCall(_) => section.render_element(app),
            };
            column = column.with_child(element);
        }

        column.finish()
    }
}

/// Joins a reasoning message's plain-text sections into a single body string.
fn reasoning_body(text: &AIAgentText) -> String {
    text.sections
        .iter()
        .filter_map(|section| match section {
            AIAgentTextSection::PlainText { text } => Some(text.text()),
            AIAgentTextSection::Code { .. }
            | AIAgentTextSection::Table { .. }
            | AIAgentTextSection::Image { .. }
            | AIAgentTextSection::MermaidDiagram { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Formats an elapsed reasoning duration as a human-readable seconds string.
fn format_elapsed_seconds(elapsed: Duration) -> String {
    let total_seconds = elapsed.as_secs();
    if total_seconds == 1 {
        "1 second".to_owned()
    } else {
        format!("{total_seconds} seconds")
    }
}

/// Converts one logical section into a renderable TUI element.
impl TuiAIBlockSection {
    fn render_element(&self, app: &AppContext) -> Box<dyn TuiElement> {
        let theme = Appearance::as_ref(app).theme();
        match self {
            Self::Input(text) => {
                let text_color = Fill::from(theme.foreground()).into();
                let accent = ThemeFill::from(theme.terminal_colors().normal.cyan);
                let background = Fill::from(
                    theme
                        .background()
                        .blend(&accent.with_opacity(10))
                        .blend(&accent.with_opacity(10)),
                )
                .into();
                // Only the first line carries the `≫` prompt marker; continuation
                // lines are indented to the marker's width so they align beneath it.
                let mut column = TuiColumn::new();
                for (index, line) in text.split('\n').enumerate() {
                    let line_text = if index == 0 {
                        format!("{INPUT_PREFIX}{line}")
                    } else {
                        format!("{}{line}", " ".repeat(INPUT_PREFIX.chars().count()))
                    };
                    column = column.child(
                        TuiText::new(line_text).with_style(
                            TuiStyle::default()
                                .fg(text_color)
                                .bg(background)
                                .add_modifier(Modifier::BOLD),
                        ),
                    );
                }
                let content = TuiContainer::new(column).with_background(background);
                TuiContainer::new(content)
                    .with_padding_bottom(BLOCK_BOTTOM_PADDING)
                    .finish()
            }
            Self::PlainText(text) => {
                let text_color =
                    Fill::from(ThemeFill::from(theme.terminal_colors().normal.white)).into();
                TuiContainer::new(
                    TuiText::new(text.clone()).with_style(TuiStyle::default().fg(text_color)),
                )
                .with_padding_bottom(BLOCK_BOTTOM_PADDING)
                .finish()
            }
            Self::ToolCall(_action) => {
                // TODO: add richer rendering for each tool call type. This is just a rendering stub to build off of.
                let text_color =
                    Fill::from(ThemeFill::from(theme.terminal_colors().bright.black)).into();
                TuiContainer::new(
                    TuiText::new("executed a tool call").with_style(
                        TuiStyle::default()
                            .fg(text_color)
                            .add_modifier(Modifier::DIM),
                    ),
                )
                .with_padding_bottom(BLOCK_BOTTOM_PADDING)
                .finish()
            }
            // Thinking sections are rendered by `TuiAIBlock::render_thinking`, which
            // has the collapse state and toggle wiring this stateless method lacks.
            Self::Thinking { .. } => {
                unreachable!("thinking sections are rendered by TuiAIBlock::render_thinking")
            }
        }
    }
}

/// Registers the view with the TUI runtime.
impl Entity for TuiAIBlock {
    type Event = ();
}

/// Renders the model-backed block as a TUI element.
impl TuiView for TuiAIBlock {
    fn ui_name() -> &'static str {
        "TuiAIBlock"
    }

    fn render(&self, app: &AppContext) -> Box<dyn TuiElement> {
        self.render_element(app)
    }
}

#[cfg(test)]
#[path = "agent_block_tests.rs"]
mod tests;
