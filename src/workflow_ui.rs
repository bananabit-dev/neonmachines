use crate::nm_config::WorkflowConfig;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;

pub fn render_workflow(f: &mut Frame, workflows: &[WorkflowConfig], index: usize, area: Rect) {
    let items: Vec<ListItem> = workflows
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let style = if i == index {
                Style::default().fg(Color::Black).bg(Color::Green)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![Span::styled(w.name.clone(), style)]))
        })
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Workflows"));
    f.render_widget(list, area);
}