use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{BarChart, Block, Borders, Gauge, Paragraph, Sparkline},
};

use crate::app::App;

pub struct MetricsDashboard;

impl MetricsDashboard {
    pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
        if app.metrics_view == crate::app::MetricsView::Heatmap {
            crate::widgets::heatmap::HeatmapWidget::draw(frame, app, area);
            return;
        }

        // Layout:
        // Top: Status Distribution (Gauge/Text) - 15%
        // Middle: Task Duration (BarChart) - 50%
        // Bottom: Event/Log Velocity (Sparkline) - 35%
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(15),
                Constraint::Percentage(50),
                Constraint::Percentage(35),
            ])
            .split(area);

        Self::draw_status_distribution(frame, app, chunks[0]);
        Self::draw_task_durations(frame, app, chunks[1]);
        Self::draw_event_velocity(frame, app, chunks[2]);
    }

    fn draw_status_distribution(frame: &mut Frame, app: &App, area: Rect) {
        let block = Block::default()
            .title("Status Distribution")
            .borders(Borders::ALL);
        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        // Calculate stats
        let total_tasks = app.history.len();
        if total_tasks == 0 {
            let p = Paragraph::new("No tasks executed yet.")
                .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(p, inner_area);
            return;
        }

        let failed = app.history.iter().filter(|t| t.failed).count();
        let changed = app.history.iter().filter(|t| t.changed).count();
        let ok = total_tasks - failed - changed; // Roughly

        let failed_pct = (failed as f64 / total_tasks as f64) * 100.0;
        let changed_pct = (changed as f64 / total_tasks as f64) * 100.0;
        let ok_pct = (ok as f64 / total_tasks as f64) * 100.0;

        // Use a Gauge for visual or just text for now.
        // Let's use 3 Gauges side-by-side or stacked?
        // Side-by-side is better.
        let gauge_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ])
            .split(inner_area);

        let g_ok = Gauge::default()
            .block(Block::default().title("OK").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Green))
            .percent(ok_pct as u16);

        let g_changed = Gauge::default()
            .block(Block::default().title("Changed").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Yellow))
            .percent(changed_pct as u16);

        let g_failed = Gauge::default()
            .block(Block::default().title("Failed").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Red))
            .percent(failed_pct as u16);

        frame.render_widget(g_ok, gauge_layout[0]);
        frame.render_widget(g_changed, gauge_layout[1]);
        frame.render_widget(g_failed, gauge_layout[2]);
    }

    fn draw_task_durations(frame: &mut Frame, app: &App, area: Rect) {
        let block = Block::default()
            .title("Top Tasks by Duration")
            .borders(Borders::ALL);
        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        if app.history.is_empty() {
            return;
        }

        // Sort by duration desc
        let mut tasks: Vec<_> = app.history.iter().collect();
        tasks.sort_by(|a, b| b.duration.partial_cmp(&a.duration).unwrap());

        // Take top 5 or however many fit
        let top_tasks: Vec<_> = tasks.iter().take(10).collect();

        // Convert to BarChart data
        // BarChart expects (&str, u64). Duration is f64. Multiply by 1000 for ms?
        let data: Vec<(&str, u64)> = top_tasks
            .iter()
            .map(|t| (t.name.as_str(), (t.duration * 1000.0) as u64))
            .collect();

        // Truncate names if too long
        let display_data: Vec<(String, u64)> = data
            .iter()
            .map(|(n, v)| {
                let name = if n.len() > 15 {
                    format!("{}...", &n[0..12])
                } else {
                    n.to_string()
                };
                (name, *v)
            })
            .collect();

        let bar_data: Vec<(&str, u64)> =
            display_data.iter().map(|(n, v)| (n.as_str(), *v)).collect();

        let barchart = BarChart::default()
            .block(Block::default())
            .data(&bar_data)
            .bar_width(16)
            .bar_gap(2)
            .value_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .label_style(Style::default().fg(Color::White));

        frame.render_widget(barchart, inner_area);
    }

    fn draw_event_velocity(frame: &mut Frame, app: &App, area: Rect) {
        let block = Block::default()
            .title("Event Velocity")
            .borders(Borders::ALL);
        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        // Real Data from App
        let history: Vec<u64> = app.event_velocity.iter().cloned().collect();
        // Append current counter for live view?
        // Sparkline shows history. The current second is still accumulating in event_counter.
        // We could append it, but typically Sparkline shows completed intervals.
        // Let's just show history.

        let sparkline = Sparkline::default()
            .block(Block::default())
            .data(&history)
            .style(Style::default().fg(Color::Magenta));

        frame.render_widget(sparkline, inner_area);
    }
}
