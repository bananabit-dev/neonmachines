use crate::commands::handle_command;
use crate::nm_config::{WorkflowConfig, save_all_nm};
use crate::runner::{AppCommand, AppEvent};
use ratatui::text::{Line, Span};
use ratatui::style::{Style, Color, Modifier};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::layout::{Layout, Constraint, Position};
use ratatui::Frame;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use unicode_segmentation::UnicodeSegmentation;

pub struct ChatMessage {
    pub from: &'static str,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Chat,
    Create,
    Workflow,
    Dashboard,
    InteractiveChat,
}

pub struct App {
    pub mode: Mode,
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub cursor_g: usize,
    pub scroll_offset: usize,
    pub is_running: bool,
    pub spinner_status: String,
    pub last_spinner_tick: Instant,
    pub tx: UnboundedSender<AppCommand>,
    pub rx: UnboundedReceiver<AppEvent>,
    pub workflows: HashMap<String, WorkflowConfig>,
    pub active_workflow: String,
    pub workflow_list: Vec<String>,
    pub workflow_index: usize,
    pub create_focus: usize,
    pub create_input: String,
    pub selected_agent: Option<usize>,
    pub metrics_collector: Option<Arc<Mutex<crate::metrics::metrics_collector::MetricsCollector>>>,
}

impl App {
    pub fn new(
        tx: UnboundedSender<AppCommand>,
        rx: UnboundedReceiver<AppEvent>,
        workflows: HashMap<String, WorkflowConfig>,
        active_workflow: String,
        metrics_collector: Option<Arc<Mutex<crate::metrics::metrics_collector::MetricsCollector>>>,
    ) -> Self {
        let workflow_list: Vec<String> = workflows.keys().cloned().collect();
        let workflow_index = workflow_list.iter().position(|w| w == &active_workflow).unwrap_or(0);
        
        Self {
            mode: Mode::Chat,
            messages: vec![ChatMessage {
                from: "system",
                text: "Welcome to Neonmachines! Type your message or use /help for commands.".to_string(),
            }],
            input: String::new(),
            cursor_g: 0,
            scroll_offset: 0,
            is_running: true,
            spinner_status: String::new(),
            last_spinner_tick: Instant::now(),
            tx,
            rx,
            workflows,
            active_workflow,
            workflow_list,
            workflow_index,
            create_focus: 0,
            create_input: String::new(),
            selected_agent: None,
            metrics_collector,
        }
    }

    /// ✅ Save all workflows on exit
    pub fn persist_on_exit(&self) {
        let all: Vec<WorkflowConfig> = self.workflows.values().cloned().collect();
        let _ = save_all_nm(&all);
    }

    pub fn tick_spinner(&mut self) {
        if self.is_running && self.last_spinner_tick.elapsed() >= Duration::from_millis(120) {
            self.last_spinner_tick = Instant::now();
        }
    }

    pub fn on_event(&mut self, ev: crossterm::event::Event) -> bool {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        
        match ev {
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, .. }) => {
                return true; // Quit
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Char('l'), modifiers: KeyModifiers::CONTROL, .. }) => {
                // Clear screen with Ctrl+L
                self.scroll_offset = 0;
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Char(c), .. }) => {
                if !crossterm::event::KeyModifiers::CONTROL.contains(crossterm::event::KeyModifiers::CONTROL) {
                    self.insert_char(c);
                }
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Enter, modifiers: KeyModifiers::SHIFT, .. }) => {
                // ✅ Insert newline instead of submitting
                self.insert_char('\n');
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Enter, .. }) => {
                self.submit();
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Backspace, .. }) => {
                self.backspace();
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Left, .. }) => {
                self.left();
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Right, .. }) => {
                self.right();
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Up, .. }) => {
                if self.scroll_offset + 1 < self.messages.len() {
                    self.scroll_offset += 1;
                }
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Down, .. }) => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Esc, .. }) => {
                if self.mode == Mode::Chat {
                    self.add_message("system", "Exited interactive chat mode".to_string());
                }
                self.mode = Mode::Chat;
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Tab, .. }) => {
                // Tab completion for commands
                if self.mode == Mode::Chat && self.input.starts_with('/') {
                    // Simple tab completion logic
                    let commands = vec!["help", "workflow", "create", "run", "chat", "history", "agent"];
                    let input = self.input.to_lowercase();
                    for cmd in commands {
                        if cmd.starts_with(&input[1..]) {
                            self.input = format!("/{}", cmd);
                            self.cursor_g = self.input.graphemes(true).count();
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
        false
    }

    pub fn on_input_change(&mut self) {
        // Handle input changes if needed
    }

    pub fn add_message(&mut self, from: &'static str, text: String) {
        self.messages.push(ChatMessage { from, text });
        if self.messages.len() > 0 {
            self.scroll_offset = self.messages.len().saturating_sub(1);
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let bi = byte_idx_for_g(&self.input, self.cursor_g);
        self.input.insert(bi, c);
        self.right();
    }

    pub fn backspace(&mut self) {
        if self.cursor_g > 0 {
            let l = byte_idx_for_g(&self.input, self.cursor_g - 1);
            let r = byte_idx_for_g(&self.input, self.cursor_g);
            self.input.replace_range(l..r, "");
            self.left();
        }
    }

    pub fn left(&mut self) {
        if self.cursor_g > 0 {
            self.cursor_g -= 1;
        }
    }

    pub fn right(&mut self) {
        let n = self.input.graphemes(true).count();
        if self.cursor_g < n {
            self.cursor_g += 1;
        }
    }

    pub fn submit(&mut self) {
    let line = self.input.clone();
    self.input.clear();

    // ✅ Treat the entire input (even multi-line) as one message
    self.add_message("you", line.clone());

    if line.starts_with('/') {
        // Pass the correct arguments including selected_agent and mutable mode reference
        handle_command(
            &line,
            &mut self.workflows,
            &mut self.active_workflow,
            &self.tx,
            &mut self.messages,
            &mut self.selected_agent, // Pass the mutable reference
            &mut self.mode,          // Pass the mutable mode reference
        );
    } else {
        // ... (rest of the else block for non-command input)
        if let Some(cfg) = self.workflows.get(&self.active_workflow) {
            // Convert Option<usize> to Option<i32> before sending
            let start_agent_i32: Option<i32> = self.selected_agent.map(|i| i as i32);
            let _ = self.tx.send(AppCommand::RunWorkflow {
                workflow_name: cfg.name.clone(),
                prompt: line.clone(),
                cfg: cfg.clone(),
                start_agent: start_agent_i32, // Use the converted value
            });
            self.add_message("system", format!("Running workflow '{}' with prompt: {}", cfg.name, line));
        } else {
            self.add_message("system", "No active workflow selected. Use /workflow to select one.".to_string());
        }
    }
}

    pub fn render(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Min(1), // Messages area
                Constraint::Length(8), // Input area
            ])
            .split(f.area());
        
        let main_area = chunks[0];
        let input_area = chunks[1];
        
        // Render messages
        let mut lines = Vec::new();
        for m in &self.messages {
            let style = match m.from {
                "you" => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                "system" => Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
                "progress" => Style::default().fg(Color::Yellow),
                "agent" => Style::default().fg(Color::Green),
                "error" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                _ => Style::default().fg(Color::White),
            };
            
            for (i, part) in m.text.lines().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(format!("{}: ", m.from), style),
                        Span::raw(part),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("   "),
                        Span::raw(part),
                    ]));
                }
            }
        }
        
        let para = Paragraph::new(lines)
            .block(Block::default()
                .borders(Borders::ALL)
                .title("💬 Messages")
                .title_style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset as u16, 0));
        f.render_widget(para, main_area);
        
        // Render performance metrics if available
        if let Some(metrics_ref) = &self.metrics_collector {
            if let Ok(metrics_guard) = metrics_ref.lock() {
                let metrics_summary = metrics_guard.get_request_summary_sync();
                let metrics_text = vec![Line::from(metrics_summary)];
                
                let metrics_block = Block::default()
                    .borders(Borders::ALL)
                    .title("📊 Performance Metrics")
                    .title_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD));
                    
                let metrics_para = Paragraph::new(metrics_text)
                    .block(metrics_block)
                    .style(Style::default().fg(Color::White));
                
                // Position metrics widget at the bottom right
                let metrics_area = Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        Constraint::Min(1),
                        Constraint::Length(3),
                    ])
                    .split(input_area)[1];
                    
                f.render_widget(metrics_para, metrics_area);
            }
        }
        
        // Enhanced multi-line input rendering with better styling
        let input_block = Block::default()
            .borders(Borders::ALL)
            .title("✍️ Input (Enter=submit, Shift+Enter=newline, Ctrl+C=quit)")
            .title_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
            
        let input = Paragraph::new(self.input.as_str())
            .style(Style::default().fg(Color::Yellow))
            .block(input_block)
            .wrap(Wrap { trim: false });
        f.render_widget(input, input_area);
        
        // Enhanced cursor positioning with visual feedback
        let lines: Vec<&str> = self.input.split('\n').collect();
        let current_line = lines.len().saturating_sub(1);
        let current_col = lines.last().map(|l| l.graphemes(true).count()).unwrap_or(0);
        let cx = input_area.x + 2 + current_col as u16; // +2 for padding
        let cy = input_area.y + 1 + current_line as u16; // +1 for padding
        
        f.set_cursor_position(Position::new(cx, cy));
    }

    pub async fn poll_async(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                AppEvent::Log(line) => {
                    self.add_message("progress", line);
                }
                AppEvent::RunStart(name) => {
                    self.spinner_status = format!("Running workflow '{}'", name);
                    self.add_message("system", format!("Starting run for workflow '{}'…", name));
                }
                AppEvent::RunResult(line) => {
                    self.spinner_status.clear();
                    self.add_message("agent", format!("Result: {}", line));
                }
                AppEvent::RunEnd(name) => {
                    self.spinner_status.clear();
                    self.add_message("system", format!("Run for '{}' completed.", name));
                }
                AppEvent::Error(line) => {
                    self.spinner_status.clear();
                    self.add_message("error", line);
                }
            }
        }
    }
}

fn grapheme_boundaries(s: &str) -> Vec<usize> {
    let mut idxs = vec![0];
    for (i, _) in s.grapheme_indices(true) {
        idxs.push(i);
    }
    idxs.push(s.len());
    idxs.sort_unstable();
    idxs.dedup();
    idxs
}

fn byte_idx_for_g(s: &str, g: usize) -> usize {
    let v = grapheme_boundaries(s);
    *v.get(g).unwrap_or(&s.len())
}