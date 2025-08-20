use crate::nm_config::WorkflowConfig;
use ratatui::layout::{Rect, Layout};
use ratatui::prelude::{Constraint, Modifier};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

pub fn render_workflow(f: &mut Frame, workflows: &[WorkflowConfig], index: usize, area: Rect) {
    if workflows.is_empty() {
        // No workflows available
        let empty_text = vec![
            Line::from("📋 No workflows available"),
            Line::from(""),
            Line::from("Create a new workflow with /create <name>"),
            Line::from(""),
            Line::from("Press Esc to return to chat mode"),
        ];
        
        let empty_para = Paragraph::new(empty_text)
            .block(Block::default()
                .borders(Borders::ALL)
                .title("Workflows")
                .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        f.render_widget(empty_para, area);
        return;
    }

    // Create workflow list items with selection indicator
    let items: Vec<ListItem> = workflows
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let is_selected = i == index;
            let prefix = if is_selected { "▶ " } else { "  " };
            
            let style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            
            // Create styled line with selection prefix
            let line = if is_selected {
                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::styled(w.name.clone(), style),
                ])
            } else {
                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Gray)),
                    Span::styled(w.name.clone(), style),
                ])
            };
            
            ListItem::new(line)
        })
        .collect();
    
    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(format!("🔄 Workflows (Selected: {})", workflows[index].name))
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    
    f.render_widget(list, area);
    
    // Add navigation instructions at the bottom
    let instructions = vec![
        Line::from("← → Navigate  |  Enter Select  |  Esc Exit"),
    ];
    
    let instructions_area = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area)[1];
    
    let instructions_para = Paragraph::new(instructions)
        .block(Block::default()
            .borders(Borders::NONE)
            .style(Style::default().fg(Color::Gray)));
    f.render_widget(instructions_para, instructions_area);
}