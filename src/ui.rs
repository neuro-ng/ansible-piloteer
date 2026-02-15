use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, Wrap,
    },
};

use crate::widgets::json_tree::JsonTree;

pub fn draw(frame: &mut Frame, app: &mut App) {
    match app.active_view {
        crate::app::ActiveView::Metrics => {
            crate::widgets::metrics::MetricsDashboard::draw(frame, app, frame.area());
        }
        crate::app::ActiveView::Analysis => {
            draw_analysis(frame, app);
        }
        crate::app::ActiveView::Dashboard => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(frame.area());

            draw_logs(frame, app, chunks[0]);

            if app.chat_active {
                crate::widgets::chat::draw_chat(frame, app, chunks[1]);
            } else {
                draw_inspector(frame, app, chunks[1]);
            }
        }
    }

    draw_notification(frame, app);

    if app.show_host_list {
        draw_host_list(frame, app);
    }
    // Help Modal logic moved to draw_help and called in draw()

    if app.show_help {
        draw_help(frame);
    }

    // [NEW] Variable Selector Modal
    if let crate::app::EditState::SelectingVariable { .. } = &app.edit_state {
        draw_variable_selector(frame, app);
    }

    // [NEW] Phase 3: Connection Alert
    // Check moved to Status Window
}

fn draw_host_list(frame: &mut Frame, app: &mut App) {
    let area = centered_rect(60, 60, frame.area());
    let block = Block::default()
        .title("Host List (j/k: Select, Enter: Filter, f: Facts, Esc: Close)")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    // Inner area for list
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .margin(1)
        .split(area);

    let mut hosts: Vec<&crate::app::HostStatus> = app.hosts.values().collect();
    hosts.sort_by(|a, b| a.name.cmp(&b.name));

    let items: Vec<ListItem> = hosts
        .iter()
        .map(|h| {
            let status = if h.failed_tasks > 0 {
                "FAILED"
            } else if h.changed_tasks > 0 {
                "CHANGED"
            } else {
                "OK"
            };

            let style = match status {
                "FAILED" => Style::default().fg(Color::Red),
                "CHANGED" => Style::default().fg(Color::Yellow),
                _ => Style::default().fg(Color::Green),
            };

            ListItem::new(format!(
                "{:<20} | OK: {:<3} Changed: {:<3} Failed: {:<3} [{}]",
                h.name, h.ok_tasks, h.changed_tasks, h.failed_tasks, status
            ))
            .style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = ListState::default();
    state.select(Some(app.host_list_index));

    frame.render_stateful_widget(list, layout[0], &mut state);
}

fn draw_notification(frame: &mut Frame, app: &mut App) {
    if let Some((msg, time)) = &app.notification
        && time.elapsed() < std::time::Duration::from_secs(3)
    {
        let area = centered_rect(40, 10, frame.area());
        let block = Block::default()
            .title("Notification")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Blue).fg(Color::White));
        let p = Paragraph::new(msg.clone())
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(Clear, area); // Clear underlying content
        frame.render_widget(p, area);
    }
}

fn draw_analysis(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(frame.area());

    // Define focus styles
    let active_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let inactive_style = Style::default().fg(Color::DarkGray);

    let (list_border_style, tree_border_style) = match app.analysis_focus {
        crate::app::AnalysisFocus::TaskList => (active_style, inactive_style),
        crate::app::AnalysisFocus::DataBrowser => (inactive_style, active_style),
    };

    // Left Pane: Task List
    let tasks: Vec<ListItem> = app
        .history
        .iter()
        .filter(|t| {
            if let Some(filter_host) = &app.host_filter {
                &t.host == filter_host
            } else {
                true
            }
        })
        .map(|t| {
            let style = if t.failed {
                Style::default().fg(Color::Red)
            } else if t.changed {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            };
            let symbol = if t.failed {
                "❌ "
            } else if t.changed {
                "⚠️  "
            } else {
                "✅ "
            };

            let mut spans = Vec::new();
            if app.breakpoints.contains(&t.name) {
                spans.push(Span::styled("● ", Style::default().fg(Color::Red)));
            } else {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::raw(symbol));
            spans.push(Span::raw(t.name.clone()));

            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();

    let tasks_list = List::new(tasks)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(list_border_style)
                .title(if let Some(h) = &app.host_filter {
                    format!("History (Filter: {})", h)
                } else {
                    "History (Up/Down to Select)".to_string()
                }),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ");

    let mut list_state = ListState::default();
    list_state.select(Some(app.analysis_index));

    frame.render_stateful_widget(tasks_list, chunks[0], &mut list_state);

    // Right Pane: Data Browser
    let title_text = if app.search_active {
        format!("Data Browser (Search: {})", app.search_query)
    } else {
        "Data Browser (Enter to Expand, / to Search)".to_string()
    };

    let tree_block = Block::default()
        .borders(Borders::ALL)
        .border_style(tree_border_style)
        .title(title_text);

    if let Some(tree) = &mut app.analysis_tree {
        frame.render_widget(tree_block.clone(), chunks[1]);
        let inner_area = tree_block.inner(chunks[1]);
        frame.render_stateful_widget(JsonTree, inner_area, tree);
    } else {
        let p = Paragraph::new("No Data Available")
            .block(tree_block)
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(p, chunks[1]);
    }
}

fn draw_logs(frame: &mut Frame, app: &mut App, area: Rect) {
    let logs: Vec<Line> = app
        .logs
        .iter()
        .enumerate()
        .filter(|(_, (_msg, color))| match app.log_filter {
            crate::app::LogFilter::All => true,
            crate::app::LogFilter::Failed => *color == Color::Red,
            crate::app::LogFilter::Changed => *color == Color::Yellow || *color == Color::Red,
        })
        .map(|(i, (msg, color))| {
            // Check for search match
            if !app.search_query.is_empty() {
                let query = app.search_query.to_lowercase();
                let msg_lower = msg.to_lowercase();

                if msg_lower.contains(&query) {
                    let is_selected = app.search_index == Some(i);
                    // Split and highlight
                    let mut spans = Vec::new();
                    let mut last_idx = 0;

                    // Find all matches
                    for (idx, match_str) in msg_lower.match_indices(&query) {
                        // Push text before match
                        if idx > last_idx {
                            spans.push(Span::styled(
                                &msg[last_idx..idx],
                                Style::default().fg(*color),
                            ));
                        }
                        // Push match
                        let match_style = if is_selected {
                            Style::default().bg(Color::Yellow).fg(Color::Black)
                        } else {
                            Style::default().bg(Color::DarkGray).fg(Color::Yellow)
                        };
                        spans.push(Span::styled(&msg[idx..idx + match_str.len()], match_style));
                        last_idx = idx + match_str.len();
                    }
                    // Push remaining text
                    if last_idx < msg.len() {
                        spans.push(Span::styled(&msg[last_idx..], Style::default().fg(*color)));
                    }

                    Line::from(spans)
                } else {
                    Line::from(Span::styled(msg, Style::default().fg(*color)))
                }
            } else {
                Line::from(Span::styled(msg, Style::default().fg(*color)))
            }
        })
        .collect();

    let title = {
        let filter_text = match app.log_filter {
            crate::app::LogFilter::All => "",
            crate::app::LogFilter::Failed => " [FILTER: FAILED]",
            crate::app::LogFilter::Changed => " [FILTER: CHANGED]",
        };

        if app.search_active {
            format!("Ansible Logs (Search: {}){}", app.search_query, filter_text)
        } else if !app.search_query.is_empty() {
            format!(
                "Ansible Logs (Search: {} [n/N]){}",
                app.search_query, filter_text
            )
        } else {
            format!("Ansible Logs{}", filter_text)
        }
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(
            if app.active_view == crate::app::ActiveView::Dashboard
                && app.dashboard_focus == crate::app::DashboardFocus::Logs
            {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        );
    let inner_height = area.height.saturating_sub(2); // Subtract borders

    let scroll = if app.auto_scroll {
        let total_lines = logs.len() as u16;
        let s = total_lines.saturating_sub(inner_height);
        // Sync app.log_scroll so manual scrolling starts from the correct position
        app.log_scroll = s;
        s
    } else {
        app.log_scroll
    };

    let paragraph = Paragraph::new(logs)
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((scroll, 0)); // Use calculated scroll

    frame.render_widget(paragraph, area);
}

fn draw_inspector(frame: &mut Frame, app: &mut App, area: Rect) {
    // Determine layout: Status (Fixed), Variables (Min), Pilot (Fixed/Min if active)
    let constraints = if app.asking_ai || app.suggestion.is_some() {
        vec![
            Constraint::Length(6),      // Status (Increased for multi-line)
            Constraint::Min(0),         // Vars
            Constraint::Percentage(30), // Pilot
        ]
    } else {
        vec![Constraint::Length(6), Constraint::Min(0)]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Status Area
    let status_text = if !app.replay_mode && !app.is_connected() {
        // Disconnected State
        vec![
            Line::from(vec![
                Span::raw("Status: "),
                Span::styled(
                    "DISCONNECTED",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::raw("Waiting for Ansible controller to reconnect...")),
            Line::from(Span::styled(
                "(Playbook process may be restarting)",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    } else if app.failed_task.is_some() {
        let mut status_text = vec![
            Line::from(vec![
                Span::raw("Status: "),
                Span::styled(
                    "TASK FAILED",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("Task: "),
                Span::styled(
                    app.failed_task.as_deref().unwrap_or("None"),
                    Style::default().fg(Color::Red),
                ),
            ]),
            Line::from(vec![
                Span::raw("Controls: "),
                Span::styled(
                    "[r]etry [e]dit [c]ontinue [a]sk Pilot",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ];

        // Add unreachable host indicator if any
        if !app.unreachable_hosts.is_empty() {
            status_text.push(Line::from(vec![Span::styled(
                format!("⚠ {} Unreachable Host(s)", app.unreachable_hosts.len()),
                Style::default().bg(Color::Red).fg(Color::White),
            )]));
        }

        status_text
    } else if app.waiting_for_proceed {
        vec![
            Line::from(vec![
                Span::raw("Status: "),
                Span::styled(
                    "FROZEN",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("Task: "),
                Span::styled(
                    app.current_task.as_deref().unwrap_or("None"),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::raw(" v       "),
                Span::styled(
                    "Toggle Detailed Analysis / Data Browser",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::raw(" /       "),
                Span::styled(
                    "Search in Data Browser / Logs",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::raw(" n/N     "),
                Span::styled("Next/Prev Search Match", Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw(" y       "),
                Span::styled(
                    "Yank (Copy) to Clipboard",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::raw(" H       "),
                Span::styled("Toggle Host List", Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw(" Ctrl+s  "),
                Span::styled("Save Session Snapshot", Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw(" Ctrl+e  "),
                Span::styled(
                    "Export Report (Markdown)",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::raw(" ?       "),
                Span::styled("Close Help", Style::default().fg(Color::Yellow)),
            ]),
        ]
    } else {
        vec![Line::from(vec![
            Span::raw("Status: "),
            Span::styled("RUNNING", Style::default().fg(Color::Green)),
        ])]
    };

    // Calculate Drift
    let total_tasks = app.history.len();
    let changed_tasks = app.history.iter().filter(|t| t.changed).count();
    let drift_style = if changed_tasks > 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    // Add Drift Info to Status
    let mut status_lines = status_text;
    status_lines.push(Line::from(vec![
        Span::raw("Drift: "),
        Span::styled(
            format!("{} changed / {} total", changed_tasks, total_tasks),
            drift_style,
        ),
    ]));

    // Add Quota Info
    if let Some(client) = &app.ai_client {
        let (tokens, cost) = client.get_usage();
        status_lines.push(Line::from(vec![
            Span::raw("AI Quota: "),
            Span::styled(
                format!("{} tokens / ${:.4}", tokens, cost),
                Style::default().fg(Color::Cyan),
            ),
        ]));
    }

    let status_block = Block::default().borders(Borders::ALL).title("Status");

    let status_p = Paragraph::new(status_lines).block(status_block);
    frame.render_widget(status_p, chunks[0]);

    // Variables Area
    // Inspector
    let content = if let Some(err) = &app.failed_result {
        serde_json::to_string_pretty(err).unwrap_or_else(|_| "Invalid JSON".to_string())
    } else {
        "No Active Failure".to_string()
    };

    // Highlight content
    let mut highlighted_text = app.highlighter.highlight(&content, "json");

    // Apply Search Highlighting
    if !app.search_query.is_empty() {
        let query = app.search_query.to_lowercase();
        // Modify lines
        for line in &mut highlighted_text.lines {
            let line_str: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let line_lower = line_str.to_lowercase();

            let matches: Vec<(usize, usize)> = line_lower
                .match_indices(&query)
                .map(|(i, m)| (i, i + m.len()))
                .collect();

            if !matches.is_empty() {
                let mut new_spans = Vec::new();
                let mut current_offset = 0;

                for span in &line.spans {
                    let span_content = span.content.as_ref();
                    let span_len = span_content.len();
                    let span_end = current_offset + span_len;

                    let mut last_processed_in_span = 0;

                    for &(m_start, m_end) in &matches {
                        // Check for overlap
                        let overlap_start = current_offset.max(m_start);
                        let overlap_end = span_end.min(m_end);

                        if overlap_start < overlap_end {
                            // We have an overlap
                            let relative_start = overlap_start - current_offset;
                            let relative_end = overlap_end - current_offset;

                            // 1. Text before match in this span
                            if relative_start > last_processed_in_span {
                                new_spans.push(Span::styled(
                                    span_content[last_processed_in_span..relative_start]
                                        .to_string(),
                                    span.style,
                                ));
                            }

                            // 2. The Matched Text
                            // Preserve FG, Add BG
                            // Determine selection style
                            // (Simplification: We don't have line index here easily to check selected match,
                            //  but for Inspector we just highlight all matches for now or use simplified selection)
                            let match_style = span.style.bg(Color::DarkGray).fg(Color::Yellow);

                            new_spans.push(Span::styled(
                                span_content[relative_start..relative_end].to_string(),
                                match_style,
                            ));

                            last_processed_in_span = relative_end;
                        }
                    }

                    // 3. Text after all matches in this span
                    if last_processed_in_span < span_len {
                        new_spans.push(Span::styled(
                            span_content[last_processed_in_span..].to_string(),
                            span.style,
                        ));
                    }

                    current_offset += span_len;
                }
                *line = Line::from(new_spans);
            }
        }
    }

    let inspector = Paragraph::new(highlighted_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Inspector")
                .border_style(
                    if app.active_view == crate::app::ActiveView::Dashboard
                        && app.dashboard_focus == crate::app::DashboardFocus::Inspector
                    {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    frame.render_widget(inspector, chunks[1]);

    // Pilot Area (if active)
    if app.asking_ai || app.suggestion.is_some() {
        let title = if app.asking_ai {
            "Pilot (Thinking...)".to_string()
        } else if let Some(s) = &app.suggestion {
            format!("Pilot (Analysis) - {} tokens", s.tokens_used)
        } else {
            "Pilot (Analysis)".to_string()
        };

        let mut content = if app.asking_ai {
            "Contacting Pilot...".to_string()
        } else if let Some(s) = &app.suggestion {
            s.analysis.clone()
        } else {
            "".to_string()
        };

        // Append Fix Hint if available
        let mut border_style = Style::default().fg(Color::Yellow);
        if let Some(suggestion) = &app.suggestion {
            border_style = Style::default().fg(Color::Green);
            if let Some(fix) = &suggestion.fix {
                content.push_str(&format!(
                    "\n\n[PROPOSED FIX]\nKey: {}\nValue: {}\n\nPress [f] to Apply Fix",
                    fix.key, fix.value
                ));
            }
        }

        let pilot_block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);

        let pilot_p = Paragraph::new(content)
            .block(pilot_block)
            .wrap(Wrap { trim: true });

        frame.render_widget(pilot_p, chunks[2]);
    }

    // Help Modal
    if app.show_help {
        draw_help(frame);
    }

    if app.show_detail_view {
        draw_detail_view(frame, app);
    }
}

fn draw_help(frame: &mut Frame) {
    let area = centered_rect(70, 85, frame.area());
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let key_style = Style::default().fg(Color::Cyan);
    let indent_style = Style::default().fg(Color::Gray);

    let rows = vec![
        Row::new(vec![
            Cell::from(""),
            Cell::from("Global").style(header_style),
            Cell::from(""),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("q / Esc").style(key_style),
            Cell::from("Quit / Close"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("?").style(key_style),
            Cell::from("Toggle Help"),
        ]),
        Row::new(vec![Cell::from(""), Cell::from(""), Cell::from("")]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("Dashboard").style(header_style),
            Cell::from(""),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("p").style(key_style),
            Cell::from("Proceed (Step)"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("c").style(key_style),
            Cell::from("Continue (Auto)"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("l").style(key_style),
            Cell::from("Filter Logs"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("H").style(key_style),
            Cell::from("Host List"),
        ]),
        Row::new(vec![Cell::from(""), Cell::from(""), Cell::from("")]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("Analysis Mode").style(header_style),
            Cell::from(""),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("v").style(key_style),
            Cell::from("Toggle Analysis"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("Tab / Shift+Arr").style(key_style),
            Cell::from("Switch Pane"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("j / k").style(key_style),
            Cell::from("Navigate"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("Enter").style(key_style),
            Cell::from("Expand/Filtering"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("Shift+h/l").style(key_style),
            Cell::from("Deep Collapse/Expand"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("w").style(key_style),
            Cell::from("Toggle Text Wrapping"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("/").style(key_style),
            Cell::from("Search"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("n / N").style(key_style),
            Cell::from("Next/Prev Match"),
        ]),
        Row::new(vec![Cell::from(""), Cell::from(""), Cell::from("")]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("Clipboard (Data Browser)").style(header_style),
            Cell::from(""),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("0-9").style(key_style),
            Cell::from("Enter count for next command"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("v").style(key_style),
            Cell::from("Toggle visual selection mode"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("y").style(key_style),
            Cell::from("Yank to clipboard (context-aware)"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("    y").style(indent_style),
            Cell::from("  → Single line yank"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("    5y").style(indent_style),
            Cell::from("  → Yank 5 lines from current position"),
        ]),
        Row::new(vec![
            Cell::from(""),
            Cell::from("    v→5j→y").style(indent_style),
            Cell::from("  → Visual select 5 lines down, then yank"),
        ]),
    ];

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(15), // Padding for centering
            Constraint::Percentage(25), // Key
            Constraint::Percentage(60), // Action
        ],
    )
    .block(
        Block::default()
            .title("Help")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Blue).fg(Color::White)),
    )
    .header(
        Row::new(vec!["", "Key", "Action"]).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    );

    frame.render_widget(Clear, area);
    frame.render_widget(table, area);
}
// Helper for centering the modal
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_detail_view(frame: &mut Frame, app: &mut App) {
    if let Some(tree) = &app.analysis_tree
        && tree.selected_line < tree.lines.len()
    {
        let line = &tree.lines[tree.selected_line];
        let area = centered_rect(60, 60, frame.area());
        let block = Block::default()
            .title("Detail View (Esc/w to Close)")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black));

        frame.render_widget(Clear, area);
        frame.render_widget(block.clone(), area);

        let inner_area = block.inner(area);

        let content = if let Some(key) = &line.key {
            format!("{}:\n{}", key, line.value_str)
        } else {
            line.value_str.clone()
        };

        let p = Paragraph::new(content)
            .wrap(Wrap { trim: false })
            .scroll((0, 0));

        frame.render_widget(p, inner_area);
    }
}

fn draw_variable_selector(frame: &mut Frame, app: &mut App) {
    if let crate::app::EditState::SelectingVariable {
        filter,
        selected_index,
    } = &app.edit_state
    {
        let area = centered_rect(60, 60, frame.area());

        let block = Block::default()
            .title("Select Variable to Edit (Enter: Select, Esc: Cancel)")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black));

        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .margin(1)
            .split(area);

        // Filter Input
        let filter_p = Paragraph::new(format!("Filter: {}", filter))
            .block(Block::default().borders(Borders::ALL).title("Search"))
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(filter_p, chunks[0]);

        // Variable List
        let all_vars = app.get_flattened_vars();
        let filtered_vars: Vec<&String> = all_vars
            .iter()
            .filter(|v| v.to_lowercase().contains(&filter.to_lowercase()))
            .collect();

        let items: Vec<ListItem> = filtered_vars
            .iter()
            .map(|v| ListItem::new(Line::from(vec![Span::raw(*v)])))
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Variables"))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut state = ListState::default();
        // Ensure index is valid
        let safe_index = if filtered_vars.is_empty() {
            0
        } else {
            *selected_index % filtered_vars.len()
        };
        state.select(Some(safe_index));

        frame.render_stateful_widget(list, chunks[1], &mut state);
    }
}
