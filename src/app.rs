use crate::commands::handle_command;
use crate::create_ui::render_create;
use crate::nm_config::{AgentRow, AgentType, WorkflowConfig, save_nm};
use crate::runner::{AppCommand, AppEvent};
use crate::workflow_ui::render_workflow;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::collections::HashMap;
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
}

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Chat,
    Create,
    Workflow,
}

impl App {
    pub fn new(
        tx: UnboundedSender<AppCommand>,
        rx: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
        workflows: HashMap<String, WorkflowConfig>,
        active: String,
    ) -> Self {
        Self {
            messages: vec![ChatMessage {
                from: "system",
                text: "Welcome! Type a message to chat with the active workflow, or use /create, /run, /agent, /workflow, /save".into(),
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
        }
    }

    pub fn persist_on_exit(&self) {
        if let Some(cfg) = self.workflows.get(&self.active_workflow) {
            let _ = save_nm(cfg);
        }
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
                        Mode::Chat => match k.code {
                            KeyCode::Enter => self.submit(),
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
                            _ => {}
                        },
                        Mode::Create => match k.code {
                            KeyCode::Tab => {
                                self.create_focus += 1;
                                let max_focus =
                                    self.workflows[&self.active_workflow].rows.len() * 5 + 5;
                                if self.create_focus > max_focus {
                                    self.create_focus = 0;
                                }
                            }
                            KeyCode::BackTab => {
                                if self.create_focus == 0 {
                                    self.create_focus =
                                        self.workflows[&self.active_workflow].rows.len() * 5 + 5;
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
                                        0 => {
                                            if !self.input.trim().is_empty() {
                                                let new_name = self.input.trim().to_string();
                                                let mut cfg_clone = cfg.clone();
                                                cfg_clone.name = new_name.clone();
                                                self.workflows.remove(&self.active_workflow);
                                                self.workflows.insert(new_name.clone(), cfg_clone);
                                                self.active_workflow = new_name;
                                            }
                                        }
                                        1 => {
                                            if !self.input.trim().is_empty() {
                                                cfg.model = self.input.trim().to_string();
                                            }
                                        }
                                        2 => {
                                            if let Ok(t) = self.input.trim().parse::<f32>() {
                                                cfg.temperature = t;
                                            }
                                        }
                                        3 => {
                                            if let Ok(n) = self.input.trim().parse::<usize>() {
                                                cfg.rows.resize_with(n, || AgentRow {
                                                    agent_type: AgentType::Agent,
                                                    files: String::new(),
                                                    max_iterations: 3,
                                                    on_success: None,
                                                    on_failure: None,
                                                });
                                            }
                                        }
                                        4 => {
                                            if let Ok(n) = self.input.trim().parse::<usize>() {
                                                cfg.maximum_traversals = n;
                                            }
                                        }
                                        _ => {
                                            let row_idx = (self.create_focus - 5) / 5;
                                            let field = (self.create_focus - 5) % 5;
                                            match field {
                                                0 => {
                                                    // toggle agent type
                                                    let row = &mut cfg.rows[row_idx];
                                                    row.agent_type = match row.agent_type {
                                                        AgentType::Agent => AgentType::ParallelAgent,
                                                        AgentType::ParallelAgent => AgentType::ValidatorAgent,
                                                        AgentType::ValidatorAgent => AgentType::Agent,
                                                    };
                                                }
                                                1 => {
                                                    cfg.rows[row_idx].files = self.input.clone();
                                                }
                                                2 => {
                                                    if let Ok(n) = self.input.trim().parse::<usize>() {
                                                        cfg.rows[row_idx].max_iterations = n;
                                                    }
                                                }
                                                3 => {
                                                    if let Ok(n) = self.input.trim().parse::<i32>() {
                                                        cfg.rows[row_idx].on_success = Some(n);
                                                    }
                                                }
                                                4 => {
                                                    if let Ok(n) = self.input.trim().parse::<i32>() {
                                                        cfg.rows[row_idx].on_failure = Some(n);
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                                self.input.clear();
                            }
                            KeyCode::Esc => {
                                if let Some(cfg) = self.workflows.get(&self.active_workflow) {
                                    let _ = save_nm(cfg);
                                }
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
                                    self.workflows.insert(wf.name.clone(), wf.clone());
                                    self.active_workflow = wf.name.clone();
                                    let _ = save_nm(&wf);
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
                for ch in s.chars() {
                    self.insert_char(ch);
                }
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
        self.add_message("you", line.clone());
        if line.starts_with('/') {
            handle_command(
                &line,
                &mut self.workflows,
                &mut self.active_workflow,
                &self.tx,
                &mut self.messages,
            );
            if line.starts_with("/create") {
                self.mode = Mode::Create;
            }
            if line.starts_with("/workflow") && !line.contains(' ') {
                self.mode = Mode::Workflow;
                self.workflow_list = self.workflows.values().cloned().collect();
                self.workflow_index = 0;
            }
        } else {
            if let Some(cfg) = self.workflows.get(&self.active_workflow) {
                let user_prompt = format!("User: {}", line);
                let _ = self.tx.send(AppCommand::RunWorkflow {
                    workflow_name: cfg.name.clone(),
                    prompt: user_prompt,
                    cfg: cfg.clone(),
                });
            }
        }
        self.input.clear();
        self.cursor_g = 0;
    }

    pub fn render(&self, f: &mut Frame) {
        match self.mode {
            Mode::Chat => {
                let layout = Layout::vertical([
                    Constraint::Min(1),
                    Constraint::Length(3),
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
                    lines.push(Line::from(vec![
                        Span::styled(format!("{}: ", m.from), style),
                        Span::styled(m.text.clone(), style),
                    ]));
                }
                let text = Text::from(lines);
                let para = Paragraph::new(text)
                    .block(Block::default().borders(Borders::ALL).title("Messages"))
                    .wrap(Wrap { trim: false })
                    .scroll((self.scroll_offset as u16, 0));
                f.render_widget(para, main_area);

                let input = Paragraph::new(self.input.as_str())
                    .style(Style::default().fg(Color::Yellow))
                    .block(Block::bordered().title("Input"));
                f.render_widget(input, input_area);

                let cx = input_area.x + 1 + self.cursor_g as u16;
                let cy = input_area.y + 1;
                f.set_cursor_position(Position::new(cx, cy));
            }
            Mode::Create => {
                use crate::create_ui::render_create;
                render_create(
                    f,
                    self.workflows.get(&self.active_workflow).unwrap(),
                    self.create_focus,
                    &self.input,
                    f.area(),
                );
            }
            Mode::Workflow => {
                use crate::workflow_ui::render_workflow;
                render_workflow(f, &self.workflow_list, self.workflow_index, f.area());
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