use color_eyre::Result;
use crossterm::{cursor, execute, terminal};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io;

pub type DefaultTerminal = Terminal<CrosstermBackend<io::Stdout>>;

pub fn setup_terminal() -> Result<DefaultTerminal> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

pub fn restore_terminal(mut terminal: DefaultTerminal) -> Result<()> {
    terminal.show_cursor()?;
    crossterm::execute!(
        terminal.backend_mut(),
        terminal::LeaveAlternateScreen,
        cursor::Show
    )?;
    terminal::disable_raw_mode()?;
    Ok(())
}