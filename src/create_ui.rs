use crate::nm_config::WorkflowConfig;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render_create(f: &mut Frame, cfg: &WorkflowConfig, focus: usize, area: Rect) {
    let mut lines = Vec::<Line>::new();

    // Static hint line
    lines.push(Line::from(vec![Span::styled(
        "Hint: you need to add role to each file!. files: role:system:security_check.poml;role:user:get_files.poml;role:ai:summary.poml",
        Style::default().fg(Color::Gray),
    )]));

    // Workflow Name
    let name_style = if focus == 0 {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };
    lines.push(Line::from(vec![Span::styled(
        format!("Workflow Name: {}", cfg.name),
        name_style,
    )]));

    // Number of agents
    let num_style = if focus == 1 {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };
    lines.push(Line::from(vec![Span::styled(
        format!("Number of Agents: {}", cfg.rows.len()),
        num_style,
    )]));

    for (i, row) in cfg.rows.iter().enumerate() {
        let type_focus = focus == (i * 3 + 2);
        let files_focus = focus == (i * 3 + 3);
        let max_focus = focus == (i * 3 + 4);

        let type_style = if type_focus {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default()
        };
        let files_style = if files_focus {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default()
        };
        let max_style = if max_focus {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default()
        };

        lines.push(Line::from(vec![Span::styled(
            format!("Agent {}: {:?}", i + 1, row.agent_type),
            type_style,
        )]));
        lines.push(Line::from(vec![Span::styled(
            format!("  Files {}: {}", i + 1, row.files),
            files_style,
        )]));
        lines.push(Line::from(vec![Span::styled(
            format!("  Max Iter {}: {}", i + 1, row.max_iterations),
            max_style,
        )]));
    }

    let p = Paragraph::new(Text::from(lines)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Create")
            .border_style(Style::default().fg(Color::Blue)),
    );
    f.render_widget(p, area);
}