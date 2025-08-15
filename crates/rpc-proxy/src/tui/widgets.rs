//! Custom widgets for the TUI interface

use super::app::App;
use ratatui::{prelude::*, widgets::*};

impl App {
    pub fn render_overview(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7), // Status cards
                Constraint::Min(8),    // Charts
            ])
            .split(area);

        // Render status cards
        self.render_status_cards(f, chunks[0]);

        // Render charts
        self.render_charts(f, chunks[1]);
    }

    fn render_status_cards(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(area);

        // Providers status card
        let healthy_providers = self.providers.iter().filter(|p| p.is_healthy).count();
        let total_providers = self.providers.len();
        let provider_status = if healthy_providers == total_providers && total_providers > 0 {
            ("🟢 Healthy", Color::Green)
        } else if healthy_providers > 0 {
            ("🟡 Degraded", Color::Yellow)
        } else {
            ("🔴 Down", Color::Red)
        };

        let provider_card = Paragraph::new(vec![
            Line::from(vec![Span::styled(
                "Providers",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                provider_status.0,
                Style::default().fg(provider_status.1),
            )]),
            Line::from(format!("{}/{} Online", healthy_providers, total_providers)),
        ])
        .block(Block::default().borders(Borders::ALL).title("RPC Providers"))
        .alignment(Alignment::Center);

        f.render_widget(provider_card, chunks[0]);

        // Cache status card
        let cache_stats = self.cache_stats.as_ref();
        let cache_utilization =
            cache_stats.map(|s| s.utilization.clone()).unwrap_or_else(|| "0%".to_string());
        let cache_entries = cache_stats.map(|s| s.total_entries).unwrap_or(0);

        let cache_color =
            if cache_utilization.trim_end_matches('%').parse::<f32>().unwrap_or(0.0) > 90.0 {
                Color::Red
            } else if cache_utilization.trim_end_matches('%').parse::<f32>().unwrap_or(0.0) > 75.0 {
                Color::Yellow
            } else {
                Color::Green
            };

        let cache_card = Paragraph::new(vec![
            Line::from(vec![Span::styled("Cache", Style::default().add_modifier(Modifier::BOLD))]),
            Line::from(""),
            Line::from(vec![Span::styled(&cache_utilization, Style::default().fg(cache_color))]),
            Line::from(format!("{} entries", cache_entries)),
        ])
        .block(Block::default().borders(Borders::ALL).title("Cache Status"))
        .alignment(Alignment::Center);

        f.render_widget(cache_card, chunks[1]);

        // EDB instances card
        let instance_count = self.active_instances.len();
        let instance_status = if instance_count > 0 {
            ("🟢 Active", Color::Green)
        } else {
            ("⚪ Idle", Color::Gray)
        };

        let instance_card = Paragraph::new(vec![
            Line::from(vec![Span::styled(
                "EDB Instances",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                instance_status.0,
                Style::default().fg(instance_status.1),
            )]),
            Line::from(format!("{} connected", instance_count)),
        ])
        .block(Block::default().borders(Borders::ALL).title("Debugging Sessions"))
        .alignment(Alignment::Center);

        f.render_widget(instance_card, chunks[2]);

        // Performance card
        let avg_response_time =
            self.providers.iter().filter_map(|p| p.response_time_ms).sum::<u64>() as f64
                / self.providers.iter().filter(|p| p.response_time_ms.is_some()).count().max(1)
                    as f64;

        let perf_color = if avg_response_time > 2000.0 {
            Color::Red
        } else if avg_response_time > 1000.0 {
            Color::Yellow
        } else {
            Color::Green
        };

        let perf_card = Paragraph::new(vec![
            Line::from(vec![Span::styled(
                "Performance",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                format!("{:.0}ms", avg_response_time),
                Style::default().fg(perf_color),
            )]),
            Line::from("Avg Response"),
        ])
        .block(Block::default().borders(Borders::ALL).title("Response Time"))
        .alignment(Alignment::Center);

        f.render_widget(perf_card, chunks[3]);
    }

    fn render_charts(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Cache size chart
        self.render_cache_chart(f, chunks[0]);

        // Provider health chart
        self.render_provider_chart(f, chunks[1]);
    }

    fn render_cache_chart(&self, f: &mut Frame, area: Rect) {
        if self.metrics_history.is_empty() {
            let empty = Paragraph::new("No data available")
                .block(Block::default().borders(Borders::ALL).title("Cache Size"))
                .alignment(Alignment::Center);
            f.render_widget(empty, area);
            return;
        }

        let data: Vec<(f64, f64)> = self
            .metrics_history
            .iter()
            .enumerate()
            .map(|(i, metric)| (i as f64, metric.cache_size as f64))
            .collect();

        let max_size = data.iter().map(|(_, y)| *y).fold(0.0, f64::max).max(1.0);

        let datasets = vec![Dataset::default()
            .name("Cache Entries")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .data(&data)];

        let chart = Chart::new(datasets)
            .block(Block::default().borders(Borders::ALL).title("Cache Size Over Time"))
            .x_axis(
                Axis::default()
                    .title("Time")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, self.metrics_history.len().max(1) as f64]),
            )
            .y_axis(
                Axis::default()
                    .title("Entries")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, max_size]),
            );

        f.render_widget(chart, area);
    }

    fn render_provider_chart(&self, f: &mut Frame, area: Rect) {
        if self.metrics_history.is_empty() {
            let empty = Paragraph::new("No data available")
                .block(Block::default().borders(Borders::ALL).title("Provider Health"))
                .alignment(Alignment::Center);
            f.render_widget(empty, area);
            return;
        }

        let healthy_data: Vec<(f64, f64)> = self
            .metrics_history
            .iter()
            .enumerate()
            .map(|(i, metric)| (i as f64, metric.healthy_providers as f64))
            .collect();

        let total_data: Vec<(f64, f64)> = self
            .metrics_history
            .iter()
            .enumerate()
            .map(|(i, metric)| (i as f64, metric.total_providers as f64))
            .collect();

        let max_providers = total_data.iter().map(|(_, y)| *y).fold(0.0, f64::max).max(1.0);

        let datasets = vec![
            Dataset::default()
                .name("Total")
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(Color::Gray))
                .data(&total_data),
            Dataset::default()
                .name("Healthy")
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(Color::Green))
                .data(&healthy_data),
        ];

        let chart = Chart::new(datasets)
            .block(Block::default().borders(Borders::ALL).title("Provider Health Over Time"))
            .x_axis(
                Axis::default()
                    .title("Time")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, self.metrics_history.len().max(1) as f64]),
            )
            .y_axis(
                Axis::default()
                    .title("Providers")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, max_providers]),
            );

        f.render_widget(chart, area);
    }

    pub fn render_providers(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        // Provider list
        self.render_provider_list(f, chunks[0]);

        // Selected provider details
        self.render_provider_details(f, chunks[1]);
    }

    fn render_provider_list(&self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .providers
            .iter()
            .enumerate()
            .map(|(_i, provider)| {
                let status_icon = if provider.is_healthy { "🟢" } else { "🔴" };
                let response_time = provider
                    .response_time_ms
                    .map(|ms| format!("{}ms", ms))
                    .unwrap_or_else(|| "N/A".to_string());

                let line = Line::from(vec![
                    Span::raw(format!("{} ", status_icon)),
                    Span::styled(
                        format!("{}", provider.url.chars().take(40).collect::<String>()),
                        if provider.is_healthy {
                            Style::default().fg(Color::Green)
                        } else {
                            Style::default().fg(Color::Red)
                        },
                    ),
                    Span::styled(format!(" ({})", response_time), Style::default().fg(Color::Gray)),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("RPC Providers"))
            .highlight_style(Style::default().bg(Color::DarkGray))
            .highlight_symbol("► ");

        let mut state = ListState::default();
        state.select(Some(self.selected_provider));

        f.render_stateful_widget(list, area, &mut state);
    }

    fn render_provider_details(&self, f: &mut Frame, area: Rect) {
        if let Some(provider) = self.providers.get(self.selected_provider) {
            let details = vec![
                Line::from(vec![
                    Span::styled("URL: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(&provider.url),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(
                        if provider.is_healthy { "Healthy" } else { "Unhealthy" },
                        if provider.is_healthy {
                            Style::default().fg(Color::Green)
                        } else {
                            Style::default().fg(Color::Red)
                        },
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Response Time: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(
                        provider
                            .response_time_ms
                            .map(|ms| format!("{}ms", ms))
                            .unwrap_or_else(|| "N/A".to_string()),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Failures: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!("{}", provider.consecutive_failures)),
                ]),
                Line::from(vec![
                    Span::styled("Last Check: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(
                        provider
                            .last_health_check_seconds_ago
                            .map(|s| format!("{}s ago", s))
                            .unwrap_or_else(|| "Never".to_string()),
                    ),
                ]),
            ];

            let details_widget = Paragraph::new(details)
                .block(Block::default().borders(Borders::ALL).title("Provider Details"))
                .wrap(Wrap { trim: true });

            f.render_widget(details_widget, area);
        } else {
            let empty = Paragraph::new("No provider selected")
                .block(Block::default().borders(Borders::ALL).title("Provider Details"))
                .alignment(Alignment::Center);
            f.render_widget(empty, area);
        }
    }

    pub fn render_cache(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // Cache stats
                Constraint::Min(0),    // Cache details
            ])
            .split(area);

        // Cache statistics
        self.render_cache_stats(f, chunks[0]);

        // Cache utilization details
        self.render_cache_details(f, chunks[1]);
    }

    fn render_cache_stats(&self, f: &mut Frame, area: Rect) {
        if let Some(stats) = &self.cache_stats {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                ])
                .split(area);

            // Total entries
            let entries_widget = Paragraph::new(vec![
                Line::from(vec![Span::styled(
                    "Total Entries",
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from(format!("{}", stats.total_entries)),
                Line::from(format!("Max: {}", stats.max_entries)),
            ])
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);

            // Utilization
            let util_color =
                if stats.utilization.trim_end_matches('%').parse::<f32>().unwrap_or(0.0) > 90.0 {
                    Color::Red
                } else if stats.utilization.trim_end_matches('%').parse::<f32>().unwrap_or(0.0)
                    > 75.0
                {
                    Color::Yellow
                } else {
                    Color::Green
                };

            let util_widget = Paragraph::new(vec![
                Line::from(vec![Span::styled(
                    "Utilization",
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    &stats.utilization,
                    Style::default().fg(util_color).add_modifier(Modifier::BOLD),
                )]),
                Line::from("of capacity"),
            ])
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);

            // Oldest entry
            let oldest_widget = Paragraph::new(vec![
                Line::from(vec![Span::styled(
                    "Oldest Entry",
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from(
                    stats
                        .oldest_entry_age_seconds
                        .map(|s| format!("{}s ago", s))
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                Line::from("age"),
            ])
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);

            // Newest entry
            let newest_widget = Paragraph::new(vec![
                Line::from(vec![Span::styled(
                    "Newest Entry",
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from(
                    stats
                        .newest_entry_age_seconds
                        .map(|s| format!("{}s ago", s))
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                Line::from("age"),
            ])
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);

            f.render_widget(entries_widget, chunks[0]);
            f.render_widget(util_widget, chunks[1]);
            f.render_widget(oldest_widget, chunks[2]);
            f.render_widget(newest_widget, chunks[3]);
        } else {
            let empty = Paragraph::new("Loading cache statistics...")
                .block(Block::default().borders(Borders::ALL).title("Cache Statistics"))
                .alignment(Alignment::Center);
            f.render_widget(empty, area);
        }
    }

    fn render_cache_details(&self, f: &mut Frame, area: Rect) {
        if let Some(stats) = &self.cache_stats {
            let details = vec![
                Line::from(vec![Span::styled(
                    "Cache File Path:",
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Line::from(stats.cache_file_path.clone()),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Cache Information:",
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Line::from(format!("• Total entries: {}", stats.total_entries)),
                Line::from(format!("• Maximum capacity: {}", stats.max_entries)),
                Line::from(format!("• Current utilization: {}", stats.utilization)),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Entry Age Range:",
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Line::from(format!(
                    "• Oldest: {}",
                    stats
                        .oldest_entry_age_seconds
                        .map(|s| format!("{}s ago", s))
                        .unwrap_or_else(|| "N/A".to_string())
                )),
                Line::from(format!(
                    "• Newest: {}",
                    stats
                        .newest_entry_age_seconds
                        .map(|s| format!("{}s ago", s))
                        .unwrap_or_else(|| "N/A".to_string())
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "Note:",
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(
                        " Cache automatically evicts oldest entries when capacity is reached.",
                    ),
                ]),
            ];

            let details_widget = Paragraph::new(details)
                .block(Block::default().borders(Borders::ALL).title("Cache Details"))
                .wrap(Wrap { trim: true })
                .scroll((self.scroll_offset as u16, 0));

            f.render_widget(details_widget, area);
        } else {
            let empty = Paragraph::new("Loading cache details...")
                .block(Block::default().borders(Borders::ALL).title("Cache Details"))
                .alignment(Alignment::Center);
            f.render_widget(empty, area);
        }
    }

    pub fn render_instances(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Summary
                Constraint::Min(0),    // Instance list
            ])
            .split(area);

        // Instance summary
        self.render_instance_summary(f, chunks[0]);

        // Instance list
        self.render_instance_list(f, chunks[1]);
    }

    fn render_instance_summary(&self, f: &mut Frame, area: Rect) {
        let instance_count = self.active_instances.len();
        let status_text = if instance_count > 0 {
            format!(
                "🟢 {} active debugging session{}",
                instance_count,
                if instance_count == 1 { "" } else { "s" }
            )
        } else {
            "⚪ No active debugging sessions".to_string()
        };

        let summary = Paragraph::new(vec![
            Line::from(vec![Span::styled(
                "EDB Instance Registry",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(status_text),
        ])
        .block(Block::default().borders(Borders::ALL).title("Summary"))
        .alignment(Alignment::Center);

        f.render_widget(summary, area);
    }

    fn render_instance_list(&self, f: &mut Frame, area: Rect) {
        if self.active_instances.is_empty() {
            let empty = Paragraph::new(vec![
                Line::from("No EDB instances are currently registered."),
                Line::from(""),
                Line::from("When EDB instances connect to this proxy, they will"),
                Line::from("appear here with their process IDs and status."),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "Note: ",
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("EDB instances automatically register when using"),
                ]),
                Line::from("this proxy as their RPC endpoint."),
            ])
            .block(Block::default().borders(Borders::ALL).title("Active Instances"))
            .alignment(Alignment::Center);

            f.render_widget(empty, area);
        } else {
            let items: Vec<ListItem> = self
                .active_instances
                .iter()
                .map(|&pid| {
                    let line = Line::from(vec![
                        Span::raw("🟢 "),
                        Span::styled(format!("PID: {}", pid), Style::default().fg(Color::Green)),
                        Span::styled(" (Active)", Style::default().fg(Color::Gray)),
                    ]);
                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Active EDB Instances"));

            f.render_widget(list, area);
        }
    }
}
