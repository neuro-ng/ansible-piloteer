use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::StatefulWidget;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct JsonTreeState {
    pub value: serde_json::Value,
    pub collapsed_paths: HashSet<String>,
    pub selected_line: usize,
    pub scroll_offset: usize,
    // Flattened lines cache
    pub lines: Vec<JsonLine>,
    // Search
    pub search_query: String,
    pub matched_lines: Vec<usize>,
    pub current_match_index: Option<usize>,
    pub height: usize,
    pub text_wrap: bool, // [NEW]
}

#[derive(Debug, Clone)]
pub struct JsonLine {
    pub path: String,
    pub depth: usize,
    pub key: Option<String>,
    pub value_str: String,
    pub is_collapsible: bool,
    pub is_expanded: bool,
    pub index_in_full: usize, // Original index if we weren't filtering? No, just path.
}

impl JsonTreeState {
    pub fn new(value: serde_json::Value) -> Self {
        let mut state = Self {
            value,
            collapsed_paths: HashSet::new(),
            selected_line: 0,
            scroll_offset: 0,
            lines: Vec::new(),
            search_query: String::new(),
            matched_lines: Vec::new(),
            current_match_index: None,
            height: 0,
            text_wrap: false,
        };
        state.recalc_lines();
        state
    }

    pub fn recalc_lines(&mut self) {
        self.lines.clear();
        let val = self.value.clone(); // Clone to avoid borrow issues
        self.flatten_value(&val, String::new(), 0, None);

        // If we have a search query, re-run search logic
        if !self.search_query.is_empty() {
            self.perform_search();
        }
    }

    fn flatten_value(
        &mut self,
        val: &serde_json::Value,
        path: String,
        depth: usize,
        key: Option<String>,
    ) {
        let is_collapsible = val.is_object() || val.is_array();
        let is_expanded = !self.collapsed_paths.contains(&path);

        // Format value string
        // For collapsible, show { ... } or [ ... ] if collapsed
        // If expanded, show opening brace/bracket
        let value_str = if is_collapsible {
            if is_expanded {
                if val.is_object() {
                    "{".to_string()
                } else {
                    "[".to_string()
                }
            } else if val.is_object() {
                "{ ... }".to_string()
            } else {
                "[ ... ]".to_string()
            }
        } else {
            // Primitive
            match val {
                serde_json::Value::Null => "null".to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => format!("\"{}\"", s), // Quote strings?
                _ => "".to_string(),
            }
        };

        self.lines.push(JsonLine {
            path: path.clone(),
            depth,
            key: key.clone(),
            value_str,
            is_collapsible,
            is_expanded,
            index_in_full: 0, // unused
        });

        if is_collapsible && is_expanded {
            match val {
                serde_json::Value::Object(map) => {
                    for (k, v) in map {
                        let new_path = if path.is_empty() {
                            k.clone()
                        } else {
                            format!("{}.{}", path, k)
                        };
                        self.flatten_value(v, new_path, depth + 1, Some(format!("\"{}\"", k)));
                    }
                    // Closing brace
                    self.lines.push(JsonLine {
                        path: format!("{}.}}", path), // Hacky path for closing
                        depth,
                        key: None,
                        value_str: "}".to_string(),
                        is_collapsible: false,
                        is_expanded: false,
                        index_in_full: 0,
                    });
                }
                serde_json::Value::Array(arr) => {
                    for (i, v) in arr.iter().enumerate() {
                        let new_path = if path.is_empty() {
                            format!("[{}]", i)
                        } else {
                            format!("{}[{}]", path, i)
                        };
                        self.flatten_value(v, new_path, depth + 1, None); // Array items have no key
                    }
                    // Closing bracket
                    self.lines.push(JsonLine {
                        path: format!("{}.]", path),
                        depth,
                        key: None,
                        value_str: "]".to_string(),
                        is_collapsible: false,
                        is_expanded: false,
                        index_in_full: 0,
                    });
                }
                _ => {}
            }
        }
    }

    pub fn expand_all(&mut self) {
        self.collapsed_paths.clear();
        self.recalc_lines();
    }

    pub fn collapse_all(&mut self) {
        // Collect all paths that are objects/arrays
        let val = self.value.clone();
        self.collect_collapsible_paths(&val, String::new());
        self.recalc_lines();
    }

    fn collect_collapsible_paths(&mut self, val: &serde_json::Value, path: String) {
        if (val.is_object() || val.is_array()) && !path.is_empty() {
            self.collapsed_paths.insert(path.clone());
        }

        match val {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    let new_path = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{}.{}", path, k)
                    };
                    self.collect_collapsible_paths(v, new_path);
                }
            }
            serde_json::Value::Array(arr) => {
                for (i, v) in arr.iter().enumerate() {
                    let new_path = if path.is_empty() {
                        format!("[{}]", i)
                    } else {
                        format!("{}[{}]", path, i)
                    };
                    self.collect_collapsible_paths(v, new_path);
                }
            }
            _ => {}
        }
    }

    pub fn expand_current_recursive(&mut self) {
        if self.selected_line < self.lines.len() {
            let line = &self.lines[self.selected_line].clone();
            // Remove current path and all children from collapsed_paths
            let path_prefix = &line.path;
            self.collapsed_paths.retain(|p| !p.starts_with(path_prefix));
            self.recalc_lines();
        }
    }

    pub fn collapse_current_recursive(&mut self) {
        if self.selected_line < self.lines.len() {
            let line = &self.lines[self.selected_line].clone();
            // Re-traverse from this value to find all paths to collapse
            // We need to find the value at this path first.
            // Better: just add all child paths of current line.path to collapsed
            // This requires knowing the structure or re-traversing.
            // Simplified: Traverse whole value, if path starts with current path, collapse it.
            let val = self.value.clone();
            self.collect_collapsible_paths_under(&val, String::new(), &line.path);
            self.recalc_lines();
        }
    }

    fn collect_collapsible_paths_under(
        &mut self,
        val: &serde_json::Value,
        current_path: String,
        target_prefix: &str,
    ) {
        if (val.is_object() || val.is_array()) && current_path.starts_with(target_prefix) {
            self.collapsed_paths.insert(current_path.clone());
        }

        match val {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    let new_path = if current_path.is_empty() {
                        k.clone()
                    } else {
                        format!("{}.{}", current_path, k)
                    };
                    self.collect_collapsible_paths_under(v, new_path, target_prefix);
                }
            }
            serde_json::Value::Array(arr) => {
                for (i, v) in arr.iter().enumerate() {
                    let new_path = if current_path.is_empty() {
                        format!("[{}]", i)
                    } else {
                        format!("{}[{}]", current_path, i)
                    };
                    self.collect_collapsible_paths_under(v, new_path, target_prefix);
                }
            }
            _ => {}
        }
    }

    pub fn toggle_collapse(&mut self) {
        if self.selected_line < self.lines.len() {
            let line = &self.lines[self.selected_line];
            if line.is_collapsible {
                if self.collapsed_paths.contains(&line.path) {
                    self.collapsed_paths.remove(&line.path);
                } else {
                    self.collapsed_paths.insert(line.path.clone());
                }
                self.recalc_lines();
            }
        }
    }

    pub fn select_next(&mut self) {
        if self.selected_line < self.lines.len().saturating_sub(1) {
            self.selected_line += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected_line > 0 {
            self.selected_line -= 1;
        }
    }

    pub fn set_search(&mut self, query: String) {
        self.search_query = query;
        self.perform_search();
    }

    pub fn perform_search(&mut self) {
        self.matched_lines.clear();
        self.current_match_index = None;
        if self.search_query.is_empty() {
            return;
        }

        let query = self.search_query.to_lowercase();
        for (i, line) in self.lines.iter().enumerate() {
            if line.value_str.to_lowercase().contains(&query)
                || line
                    .key
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&query)
            {
                self.matched_lines.push(i);
            }
        }

        if !self.matched_lines.is_empty() {
            self.current_match_index = Some(0);
            // Jump to first match?
            self.selected_line = self.matched_lines[0];
        }
    }

    pub fn next_match(&mut self) {
        if self.matched_lines.is_empty() {
            return;
        }
        if let Some(curr) = self.current_match_index {
            let next = (curr + 1) % self.matched_lines.len();
            self.current_match_index = Some(next);
            self.selected_line = self.matched_lines[next];
        }
    }

    pub fn prev_match(&mut self) {
        if self.matched_lines.is_empty() {
            return;
        }
        if let Some(curr) = self.current_match_index {
            let prev = if curr == 0 {
                self.matched_lines.len() - 1
            } else {
                curr - 1
            };
            self.current_match_index = Some(prev);
            self.selected_line = self.matched_lines[prev];
        }
    }
    pub fn page_up(&mut self) {
        // Page size based on current height or default
        let page_size = self.height.saturating_sub(2).max(1);
        if self.selected_line >= page_size {
            self.selected_line -= page_size;
        } else {
            self.selected_line = 0;
        }
    }

    pub fn page_down(&mut self) {
        let page_size = self.height.saturating_sub(2).max(1);
        let max_idx = self.lines.len().saturating_sub(1);
        if self.selected_line + page_size <= max_idx {
            self.selected_line += page_size;
        } else {
            self.selected_line = max_idx;
        }
    }
    pub fn collapse_or_parent(&mut self) {
        if let Some(line) = self.lines.get(self.selected_line) {
            if line.is_collapsible && line.is_expanded {
                self.collapsed_paths.insert(line.path.clone());
                self.recalc_lines();
            } else {
                // Find parent: scan backwards for depth < current.depth
                let current_depth = line.depth;
                if current_depth > 0 {
                    for i in (0..self.selected_line).rev() {
                        if self.lines[i].depth < current_depth {
                            self.selected_line = i;
                            break;
                        }
                    }
                }
            }
        }
    }

    pub fn expand_or_child(&mut self) {
        if let Some(line) = self.lines.get(self.selected_line) {
            if line.is_collapsible && !line.is_expanded {
                self.collapsed_paths.remove(&line.path);
                self.recalc_lines();
            } else {
                // Move down if possible
                self.select_next();
            }
        }
    }

    pub fn get_selected_content(&self) -> Option<String> {
        self.lines.get(self.selected_line).map(|line| {
            if let Some(key) = &line.key {
                format!("{}: {}", key, line.value_str)
            } else {
                line.value_str.clone()
            }
        })
    }

    pub fn get_selected_path(&self) -> Option<String> {
        self.lines
            .get(self.selected_line)
            .map(|line| line.path.clone())
    }

    /// Get content for a range of lines (Phase 16: Multi-Line Copy)
    pub fn get_range_content(&self, start: usize, end: usize) -> Option<String> {
        let start_idx = start.min(end);
        let end_idx = start.max(end);

        if end_idx >= self.lines.len() {
            return None;
        }

        let mut result = Vec::new();
        for i in start_idx..=end_idx {
            if let Some(line) = self.lines.get(i) {
                let content = if let Some(key) = &line.key {
                    format!("{}: {}", key, line.value_str)
                } else {
                    line.value_str.clone()
                };
                result.push(content);
            }
        }

        Some(result.join("\n"))
    }

    /// Get content from current line + count lines down (Phase 16: Multi-Line Copy)
    pub fn get_content_with_count(&self, count: usize) -> Option<String> {
        let start = self.selected_line;
        let end = (self.selected_line + count).min(self.lines.len().saturating_sub(1));
        self.get_range_content(start, end)
    }
}

pub struct JsonTree;

impl StatefulWidget for JsonTree {
    type State = JsonTreeState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let height = area.height as usize;
        state.height = height; // Track height for paging logic

        // Ensure selection is visible
        if state.selected_line >= state.scroll_offset + height {
            state.scroll_offset = state.selected_line - height + 1;
        } else if state.selected_line < state.scroll_offset {
            state.scroll_offset = state.selected_line;
        }

        // Calculate gutter width
        let total_lines = state.lines.len();
        let gutter_width = total_lines.to_string().len() + 1; // +1 for padding

        let mut current_y = 0; // Track current display row

        for line_idx in state.scroll_offset..total_lines {
            if current_y >= height {
                break; // No more room
            }

            let line = &state.lines[line_idx];

            let search_query_lower = state.search_query.to_lowercase();
            let is_searching = !state.search_query.is_empty();

            // Build the base spans (line number, indent, collapser, key)
            let mut base_spans = Vec::new();

            // Line Number (only on first row of this line)
            let line_num_str = format!("{:>width$} ", line_idx + 1, width = gutter_width - 1);
            base_spans.push(Span::styled(
                line_num_str.clone(),
                Style::default().fg(Color::DarkGray),
            ));

            // Indent
            let indent = "  ".repeat(line.depth);
            base_spans.push(Span::raw(indent.clone()));

            // Collapser
            if line.is_collapsible {
                base_spans.push(Span::styled(
                    if line.is_expanded { "[-] " } else { "[+] " },
                    Style::default().fg(Color::DarkGray),
                ));
            } else {
                base_spans.push(Span::raw("    "));
            }

            // Key
            let key_str = if let Some(key) = &line.key {
                let mut key_style = Style::default().fg(Color::Blue);
                if is_searching && key.to_lowercase().contains(&search_query_lower) {
                    key_style = key_style.add_modifier(Modifier::BOLD).bg(Color::DarkGray);
                }
                Some((format!("{}: ", key), key_style))
            } else {
                None
            };

            // Calculate available width for value
            let used_width = gutter_width + (line.depth * 2) + 4; // gutter + indent + collapser
            let key_width = key_str.as_ref().map(|(s, _)| s.len()).unwrap_or(0);
            let available_width = (area.width as usize).saturating_sub(used_width + key_width);

            // Value style
            let mut val_style = if line.is_collapsible {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            };
            if is_searching && line.value_str.to_lowercase().contains(&search_query_lower) {
                val_style = val_style.bg(Color::DarkGray);
            }

            // Determine if we need to wrap
            let value_lines: Vec<String> = if state.text_wrap
                && line.value_str.len() > available_width
            {
                // Split value into chunks that fit
                let mut chunks = Vec::new();
                let mut remaining = line.value_str.as_str();
                while !remaining.is_empty() {
                    let chunk_len = remaining.len().min(available_width);
                    chunks.push(remaining[..chunk_len].to_string());
                    remaining = &remaining[chunk_len..];
                }
                chunks
            } else {
                // No wrapping - truncate if needed
                if !state.text_wrap && line.value_str.len() > available_width {
                    if available_width > 3 {
                        vec![format!(
                            "{}...",
                            &line.value_str[..available_width.saturating_sub(3)]
                        )]
                    } else {
                        vec![
                            line.value_str[..available_width.min(line.value_str.len())].to_string(),
                        ]
                    }
                } else {
                    vec![line.value_str.clone()]
                }
            };

            // Line style for selection/highlighting
            let mut line_style = Style::default();
            if line_idx == state.selected_line {
                line_style = line_style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
            }
            if !state.search_query.is_empty() && state.matched_lines.contains(&line_idx) {
                line_style = line_style.bg(Color::Red);
                if line_idx == state.selected_line {
                    line_style = line_style.bg(Color::Magenta);
                }
            }

            // Render each wrapped line
            for (wrap_idx, value_chunk) in value_lines.iter().enumerate() {
                if current_y >= height {
                    break;
                }

                let y = area.y + current_y as u16;
                let mut spans = Vec::new();

                if wrap_idx == 0 {
                    // First line: show line number, indent, collapser, key
                    spans.extend(base_spans.clone());
                    if let Some((key_text, key_style)) = &key_str {
                        spans.push(Span::styled(key_text.clone(), *key_style));
                    }
                } else {
                    // Continuation line: blank line number, same indent
                    let blank_num = " ".repeat(gutter_width);
                    spans.push(Span::raw(blank_num));
                    spans.push(Span::raw(indent.clone()));
                    spans.push(Span::raw("    ")); // Blank collapser space
                    if key_str.is_some() {
                        spans.push(Span::raw(" ".repeat(key_width))); // Blank key space
                    }
                }

                spans.push(Span::styled(value_chunk.clone(), val_style));

                buf.set_line(area.x, y, &Line::from(spans), area.width);

                // Apply line style to the full width
                if let Some(bg) = line_style.bg {
                    for x_pos in area.x..area.x + area.width {
                        if let Some(cell) = buf.cell_mut((x_pos, y)) {
                            cell.set_bg(bg);
                        }
                    }
                }

                current_y += 1;
            }
        }
    }
}
