use crate::commands::handle_command;
use crate::nm_config::{WorkflowConfig, save_all_nm, AgentType, AgentRow};
use crate::runner::{AppCommand, AppEvent};
use crate::create_ui;
use crate::workflow_ui;
use ratatui::text::{Line, Span};
use ratatui::style::{Style, Color, Modifier};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::layout::{Layout, Constraint, Position, Rect};
use ratatui::Frame;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use unicode_segmentation::UnicodeSegmentation;
use std::collections::VecDeque;

pub struct ChatMessage {
    pub from: &'static str,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Chat,
    Create,
    Workflow,
    Options,
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
    pub options_input: String,
    pub selected_agent: Option<usize>,
    pub metrics_collector: Option<Arc<Mutex<crate::metrics::metrics_collector::MetricsCollector>>>,
    pub cached_metrics_text: String,
    pub last_metrics_update: Instant,
    pub event_queue: VecDeque<crossterm::event::Event>,
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
            options_input: String::new(),
            selected_agent: None,
            metrics_collector,
            cached_metrics_text: "No metrics data".to_string(),
            last_metrics_update: Instant::now(),
            event_queue: VecDeque::new(),
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
        
        // Handle key events immediately without blocking
        match ev {
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, .. }) => {
                // Immediately return true to quit, no need to process further
                return true;
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Char('l'), modifiers: KeyModifiers::CONTROL, .. }) => {
                // Clear screen with Ctrl+L
                self.scroll_offset = 0;
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Char('d'), modifiers: KeyModifiers::CONTROL, .. }) => {
                // Ctrl+D to quit (alternative to Ctrl+C)
                return true;
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Char(c), .. }) => {
                // Handle character input based on mode
                match self.mode {
                    Mode::Create => {
                        // Handle create mode input
                        if let Some(_cfg) = self.workflows.get_mut(&self.active_workflow) {
                            self.handle_create_input(c);
                        }
                    }
                    Mode::Workflow => {
                        // Handle workflow mode input - ignore character input, only handle navigation
                    }
                    Mode::Options => {
                        // Handle options mode input
                        self.options_input.push(c);
                    }
                    _ => {
                        // Handle regular character input - check if it's not a modifier key
                        self.insert_char(c);
                    }
                }
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Enter, modifiers: KeyModifiers::SHIFT, .. }) => {
                // Insert newline instead of submitting
                self.insert_char('\n');
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Enter, .. }) => {
                match self.mode {
                    Mode::Create => {
                        self.handle_create_submit();
                    }
                    Mode::Workflow => {
                        // Select the current workflow and exit workflow mode
                        if self.workflow_index < self.workflow_list.len() {
                            let selected_workflow = self.workflow_list[self.workflow_index].clone();
                            self.add_message("system", format!("Selected workflow: '{}'", selected_workflow));
                            self.active_workflow = selected_workflow;
                        }
                        self.mode = Mode::Chat;
                    }
                    Mode::Options => {
                        self.handle_options_submit();
                    }
                    _ => {
                        self.submit();
                    }
                }
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Backspace, .. }) => {
                match self.mode {
                    Mode::Create => {
                        self.handle_create_backspace();
                    }
                    Mode::Options => {
                        self.handle_options_backspace();
                    }
                    _ => {
                        self.backspace();
                    }
                }
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Left, .. }) => {
                match self.mode {
                    Mode::Create => {
                        self.handle_create_left();
                    }
                    Mode::Workflow => {
                        // Navigate left in workflow mode (previous workflow)
                        if self.workflow_index > 0 {
                            self.workflow_index -= 1;
                            self.active_workflow = self.workflow_list[self.workflow_index].clone();
                        }
                    }
                    _ => {
                        self.move_cursor_left();
                    }
                }
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Right, .. }) => {
                match self.mode {
                    Mode::Create => {
                        self.handle_create_right();
                    }
                    Mode::Workflow => {
                        // Navigate right in workflow mode (next workflow)
                        if self.workflow_index < self.workflow_list.len() - 1 {
                            self.workflow_index += 1;
                            self.active_workflow = self.workflow_list[self.workflow_index].clone();
                        }
                    }
                    _ => {
                        self.move_cursor_right();
                    }
                }
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Up, .. }) => {
                match self.mode {
                    Mode::Create => {
                        self.handle_create_up();
                    }
                    _ => {
                        self.move_cursor_up();
                    }
                }
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Down, .. }) => {
                match self.mode {
                    Mode::Create => {
                        self.handle_create_down();
                    }
                    _ => {
                        self.move_cursor_down();
                    }
                }
            }
            crossterm::event::Event::Key(KeyEvent { code: KeyCode::Esc, .. }) => {
                // Exit special modes
                match self.mode {
                    Mode::Create | Mode::Workflow | Mode::Options => {
                        self.add_message("system", format!("Exited {} mode", match self.mode {
                            Mode::Create => "create",
                            Mode::Workflow => "workflow", 
                            Mode::Options => "options",
                            _ => "unknown"
                        }));
                        self.mode = Mode::Chat;
                    }
                    Mode::Chat => {
                        // In chat mode, Esc does nothing or could clear input
                    }
                    _ => {
                        self.mode = Mode::Chat;
                    }
                }
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
            crossterm::event::Event::Paste(text) => {
                // Handle paste events - treat pasted content as a single input
                self.insert_paste_content(&text);
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

    /// Enhanced cursor movement for multi-line input
    pub fn move_cursor_left(&mut self) {
        if self.cursor_g > 0 {
            self.cursor_g -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        let n = self.input.graphemes(true).count();
        if self.cursor_g < n {
            self.cursor_g += 1;
        }
    }

    pub fn move_cursor_up(&mut self) {
        let lines: Vec<&str> = self.input.split('\n').collect();
        if lines.len() <= 1 {
            // If only one line, move to beginning
            self.cursor_g = 0;
            return;
        }

        let current_line_index = self.get_current_line_index();
        if current_line_index > 0 {
            let _target_line = &lines[current_line_index - 1];
            let current_col_in_line = self.get_current_column_in_line();
            
            // Move to same column in previous line, or end of line if column is beyond
            let new_cursor_pos = self.calculate_position_for_line_and_col(current_line_index - 1, current_col_in_line);
            self.cursor_g = new_cursor_pos;
        }
    }

    pub fn move_cursor_down(&mut self) {
        let lines: Vec<&str> = self.input.split('\n').collect();
        if lines.len() <= 1 {
            // If only one line, move to end
            self.cursor_g = self.input.graphemes(true).count();
            return;
        }

        let current_line_index = self.get_current_line_index();
        if current_line_index < lines.len() - 1 {
            let _target_line = &lines[current_line_index + 1];
            let current_col_in_line = self.get_current_column_in_line();
            
            // Move to same column in next line, or end of line if column is beyond
            let new_cursor_pos = self.calculate_position_for_line_and_col(current_line_index + 1, current_col_in_line);
            self.cursor_g = new_cursor_pos;
        } else {
            // Move to end of last line
            self.cursor_g = self.input.graphemes(true).count();
        }
    }

    /// Handle paste content properly for multi-line text
    pub fn insert_paste_content(&mut self, content: &str) {
        // Normalize line endings to \n for consistent handling
        let normalized_content = content.replace("\r\n", "\n").replace('\r', "\n");
        
        // Find the position to insert
        let bi = byte_idx_for_g(&self.input, self.cursor_g);
        
        // Insert the content
        self.input.insert_str(bi, &normalized_content);
        
        // Move cursor to end of pasted content
        let pasted_graphemes = UnicodeSegmentation::graphemes(normalized_content.as_str(), true).count();
        self.cursor_g += pasted_graphemes;
    }

    /// Helper method to get current line index (0-based)
    fn get_current_line_index(&self) -> usize {
        let lines: Vec<&str> = self.input.split('\n').collect();
        let mut current_pos = 0;
        
        for (i, line) in lines.iter().enumerate() {
            let line_end = current_pos + line.graphemes(true).count() + (if i < lines.len() - 1 { 1 } else { 0 });
            if self.cursor_g < line_end {
                return i;
            }
            current_pos = line_end;
        }
        
        lines.len().saturating_sub(1)
    }

    /// Helper method to get current column within current line
    fn get_current_column_in_line(&self) -> usize {
        let lines: Vec<&str> = self.input.split('\n').collect();
        let current_line_index = self.get_current_line_index();
        let current_line = lines[current_line_index];
        
        let _line_pos = 0;
        for (i, _) in current_line.grapheme_indices(true) {
            if self.cursor_g == _line_pos + (if current_line_index < lines.len() - 1 { 1 } else { 0 }) + i {
                return i;
            }
        }
        
        current_line.graphemes(true).count()
    }

    /// Helper method to calculate cursor position from line and column
    fn calculate_position_for_line_and_col(&self, line_index: usize, col: usize) -> usize {
        let lines: Vec<&str> = self.input.split('\n').collect();
        let mut position = 0;
        
        for (i, line) in lines.iter().enumerate() {
            if i == line_index {
                // Return min of requested column or line length
                return position + col.min(line.graphemes(true).count());
            }
            position += line.graphemes(true).count() + 1; // +1 for newline
        }
        
        self.input.graphemes(true).count() // Fallback to end
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
        // Handle different modes
        match self.mode {
            Mode::Create => {
                // Create mode layout - full screen for create interface
                let area = f.area();
                self.render_create_mode(f, area);
            }
            Mode::Workflow => {
                // Workflow mode layout - full screen for workflow selection
                let area = f.area();
                self.render_workflow_mode(f, area);
            }
            Mode::Options => {
                // Options mode layout - full screen for options input
                let area = f.area();
                self.render_options_mode(f, area);
            }
            _ => {
                // Normal chat mode layout
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
                
                // Render performance metrics if available using cached text
                let metrics_text = if self.cached_metrics_text.is_empty() {
                    vec![Line::from("No metrics data")]
                } else {
                    vec![Line::from(self.cached_metrics_text.clone())]
                };
                
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
                
                // Enhanced cursor positioning with visual feedback using helper methods
                let _lines: Vec<&str> = self.input.split('\n').collect();
                let current_line = self.get_current_line_index() as u16;
                let current_col = self.get_current_column_in_line() as u16;
                let cx = input_area.x + 2 + current_col; // +2 for padding (block borders)
                let cy = input_area.y + 1 + current_line; // +1 for padding (block title)
                
                // Fix cursor position - don't go past the end of the input
                if self.input.is_empty() {
                    // When input is empty, position at start
                    f.set_cursor_position(Position::new(cx, cy));
                } else if self.cursor_g == self.input.graphemes(true).count() {
                    // When cursor is at end, position it properly without extra space
                    f.set_cursor_position(Position::new(cx, cy));
                } else {
                    // Normal cursor position
                    f.set_cursor_position(Position::new(cx, cy));
                }
            }
        }
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

    pub fn update_cached_metrics(&mut self) {
        // Only update metrics every 500ms to avoid excessive lock contention
        if self.last_metrics_update.elapsed() >= Duration::from_millis(500) {
            if let Some(metrics_ref) = &self.metrics_collector {
                // Use try_lock to avoid blocking, and skip if lock is unavailable
                if let Ok(metrics_guard) = metrics_ref.try_lock() {
                    self.cached_metrics_text = metrics_guard.get_request_summary_sync();
                    self.last_metrics_update = Instant::now();
                }
            }
        }
    }

    /// Add event to the queue for non-blocking processing
    pub fn queue_event(&mut self, event: crossterm::event::Event) {
        self.event_queue.push_back(event);
    }

    /// Process all queued events without blocking
    pub fn process_events(&mut self) -> bool {
        while let Some(event) = self.event_queue.pop_front() {
            if self.on_event(event) {
                return true; // Quit signal
            }
        }
        false
    }

    /// Create mode handling methods
    pub fn handle_create_input(&mut self, c: char) {
        // Handle input in create mode based on focus field
        match self.create_focus {
            0 => {
                // Workflow Name - append to end
                self.create_input.push(c);
            }
            1 => {
                // Model - append to end
                self.create_input.push(c);
            }
            2 => {
                // Temperature - append to end
                self.create_input.push(c);
            }
            3 => {
                // Number of Agents - append to end
                self.create_input.push(c);
            }
            4 => {
                // Maximum Traversals - append to end
                self.create_input.push(c);
            }
            5 => {
                // Working Directory - append to end
                self.create_input.push(c);
            }
            _ => {
                // Handle agent-specific fields
                let agent_idx = (self.create_focus - 6) / 5;
                let field_type = (self.create_focus - 6) % 5;
                
                if let Some(cfg) = self.workflows.get_mut(&self.active_workflow) {
                    if agent_idx < cfg.rows.len() {
                        match field_type {
                            0 => {
                                // Agent Type - append to input and update immediately
                                self.create_input.push(c);
                                // Parse agent type and update workflow using static function
                                let new_type = Self::parse_agent_type_static(&self.create_input);
                                cfg.rows[agent_idx].agent_type = new_type;
                            }
                            1 => {
                                // Files - append to end
                                cfg.rows[agent_idx].files.push(c);
                            }
                            2 => {
                                // Max Iterations - append to end
                                self.create_input.push(c);
                                cfg.rows[agent_idx].max_iterations = self.create_input.parse().unwrap_or(3);
                            }
                            3 => {
                                // On Success - append to end
                                self.create_input.push(c);
                                cfg.rows[agent_idx].on_success = self.create_input.parse().ok();
                            }
                            4 => {
                                // On Failure - append to end
                                self.create_input.push(c);
                                cfg.rows[agent_idx].on_failure = self.create_input.parse().ok();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    pub fn handle_create_submit(&mut self) {
        // Submit create mode input and save workflow
        if let Some(cfg) = self.workflows.get_mut(&self.active_workflow) {
            // Update workflow based on focus field
            match self.create_focus {
                0 => cfg.name = self.create_input.clone(),
                1 => cfg.model = self.create_input.clone(),
                2 => cfg.temperature = self.create_input.parse().unwrap_or(0.7),
                3 => {
                    let num_agents = self.create_input.parse().unwrap_or(1);
                    if num_agents != cfg.rows.len() {
                        // Resize agent rows
                        if num_agents > cfg.rows.len() {
                            for _ in cfg.rows.len()..num_agents {
                                cfg.rows.push(AgentRow::default());
                            }
                        } else {
                            cfg.rows.truncate(num_agents);
                        }
                    }
                }
                4 => cfg.maximum_traversals = self.create_input.parse().unwrap_or(20),
                5 => cfg.working_dir = self.create_input.clone(),
                _ => {
                    // Handle agent-specific fields
                    let agent_idx = (self.create_focus - 6) / 5;
                    let field_type = (self.create_focus - 6) % 5;
                    
                    if agent_idx < cfg.rows.len() {
                        // Extract the input value before the match to avoid borrow checker issues
                        let input_value = self.create_input.clone();
                        
                        // Parse agent type inline to avoid method calls that cause borrow conflicts
                        let agent_type = if field_type == 0 {
                            Some(match input_value.to_lowercase().as_str() {
                                "validator" => AgentType::Validator,
                                "parallel" | "parallelagent" => AgentType::ParallelAgent,
                                _ => AgentType::Agent,
                            })
                        } else {
                            None
                        };
                        
                        // Update agent field
                        match field_type {
                            0 => {
                                if let Some(at) = agent_type {
                                    cfg.rows[agent_idx].agent_type = at;
                                }
                            }
                            1 => cfg.rows[agent_idx].files = input_value,
                            2 => cfg.rows[agent_idx].max_iterations = input_value.parse().unwrap_or(3),
                            3 => cfg.rows[agent_idx].on_success = input_value.parse().ok(),
                            4 => cfg.rows[agent_idx].on_failure = input_value.parse().ok(),
                            _ => {}
                        }
                    }
                }
            }
            
            // Save the workflow and collect message content before adding
            let workflow_name = cfg.name.clone();
            let all: Vec<WorkflowConfig> = self.workflows.values().cloned().collect();
            let save_result = save_all_nm(&all);
            
            // Handle save result after we're done with the mutable borrow
            if let Err(e) = save_result {
                self.add_message("error", format!("Failed to save workflow: {}", e));
            } else {
                self.add_message("system", format!("Workflow '{}' updated successfully", workflow_name));
            }
        }
        
        // Clear create input but stay in create mode
        self.create_input.clear();
        // Don't change mode - stay in Mode::Create
    }

    pub fn handle_create_backspace(&mut self) {
        if !self.create_input.is_empty() {
            self.create_input.pop();
        }
    }

    pub fn handle_create_left(&mut self) {
        // Navigate to previous agent in create mode
        if self.create_focus >= 11 { // Only allow navigation if we're past the first agent
            self.create_focus -= 1; // Move left by 5 fields to previous agent
            self.create_input.clear(); // Clear input for new field
        }
    }

    pub fn handle_create_right(&mut self) {
        // Navigate to next agent in create mode
        if let Some(cfg) = self.workflows.get(&self.active_workflow) {
            let max_focus = 6 + (cfg.rows.len() * 5); // 6 base fields + 5 per agent
            if self.create_focus < max_focus - 5 { // Don't go past the last agent
                self.create_focus += 1; // Move right by 5 fields to next agent
                self.create_input.clear(); // Clear input for new field
            }
        }
    }

    pub fn handle_create_up(&mut self) {
        // Navigate up in create mode (previous field in same column)
        if self.create_focus >= 1 {
            self.create_focus -= 1; // Move up by 5 fields to stay in same column
            self.create_input.clear();
        }
    }

    pub fn handle_create_down(&mut self) {
        // Navigate down in create mode (next field in same column)
        if let Some(cfg) = self.workflows.get(&self.active_workflow) {
            let max_focus = 6 + (cfg.rows.len() * 5); // 6 base fields + 5 per agent
            // Only move down if not already at the bottom
            if self.create_focus < max_focus - 1 { // -1 to prevent overflow
                self.create_focus += 1; // Move down by 1 field
                self.create_input.clear(); // Clear input for new field
            }
        }
    }

    /// Parse agent type from string
    fn parse_agent_type(&self, input: &str) -> AgentType {
        match input.to_lowercase().as_str() {
            "validator" => AgentType::Validator,
            "parallel" | "parallelagent" => AgentType::ParallelAgent,
            _ => AgentType::Agent,
        }
    }

    /// Parse agent type from string without borrowing self
    fn parse_agent_type_static(input: &str) -> AgentType {
        match input.to_lowercase().as_str() {
            "validator" => AgentType::Validator,
            "parallel" | "parallelagent" => AgentType::ParallelAgent,
            _ => AgentType::Agent,
        }
    }

    /// Set agent type for a specific agent index
    fn set_agent_type(&mut self, agent_idx: usize, input: &str) {
        let agent_type = self.parse_agent_type(input);
        if let Some(cfg) = self.workflows.get_mut(&self.active_workflow) {
            if agent_idx < cfg.rows.len() {
                cfg.rows[agent_idx].agent_type = agent_type;
            }
        }
    }

    /// Render create mode UI
    pub fn render_create_mode(&self, f: &mut Frame, area: Rect) {
        if let Some(cfg) = self.workflows.get(&self.active_workflow) {
            create_ui::render_create(f, cfg, self.create_focus, &self.create_input, area);
        }
    }

    /// Render workflow mode UI
    pub fn render_workflow_mode(&self, f: &mut Frame, area: Rect) {
        // Convert workflow_list to the expected type for the workflow_ui function
        let workflow_configs: Vec<WorkflowConfig> = self.workflow_list.iter()
            .filter_map(|name| self.workflows.get(name).cloned())
            .collect();
        workflow_ui::render_workflow(f, &workflow_configs, self.workflow_index, area);
    }

    /// Render options mode UI
    pub fn render_options_mode(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Title
                Constraint::Min(1),   // Instructions
                Constraint::Length(3), // Input area
            ])
            .split(area);

        // Title
        let title = Paragraph::new("🔧 Options Mode")
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .alignment(ratatui::prelude::Alignment::Center);
        f.render_widget(title, chunks[0]);

        // Instructions
        let instructions = vec![
            Line::from("Type your input to be sent to poml template:"),
            Line::from(""),
            Line::from("Example: <let name=\"input2\">place input here</let>"),
            Line::from(""),
            Line::from("Press Enter to submit, Esc to exit options mode"),
        ];
        
        let instructions_para = Paragraph::new(instructions)
            .block(Block::default()
                .borders(Borders::NONE)
                .style(Style::default().fg(Color::Gray)));
        f.render_widget(instructions_para, chunks[1]);

        // Input area
        let input_block = Block::default()
            .borders(Borders::ALL)
            .title("📝 Input")
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
            
        let input_para = Paragraph::new(self.options_input.as_str())
            .style(Style::default().fg(Color::White))
            .block(input_block)
            .wrap(Wrap { trim: false });
        f.render_widget(input_para, chunks[2]);

        // Set cursor position for options input
        let cx = chunks[2].x + 2 + self.options_input.graphemes(true).count() as u16;
        let cy = chunks[2].y + 1;
        f.set_cursor_position(Position::new(cx, cy));
    }

    /// Handle options mode input submission
    pub fn handle_options_submit(&mut self) {
        let input = self.options_input.clone();
        self.options_input.clear();
        
        // Add the submitted input as a message
        self.add_message("you", input.clone());
        
        // Process the poml template injection
        if let Some(cfg) = self.workflows.get_mut(&self.active_workflow) {
            // Save the current state before processing
            let workflow_name = cfg.name.clone();
            let all: Vec<WorkflowConfig> = self.workflows.values().cloned().collect();
            let save_result = save_all_nm(&all);
            
            if let Err(e) = save_result {
                self.add_message("error", format!("Failed to save workflow: {}", e));
            } else {
                self.add_message("system", format!("Options input processed for workflow '{}'", workflow_name));
            }
        }
        
        // Exit options mode
        self.mode = Mode::Chat;
    }

    /// Handle options mode backspace
    pub fn handle_options_backspace(&mut self) {
        if !self.options_input.is_empty() {
            self.options_input.pop();
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