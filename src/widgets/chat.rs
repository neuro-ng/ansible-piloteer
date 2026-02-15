use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn draw_chat(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Stats panel
            Constraint::Min(0),    // Messages
            Constraint::Length(3), // Input box
        ])
        .split(area);

    let stats_area = chunks[0];
    let messages_area = chunks[1];
    let input_area = chunks[2];

    // ─── Stats Panel ───
    {
        let mut stats_spans = Vec::new();

        if let Some(client) = &app.ai_client {
            let model_name = client.get_model();
            stats_spans.push(Span::styled(
                format!(" Model: {} ", model_name),
                Style::default().fg(Color::Cyan),
            ));
            stats_spans.push(Span::raw(" │ "));

            let status = client.get_quota_status();
            let reset_hours = status.reset_in.as_secs() / 3600;
            let reset_mins = (status.reset_in.as_secs() % 3600) / 60;

            if let Some(limit) = status.limit_tokens {
                let pct = (status.used_tokens as f64 / limit as f64) * 100.0;
                let bar_width: usize = 10;
                let filled = ((pct / 100.0) * bar_width as f64).round() as usize;
                let empty = bar_width.saturating_sub(filled);
                let bar_color = if pct > 80.0 {
                    Color::Red
                } else if pct > 50.0 {
                    Color::Yellow
                } else {
                    Color::Green
                };

                stats_spans.push(Span::styled(
                    format!("Tokens: {}/{} ", status.used_tokens, limit),
                    Style::default().fg(Color::White),
                ));
                stats_spans.push(Span::styled(
                    "█".repeat(filled),
                    Style::default().fg(bar_color),
                ));
                stats_spans.push(Span::styled(
                    "░".repeat(empty),
                    Style::default().fg(Color::DarkGray),
                ));
                stats_spans.push(Span::styled(
                    format!(" {:.0}%", pct),
                    Style::default().fg(bar_color),
                ));
            } else {
                stats_spans.push(Span::styled(
                    format!("Tokens: {} ", status.used_tokens),
                    Style::default().fg(Color::White),
                ));
            }

            stats_spans.push(Span::raw(" │ "));
            stats_spans.push(Span::styled(
                format!("Reset: {}h{}m ", reset_hours, reset_mins),
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            stats_spans.push(Span::styled(
                " AI Client: Not Configured ",
                Style::default().fg(Color::Red),
            ));
        }

        let stats_block = Block::default()
            .borders(Borders::ALL)
            .title("AI Stats")
            .border_style(Style::default().fg(Color::DarkGray));

        let stats_paragraph = Paragraph::new(Line::from(stats_spans)).block(stats_block);
        frame.render_widget(stats_paragraph, stats_area);
    }

    let mut list_items = Vec::new();

    // Iterate history
    for (i, msg) in app.chat_history.iter().enumerate() {
        let is_selected =
            app.chat_mode != crate::app::ChatMode::Insert && app.chat_selected_index == Some(i);

        let (role_style, prefix) = match msg.role.as_str() {
            "user" => (Style::default().fg(Color::Cyan), "You"),
            "assistant" => (Style::default().fg(Color::Green), "AI"),
            "system" => (Style::default().fg(Color::Yellow), "System"),
            _ => (Style::default().fg(Color::Gray), "Unknown"),
        };

        // Header Line
        let mut header_spans = vec![Span::styled(format!("{}: ", prefix), role_style)];
        if msg.collapsed {
            header_spans.push(Span::styled(
                " [Collapsed] ",
                Style::default().fg(Color::DarkGray),
            ));
        }
        if is_selected {
            header_spans.push(Span::styled(
                " [SELECTED] ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ));
        }

        list_items.push(Line::from(header_spans));

        if !msg.collapsed {
            for line in msg.content.lines() {
                // Search Highlighting
                if !app.chat_search_query.is_empty()
                    && app.chat_mode == crate::app::ChatMode::Search
                {
                    let lower_line = line.to_lowercase();
                    let query = app.chat_search_query.to_lowercase();
                    let mut start = 0;
                    let mut spans = Vec::new();

                    while let Some(pos) = lower_line[start..].find(&query) {
                        let actual_pos = start + pos;
                        // Before match
                        if actual_pos > start {
                            spans.push(Span::raw(&line[start..actual_pos]));
                        }
                        // Match
                        spans.push(Span::styled(
                            &line[actual_pos..actual_pos + query.len()],
                            Style::default().bg(Color::Yellow).fg(Color::Black),
                        ));
                        start = actual_pos + query.len();
                    }
                    // Remaining
                    if start < line.len() {
                        spans.push(Span::raw(&line[start..]));
                    }
                    if spans.is_empty() {
                        // No match
                        spans.push(Span::raw(line));
                    }
                    list_items.push(Line::from(spans));
                } else {
                    list_items.push(Line::from(vec![Span::raw(line)]));
                }
            }
        }
        list_items.push(Line::from("")); // Spacer
    }

    let title = match app.chat_mode {
        crate::app::ChatMode::Insert => {
            "Chat History (Insert Mode — Alt+↑/↓ to scroll)".to_string()
        }
        crate::app::ChatMode::Normal => {
            "Chat History (Normal Mode: j/k scroll, Enter expand/collapse)".to_string()
        }
        crate::app::ChatMode::Search => format!("Chat Search: {}", app.chat_search_query),
    };

    let messages_block = Block::default().borders(Borders::ALL).title(title);

    // [NEW] Phase 34: Auto-scroll support
    let total_lines = list_items.len() as u16;
    let visible_height = messages_area.height.saturating_sub(2); // account for borders

    if app.chat_auto_scroll && total_lines > visible_height {
        app.chat_scroll = total_lines.saturating_sub(visible_height);
    }

    // Clamp scroll to valid range
    let max_scroll = total_lines.saturating_sub(visible_height);
    if app.chat_scroll > max_scroll {
        app.chat_scroll = max_scroll;
    }

    let messages_paragraph = Paragraph::new(list_items)
        .block(messages_block)
        .wrap(Wrap { trim: false })
        .scroll((app.chat_scroll, 0));

    frame.render_widget(messages_paragraph, messages_area);

    // [NEW] Phase 34: Scroll indicator
    if total_lines > visible_height {
        let at_bottom = app.chat_scroll >= max_scroll;
        let at_top = app.chat_scroll == 0;

        if !at_bottom {
            let indicator = Span::styled(
                " ↓ more ",
                Style::default().fg(Color::Yellow).bg(Color::DarkGray),
            );
            let indicator_x = messages_area.right().saturating_sub(10);
            let indicator_y = messages_area.bottom().saturating_sub(1);
            if indicator_x > messages_area.x && indicator_y > messages_area.y {
                frame.render_widget(
                    Paragraph::new(Line::from(indicator)),
                    Rect::new(indicator_x, indicator_y, 8, 1),
                );
            }
        }
        if !at_top {
            let indicator = Span::styled(
                " ↑ more ",
                Style::default().fg(Color::Cyan).bg(Color::DarkGray),
            );
            let indicator_x = messages_area.right().saturating_sub(10);
            let indicator_y = messages_area.y;
            if indicator_x > messages_area.x {
                frame.render_widget(
                    Paragraph::new(Line::from(indicator)),
                    Rect::new(indicator_x, indicator_y, 8, 1),
                );
            }
        }
    }

    // Draw Input
    let (input_border_style, title) = if app.chat_loading {
        (Style::default().fg(Color::Yellow), "AI is thinking...")
    } else if app.chat_mode == crate::app::ChatMode::Search {
        (
            Style::default().fg(Color::Blue),
            "Search Query (Enter to find, Esc to cancel)",
        )
    } else {
        match app.chat_mode {
            crate::app::ChatMode::Insert => (
                Style::default().fg(Color::White),
                "Message (Enter to send, Esc to Normal Mode)",
            ),
            crate::app::ChatMode::Normal => (
                Style::default().fg(Color::Green),
                "Normal Mode (i to Insert, / to Search)",
            ),
            _ => (Style::default().fg(Color::White), ""),
        }
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(input_border_style);

    let input_text = if app.chat_mode == crate::app::ChatMode::Search {
        &app.chat_search_query
    } else {
        &app.chat_input
    };

    let input_paragraph = Paragraph::new(input_text.as_str())
        .block(input_block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(input_paragraph, input_area);

    // Set cursor
    if app.chat_mode == crate::app::ChatMode::Insert
        || app.chat_mode == crate::app::ChatMode::Search
    {
        let cursor_x = input_area.x + 1 + input_text.len() as u16;
        let cursor_y = input_area.y + 1;
        let cursor_x = cursor_x.min(input_area.right() - 1);
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
