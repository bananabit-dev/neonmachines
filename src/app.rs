use crate::commands::handle_command;
use crate::nm_config::{save_all_nm, AgentRow, AgentType, WorkflowConfig};
use crate::runner::{AppCommand, AppEvent};
use crate::metrics::{MetricsCollector, charts};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap, Gauge, BarChart};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct ChatMessage {
    pub from: &'static str,
    pub text: String,
}

pub struct App {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub cursor_g: usize,

    pub workflows: HashMap<String, WorkflowConfig>,
    pub active_workflow: String,

    pub workflow_list: Vec<WorkflowConfig>,
    pub workflow_index: usize,

    pub tx: UnboundedSender<AppCommand>,
    pub rx: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,

    pub is_running: bool,
    pub spinner_phase: usize,
    pub last_spinner_tick: Instant,
    pub spinner_status: String,

    pub mode: Mode,
    pub create_focus: usize,
    pub scroll_offset: usize,
    pub auto_scroll: bool,
    
    pub selected_agent: Option<usize>,
    
    pub metrics: Option<Arc<Mutex<MetricsCollector>>>,
    
    pub dashboard_mode: charts::DashboardMode,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Chat,
    Create,
    Workflow,
    InteractiveChat,
    Dashboard,
}

impl App {
    pub fn new(
        tx: UnboundedSender<AppCommand>,
        rx: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
        workflows: HashMap<String, WorkflowConfig>,
        active: String,
        metrics: Option<Arc<Mutex<MetricsCollector>>>,
    ) -> Self {
        Self {
            messages: vec![ChatMessage {
                from: "system",
                text: "Welcome! Type a message to chat with the active workflow, or use /create, /run, /agent, /workflow, /save, /chat".into(),
            }],
            input: String::new(),
            cursor_g: 0,
            workflows,
            active_workflow: active,
            workflow_list: Vec::new(),
            workflow_index: 0,
            tx,
            rx,
            is_running: false,
            spinner_phase: 0,
            last_spinner_tick: Instant::now(),
            spinner_status: String::new(),
            mode: Mode::Chat,
            create_focus: 0,
            scroll_offset: 0,
            auto_scroll: true,
            selected_agent: None,
            metrics,
            dashboard_mode: charts::DashboardMode::Overview,
        }
    }

    /// ✅ Save all workflows on exit
    pub fn persist_on_exit(&self) {
        let all: Vec<WorkflowConfig> = self.workflows.values().cloned().collect();
        let _ = save_all_nm(&all);
    }

    pub fn tick_spinner(&mut self) {
        if self.is_running && self.last_spinner_tick.elapsed() >= Duration::from_millis(120) {
            self.spinner_phase = (self.spinner_phase + 1) % 4;
            self.last_spinner_tick = Instant::now();
        }
    }

    pub fn on_event(&mut self, ev: crossterm::event::Event) -> bool {
        use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
        match ev {
            Event::Key(k) => {
                if k.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(k.code, KeyCode::Char('c'))
                {
                    return true;
                }
                if k.kind == KeyEventKind::Press {
                    match self.mode {
                        Mode::Chat | Mode::InteractiveChat => match k.code {
                            KeyCode::Enter => {
                                if k.modifiers.contains(KeyModifiers::SHIFT) {
                                    // ✅ Insert newline instead of submitting
                                    self.insert_char('\n');
                                } else {
                                    self.submit();
                                }
                            }
                            KeyCode::Char(c) => {
                                if !k.modifiers.contains(KeyModifiers::CONTROL) {
                                    self.insert_char(c);
                                }
                            }
                            KeyCode::Backspace => self.backspace(),
                            KeyCode::Left => self.left(),
                            KeyCode::Right => self.right(),
                            KeyCode::Up => {
                                if self.scroll_offset > 0 {
                                    self.scroll_offset -= 1;
                                    self.auto_scroll = false;
                                }
                            }
                            KeyCode::Down => {
                                if self.scroll_offset + 1 < self.messages.len() {
                                    self.scroll_offset += 1;
                                } else {
                                    self.auto_scroll = true;
                                }
                            }
                            KeyCode::Esc => {
                                if self.mode == Mode::InteractiveChat {
                                    self.mode = Mode::Chat;
                                    self.add_message("system", "Exited interactive chat mode".to_string());
                                }
                            }
                            _ => {}
                        },
                        Mode::Create => match k.code {
                            KeyCode::Tab => {
                                self.create_focus += 1;
                                let max_focus =
                                    self.workflows[&self.active_workflow].rows.len() * 5 + 6; // +6 because working_dir added
                                if self.create_focus > max_focus {
                                    self.create_focus = 0;
                                }
                            }
                            KeyCode::BackTab => {
                                if self.create_focus == 0 {
                                    self.create_focus =
                                        self.workflows[&self.active_workflow].rows.len() * 5 + 6;
                                } else {
                                    self.create_focus -= 1;
                                }
                            }
                            KeyCode::Char(c) => {
                                self.insert_char(c);
                            }
                            KeyCode::Backspace => {
                                self.backspace();
                            }
                     KeyCode::Enter => {
                                if let Some(cfg) = self.workflows.get_mut(&self.active_workflow) {
                                    match self.create_focus {
                                        0 => cfg.name = self.input.clone(),
                                        1 => cfg.model = self.input.clone(),
                                        2 => {
                                            if let Ok(val) = self.input.parse::<f32>() {
                                                cfg.temperature = val;
                                            }
                                        }
                                        3 => {
                                            if let Ok(val) = self.input.parse::<usize>() {
                                                // resize rows if number of agents changes
                                                if val > cfg.rows.len() {
                                                    cfg.rows.resize(val, AgentRow::default());
                                                } else {
                                                    cfg.rows.truncate(val);
                                                }
                                            }
                                        }
                                        4 => {
                                            if let Ok(val) = self.input.parse::<usize>() {
                                                cfg.maximum_traversals = val;
                                            }
                                        }
                                        5 => cfg.working_dir = self.input.clone(),
                                        _ => {
                                            // agent-specific fields
                                            let agent_idx = (self.create_focus - 6) / 5;
                                            let field = (self.create_focus - 6) % 5;
                                            if let Some(row) = cfg.rows.get_mut(agent_idx) {
                                                match field {
                                                    0 => {
                                                        // agent type
                                                        row.agent_type = match self
                                                            .input
                                                            .to_lowercase()
                                                            .as_str()
                                                        {
                                                            "validator" => {
                                                                AgentType::ValidatorAgent
                                                            }
                                                            "parallel" => AgentType::ParallelAgent,
                                                            _ => AgentType::Agent,
                                                        };
                                                    }
                                                    1 => row.files = self.input.clone(),
                                                    2 => {
                                                        if let Ok(val) = self.input.parse::<usize>()
                                                        {
                                                            row.max_iterations = val;
                                                        }
                                                    }
                                                    3 => {
                                                        if let Ok(val) = self.input.parse::<i32>() {
                                                            row.on_success = if val >= 0 {
                                                                Some(val)
                                                            } else {
                                                                None
                                                            };
                                                        }
                                                    }
                                                    4 => {
                                                        if let Ok(val) = self.input.parse::<i32>() {
                                                            row.on_failure = if val >= 0 {
                                                                Some(val)
                                                            } else {
                                                                None
                                                            };
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                }
                                self.input.clear();
                            }
                            KeyCode::Esc => {
                                // ✅ Save all workflows when exiting create mode
                                let all: Vec<WorkflowConfig> = self.workflows.values().cloned().collect();
                                let _ = save_all_nm(&all);
                                self.mode = Mode::Chat;
                            }
                            _ => {}
                        },
                        Mode::Workflow => match k.code {
                            KeyCode::Up => {
                                if self.workflow_index > 0 {
                                    self.workflow_index -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if self.workflow_index + 1 < self.workflow_list.len() {
                                    self.workflow_index += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(wf) =
                                    self.workflow_list.get(self.workflow_index).cloned()
                                {
                                    // ✅ Only update active workflow in memory, don’t overwrite config
                                    self.workflows.insert(wf.name.clone(), wf.clone());
                                    self.active_workflow = wf.name.clone();
                                    self.messages.push(ChatMessage {
                                        from: "system",
                                        text: format!("Workflow set to '{}'", wf.name),
                                    });
                                }
                                self.mode = Mode::Chat;
                            }
                            KeyCode::Esc => {
                                self.mode = Mode::Chat;
                            }
                            _ => {}
                        },
                    }
                }
            }
            Event::Paste(s) => {
                // ✅ Multi-line paste support: keep entire paste in input buffer
                self.input.push_str(&s);
                self.cursor_g = self.input.graphemes(true).count();
            }
            _ => {}
        }
        false
    }

    pub async fn poll_async(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                AppEvent::RunStart(name) => {
                    self.is_running = true;
                    self.spinner_phase = 0;
                    self.spinner_status = format!("Running workflow '{}'", name);
                    self.add_message("system", format!("Starting run for workflow '{}'…", name));
                }
                AppEvent::Log(line) => {
                    self.spinner_status = line.clone();
                    self.add_message("progress", line);
                }
                AppEvent::RunResult(s) => {
                    if s.starts_with("Error:") {
                        self.add_message("error", s);
                    } else {
                        self.add_message("agent", format!("Result: {}", s));
                    }
                }
                AppEvent::RunEnd(name) => {
                    self.is_running = false;
                    self.spinner_status.clear();
                    self.add_message("system", format!("Run for '{}' completed.", name));
                }
            }
        }
    }

    fn left(&mut self) {
        if self.cursor_g > 0 {
            self.cursor_g -= 1;
        }
    }
    fn right(&mut self) {
        let n = self.input.graphemes(true).count();
        if self.cursor_g < n {
            self.cursor_g += 1;
        }
    }
    fn insert_char(&mut self, c: char) {
        let bi = byte_idx_for_g(&self.input, self.cursor_g);
        self.input.insert(bi, c);
        self.right();
    }
    fn backspace(&mut self) {
        if self.cursor_g == 0 {
            return;
        }
        let l = byte_idx_for_g(&self.input, self.cursor_g - 1);
        let r = byte_idx_for_g(&self.input, self.cursor_g);
        self.input.replace_range(l..r, "");
        self.left();
    }

    fn submit(&mut self) {
        let line = self.input.clone();

        // ✅ Treat the entire input (even multi-line) as one message
        self.add_message("you", line.clone());

        if line.starts_with('/') {
            handle_command(
                &line,
                &mut self.workflows,
                &mut self.active_workflow,
                &self.tx,
                &mut self.messages,
                &mut self.selected_agent,
                &mut self.mode, // ✅ pass mode so /create can switch
            );
        } else {
            if let Some(cfg) = self.workflows.get(&self.active_workflow) {
                let user_prompt = if self.mode == Mode::InteractiveChat {
                    line.clone()
                } else {
                    format!("User: {}", line)
                };
                let _ = self.tx.send(AppCommand::RunWorkflow {
                    workflow_name: cfg.name.clone(),
                    prompt: user_prompt,
                    cfg: cfg.clone(),
                    start_agent: self.selected_agent,
                });
            } else {
                self.add_message("system", "No active workflow selected. Use /workflow to select one.".to_string());
            }
        }

        // ✅ Clear input after sending
        self.input.clear();
        self.cursor_g = 0;
    }

    pub fn render(&self, f: &mut Frame) {
        match self.mode {
            Mode::Chat | Mode::InteractiveChat => {
                let layout = Layout::vertical([
                    Constraint::Min(1),
                    Constraint::Length(8), // Increased input box for better multi-line support
                ]);
                let chunks = layout.split(f.area());
                let main_area = chunks[0];
                let input_area = chunks[1];

                let mut lines: Vec<Line> = Vec::new();
                for m in &self.messages {
                    let style = match m.from {
                        "you" => Style::default().fg(Color::Cyan).bold(),
                        "system" => Style::default().fg(Color::Gray).italic(),
                        "progress" => Style::default().fg(Color::Yellow),
                        "agent" => Style::default().fg(Color::Green),
                        "error" => Style::default().fg(Color::Red).bold(),
                        _ => Style::default(),
                    };
                    for (i, part) in m.text.lines().enumerate() {
                        if i == 0 {
                            lines.push(Line::from(vec![
                                Span::styled(format!("{}: ", m.from), style),
                                Span::styled(part.to_string(), style),
                            ]));
                        } else {
                            lines.push(Line::from(vec![
                                Span::styled("    ", style),
                                Span::styled(part.to_string(), style),
                            ]));
                        }
                    }
                }
                let text = Text::from(lines);
                let para = Paragraph::new(text)
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .title("💬 Messages")
                        .title_style(Style::default().fg(Color::Blue).bold()))
                    .wrap(Wrap { trim: false })
                    .scroll((self.scroll_offset as u16, 0));
                f.render_widget(para, main_area);

                // Render performance metrics if available
                if let Some(metrics_ref) = &self.metrics {
                    if let Ok(mut metrics_guard) = metrics_ref.lock() {
                        let metrics_summary = metrics_guard.get_request_summary_sync();
                        let metrics_block = Block::default()
                            .borders(Borders::ALL)
                            .title("📊 Performance Metrics")
                            .title_style(Style::default().fg(Color::Magenta).bold());
                        
                        let metrics_text = Text::from(metrics_summary);
                        let metrics_para = Paragraph::new(metrics_text)
                            .block(metrics_block)
                            .style(Style::default().fg(Color::White));
                        
                        // Position metrics widget at the bottom right
                        let metrics_area = Layout::horizontal([
                            Constraint::Percentage(70),
                            Constraint::Percentage(30),
                        ]).split(input_area)[1];
                        
                        f.render_widget(metrics_para, metrics_area);
                    }
                }

                // Enhanced multi-line input rendering with better styling
                let input_block = Block::default()
                    .borders(Borders::ALL)
                    .title("✍️ Input (Enter=submit, Shift+Enter=newline, Ctrl+C=quit)")
                    .title_style(Style::default().fg(Color::Green).bold());
                
                let input = Paragraph::new(self.input.as_str())
                    .style(Style::default().fg(Color::Yellow))
                    .block(input_block)
                    .wrap(Wrap { trim: false });
                f.render_widget(input, input_area);

                // Enhanced cursor positioning with visual feedback
                let lines: Vec<&str> = self.input.split('\n').collect();
                let current_line = lines.len().saturating_sub(1);
                let current_col = lines.last().map(|l| l.graphemes(true).count()).unwrap_or(0);

                let cx = input_area.x + 1 + current_col as u16;
                let cy = input_area.y + 1 + current_line as u16;
                f.set_cursor_position(Position::new(cx, cy));
            }
            Mode::Create => {
                // Create mode temporarily disabled - UI functions not available
                let area = f.area();
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title("Create Mode (Temporarily Disabled)")
                    .title_style(Style::default().fg(Color::Red).bold());
                let text = Text::from("Create mode UI functions are not available. Please use other modes.");
                let para = Paragraph::new(text).block(block);
                f.render_widget(para, area);
            }
            Mode::Workflow => {
                // Workflow mode temporarily disabled - UI functions not available
                let area = f.area();
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title("Workflow Mode (Temporarily Disabled)")
                    .title_style(Style::default().fg(Color::Red).bold());
                let text = Text::from("Workflow mode UI functions are not available. Please use other modes.");
                let para = Paragraph::new(text).block(block);
                f.render_widget(para, area);
            }
            Mode::Dashboard => {
    // TODO: implement dashboard rendering
            }
        }
    }

    pub fn add_message(&mut self, from: &'static str, text: String) {
        self.messages.push(ChatMessage { from, text });
        if self.auto_scroll {
            if self.messages.len() > 0 {
                self.scroll_offset = self.messages.len().saturating_sub(1);
            }
        }
    }
}

fn grapheme_boundaries(s: &str) -> Vec<usize> {
    let mut idxs = vec![0];
    for (i, _) in s.grapheme_indices(true) {
        if i != 0 {
            idxs.push(i);
        }
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