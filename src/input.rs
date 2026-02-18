use crate::app::{
    Action, ActiveView, AnalysisFocus, App, ChatMode, DashboardFocus, EditState, LogFilter,
};
use crossterm::event::{Event, KeyCode, KeyEventKind};

impl App {
    pub fn get_flattened_vars(&self) -> Vec<String> {
        let mut keys = Vec::new();
        if let Some(vars) = &self.task_vars
            && let Some(obj) = vars.as_object()
        {
            for (k, _) in obj {
                keys.push(k.clone());
            }
        }
        if let Some(facts) = &self.facts
            && let Some(obj) = facts.as_object()
        {
            for (k, _) in obj {
                keys.push(format!("ansible_facts.{}", k));
            }
        }
        keys.sort();
        keys
    }

    pub fn get_var_value(&self, key: &str) -> Option<serde_json::Value> {
        if key.starts_with("ansible_facts.") {
            let fact_key = key.trim_start_matches("ansible_facts.");
            self.facts.as_ref().and_then(|f| f.get(fact_key).cloned())
        } else {
            self.task_vars.as_ref().and_then(|v| v.get(key).cloned())
        }
    }

    pub fn handle_event(&mut self, event: Event) -> Action {
        #[allow(clippy::collapsible_if)]
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return Action::None;
            }

            if key.code == KeyCode::Char('?') && !self.chat_active {
                self.show_help = !self.show_help;
                return Action::None;
            }

            if self.chat_active {
                return self.handle_chat_key(key);
            }

            if let action @ Action::EditVar = self.handle_var_selection_key(key) {
                return action;
            }
            if matches!(self.edit_state, EditState::SelectingVariable { .. }) {
                return Action::None;
            }

            if self.search_active {
                return self.handle_search_key(key);
            }

            if self.show_host_list {
                return self.handle_host_list_key(key);
            }

            // Tab cycling between views
            if key.code == KeyCode::Tab {
                self.active_view = match self.active_view {
                    ActiveView::Dashboard => ActiveView::Analysis,
                    ActiveView::Analysis => ActiveView::Metrics,
                    ActiveView::Metrics => ActiveView::Dashboard,
                };
                return Action::None;
            }
            if key.code == KeyCode::BackTab {
                self.active_view = match self.active_view {
                    ActiveView::Dashboard => ActiveView::Metrics,
                    ActiveView::Analysis => ActiveView::Dashboard,
                    ActiveView::Metrics => ActiveView::Analysis,
                };
                return Action::None;
            }

            if self.active_view == ActiveView::Analysis {
                return self.handle_analysis_key(key);
            }

            // Global keys
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    if self.show_help {
                        self.show_help = false;
                        return Action::None;
                    }
                    return Action::Quit;
                }
                KeyCode::Char('c') => {
                    return if self.failed_task.is_some() {
                        Action::Continue
                    } else {
                        Action::Proceed
                    };
                }
                KeyCode::Char('r') => return Action::Retry,
                KeyCode::Char('e') => {
                    self.edit_state = EditState::SelectingVariable {
                        filter: String::new(),
                        selected_index: 0,
                    };
                    return Action::None;
                }
                KeyCode::Char('a') => return Action::AskAi,
                KeyCode::Char('f') => return Action::ApplyFix,
                KeyCode::Char('F') => return Action::ToggleFollow,
                KeyCode::Char('l') => {
                    self.log_filter = match self.log_filter {
                        LogFilter::All => LogFilter::Failed,
                        LogFilter::Failed => LogFilter::Changed,
                        LogFilter::Changed => LogFilter::All,
                    };
                    return Action::ToggleFilter;
                }
                KeyCode::Char('H') => {
                    self.show_host_list = !self.show_host_list;
                    return Action::None;
                }
                KeyCode::Char('/') => {
                    self.search_active = true;
                    self.search_query.clear();
                    return Action::Search;
                }
                KeyCode::Char('n') => {
                    self.find_next_match();
                    return Action::NextMatch;
                }
                KeyCode::Char('N') => {
                    self.find_prev_match();
                    return Action::PrevMatch;
                }
                KeyCode::Char('C') => {
                    self.chat_active = !self.chat_active;
                    return Action::None;
                }
                _ => {}
            }

            if self.active_view == ActiveView::Dashboard {
                return self.handle_dashboard_key(key);
            }

            match key.code {
                KeyCode::Char('y') => return Action::Yank,
                KeyCode::Char('v') => {
                    self.active_view = if self.active_view == ActiveView::Analysis {
                        ActiveView::Dashboard
                    } else {
                        ActiveView::Analysis
                    };
                    return Action::None;
                }
                KeyCode::Char('m') => {
                    self.active_view = if self.active_view == ActiveView::Metrics {
                        ActiveView::Dashboard
                    } else {
                        ActiveView::Metrics
                    };
                    return Action::None;
                }
                _ => {}
            }
        } else if let Event::Mouse(mouse) = event {
            match mouse.kind {
                crossterm::event::MouseEventKind::ScrollDown => {
                    if self.chat_active {
                        self.chat_scroll = self.chat_scroll.saturating_add(3);
                    } else {
                        self.scroll_offset = self.scroll_offset.saturating_add(3);
                    }
                }
                crossterm::event::MouseEventKind::ScrollUp => {
                    if self.chat_active {
                        self.chat_auto_scroll = false;
                        self.chat_scroll = self.chat_scroll.saturating_sub(3);
                    } else {
                        self.scroll_offset = self.scroll_offset.saturating_sub(3);
                    }
                }
                _ => {}
            }
        }
        Action::None
    }

    fn handle_chat_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        match self.chat_mode {
            ChatMode::Insert => self.handle_chat_insert_key(key),
            ChatMode::Normal => self.handle_chat_normal_key(key),
            ChatMode::Search => self.handle_chat_search_key(key),
        }
    }

    fn handle_chat_insert_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.chat_mode = ChatMode::Normal;
                self.chat_selected_index = Some(self.chat_history.len().saturating_sub(1));
            }
            KeyCode::Enter => {
                if !self.chat_input.trim().is_empty() {
                    return Action::SubmitChat;
                }
            }
            KeyCode::Char(c) => self.chat_input.push(c),
            KeyCode::Backspace => {
                self.chat_input.pop();
            }
            KeyCode::Up if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                self.chat_auto_scroll = false;
                self.chat_scroll = self.chat_scroll.saturating_sub(1);
            }
            KeyCode::Down if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                self.chat_scroll = self.chat_scroll.saturating_add(1);
            }
            KeyCode::PageUp if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                self.chat_auto_scroll = false;
                self.chat_scroll = self.chat_scroll.saturating_sub(10);
            }
            KeyCode::PageDown if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                self.chat_scroll = self.chat_scroll.saturating_add(10);
            }
            _ => {}
        }
        Action::None
    }

    fn handle_chat_normal_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.chat_active = false;
                self.chat_mode = ChatMode::Insert;
            }
            KeyCode::Char('i') | KeyCode::Enter => {
                self.chat_mode = ChatMode::Insert;
                self.chat_selected_index = None;
            }
            KeyCode::Char('/') => {
                self.chat_mode = ChatMode::Search;
                self.chat_search_query.clear();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.chat_history.is_empty() {
                    let idx = self.chat_selected_index.unwrap_or(0);
                    if idx < self.chat_history.len() - 1 {
                        self.chat_selected_index = Some(idx + 1);
                        if (idx + 1) as u16 >= self.chat_scroll + 10 {
                            self.chat_scroll = self.chat_scroll.saturating_add(1);
                        }
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if !self.chat_history.is_empty() {
                    self.chat_auto_scroll = false;
                    let idx = self.chat_selected_index.unwrap_or(0);
                    if idx > 0 {
                        self.chat_selected_index = Some(idx - 1);
                        if (idx - 1) as u16 + 2 < self.chat_scroll {
                            self.chat_scroll = self.chat_scroll.saturating_sub(1);
                        }
                    }
                }
            }
            KeyCode::Char(' ') => {
                if let Some(idx) = self.chat_selected_index
                    && let Some(msg) = self.chat_history.get_mut(idx)
                {
                    msg.collapsed = !msg.collapsed;
                }
            }
            KeyCode::Char('y') => {
                if let Some(idx) = self.chat_selected_index
                    && let Some(msg) = self.chat_history.get(idx)
                {
                    let content = msg.content.clone();
                    self.copy_to_clipboard(content);
                }
            }
            KeyCode::PageUp => {
                self.chat_auto_scroll = false;
                self.chat_scroll = self.chat_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.chat_scroll = self.chat_scroll.saturating_add(10);
            }
            KeyCode::Home => {
                self.chat_auto_scroll = false;
                self.chat_scroll = 0;
                if !self.chat_history.is_empty() {
                    self.chat_selected_index = Some(0);
                }
            }
            KeyCode::End => {
                self.chat_auto_scroll = true;
                self.chat_scroll = self.chat_history.len().saturating_sub(1) as u16;
                if !self.chat_history.is_empty() {
                    self.chat_selected_index = Some(self.chat_history.len() - 1);
                }
            }
            _ => {}
        }
        Action::None
    }

    fn handle_chat_search_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.chat_mode = ChatMode::Normal;
                self.chat_search_query.clear();
            }
            KeyCode::Enter => {
                let query = self.chat_search_query.to_lowercase();
                if !query.is_empty() {
                    let start_idx = self.chat_selected_index.unwrap_or(0);
                    let found = self
                        .chat_history
                        .iter()
                        .enumerate()
                        .skip(start_idx + 1)
                        .find(|(_, m)| m.content.to_lowercase().contains(&query))
                        .or_else(|| {
                            self.chat_history
                                .iter()
                                .enumerate()
                                .take(start_idx + 1)
                                .find(|(_, m)| m.content.to_lowercase().contains(&query))
                        });
                    if let Some((idx, _)) = found {
                        self.chat_selected_index = Some(idx);
                        self.chat_scroll = idx.saturating_sub(5) as u16;
                    }
                }
            }
            KeyCode::Char(c) => self.chat_search_query.push(c),
            KeyCode::Backspace => {
                self.chat_search_query.pop();
            }
            _ => {}
        }
        Action::None
    }

    fn handle_var_selection_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        let mut selection_action = Action::None;

        if let EditState::SelectingVariable {
            filter,
            selected_index,
        } = &mut self.edit_state
        {
            match key.code {
                KeyCode::Esc => {}
                KeyCode::Down | KeyCode::Char('j') => {
                    *selected_index = selected_index.saturating_add(1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    *selected_index = selected_index.saturating_sub(1);
                }
                KeyCode::Char(c) => {
                    filter.push(c);
                    *selected_index = 0;
                }
                KeyCode::Backspace => {
                    filter.pop();
                    *selected_index = 0;
                }
                KeyCode::Enter => {
                    selection_action = Action::EditVar;
                }
                _ => {}
            }

            if key.code == KeyCode::Esc {
                self.edit_state = EditState::Idle;
                return Action::None;
            }
        }

        if let Action::EditVar = selection_action
            && let EditState::SelectingVariable {
                filter,
                selected_index,
            } = &self.edit_state
        {
            let all_vars = self.get_flattened_vars();
            let filtered: Vec<&String> = all_vars
                .iter()
                .filter(|v| v.to_lowercase().contains(&filter.to_lowercase()))
                .collect();

            if let Some(selected_key) = filtered.get(*selected_index % filtered.len().max(1)) {
                let key_clone = selected_key.to_string();
                if let Err(e) = self.prepare_edit(key_clone) {
                    self.notification = Some((format!("Error: {}", e), std::time::Instant::now()));
                    self.edit_state = EditState::Idle;
                    return Action::None;
                }
                return Action::EditVar;
            }
        }

        Action::None
    }

    fn handle_search_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.search_active = false;
                self.search_query.clear();
            }
            KeyCode::Enter => {
                self.search_active = false;
                let query = self.search_query.trim().to_string();
                if query.starts_with("::query::") {
                    let query_str = query.trim_start_matches("::query::").trim().to_string();
                    if !query_str.is_empty() {
                        return Action::SubmitQuery(query_str);
                    }
                    return Action::None;
                }
                if self.active_view == ActiveView::Analysis
                    && let Some(tree) = &mut self.analysis_tree
                {
                    tree.set_search(self.search_query.clone());
                }
                return Action::SubmitSearch;
            }
            KeyCode::Char(c) => self.search_query.push(c),
            KeyCode::Backspace => {
                self.search_query.pop();
            }
            _ => {}
        }
        Action::None
    }

    fn handle_host_list_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        let host_count = self.hosts.len();
        let mut sorted_hosts: Vec<String> = self.hosts.keys().cloned().collect();
        sorted_hosts.sort();

        match key.code {
            KeyCode::Esc => self.show_host_list = false,
            KeyCode::Down | KeyCode::Char('j') => {
                if host_count > 0 {
                    self.host_list_index = (self.host_list_index + 1) % host_count;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if host_count > 0 {
                    self.host_list_index = if self.host_list_index == 0 {
                        host_count - 1
                    } else {
                        self.host_list_index - 1
                    };
                }
            }
            KeyCode::Enter => {
                if host_count > 0
                    && let Some(host) = sorted_hosts.get(self.host_list_index)
                {
                    self.host_filter = Some(host.clone());
                    self.show_host_list = false;
                }
            }
            KeyCode::Char('x') => {
                self.host_filter = None;
                self.show_host_list = false;
            }
            KeyCode::Char('f') => {
                if host_count > 0
                    && let Some(host) = sorted_hosts.get(self.host_list_index)
                    && let Some(facts) = self.host_facts.get(host)
                {
                    self.active_view = ActiveView::Analysis;
                    self.analysis_focus = AnalysisFocus::DataBrowser;
                    self.analysis_tree =
                        Some(crate::widgets::json_tree::JsonTreeState::new(facts.clone()));
                    self.show_host_list = false;
                }
            }
            _ => {}
        }
        Action::None
    }

    fn handle_analysis_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        match key.code {
            KeyCode::Right if matches!(self.analysis_focus, AnalysisFocus::TaskList) => {
                self.analysis_focus = AnalysisFocus::DataBrowser;
                return Action::None;
            }
            KeyCode::Left
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                self.analysis_focus = AnalysisFocus::TaskList;
                return Action::None;
            }
            KeyCode::Right
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                self.analysis_focus = AnalysisFocus::DataBrowser;
                return Action::None;
            }
            _ => {}
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('v') => {
                if self.show_detail_view {
                    self.show_detail_view = false;
                    return Action::None;
                }
                self.active_view = ActiveView::Dashboard;
                self.analysis_focus = AnalysisFocus::TaskList;
                return Action::None;
            }
            KeyCode::Char('y') => {
                return if self.visual_mode {
                    Action::YankVisual
                } else if self.pending_count.is_some() {
                    Action::YankWithCount
                } else {
                    Action::Yank
                };
            }
            KeyCode::Char('V') => {
                if self.visual_mode {
                    self.visual_mode = false;
                    self.visual_start_index = None;
                } else {
                    self.visual_mode = true;
                    if let Some(tree) = &self.analysis_tree {
                        self.visual_start_index = Some(tree.selected_line);
                    }
                }
                return Action::None;
            }
            KeyCode::Char('w') => {
                if let Some(tree) = &mut self.analysis_tree {
                    tree.text_wrap = !tree.text_wrap;
                }
                return Action::None;
            }
            _ => match self.analysis_focus {
                AnalysisFocus::TaskList => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => return Action::AnalysisPrev,
                    KeyCode::Down | KeyCode::Char('j') => return Action::AnalysisNext,
                    KeyCode::Char('b') => return Action::ToggleBreakpoint,
                    _ => {}
                },
                AnalysisFocus::DataBrowser => {
                    if let Some(tree) = &mut self.analysis_tree {
                        match key.code {
                            KeyCode::Char(c @ '0'..='9') => {
                                let digit = c.to_digit(10).unwrap() as usize;
                                self.pending_count =
                                    Some(self.pending_count.unwrap_or(0) * 10 + digit);
                                return Action::None;
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                let count = self.pending_count.take().unwrap_or(1);
                                for _ in 0..count {
                                    tree.select_prev();
                                }
                                return Action::None;
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                let count = self.pending_count.take().unwrap_or(1);
                                for _ in 0..count {
                                    tree.select_next();
                                }
                                return Action::None;
                            }
                            KeyCode::Char('l') | KeyCode::Right => {
                                if key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::SHIFT)
                                {
                                    tree.expand_current_recursive();
                                } else {
                                    tree.expand_or_child();
                                }
                                return Action::None;
                            }
                            KeyCode::Enter | KeyCode::Char(' ') => {
                                tree.toggle_collapse();
                                return Action::None;
                            }
                            KeyCode::Left | KeyCode::Char('h') => {
                                if key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::SHIFT)
                                {
                                    tree.collapse_current_recursive();
                                } else {
                                    tree.collapse_or_parent();
                                }
                                return Action::None;
                            }
                            KeyCode::Char('/') => {
                                self.search_active = true;
                                self.search_query.clear();
                                return Action::Search;
                            }
                            KeyCode::Char('n') => {
                                tree.next_match();
                                return Action::None;
                            }
                            KeyCode::Char('N') => {
                                tree.prev_match();
                                return Action::None;
                            }
                            KeyCode::PageUp => {
                                tree.page_up();
                                return Action::None;
                            }
                            KeyCode::PageDown => {
                                tree.page_down();
                                return Action::None;
                            }
                            _ => {}
                        }
                    }
                }
            },
        }
        Action::None
    }

    fn handle_dashboard_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        match key.code {
            KeyCode::Right => {
                self.dashboard_focus = DashboardFocus::Inspector;
            }
            KeyCode::Left => {
                self.dashboard_focus = DashboardFocus::Logs;
            }
            KeyCode::Up => match self.dashboard_focus {
                DashboardFocus::Logs => {
                    self.auto_scroll = false;
                    self.log_scroll = self.log_scroll.saturating_sub(1);
                }
                DashboardFocus::Inspector => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                }
            },
            KeyCode::Down => match self.dashboard_focus {
                DashboardFocus::Logs => {
                    self.log_scroll = self.log_scroll.saturating_add(1);
                }
                DashboardFocus::Inspector => {
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                }
            },
            KeyCode::PageUp => match self.dashboard_focus {
                DashboardFocus::Logs => {
                    self.auto_scroll = false;
                    self.log_scroll = self.log_scroll.saturating_sub(10);
                }
                DashboardFocus::Inspector => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(10);
                }
            },
            KeyCode::PageDown => match self.dashboard_focus {
                DashboardFocus::Logs => {
                    self.log_scroll = self.log_scroll.saturating_add(10);
                }
                DashboardFocus::Inspector => {
                    self.scroll_offset = self.scroll_offset.saturating_add(10);
                }
            },
            _ => {}
        }
        Action::None
    }

    fn find_next_match(&mut self) {
        if self.search_query.is_empty() || self.active_view != ActiveView::Dashboard {
            return;
        }
        match self.dashboard_focus {
            DashboardFocus::Logs => {
                let start = self.search_index.unwrap_or(0);
                let q = self.search_query.to_lowercase();
                let found = self
                    .logs
                    .iter()
                    .enumerate()
                    .skip(start + 1)
                    .find(|(_, (msg, _))| msg.to_lowercase().contains(&q))
                    .or_else(|| {
                        self.logs
                            .iter()
                            .enumerate()
                            .take(start + 1)
                            .find(|(_, (msg, _))| msg.to_lowercase().contains(&q))
                    });
                if let Some((i, _)) = found {
                    self.search_index = Some(i);
                    self.log_scroll = i as u16;
                    self.auto_scroll = false;
                }
            }
            DashboardFocus::Inspector => {
                let content = self
                    .failed_result
                    .as_ref()
                    .and_then(|r| serde_json::to_string_pretty(r).ok())
                    .unwrap_or_default();
                if let Some(idx) = content
                    .to_lowercase()
                    .find(&self.search_query.to_lowercase())
                {
                    self.scroll_offset =
                        content[..idx].chars().filter(|&c| c == '\n').count() as u16;
                }
            }
        }
    }

    fn find_prev_match(&mut self) {
        if self.search_query.is_empty() || self.active_view != ActiveView::Dashboard {
            return;
        }
        if let DashboardFocus::Logs = self.dashboard_focus {
            let start = self.search_index.unwrap_or(self.logs.len());
            let q = self.search_query.to_lowercase();
            let found = (0..start)
                .rev()
                .find(|&i| {
                    self.logs
                        .get(i)
                        .map(|(msg, _)| msg.to_lowercase().contains(&q))
                        .unwrap_or(false)
                })
                .or_else(|| {
                    (start..self.logs.len()).rev().find(|&i| {
                        self.logs
                            .get(i)
                            .map(|(msg, _)| msg.to_lowercase().contains(&q))
                            .unwrap_or(false)
                    })
                });
            if let Some(i) = found {
                self.search_index = Some(i);
                self.log_scroll = if i > 5 { i as u16 - 5 } else { 0 };
            }
        }
    }
}
