//! Trace panel for displaying execution trace
//!
//! This panel shows the call trace and allows navigation through trace entries.

use super::{EventResponse, PanelTr, PanelType};
use crate::managers::{ExecutionManager, ResourceManager, ThemeManager};
use crate::ui::borders::BorderPresets;
use crate::ui::status::StatusBar;
use crate::ColorScheme;
use alloy_dyn_abi::{DynSolValue, JsonAbiExt};
use alloy_json_abi::Function;
use alloy_primitives::{hex, Address, Bytes, U256};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use edb_common::types::{CallResult, CallType, Trace, TraceEntry};
use eyre::Result;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
    Frame,
};
use revm::{context::CreateScheme, interpreter::CallScheme};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tokio::runtime::Handle;
use tracing::{debug, warn};

/// Trace panel implementation
#[derive(Debug)]
pub struct TracePanel {
    // ========== Display ==========
    /// Currently selected trace entry index
    selected_index: usize,
    /// Scroll offset
    scroll_offset: usize,
    /// Current content height
    context_height: usize,
    /// Whether this panel is focused
    focused: bool,

    // ========== Data ==========
    /// Trace
    trace_data: Option<Trace>,
    /// Color Scheme:
    color_scheme: ColorScheme,

    // ========== Managers ==========
    /// Shared execution state manager
    execution_manager: Arc<RwLock<ExecutionManager>>,
    /// Shared resource manager
    resource_manager: Arc<RwLock<ResourceManager>>,
    /// Theme manager for styling
    theme_manager: Arc<RwLock<ThemeManager>>,
}

impl TracePanel {
    /// Create a new trace panel
    pub fn new(
        execution_manager: Arc<RwLock<ExecutionManager>>,
        resource_manager: Arc<RwLock<ResourceManager>>,
        theme_manager: Arc<RwLock<ThemeManager>>,
    ) -> Self {
        Self {
            selected_index: 0,
            focused: false,
            scroll_offset: 0,
            context_height: 0,
            trace_data: None,
            color_scheme: ColorScheme::default(),
            execution_manager,
            resource_manager,
            theme_manager,
        }
    }

    /// Move selection up
    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    /// Move selection down
    fn move_down(&mut self) {
        let max_lines = if let Some(trace) = &self.trace_data { trace.len() } else { 0 };

        if self.selected_index < max_lines.saturating_sub(1) {
            self.selected_index += 1;
            let viewport_height = self.context_height;
            if self.selected_index >= self.scroll_offset + viewport_height {
                self.scroll_offset = (self.selected_index + 1).saturating_sub(viewport_height);
            }
        }
    }

    /// Get currently selected trace entry
    pub fn selected_entry(&self) -> Option<TraceEntry> {
        if let Some(trace) = &self.trace_data {
            trace.get(self.selected_index).cloned()
        } else {
            None
        }
    }

    /// Update trace data from execution manager (simplified for now)
    fn update_trace_data(&mut self) {
        // For now, we'll implement a simple polling approach
        // In a real implementation, this would need proper async handling
        // or message passing between async tasks and the UI thread
    }

    /// Format a trace entry into a display string with smart labeling and decoding
    fn format_trace_entry(&self, entry: &TraceEntry, depth: usize) -> Line<'_> {
        let indent = "  ".repeat(depth);

        // Determine call type and color
        let (call_type_str, call_color) = match &entry.call_type {
            CallType::Call(CallScheme::Call) => ("CALL", Color::Blue),
            CallType::Call(CallScheme::CallCode) => ("CALLCODE", Color::LightBlue),
            CallType::Call(CallScheme::DelegateCall) => ("DELEGATECALL", Color::Cyan),
            CallType::Call(CallScheme::StaticCall) => ("STATICCALL", Color::Magenta),
            CallType::Create(CreateScheme::Create) => ("CREATE", Color::Yellow),
            CallType::Create(CreateScheme::Create2 { .. }) => ("CREATE2", Color::LightYellow),
            CallType::Create(CreateScheme::Custom { .. }) => ("CREATE_CUSTOM", Color::DarkGray),
        };

        // Format caller address (shortened)
        let caller_str = self.format_address_display(entry.caller, None);

        // Format target address with label if available
        let target_str = self.format_address_display(entry.target, entry.target_label.as_deref());

        // Format function call if we have ABI
        let call_detail = if let Some(function_abi) = &entry.function_abi {
            self.format_function_call(function_abi, &entry.input)
        } else if !entry.input.is_empty() {
            format!("data: 0x{}...", hex::encode(&entry.input[..entry.input.len().min(4)]))
        } else {
            String::new()
        };

        // Format value if present
        let value_str = if entry.value > U256::ZERO {
            format!(" value: {} ETH", self.format_ether(entry.value))
        } else {
            String::new()
        };

        // Result indicator
        let result_char = match &entry.result {
            Some(CallResult::Success { .. }) => "✓",
            Some(CallResult::Revert { .. }) => "✗",
            None => " ",
        };

        let result_color = match &entry.result {
            Some(CallResult::Success { .. }) => Color::Green,
            Some(CallResult::Revert { .. }) => Color::Red,
            None => Color::Gray,
        };

        // Build the line with spans
        let mut spans = vec![
            Span::raw(indent),
            Span::styled(format!("{:<12}", call_type_str), Style::default().fg(call_color)),
            Span::raw(" "),
            Span::styled(caller_str, Style::default().fg(Color::White)),
            Span::raw(" → "),
            Span::styled(target_str, Style::default().fg(Color::LightGreen)),
        ];

        if !call_detail.is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(call_detail, Style::default().fg(Color::Cyan)));
        }

        if !value_str.is_empty() {
            spans.push(Span::styled(value_str, Style::default().fg(Color::Yellow)));
        }

        spans.push(Span::raw(" "));
        spans.push(Span::styled(result_char, Style::default().fg(result_color)));

        Line::from(spans)
    }

    /// Format an address for display, using label if available
    fn format_address_display(&self, address: Address, label: Option<&str>) -> String {
        if let Some(label) = label {
            format!("{} ({})", label, self.format_address_short(address))
        } else {
            self.format_address_short(address)
        }
    }

    /// Format address to short form
    fn format_address_short(&self, address: Address) -> String {
        if address == Address::ZERO {
            "0x0".to_string()
        } else {
            let addr_str = format!("{:?}", address);
            if addr_str.len() > 10 {
                format!("{}...{}", &addr_str[..6], &addr_str[addr_str.len() - 4..])
            } else {
                addr_str
            }
        }
    }

    /// Format function call with ABI decoding
    fn format_function_call(&self, function_abi: &Function, input_data: &Bytes) -> String {
        if input_data.len() < 4 {
            return format!("{}()", function_abi.name);
        }

        // Try to decode the input data
        match function_abi.abi_decode_input(&input_data[4..]) {
            Ok(decoded) => {
                let params: Vec<String> =
                    decoded.iter().map(|param| self.format_solidity_value(param)).collect();

                format!("{}({})", function_abi.name, params.join(", "))
            }
            Err(_) => {
                // Fallback to raw data display
                format!(
                    "{}(0x{}...)",
                    function_abi.name,
                    hex::encode(&input_data[4..input_data.len().min(8)])
                )
            }
        }
    }

    /// Format a Solidity value for display
    fn format_solidity_value(&self, value: &DynSolValue) -> String {
        match value {
            DynSolValue::Address(addr) => format!("0x{:x}", addr),
            DynSolValue::Uint(n, _) => n.to_string(),
            DynSolValue::Int(n, _) => n.to_string(),
            DynSolValue::Bool(b) => b.to_string(),
            DynSolValue::Bytes(b) => format!("0x{}", hex::encode(&b[..b.len().min(8)])),
            DynSolValue::FixedBytes(b, _) => format!("0x{}", hex::encode(b)),
            DynSolValue::String(s) => format!("\"{}\"", s),
            DynSolValue::Array(arr) => {
                if arr.len() <= 3 {
                    let items: Vec<String> =
                        arr.iter().map(|v| self.format_solidity_value(v)).collect();
                    format!("[{}]", items.join(", "))
                } else {
                    format!("[...{} items]", arr.len())
                }
            }
            DynSolValue::FixedArray(arr) => {
                if arr.len() <= 3 {
                    let items: Vec<String> =
                        arr.iter().map(|v| self.format_solidity_value(v)).collect();
                    format!("[{}]", items.join(", "))
                } else {
                    format!("[...{} items]", arr.len())
                }
            }
            DynSolValue::Tuple(tuple) => {
                if tuple.len() <= 2 {
                    let items: Vec<String> =
                        tuple.iter().map(|v| self.format_solidity_value(v)).collect();
                    format!("({})", items.join(", "))
                } else {
                    format!("(...{} fields)", tuple.len())
                }
            }
            DynSolValue::Function(_) => "<function>".to_string(),
        }
    }

    /// Format Wei value to ETH
    fn format_ether(&self, value: U256) -> String {
        // Convert Wei to ETH (1 ETH = 10^18 Wei)
        let eth_value = value.to_string();
        if eth_value.len() <= 18 {
            // Less than 1 ETH - show significant digits only
            let padded = format!("{:0>18}", eth_value);
            let trimmed = padded.trim_end_matches('0');
            if trimmed.is_empty() {
                "0".to_string()
            } else {
                format!("0.{}", &trimmed[..trimmed.len().min(6)])
            }
        } else {
            // More than 1 ETH
            let (whole, decimal) = eth_value.split_at(eth_value.len() - 18);
            let decimal_trimmed = decimal[..4.min(decimal.len())].trim_end_matches('0');
            if decimal_trimmed.is_empty() {
                whole.to_string()
            } else {
                format!("{}.{}", whole, decimal_trimmed)
            }
        }
    }
}

impl PanelTr for TracePanel {
    fn panel_type(&self) -> PanelType {
        PanelType::Trace
    }

    fn title(&self) -> String {
        if let Some(trace) = &self.trace_data {
            format!("Trace ({} entries)", trace.len())
        } else {
            "Trace (Loading...)".to_string()
        }
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        // Get theme manager colors, using defaults if not available
        let (focused_color, unfocused_color, accent_color, selected_bg, selected_fg, highlight_bg) =
                (
                    self.color_scheme.focused_border,
                    self.color_scheme.unfocused_border,
                    self.color_scheme.accent_color,
                    self.color_scheme.selection_bg,
                    self.color_scheme.selection_fg,
                    self.color_scheme.highlight_bg,
                );

        self.context_height = if self.focused && area.height > 10 {
            area.height.saturating_sub(4) // Account for borders and status lines
        } else {
            area.height.saturating_sub(2) // Just borders
        } as usize;

        // Handle different display states
        match self.trace_data {
            // No data: show spinner
            None => {
                let paragraph = Paragraph::new(Line::from(vec![
                    Span::raw("Fetching execution trace "),
                    Span::styled("⠋", Style::default().fg(self.color_scheme.accent_color)),
                ]))
                .block(BorderPresets::trace(
                    self.focused,
                    self.title(),
                    self.color_scheme.focused_border,
                    self.color_scheme.unfocused_border,
                ));
                frame.render_widget(paragraph, area);
                return;
            }
            // Data available
            Some(ref trace) => {
                if trace.is_empty() {
                    let paragraph = Paragraph::new("Trace is empty").block(BorderPresets::trace(
                        self.focused,
                        self.title(),
                        self.color_scheme.focused_border,
                        self.color_scheme.unfocused_border,
                    ));
                    frame.render_widget(paragraph, area);
                    return;
                }

                // Create list items with smart formatting
                let items: Vec<ListItem<'_>> = trace
                    .iter()
                    .enumerate()
                    .skip(self.scroll_offset)
                    .take(self.context_height)
                    .map(|(i, entry)| {
                        let formatted_line = self.format_trace_entry(entry, entry.depth);

                        let style = if i == self.selected_index && self.focused {
                            Style::default()
                                .bg(self.color_scheme.selection_bg)
                                .fg(self.color_scheme.selection_fg)
                        } else if i == self.selected_index {
                            Style::default().bg(self.color_scheme.highlight_bg)
                        } else {
                            Style::default()
                        };

                        ListItem::new(formatted_line).style(style)
                    })
                    .collect();

                let list = List::new(items)
                    .block(BorderPresets::trace(
                        self.focused,
                        self.title(),
                        self.color_scheme.focused_border,
                        self.color_scheme.unfocused_border,
                    ))
                    .highlight_style(Style::default().bg(self.color_scheme.selection_bg));

                frame.render_widget(list, area);

                // Add status and help text at the bottom if focused
                if self.focused && area.height > 10 {
                    // Status line
                    let status_area = Rect {
                        x: area.x + 1,
                        y: area.y + area.height - 3,
                        width: area.width - 2,
                        height: 1,
                    };

                    let status_bar =
                        StatusBar::new().current_panel("Trace".to_string()).message(format!(
                            "Entry: {}/{} | Depth: {}",
                            self.selected_index + 1,
                            trace.len(),
                            if let Some(entry) = trace.get(self.selected_index) {
                                entry.depth
                            } else {
                                0
                            }
                        ));

                    let status_text = status_bar.build();
                    let status_paragraph = Paragraph::new(status_text)
                        .style(Style::default().fg(self.color_scheme.accent_color));
                    frame.render_widget(status_paragraph, status_area);

                    let help_area = Rect {
                        x: area.x + 1,
                        y: area.y + area.height - 2,
                        width: area.width - 2,
                        height: 1,
                    };
                    let help_text =
                        "↑/↓: Navigate • Enter: Jump to snapshot • r: Refresh • Tab: Next panel";
                    let help_paragraph = Paragraph::new(help_text)
                        .style(Style::default().fg(self.color_scheme.help_text_color));
                    frame.render_widget(help_paragraph, help_area);
                }
            }
        }
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> Result<EventResponse> {
        if !self.focused || event.kind != KeyEventKind::Press {
            return Ok(EventResponse::NotHandled);
        }

        match event.code {
            KeyCode::Up => {
                self.move_up();
                Ok(EventResponse::Handled)
            }
            KeyCode::Down => {
                self.move_down();
                Ok(EventResponse::Handled)
            }
            KeyCode::Enter => {
                if let Some(entry) = self.selected_entry() {
                    debug!("Selected trace entry ID: {} at depth: {}", entry.id, entry.depth);

                    // Jump to the first snapshot of this trace entry if available
                    if let Some(snapshot_id) = entry.first_snapshot_id {
                        debug!("Jumping to snapshot: {}", snapshot_id);
                        // TODO: Use execution manager to set current snapshot
                        // This would require an async RPC call to set_current_snapshot
                    }
                }
                Ok(EventResponse::Handled)
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                // Refresh trace data
                debug!("Refreshing trace data");
                self.trace_data = None;

                Ok(EventResponse::Handled)
            }
            _ => Ok(EventResponse::NotHandled),
        }
    }

    fn on_focus(&mut self) {
        self.focused = true;
        debug!("Trace panel gained focus");
    }

    fn on_blur(&mut self) {
        self.focused = false;
        debug!("Trace panel lost focus");
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// Get execution manager read-only reference
    fn exec_mgr(&self) -> RwLockReadGuard<'_, ExecutionManager> {
        self.execution_manager.read().expect("ExecutionManager lock poisoned")
    }

    /// Get execution manager reference
    fn exec_mgr_mut(&self) -> RwLockWriteGuard<'_, ExecutionManager> {
        self.execution_manager.write().expect("ExecutionManager lock poisoned")
    }

    /// Get resource manager read-only reference
    fn res_mgr(&self) -> RwLockReadGuard<'_, ResourceManager> {
        self.resource_manager.read().expect("ResourceManager lock poisoned")
    }

    /// Get resource manager reference
    fn res_mgr_mut(&self) -> RwLockWriteGuard<'_, ResourceManager> {
        self.resource_manager.write().expect("ResourceManager lock poisoned")
    }

    /// Get theme manager reference
    fn theme_mgr(&self) -> RwLockReadGuard<'_, ThemeManager> {
        self.theme_manager.read().expect("ThemeManager lock poisoned")
    }

    /// Get theme manager reference
    fn theme_mgr_mut(&self) -> RwLockWriteGuard<'_, ThemeManager> {
        self.theme_manager.write().expect("ThemeManager lock poisoned")
    }

    async fn fetch_data(&mut self) -> Result<()> {
        self.res_mgr_mut().fetch_trace().await?;
        let trace_data = self.res_mgr().get_trace().await.cloned();
        self.trace_data = trace_data;

        self.color_scheme = self.theme_mgr().get_current_colors();
        Ok(())
    }
}
