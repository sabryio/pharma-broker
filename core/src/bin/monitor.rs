//! # Interactive Monitor TUI
//!
//! Interactive app configuration and group monitoring.
//! Ported from legacy/cmd/monitor.go using ratatui.
//!
//! ## Usage
//! ```bash
//! cargo run --bin monitor
//! ```
//!
//! ## Controls
//! - Tab/1/2: Switch between tabs
//! - ↑/↓: Navigate items
//! - Space: Toggle selection
//! - Enter: Save and exit
//! - q: Quit without saving

use std::env;
use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use pharma_core::domain::Group;
use pharma_core::repository::GroupRepository;
use pharma_core::repository::postgres::PostgresGroupRepo;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
};
use sqlx::postgres::PgPoolOptions;

// ============================================================================
// Colors (matching Go version)
// ============================================================================

const COLOR_BG: Color = Color::Rgb(15, 23, 42); // Deep navy
const COLOR_SURFACE: Color = Color::Rgb(30, 41, 59); // Card bg
const COLOR_BORDER: Color = Color::Rgb(51, 65, 85); // Border
const COLOR_TEXT: Color = Color::Rgb(241, 245, 249); // Primary text
const COLOR_MUTED: Color = Color::Rgb(100, 116, 139); // Muted text
const COLOR_PRIMARY: Color = Color::Rgb(168, 85, 247); // Purple
const COLOR_SECONDARY: Color = Color::Rgb(34, 211, 238); // Cyan
const COLOR_SUCCESS: Color = Color::Rgb(34, 197, 94); // Green

// ============================================================================
// App State
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Groups,
    Config,
}

impl Tab {
    fn titles() -> Vec<&'static str> {
        vec!["1 📱 Groups", "2 ⚙️  Config"]
    }

    fn index(&self) -> usize {
        match self {
            Tab::Groups => 0,
            Tab::Config => 1,
        }
    }

    fn from_index(i: usize) -> Self {
        match i {
            0 => Tab::Groups,
            1 => Tab::Config,
            _ => Tab::Groups,
        }
    }
}

#[derive(Debug, Clone)]
struct GroupItem {
    jid: String,
    name: String,
    monitored: bool,
}

#[derive(Debug, Clone)]
struct ConfigItem {
    #[allow(dead_code)]
    key: String,
    label: String,
    description: String,
    enabled: bool,
}

struct App {
    // State
    active_tab: Tab,
    groups: Vec<GroupItem>,
    configs: Vec<ConfigItem>,
    group_state: ListState,
    config_state: ListState,
    should_quit: bool,
    saved: bool,

    // Repository
    group_repo: PostgresGroupRepo,
}

impl App {
    fn new(group_repo: PostgresGroupRepo, groups: Vec<Group>) -> Self {
        let group_items: Vec<GroupItem> = groups
            .into_iter()
            .map(|g| GroupItem {
                jid: g.jid,
                name: g.name,
                monitored: g.monitored,
            })
            .collect();

        let configs = vec![
            ConfigItem {
                key: "skip_own_messages".to_string(),
                label: "Skip Own Messages".to_string(),
                description: "Don't process messages sent by this account".to_string(),
                enabled: true,
            },
            ConfigItem {
                key: "auto_parse_enabled".to_string(),
                label: "Auto Parse Enabled".to_string(),
                description: "Automatically parse incoming messages with AI".to_string(),
                enabled: true,
            },
        ];

        let mut group_state = ListState::default();
        if !group_items.is_empty() {
            group_state.select(Some(0));
        }

        let mut config_state = ListState::default();
        if !configs.is_empty() {
            config_state.select(Some(0));
        }

        Self {
            active_tab: Tab::Groups,
            groups: group_items,
            configs,
            group_state,
            config_state,
            should_quit: false,
            saved: false,
            group_repo,
        }
    }

    fn next_tab(&mut self) {
        self.active_tab = Tab::from_index((self.active_tab.index() + 1) % 2);
    }

    fn prev_tab(&mut self) {
        self.active_tab = Tab::from_index((self.active_tab.index() + 1) % 2);
    }

    fn next_item(&mut self) {
        match self.active_tab {
            Tab::Groups => {
                if self.groups.is_empty() {
                    return;
                }
                let i = match self.group_state.selected() {
                    Some(i) => (i + 1) % self.groups.len(),
                    None => 0,
                };
                self.group_state.select(Some(i));
            }
            Tab::Config => {
                if self.configs.is_empty() {
                    return;
                }
                let i = match self.config_state.selected() {
                    Some(i) => (i + 1) % self.configs.len(),
                    None => 0,
                };
                self.config_state.select(Some(i));
            }
        }
    }

    fn prev_item(&mut self) {
        match self.active_tab {
            Tab::Groups => {
                if self.groups.is_empty() {
                    return;
                }
                let i = match self.group_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.groups.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.group_state.select(Some(i));
            }
            Tab::Config => {
                if self.configs.is_empty() {
                    return;
                }
                let i = match self.config_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.configs.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.config_state.select(Some(i));
            }
        }
    }

    fn toggle_current(&mut self) {
        match self.active_tab {
            Tab::Groups => {
                if let Some(i) = self.group_state.selected() {
                    if i < self.groups.len() {
                        self.groups[i].monitored = !self.groups[i].monitored;
                    }
                }
            }
            Tab::Config => {
                if let Some(i) = self.config_state.selected() {
                    if i < self.configs.len() {
                        self.configs[i].enabled = !self.configs[i].enabled;
                    }
                }
            }
        }
    }

    async fn save_all(&mut self) -> anyhow::Result<()> {
        // Save group monitoring status
        for group in &self.groups {
            self.group_repo
                .update_monitored(&group.jid, group.monitored)
                .await?;
        }
        self.saved = true;
        Ok(())
    }

    fn count_monitored(&self) -> usize {
        self.groups.iter().filter(|g| g.monitored).count()
    }
}

// ============================================================================
// UI Rendering
// ============================================================================

fn ui(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Tabs
            Constraint::Min(10),   // Content
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    // Header
    render_header(frame, chunks[0]);

    // Tabs
    render_tabs(frame, chunks[1], app);

    // Content
    match app.active_tab {
        Tab::Groups => render_groups(frame, chunks[2], app),
        Tab::Config => render_config(frame, chunks[2], app),
    }

    // Status bar
    render_status_bar(frame, chunks[3], app);
}

fn render_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new("💊 PharmaBroker Monitor")
        .style(
            Style::default()
                .fg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_PRIMARY))
                .style(Style::default().bg(COLOR_SURFACE)),
        );
    frame.render_widget(header, area);
}

fn render_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles: Vec<Line> = Tab::titles().iter().map(|t| Line::from(*t)).collect();

    let tabs = Tabs::new(titles)
        .select(app.active_tab.index())
        .style(Style::default().fg(COLOR_MUTED))
        .highlight_style(
            Style::default()
                .fg(COLOR_BG)
                .bg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" │ ")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_BORDER))
                .style(Style::default().bg(COLOR_SURFACE)),
        );
    frame.render_widget(tabs, area);
}

fn render_groups(frame: &mut Frame, area: Rect, app: &mut App) {
    let monitored = app.count_monitored();
    let total = app.groups.len();

    let title = format!("📱 WhatsApp Groups ({}/{} active)", monitored, total);

    let items: Vec<ListItem> = app
        .groups
        .iter()
        .map(|g| {
            let check = if g.monitored {
                Span::styled("● ", Style::default().fg(COLOR_SUCCESS).bold())
            } else {
                Span::styled("○ ", Style::default().fg(COLOR_MUTED))
            };

            let name = Span::styled(&g.name, Style::default().fg(COLOR_TEXT));

            ListItem::new(Line::from(vec![check, name]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .title_style(Style::default().fg(COLOR_SECONDARY).bold())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_BORDER))
                .style(Style::default().bg(COLOR_SURFACE)),
        )
        .highlight_style(
            Style::default()
                .fg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    frame.render_stateful_widget(list, area, &mut app.group_state);
}

fn render_config(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .configs
        .iter()
        .map(|c| {
            let check = if c.enabled {
                Span::styled("● ", Style::default().fg(COLOR_SUCCESS).bold())
            } else {
                Span::styled("○ ", Style::default().fg(COLOR_MUTED))
            };

            let label = Span::styled(&c.label, Style::default().fg(COLOR_TEXT));
            let desc = Span::styled(
                format!("  - {}", c.description),
                Style::default().fg(COLOR_MUTED).italic(),
            );

            ListItem::new(vec![Line::from(vec![check, label]), Line::from(vec![desc])])
                .style(Style::default())
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("⚙️  Application Settings")
                .title_style(Style::default().fg(COLOR_SECONDARY).bold())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_BORDER))
                .style(Style::default().bg(COLOR_SURFACE)),
        )
        .highlight_style(
            Style::default()
                .fg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    frame.render_stateful_widget(list, area, &mut app.config_state);
}

fn render_status_bar(frame: &mut Frame, area: Rect, _app: &App) {
    let help = "Tab: switch │ ↑↓: navigate │ Space: toggle │ Enter: save │ q: quit";

    let status = Paragraph::new(help)
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_BORDER))
                .style(Style::default().bg(COLOR_SURFACE)),
        );
    frame.render_widget(status, area);
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/pharmabroker".to_string());

    // Connect to database
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let group_repo = PostgresGroupRepo::new(pool);

    // Load groups
    let groups = group_repo.get_all().await?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new(group_repo, groups);

    // Run event loop
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Show result
    if let Err(e) = result {
        eprintln!("Error: {}", e);
        return Err(e);
    }

    if app.saved {
        let count = app.count_monitored();
        println!("\n  ✅ Settings saved! Monitoring {} groups.\n", count);
    } else {
        println!("\n  Cancelled.\n");
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => {
                        app.should_quit = true;
                    }
                    KeyCode::Tab | KeyCode::Right => {
                        app.next_tab();
                    }
                    KeyCode::BackTab | KeyCode::Left => {
                        app.prev_tab();
                    }
                    KeyCode::Char('1') => {
                        app.active_tab = Tab::Groups;
                    }
                    KeyCode::Char('2') => {
                        app.active_tab = Tab::Config;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.next_item();
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.prev_item();
                    }
                    KeyCode::Char(' ') => {
                        app.toggle_current();
                    }
                    KeyCode::Enter => {
                        app.save_all().await?;
                        app.should_quit = true;
                    }
                    _ => {}
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
