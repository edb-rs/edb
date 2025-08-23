// EDB - Ethereum Debugger
// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.


//! TUI application state and logic

use super::remote::{
    RemoteCacheStats, RemoteDataFetcher, RemoteMetricData, RemoteProviderStatus, RemoteProxyClient,
};
use ratatui::{prelude::*, widgets::*};
use std::{
    collections::VecDeque,
    time::{SystemTime, UNIX_EPOCH},
};

/// Maximum number of data points to keep in history
const MAX_HISTORY: usize = 100;

// Use remote types directly
pub type MetricData = RemoteMetricData;
pub type ProviderStatus = RemoteProviderStatus;
pub type CacheStats = RemoteCacheStats;

pub enum Tab {
    Overview,
    Providers,
    Cache,
    Methods,
    Instances,
}

impl Tab {
    fn title(&self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Providers => "Providers",
            Tab::Cache => "Cache",
            Tab::Methods => "Methods",
            Tab::Instances => "EDB Instances",
        }
    }
}

pub struct App {
    // Remote data fetcher
    pub fetcher: RemoteDataFetcher,
    pub refresh_interval: u64,
    pub proxy_url: String,

    // UI state
    pub current_tab: Tab,
    pub show_help: bool,

    // Data
    pub metrics_history: VecDeque<MetricData>,
    pub providers: Vec<ProviderStatus>,
    pub cache_stats: Option<CacheStats>,
    pub active_instances: Vec<u32>,

    // Enhanced metrics data (JSON values from API)
    pub cache_metrics: Option<serde_json::Value>,
    pub provider_metrics: Option<serde_json::Value>,
    pub request_metrics: Option<serde_json::Value>,
    pub system_info: Option<serde_json::Value>,

    // UI state
    pub selected_provider: usize,

    // Separate scroll offsets for different tables/sections
    pub provider_analytics_scroll: usize,
    pub _provider_list_scroll: usize,
    pub cache_details_scroll: usize,
    pub methods_scroll: usize,
    pub instances_scroll: usize,

    // Performance tracking
    pub last_update: u64,
    pub update_count: u64,
}

impl App {
    /// Create a new app for remote proxy monitoring
    pub fn new_remote(client: RemoteProxyClient, refresh_interval: u64, proxy_url: String) -> Self {
        let fetcher = RemoteDataFetcher::new(client);
        Self {
            fetcher,
            refresh_interval,
            proxy_url,
            current_tab: Tab::Overview,
            show_help: false,
            metrics_history: VecDeque::with_capacity(MAX_HISTORY),
            providers: Vec::new(),
            cache_stats: None,
            active_instances: Vec::new(),
            cache_metrics: None,
            provider_metrics: None,
            request_metrics: None,
            system_info: None,
            selected_provider: 0,
            provider_analytics_scroll: 0,
            _provider_list_scroll: 0,
            cache_details_scroll: 0,
            methods_scroll: 0,
            instances_scroll: 0,
            last_update: 0,
            update_count: 0,
        }
    }

    pub async fn update(&mut self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

        self.last_update = now;
        self.update_count += 1;

        // Fetch all data from remote proxy
        match self.fetcher.fetch_all_data().await {
            Ok(data) => {
                // Update providers
                self.providers = data.providers;

                // Update cache stats
                self.cache_stats = data.cache_stats;

                // Update active instances
                self.active_instances = data.active_instances;

                // Update enhanced metrics
                self.cache_metrics = data.cache_metrics;
                self.provider_metrics = data.provider_metrics;
                self.request_metrics = data.request_metrics;
                self.system_info = data.system_info;

                // Update metrics history from remote data
                if !data.metrics_history.is_empty() {
                    self.metrics_history = data.metrics_history.into();
                } else {
                    // If no remote history, create a current data point from enhanced metrics
                    let cache_hits = self
                        .cache_metrics
                        .as_ref()
                        .and_then(|m| m.get("cache_hits").and_then(|v| v.as_u64()))
                        .unwrap_or(0);
                    let cache_misses = self
                        .cache_metrics
                        .as_ref()
                        .and_then(|m| m.get("cache_misses").and_then(|v| v.as_u64()))
                        .unwrap_or(0);

                    // Get hit_rate from backend - it comes as a string like "75.5%"
                    let hit_rate = self
                        .cache_metrics
                        .as_ref()
                        .and_then(|m| m.get("hit_rate").and_then(|v| v.as_str()))
                        .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())
                        .unwrap_or(0.0);

                    let requests_per_minute = self
                        .request_metrics
                        .as_ref()
                        .and_then(|m| m.get("requests_per_minute").and_then(|v| v.as_u64()))
                        .unwrap_or(0);
                    let avg_response_time = self
                        .providers
                        .iter()
                        .filter_map(|p| p.response_time_ms)
                        .map(|ms| ms as f64)
                        .sum::<f64>()
                        / self.providers.len().max(1) as f64;

                    let metric = MetricData {
                        timestamp: now,
                        _cache_hits: cache_hits,
                        _cache_misses: cache_misses,
                        cache_size: self.cache_stats.as_ref().map(|s| s.total_entries).unwrap_or(0),
                        hit_rate,
                        _healthy_providers: self.providers.iter().filter(|p| p.is_healthy).count()
                            as u64,
                        _total_providers: self.providers.len() as u64,
                        requests_per_minute,
                        avg_response_time_ms: avg_response_time,
                        _active_instances: self.active_instances.len(),
                    };

                    self.metrics_history.push_back(metric);
                    if self.metrics_history.len() > MAX_HISTORY {
                        self.metrics_history.pop_front();
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to fetch remote data: {}", e);
                // Continue with stale data
            }
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
            Tab::Cache => Tab::Methods,
            Tab::Methods => Tab::Instances,
            Tab::Instances => Tab::Overview,
        };
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = match self.current_tab {
            Tab::Overview => Tab::Instances,
            Tab::Providers => Tab::Overview,
            Tab::Cache => Tab::Providers,
            Tab::Methods => Tab::Cache,
            Tab::Instances => Tab::Methods,
        };
    }

    pub fn scroll_up(&mut self) {
        match self.current_tab {
            Tab::Overview => {
                // Overview doesn't have scrollable content currently
            }
            Tab::Providers => {
                // For now, scroll the provider analytics table
                // Could be enhanced to support focus switching between analytics/list
                self.provider_analytics_scroll = self.provider_analytics_scroll.saturating_sub(1);
            }
            Tab::Cache => {
                self.cache_details_scroll = self.cache_details_scroll.saturating_sub(1);
            }
            Tab::Methods => {
                self.methods_scroll = self.methods_scroll.saturating_sub(1);
            }
            Tab::Instances => {
                self.instances_scroll = self.instances_scroll.saturating_sub(1);
            }
        }
    }

    pub fn scroll_down(&mut self) {
        match self.current_tab {
            Tab::Overview => {
                // Overview doesn't have scrollable content currently
            }
            Tab::Providers => {
                self.provider_analytics_scroll =
                    self.provider_analytics_scroll.saturating_add(1) % (self.providers.len() + 1);
            }
            Tab::Cache => {
                self.cache_details_scroll = self.cache_details_scroll.saturating_add(1);
            }
            Tab::Methods => {
                self.methods_scroll = self.methods_scroll.saturating_add(1);
            }
            Tab::Instances => {
                self.instances_scroll = self.instances_scroll.saturating_add(1);
            }
        }
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

    pub fn render(&mut self, f: &mut Frame<'_>) {
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
            Tab::Methods => self.render_methods(f, chunks[1]),
            Tab::Instances => self.render_instances(f, chunks[1]),
        }

        // Render footer
        self.render_footer(f, chunks[2]);

        // Render help popup if shown
        if self.show_help {
            self.render_help(f);
        }
    }

    fn render_header(&self, f: &mut Frame<'_>, area: Rect) {
        let titles = vec![
            Tab::Overview.title(),
            Tab::Providers.title(),
            Tab::Cache.title(),
            Tab::Methods.title(),
            Tab::Instances.title(),
        ];

        let selected_tab = match self.current_tab {
            Tab::Overview => 0,
            Tab::Providers => 1,
            Tab::Cache => 2,
            Tab::Methods => 3,
            Tab::Instances => 4,
        };

        let title = format!("EDB RPC Proxy Monitor - {}", self.proxy_url);
        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL).title(title))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .select(selected_tab);

        f.render_widget(tabs, area);
    }

    fn render_footer(&self, f: &mut Frame<'_>, area: Rect) {
        let help_text = if self.show_help {
            "Press 'h' to hide help"
        } else {
            "q:Quit | h:Help | r:Refresh | c:Clear Cache | Tab:Switch | ←→:Navigate | ↑↓:Scroll"
        };

        // Extract system info
        let system_text = if let Some(info) = &self.system_info {
            let uptime = info
                .get("uptime")
                .and_then(|v| v.as_u64())
                .map(|seconds| {
                    let days = seconds / 86400;
                    let hours = (seconds % 86400) / 3600;
                    let minutes = (seconds % 3600) / 60;
                    if days > 0 {
                        format!("{}d {}h {}m", days, hours, minutes)
                    } else if hours > 0 {
                        format!("{}h {}m", hours, minutes)
                    } else {
                        format!("{}m", minutes)
                    }
                })
                .unwrap_or_else(|| "Unknown".to_string());

            let pid = info
                .get("pid")
                .and_then(|v| v.as_u64())
                .map(|p| p.to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            format!("Proxy Server - PID: {} | Up: {}", pid, uptime)
        } else {
            "System info unavailable".to_string()
        };

        let status_text = format!(
            "TUI - Updates: {} | Last: {}s ago",
            self.update_count,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .saturating_sub(self.last_update)
        );

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(area);

        let help = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray));

        let system = Paragraph::new(system_text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center);

        let status = Paragraph::new(status_text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Right);

        f.render_widget(help, chunks[0]);
        f.render_widget(system, chunks[1]);
        f.render_widget(status, chunks[2]);
    }

    fn render_help(&self, f: &mut Frame<'_>) {
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
