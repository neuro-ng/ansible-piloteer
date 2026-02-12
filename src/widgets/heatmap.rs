use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};

use crate::app::App;

pub struct HeatmapWidget;

impl HeatmapWidget {
    pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
        let block = Block::default()
            .title("Performance Heatmap (Duration)")
            .borders(Borders::ALL);
        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        if app.history.is_empty() {
            return;
        }

        // Prepare data: Group tasks by Host
        // Rows: Hosts
        // Cols: Tasks (Sequence)

        let mut hosts: Vec<String> = app.hosts.keys().cloned().collect();
        hosts.sort();

        // Find max tasks for any host to determine columns
        // Actually, tasks are sequential in history.
        // We want to visualize:
        // Host A: [Task1][Task2]...
        // Host B: [Task1][Task2]...

        // Let's iterate history and build a map of Host -> Vec<TaskHistory>
        let mut host_tasks: std::collections::HashMap<String, Vec<&crate::app::TaskHistory>> =
            std::collections::HashMap::new();
        for task in &app.history {
            host_tasks.entry(task.host.clone()).or_default().push(task);
        }

        let header_cells = ["Host", "Tasks (Each block is a task, Color=Duration)"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
        let header = Row::new(header_cells).height(1).bottom_margin(1);

        let rows = hosts.iter().map(|host| {
            let tasks = host_tasks.get(host).map(|v| v.as_slice()).unwrap_or(&[]);

            // Render tasks as a string of blocks with different colors
            // Since Table cells expect Text/Spans, we can use a Line of Spans
            let mut spans = Vec::new();
            for task in tasks {
                let color = if task.duration < 1.0 {
                    Color::Green
                } else if task.duration < 5.0 {
                    Color::Yellow
                } else {
                    Color::Red
                };

                // Use a block character
                spans.push(ratatui::text::Span::styled(
                    "■ ",
                    Style::default().fg(color),
                ));
            }

            Row::new(vec![
                Cell::from(host.as_str()),
                Cell::from(ratatui::text::Line::from(spans)),
            ])
        });

        let table = Table::new(rows, [Constraint::Length(20), Constraint::Min(0)])
            .header(header)
            .block(Block::default());

        frame.render_widget(table, inner_area);
    }
}
