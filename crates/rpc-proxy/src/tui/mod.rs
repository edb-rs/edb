//! TUI (Terminal User Interface) for monitoring the RPC proxy
//!
//! Provides a real-time monitoring interface showing:
//! - Provider health and response times
//! - Cache statistics and hit rates
//! - EDB instance registry
//! - Request metrics and performance charts

use crate::proxy::ProxyServer;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use eyre::Result;
use ratatui::prelude::*;
use std::{
    io,
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::time::sleep;

mod app;
mod widgets;

use app::App;

/// Run the TUI interface for monitoring the proxy server
pub async fn run_tui(proxy: ProxyServer, addr: SocketAddr) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(proxy, addr);

    // Run TUI loop
    let result = run_tui_loop(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

async fn run_tui_loop<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(250); // 4 FPS

    loop {
        // Handle events
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('r') => app.refresh().await,
                        KeyCode::Char('c') => app.clear_cache().await,
                        KeyCode::Char('h') => app.toggle_help(),
                        KeyCode::Tab => app.next_tab(),
                        KeyCode::BackTab => app.previous_tab(),
                        KeyCode::Up => app.scroll_up(),
                        KeyCode::Down => app.scroll_down(),
                        KeyCode::Left => app.previous_provider(),
                        KeyCode::Right => app.next_provider(),
                        _ => {}
                    }
                }
            }
        }

        // Update app state on tick
        if last_tick.elapsed() >= tick_rate {
            app.update().await;
            last_tick = Instant::now();
        }

        // Render UI
        terminal.draw(|f| app.render(f))?;

        // Small sleep to prevent CPU spinning
        sleep(Duration::from_millis(10)).await;
    }

    Ok(())
}
