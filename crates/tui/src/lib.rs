// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
// SPDX-License-Identifier: AGPL-3.0
//! Terminal User Interface for EDB
//!
//! This crate provides a terminal-based interface for interacting with the EDB engine.

mod app;
mod config;
mod layout;
mod managers;
mod panels;
mod rpc;
mod ui;

pub use app::App;
pub use config::{ColorScheme as ConfigColorScheme, Config, Theme};
pub use layout::{LayoutConfig, LayoutManager, LayoutType};
pub use managers::ThemeManager;
pub use panels::EventResponse;
pub use rpc::RpcClient;
pub use ui::{
    BorderPresets, BreakpointStatus, ColorScheme, ConnectionStatus, EnhancedBorder,
    ExecutionStatus, FileStatus, Icons, PanelStatus, RpcSpinner, RpcStatus, Spinner, SpinnerStyles,
    StatusBar, Theme as UiTheme,
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
use tracing::{debug, error, info};

/// Configuration for the TUI
#[derive(Debug, Clone)]
pub struct TuiConfig {
    /// RPC endpoint URL
    pub rpc_url: String,
    /// Terminal refresh interval
    pub refresh_interval: Duration,
    /// Enable mouse support
    pub enable_mouse: bool,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:3030".to_string(),
            refresh_interval: Duration::from_millis(50),
            enable_mouse: false,
        }
    }
}

/// Main TUI runner
pub struct Tui {
    app: App,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
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

        let mut event_stream = EventStream::new();
        let mut ticker = interval(self.config.refresh_interval);

        loop {
            // Render current state
            self.terminal.draw(|frame| {
                if let Err(e) = self.app.render(frame) {
                    error!("Render error: {}", e);
                }
            })?;

            // Handle events
            select! {
                // Handle terminal events (keyboard, mouse, resize)
                event_result = event_stream.next() => {
                    if let Some(Ok(event)) = event_result {
                        debug!("Received event: {:?}", event);

                        match event {
                            Event::Key(key_event) => {
                                match self.app.handle_key_event(key_event).await? {
                                    EventResponse::Exit => {
                                        info!("Exit requested");
                                        break;
                                    }
                                    EventResponse::Handled => {},
                                    EventResponse::NotHandled => {
                                        debug!("Unhandled key event: {:?}", key_event);
                                    }
                                    EventResponse::ChangeFocus(_panel_type) => {
                                        // Handle panel focus changes if needed
                                        debug!("Focus change requested");
                                    }
                                }
                            }
                            Event::Mouse(mouse_event) if self.config.enable_mouse => {
                                if let Err(e) = self.app.handle_mouse_event(mouse_event).await {
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
                }
            }

            // Check if app wants to exit
            if self.app.should_exit() {
                info!("App requested exit");
                break;
            }
        }

        info!("TUI event loop ended");
        Ok(())
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
            let url = format!("http://localhost:{}", port);
            if RpcClient::test_connection(&url).await.is_ok() {
                config.rpc_url = url;
                break;
            }
        }

        start_tui(config).await
    }
}
