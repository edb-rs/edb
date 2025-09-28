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

// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
// SPDX-License-Identifier: AGPL-3.0
//! Terminal User Interface for EDB
//!
//! This crate provides a terminal-based interface for interacting with the EDB engine.

mod app;
mod config;
mod data;
mod layout;
mod panels;
mod rpc;
mod ui;

pub use app::App;
pub use config::Config;
pub use layout::{LayoutConfig, LayoutManager, LayoutType};
pub use panels::EventResponse;
pub use rpc::RpcClient;
pub use ui::{
    BorderPresets, BreakpointStatus, ColorScheme, ConnectionStatus, EnhancedBorder,
    ExecutionStatus, FileStatus, Icons, PanelStatus, RpcStatus, Spinner, SpinnerAnimation,
    SpinnerStyles, StatusBar, Theme,
};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use eyre::Result;
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, sync::Arc, time::Duration};
use tokio::{select, time::interval};
use tracing::{debug, error, info, warn};

/// Configuration for the TUI
#[derive(Debug, Clone)]
pub struct TuiConfig {
    /// RPC endpoint URL
    pub rpc_url: String,
    /// Terminal refresh interval
    pub refresh_interval: Duration,
    /// Data fetch interval
    pub data_fetch_interval: Duration,
    /// Enable mouse support
    pub enable_mouse: bool,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:3030".to_string(),
            refresh_interval: Duration::from_millis(50),
            data_fetch_interval: Duration::from_millis(200),
            enable_mouse: false,
        }
    }
}

/// Main TUI runner that manages the terminal interface and event loop
pub struct Tui {
    /// The main application state and panel management
    app: App,
    /// Terminal backend for rendering and input handling
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    /// Configuration settings for the TUI behavior
    config: TuiConfig,
}

impl Tui {
    /// Create a new TUI instance
    pub async fn new(config: TuiConfig) -> Result<Self> {
        info!("Initializing TUI with config: {:?}", config);

        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if config.enable_mouse {
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        } else {
            execute!(stdout, EnterAlternateScreen)?;
        }

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        // Create RPC client
        let rpc_client = Arc::new(RpcClient::new(&config.rpc_url).await?);

        // Create app with layout manager
        let layout_config = LayoutConfig { enable_mouse: config.enable_mouse };
        let app = App::new(rpc_client, layout_config).await?;

        Ok(Self { app, terminal, config })
    }

    /// Run the main TUI event loop
    pub async fn run(mut self) -> Result<()> {
        info!("Starting TUI event loop");

        // Create DataManager
        let mut data_manager = crate::data::DataManager::new(self.app.rpc_client.clone()).await?;

        // Get cores for background processing
        let exec_core = data_manager.get_execution_core();
        let resolver_core = data_manager.get_resolver_core();

        // Spawn background task for execution core processing
        let exec_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.config.data_fetch_interval);
            loop {
                interval.tick().await;
                let mut core = exec_core.write().await;
                if let Err(e) = core.process_pending_requests().await {
                    error!("Error processing execution requests: {}", e);
                }
            }
        });

        // Spawn background task for resolver core processing
        let resolver_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.config.data_fetch_interval);
            loop {
                interval.tick().await;
                let mut core = resolver_core.write().await;
                if let Err(e) = core.process_pending_requests().await {
                    error!("Error processing resolver requests: {}", e);
                }
            }
        });

        let mut event_stream = EventStream::new();
        let mut ticker = interval(self.config.refresh_interval);

        let result = loop {
            // Render current state
            let render_result = self.terminal.draw(|frame| {
                self.app.render(frame, &mut data_manager);
            });

            if let Err(e) = render_result {
                break Err(e.into());
            }

            // Handle events
            select! {
                // Handle terminal events (keyboard, mouse, resize)
                event_result = event_stream.next() => {
                    if let Some(Ok(event)) = event_result {
                        debug!("Received event: {:?}", event);

                        match event {
                            Event::Key(key_event) => {
                                match self.app.handle_key_event(key_event, &mut data_manager).await? {
                                    EventResponse::Exit => {
                                        info!("Exit requested");
                                        break Ok(());
                                    }
                                    EventResponse::Handled => {},
                                    EventResponse::NotHandled => {
                                        warn!("Unhandled key event: {:?}", key_event);
                                    }
                                    EventResponse::ChangeFocus(panel_type) => {
                                        // Handle panel focus changes
                                        debug!("Focus change requested to {:?}", panel_type);
                                        self.app.change_focus(panel_type);
                                    }
                                }
                            }
                            Event::Mouse(mouse_event) if self.config.enable_mouse => {
                                if let Err(e) = self.app.handle_mouse_event(mouse_event, &mut data_manager).await {
                                    error!("Mouse event error: {}", e);
                                }
                            }
                            Event::Resize(width, height) => {
                                debug!("Terminal resized: {}x{}", width, height);
                                self.app.handle_resize(width, height);
                            }
                            _ => {}
                        }
                    }
                }

                // Periodic refresh tick
                _ = ticker.tick() => {
                    // Update app state periodically
                    if let Err(e) = self.app.update().await {
                        error!("App update error: {}", e);
                    }

                    // Pull updates from cores (the first time we try to get more cached data)
                    data_manager.process_core_updates()?;

                    // Push pending requests to cores
                    data_manager.update_pending_requests().await?;

                    // Pull updates from cores
                    data_manager.process_core_updates()?;
                }
            }

            // Check if app wants to exit
            if self.app.should_exit() {
                info!("App requested exit");
                break Ok(());
            }
        };

        // Abort background tasks
        exec_handle.abort();
        resolver_handle.abort();

        info!("TUI event loop ended");
        result
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        // Restore terminal state
        let _ = disable_raw_mode();
        if self.config.enable_mouse {
            let _ =
                execute!(self.terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture);
        } else {
            let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        }
        let _ = self.terminal.show_cursor();
    }
}

/// Public API for the TUI module
pub mod api {
    use super::*;

    /// Start the TUI with the given configuration
    pub async fn start_tui(config: TuiConfig) -> Result<()> {
        let tui = Tui::new(config).await?;
        tui.run().await
    }

    /// Start the TUI with default configuration
    pub async fn start_default_tui() -> Result<()> {
        start_tui(TuiConfig::default()).await
    }

    /// Start the TUI with auto-detected RPC port
    pub async fn start_auto_tui() -> Result<()> {
        // Try to detect RPC server port
        let mut config = TuiConfig::default();

        // Try common ports
        for port in [3030, 8545, 8546, 9944] {
            let url = format!("http://localhost:{port}");
            if RpcClient::test_connection(&url).await.is_ok() {
                config.rpc_url = url;
                break;
            }
        }

        start_tui(config).await
    }
}
