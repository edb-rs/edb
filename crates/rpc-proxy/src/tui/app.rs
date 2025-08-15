//! TUI application state and logic

use crate::proxy::ProxyServer;
use ratatui::{prelude::*, widgets::*};
use std::{
    collections::VecDeque,
    net::SocketAddr,
    time::{SystemTime, UNIX_EPOCH},
};

/// Maximum number of data points to keep in history
const MAX_HISTORY: usize = 100;

#[derive(Debug, Clone)]
pub struct MetricData {
    pub timestamp: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_size: usize,
    pub healthy_providers: usize,
    pub total_providers: usize,
    pub active_instances: usize,
}

#[derive(Debug, Clone)]
pub struct ProviderStatus {
    pub url: String,
    pub is_healthy: bool,
    pub response_time_ms: Option<u64>,
    pub consecutive_failures: u32,
    pub last_health_check_seconds_ago: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: u64,
    pub max_entries: u64,
    pub utilization: String,
    pub oldest_entry_age_seconds: Option<u64>,
    pub newest_entry_age_seconds: Option<u64>,
    pub cache_file_path: String,
}

pub enum Tab {
    Overview,
    Providers,
    Cache,
    Instances,
}

impl Tab {
    fn title(&self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Providers => "Providers",
            Tab::Cache => "Cache",
            Tab::Instances => "EDB Instances",
        }
    }
}

pub struct App {
    pub proxy: ProxyServer,
    pub addr: SocketAddr,
    pub current_tab: Tab,
    pub show_help: bool,

    // Data
    pub metrics_history: VecDeque<MetricData>,
    pub providers: Vec<ProviderStatus>,
    pub cache_stats: Option<CacheStats>,
    pub active_instances: Vec<u32>,

    // UI state
    pub selected_provider: usize,
    pub scroll_offset: usize,

    // Performance tracking
    pub last_update: u64,
    pub update_count: u64,
}

impl App {
    pub fn new(proxy: ProxyServer, addr: SocketAddr) -> Self {
        Self {
            proxy,
            addr,
            current_tab: Tab::Overview,
            show_help: false,
            metrics_history: VecDeque::with_capacity(MAX_HISTORY),
            providers: Vec::new(),
            cache_stats: None,
            active_instances: Vec::new(),
            selected_provider: 0,
            scroll_offset: 0,
            last_update: 0,
            update_count: 0,
        }
    }

    pub async fn update(&mut self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

        self.last_update = now;
        self.update_count += 1;

        // Update providers
        let providers_info = self.proxy.rpc_handler.provider_manager().get_providers_info().await;
        self.providers = providers_info
            .into_iter()
            .map(|p| ProviderStatus {
                url: p.url,
                is_healthy: p.is_healthy,
                response_time_ms: p.response_time_ms,
                consecutive_failures: p.consecutive_failures,
                last_health_check_seconds_ago: p.last_health_check_seconds_ago,
            })
            .collect();

        // Update cache stats
        let cache_stats_json = self.proxy.cache_manager().detailed_stats().await;
        self.cache_stats = Some(CacheStats {
            total_entries: cache_stats_json["total_entries"].as_u64().unwrap_or(0),
            max_entries: cache_stats_json["max_entries"].as_u64().unwrap_or(0),
            utilization: cache_stats_json["utilization"].as_str().unwrap_or("0%").to_string(),
            oldest_entry_age_seconds: cache_stats_json["oldest_entry_age_seconds"].as_u64(),
            newest_entry_age_seconds: cache_stats_json["newest_entry_age_seconds"].as_u64(),
            cache_file_path: cache_stats_json["cache_file_path"].as_str().unwrap_or("").to_string(),
        });

        // Update active instances
        self.active_instances = self.proxy.registry.get_active_instances().await;

        // Add to metrics history
        let metric = MetricData {
            timestamp: now,
            cache_hits: 0,   // TODO: Add cache hit tracking
            cache_misses: 0, // TODO: Add cache miss tracking
            cache_size: self.cache_stats.as_ref().map(|s| s.total_entries as usize).unwrap_or(0),
            healthy_providers: self.providers.iter().filter(|p| p.is_healthy).count(),
            total_providers: self.providers.len(),
            active_instances: self.active_instances.len(),
        };

        self.metrics_history.push_back(metric);
        if self.metrics_history.len() > MAX_HISTORY {
            self.metrics_history.pop_front();
        }

        // Clamp selected provider
        if !self.providers.is_empty() {
            self.selected_provider = self.selected_provider.min(self.providers.len() - 1);
        }
    }

    pub async fn refresh(&mut self) {
        self.update().await;
    }

    pub async fn clear_cache(&mut self) {
        // Note: This would require adding a clear method to CacheManager
        // For now, we'll just refresh the data
        self.update().await;
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn next_tab(&mut self) {
        self.current_tab = match self.current_tab {
            Tab::Overview => Tab::Providers,
            Tab::Providers => Tab::Cache,
            Tab::Cache => Tab::Instances,
            Tab::Instances => Tab::Overview,
        };
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = match self.current_tab {
            Tab::Overview => Tab::Instances,
            Tab::Providers => Tab::Overview,
            Tab::Cache => Tab::Providers,
            Tab::Instances => Tab::Cache,
        };
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn next_provider(&mut self) {
        if !self.providers.is_empty() {
            self.selected_provider = (self.selected_provider + 1) % self.providers.len();
        }
    }

    pub fn previous_provider(&mut self) {
        if !self.providers.is_empty() {
            self.selected_provider = if self.selected_provider == 0 {
                self.providers.len() - 1
            } else {
                self.selected_provider - 1
            };
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Footer
            ])
            .split(f.area());

        // Render header
        self.render_header(f, chunks[0]);

        // Render content based on current tab
        match self.current_tab {
            Tab::Overview => self.render_overview(f, chunks[1]),
            Tab::Providers => self.render_providers(f, chunks[1]),
            Tab::Cache => self.render_cache(f, chunks[1]),
            Tab::Instances => self.render_instances(f, chunks[1]),
        }

        // Render footer
        self.render_footer(f, chunks[2]);

        // Render help popup if shown
        if self.show_help {
            self.render_help(f);
        }
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let titles = vec![
            Tab::Overview.title(),
            Tab::Providers.title(),
            Tab::Cache.title(),
            Tab::Instances.title(),
        ];

        let selected_tab = match self.current_tab {
            Tab::Overview => 0,
            Tab::Providers => 1,
            Tab::Cache => 2,
            Tab::Instances => 3,
        };

        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL).title("EDB RPC Proxy Monitor"))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .select(selected_tab);

        f.render_widget(tabs, area);
    }

    fn render_footer(&self, f: &mut Frame, area: Rect) {
        let help_text = if self.show_help {
            "Press 'h' to hide help"
        } else {
            "q:Quit | h:Help | r:Refresh | c:Clear Cache | Tab:Switch | ←→:Navigate"
        };

        let status_text = format!(
            "Listening: {} | Updates: {} | Last: {}s ago",
            self.addr,
            self.update_count,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .saturating_sub(self.last_update)
        );

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);

        let help = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray));

        let status = Paragraph::new(status_text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Right);

        f.render_widget(help, chunks[0]);
        f.render_widget(status, chunks[1]);
    }

    fn render_help(&self, f: &mut Frame) {
        let area = centered_rect(60, 50, f.area());

        let help_text = vec![
            Line::from(vec![Span::styled(
                "Keyboard Shortcuts",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("q, Esc", Style::default().fg(Color::Yellow)),
                Span::raw("    Quit application"),
            ]),
            Line::from(vec![
                Span::styled("h", Style::default().fg(Color::Yellow)),
                Span::raw("        Toggle this help"),
            ]),
            Line::from(vec![
                Span::styled("r", Style::default().fg(Color::Yellow)),
                Span::raw("        Refresh data"),
            ]),
            Line::from(vec![
                Span::styled("c", Style::default().fg(Color::Yellow)),
                Span::raw("        Clear cache"),
            ]),
            Line::from(vec![
                Span::styled("Tab", Style::default().fg(Color::Yellow)),
                Span::raw("       Next tab"),
            ]),
            Line::from(vec![
                Span::styled("Shift+Tab", Style::default().fg(Color::Yellow)),
                Span::raw("  Previous tab"),
            ]),
            Line::from(vec![
                Span::styled("↑↓", Style::default().fg(Color::Yellow)),
                Span::raw("        Scroll content"),
            ]),
            Line::from(vec![
                Span::styled("←→", Style::default().fg(Color::Yellow)),
                Span::raw("        Navigate providers"),
            ]),
        ];

        let help_block = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title("Help")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true });

        f.render_widget(Clear, area);
        f.render_widget(help_block, area);
    }
}

/// Helper function to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
