use crate::nm_config::WorkflowConfig;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render_create(
    f: &mut Frame,
    cfg: &WorkflowConfig,
    focus: usize,
    input: &str,
    area: Rect,
) {
    let mut lines = Vec::<Line>::new();

    lines.push(Line::from(vec![Span::styled(
        "Hint: add role:file mappings like role:system:security.poml;role:user:query.poml",
        Style::default().fg(Color::Gray),
    )]));

    // Workflow Name
    let name_style = if focus == 0 {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };
    let name_val = if focus == 0 && !input.is_empty() {
        input.to_string()
    } else {
        cfg.name.clone()
    };
    lines.push(Line::from(vec![Span::styled(
        format!("Workflow Name: {}", name_val),
        name_style,
    )]));

    // Model
    let model_style = if focus == 1 {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };
    let model_val = if focus == 1 && !input.is_empty() {
        input.to_string()
    } else {
        cfg.model.clone()
    };
    lines.push(Line::from(vec![Span::styled(
        format!("Model: {}", model_val),
        model_style,
    )]));

    // Temperature
    let temp_style = if focus == 2 {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };
    let temp_val = if focus == 2 && !input.is_empty() {
        input.to_string()
    } else {
        cfg.temperature.to_string()
    };
    lines.push(Line::from(vec![Span::styled(
        format!("Temperature: {}", temp_val),
        temp_style,
    )]));

    // Number of agents
    let num_style = if focus == 3 {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };
    let num_val = if focus == 3 && !input.is_empty() {
        input.to_string()
    } else {
        cfg.rows.len().to_string()
    };
    lines.push(Line::from(vec![Span::styled(
        format!("Number of Agents: {}", num_val),
        num_style,
    )]));

    // ✅ Maximum traversals
    let trav_style = if focus == 4 {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };
    let trav_val = if focus == 4 && !input.is_empty() {
        input.to_string()
    } else {
        cfg.maximum_traversals.to_string()
    };
    lines.push(Line::from(vec![Span::styled(
        format!("Maximum Traversals: {}", trav_val),
        trav_style,
    )]));

    for (i, row) in cfg.rows.iter().enumerate() {
        let type_focus = focus == (i * 5 + 5);
        let files_focus = focus == (i * 5 + 6);
        let max_focus = focus == (i * 5 + 7);
        let success_focus = focus == (i * 5 + 8);
        let failure_focus = focus == (i * 5 + 9);

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

        let files_val = if files_focus && !input.is_empty() {
            input.to_string()
        } else {
            row.files.clone()
        };
        lines.push(Line::from(vec![Span::styled(
            format!("  Files {}: {}", i + 1, files_val),
            files_style,
        )]));

        let max_val = if max_focus && !input.is_empty() {
            input.to_string()
        } else {
            row.max_iterations.to_string()
        };
        lines.push(Line::from(vec![Span::styled(
            format!("  Max Iter {}: {}", i + 1, max_val),
            max_style,
        )]));

        let success_val = if success_focus && !input.is_empty() {
            input.to_string()
        } else {
            row.on_success.map(|v| v.to_string()).unwrap_or_default()
        };
        lines.push(Line::from(vec![Span::styled(
            format!("  On Success {}: {}", i + 1, success_val),
            if success_focus { Style::default().fg(Color::Black).bg(Color::Cyan) } else { Style::default() },
        )]));

        let failure_val = if failure_focus && !input.is_empty() {
            input.to_string()
        } else {
            row.on_failure.map(|v| v.to_string()).unwrap_or_default()
        };
        lines.push(Line::from(vec![Span::styled(
            format!("  On Failure {}: {}", i + 1, failure_val),
            if failure_focus { Style::default().fg(Color::Black).bg(Color::Cyan) } else { Style::default() },
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