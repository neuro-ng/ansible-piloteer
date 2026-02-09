use crate::ipc::Message;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use std::collections::VecDeque;
use tokio::sync::mpsc;

pub enum Action {
    Quit,
    Proceed,
    Retry,
    EditVar,
    AskAi,
    ApplyFix,
    Continue,
    Search,       // [NEW]
    SubmitSearch, // [NEW]
    NextMatch,    // [NEW]
    PrevMatch,    // [NEW]
    ToggleFollow,
    ToggleFilter,   // [NEW]
    ToggleAnalysis, // [NEW] - Enter/Exit Data Browser
    AnalysisNext,
    AnalysisPrev,
    Yank,
    SaveSession,
    ExportReport, // [NEW]
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogFilter {
    All,
    Failed,
    Changed,
} // [NEW]

use crate::ai::{AiClient, Analysis};
use crate::clipboard::ClipboardHandler;
use crate::highlight::SyntaxHighlighter; // [NEW] // [MODIFY] // [NEW]

pub struct App {
    pub running: bool,
    pub logs: VecDeque<(String, ratatui::style::Color)>,
    pub current_task: Option<String>,
    pub task_vars: Option<serde_json::Value>,
    pub facts: Option<serde_json::Value>, // [NEW]
    pub failed_task: Option<String>,
    pub failed_result: Option<serde_json::Value>,
    pub waiting_for_proceed: bool,
    pub ipc_tx: Option<mpsc::Sender<Message>>,
    pub ai_client: Option<AiClient>,
    pub suggestion: Option<Analysis>,
    pub asking_ai: bool,
    pub show_help: bool,
    pub scroll_offset: u16,
    pub highlighter: SyntaxHighlighter,
    pub history: Vec<TaskHistory>,
    pub search_query: String,
    pub search_active: bool,
    pub search_index: Option<usize>,
    pub log_scroll: u16,
    pub auto_scroll: bool,
    pub log_filter: LogFilter, // [NEW]
    pub play_recap: Option<serde_json::Value>,
    // Analysis Mode State
    pub show_analysis: bool,
    pub analysis_index: usize,
    pub analysis_focus: AnalysisFocus, // [NEW]
    pub analysis_tree: Option<crate::widgets::json_tree::JsonTreeState>,
    pub clipboard: ClipboardHandler,                        // [NEW]
    pub notification: Option<(String, std::time::Instant)>, // [NEW]
    pub replay_mode: bool,                                  // [NEW]
    // Phase 18 fields
    pub host_facts: std::collections::HashMap<String, serde_json::Value>,
    pub host_filter: Option<String>,
    pub show_host_list: bool,
    pub hosts: std::collections::HashMap<String, HostStatus>,
    pub host_list_index: usize,
    pub show_detail_view: bool,
    pub unreachable_hosts: std::collections::HashSet<String>, // [NEW]
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HostStatus {
    pub name: String,
    pub ok_tasks: usize,
    pub changed_tasks: usize,
    pub failed_tasks: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnalysisFocus {
    TaskList,
    DataBrowser,
} // [NEW]

use crate::config::Config;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskHistory {
    pub name: String,
    pub host: String,
    pub changed: bool,
    pub failed: bool,
    pub error: Option<String>,
    pub verbose_result: Option<crate::execution::ExecutionDetails>,
    pub analysis: Option<crate::ai::Analysis>, // [NEW]
}

impl App {
    pub fn new(config: Config) -> Self {
        // Enable AI if Key is set OR if using a custom Base URL (e.g. Local LLM)
        let enable_ai =
            config.openai_api_key.is_some() || config.api_base != "https://api.openai.com/v1";

        let ai_client = if enable_ai {
            Some(AiClient::new(config.clone()))
        } else {
            None
        };

        Self {
            running: true,
            logs: VecDeque::new(),
            history: Vec::new(), // [NEW]
            current_task: None,
            task_vars: None,
            facts: None,
            failed_task: None,
            failed_result: None,
            waiting_for_proceed: false,
            ipc_tx: None,
            ai_client,
            suggestion: None,
            asking_ai: false,
            show_help: false,
            scroll_offset: 0,
            highlighter: SyntaxHighlighter::new(),
            search_query: String::new(),
            search_active: false,
            search_index: None,
            log_scroll: 0,
            auto_scroll: true,
            log_filter: LogFilter::All,
            play_recap: None,
            show_analysis: false,
            analysis_index: 0,
            analysis_focus: AnalysisFocus::TaskList, // [NEW]
            analysis_tree: None,
            clipboard: ClipboardHandler::new(), // [NEW]
            notification: None,                 // [NEW]
            replay_mode: false,                 // [NEW]
            // Phase 18: Host Targeting
            host_facts: std::collections::HashMap::new(),
            host_filter: None,
            show_host_list: false,
            hosts: std::collections::HashMap::new(),
            host_list_index: 0,
            show_detail_view: false,
            unreachable_hosts: std::collections::HashSet::new(),
        }
    }

    pub fn set_ipc_tx(&mut self, tx: mpsc::Sender<Message>) {
        self.ipc_tx = Some(tx);
    }

    pub fn handle_event(&mut self, event: Event) -> Action {
        #[allow(clippy::collapsible_if)]
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                // Toggle help
                if key.code == KeyCode::Char('?') {
                    self.show_help = !self.show_help;
                    return Action::None;
                }

                // If search is active, handle input
                if self.search_active {
                    match key.code {
                        KeyCode::Esc => {
                            self.search_active = false;
                            self.search_query.clear();
                            return Action::None;
                        }
                        KeyCode::Enter => {
                            self.search_active = false;
                            // Trigger search
                            if self.show_analysis {
                                if let Some(tree) = &mut self.analysis_tree {
                                    tree.set_search(self.search_query.clone());
                                }
                            } else {
                                self.find_next_match();
                            }
                            return Action::SubmitSearch;
                        }
                        KeyCode::Char(c) => {
                            self.search_query.push(c);
                            return Action::None;
                        }
                        KeyCode::Backspace => {
                            self.search_query.pop();
                            return Action::None;
                        }
                        _ => return Action::None,
                    }
                }

                if self.show_host_list {
                    let host_count = self.hosts.len();
                    let mut sorted_hosts: Vec<String> = self.hosts.keys().cloned().collect();
                    sorted_hosts.sort();

                    match key.code {
                        KeyCode::Esc => {
                            self.show_host_list = false;
                            return Action::None;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if host_count > 0 {
                                self.host_list_index = (self.host_list_index + 1) % host_count;
                            }
                            return Action::None;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if host_count > 0 {
                                if self.host_list_index == 0 {
                                    self.host_list_index = host_count - 1;
                                } else {
                                    self.host_list_index -= 1;
                                }
                            }
                            return Action::None;
                        }
                        KeyCode::Enter => {
                            if host_count > 0 {
                                if let Some(host) = sorted_hosts.get(self.host_list_index) {
                                    self.host_filter = Some(host.clone());
                                    self.show_host_list = false;
                                }
                            }
                            return Action::None;
                        }
                        KeyCode::Char('x') => {
                            self.host_filter = None;
                            self.show_host_list = false;
                            return Action::None;
                        }
                        KeyCode::Char('f') => {
                            if host_count > 0 {
                                if let Some(host) = sorted_hosts.get(self.host_list_index) {
                                    if let Some(facts) = self.host_facts.get(host) {
                                        // Load facts into Data Browser
                                        self.show_analysis = true;
                                        self.analysis_focus = AnalysisFocus::DataBrowser;
                                        self.analysis_tree =
                                            Some(crate::widgets::json_tree::JsonTreeState::new(
                                                facts.clone(),
                                            ));
                                        self.show_host_list = false;
                                    }
                                }
                            }
                            return Action::None;
                        }
                        _ => return Action::None,
                    }
                }

                // Analysis Mode Navigation
                if self.show_analysis {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('v') => {
                            return Action::ToggleAnalysis;
                        }
                        // Switch Focus
                        KeyCode::Tab => {
                            self.analysis_focus = match self.analysis_focus {
                                AnalysisFocus::TaskList => AnalysisFocus::DataBrowser,
                                AnalysisFocus::DataBrowser => AnalysisFocus::TaskList,
                            };
                            return Action::None;
                        }
                        // Allow Arrow Keys for Focus Switching
                        KeyCode::Right
                            if matches!(self.analysis_focus, AnalysisFocus::TaskList) =>
                        {
                            self.analysis_focus = AnalysisFocus::DataBrowser;
                            return Action::None;
                        }
                        // Shift+Arrows for explicit pane switching
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
                }

                // Analysis Mode Actions
                if self.show_analysis {
                    match key.code {
                        KeyCode::Esc => {
                            if self.show_detail_view {
                                self.show_detail_view = false;
                                return Action::None;
                            }
                            self.show_analysis = false;
                            self.analysis_focus = AnalysisFocus::TaskList;
                            return Action::None;
                        }
                        KeyCode::Char('y') => return Action::Yank,
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
                                _ => {}
                            },
                            AnalysisFocus::DataBrowser => {
                                if let Some(tree) = &mut self.analysis_tree {
                                    match key.code {
                                        KeyCode::Up | KeyCode::Char('k') => {
                                            tree.select_prev();
                                            return Action::None;
                                        }
                                        KeyCode::Down | KeyCode::Char('j') => {
                                            tree.select_next();
                                            return Action::None;
                                        }
                                        KeyCode::Char('h') => {
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
                                        KeyCode::Char('l') => {
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
                    return Action::None;
                }

                // Normal Mode Keys
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        if self.show_help {
                            self.show_help = false;
                            return Action::None;
                        }
                        return Action::Quit;
                    }
                    KeyCode::Char('c') => {
                        if self.failed_task.is_some() {
                            return Action::Continue;
                        } else {
                            return Action::Proceed;
                        }
                    }
                    KeyCode::Char('r') => return Action::Retry,
                    KeyCode::Char('e') => return Action::EditVar,
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

                    // Host List
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

                    // Inspector Scroll
                    KeyCode::Up => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        return Action::None;
                    }
                    KeyCode::Down => {
                        self.scroll_offset = self.scroll_offset.saturating_add(1);
                        return Action::None;
                    }
                    // Log Scroll (PageUp/PageDown)
                    KeyCode::PageUp => {
                        self.auto_scroll = false;
                        self.log_scroll = self.log_scroll.saturating_sub(10);
                        return Action::None;
                    }
                    KeyCode::PageDown => {
                        self.log_scroll = self.log_scroll.saturating_add(10);
                        return Action::None;
                    }
                    KeyCode::Char('v') => return Action::ToggleAnalysis,
                    KeyCode::Char('y') => return Action::Yank,

                    _ => {}
                }
            }
        } else if let Event::Mouse(mouse) = event {
            match mouse.kind {
                crossterm::event::MouseEventKind::ScrollDown => {
                    self.scroll_offset = self.scroll_offset.saturating_add(3);
                }
                crossterm::event::MouseEventKind::ScrollUp => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
                _ => {}
            }
        }
        Action::None
    }

    pub fn log(&mut self, msg: String, color: Option<ratatui::style::Color>) {
        self.logs
            .push_back((msg, color.unwrap_or(ratatui::style::Color::White)));
        if self.logs.len() > 1000 {
            self.logs.pop_front();
        }
        // Note: auto_scroll with log_scroll = logs.len() would scroll past the content.
        // For now, keep scroll at 0 to show all logs from the top.
        // A proper implementation would calculate based on viewport height.
        if self.auto_scroll {
            self.log_scroll = 0;
        }
    }

    pub fn set_task(
        &mut self,
        name: String,
        vars: serde_json::Value,
        facts: Option<serde_json::Value>,
    ) {
        self.current_task = Some(name);
        self.task_vars = Some(vars);
        self.facts = facts.clone();

        // Phase 18: Extract Host and Update State
        if let Some(f) = &facts
            && let Some(host) = f.get("inventory_hostname").and_then(|h| h.as_str())
        {
            let host_name = host.to_string();

            // Store Facts
            self.host_facts.insert(host_name.clone(), f.clone());

            // Update or Create Host Status
            self.hosts.entry(host_name.clone()).or_insert(HostStatus {
                name: host_name,
                ok_tasks: 0,
                changed_tasks: 0,
                failed_tasks: 0,
            });
            // We'll update counters when we get result?
            // Currently set_task is called at start of task.
            // We don't know the result yet.
            // But we know a task started.
        }

        // Update per-host facts if available (assuming we can derive host somehow?
        // Actually, set_task doesn't take host name yet. We need to find where host name comes from.
        // It seems `TaskHistory` has `host`. Let's check where `TaskHistory` is created.
        // Ah, `TaskHistory` is likely created in `ipc_loop`.
        // Ideally `set_task` should take `host` or we update it elsewhere.
        // Let's modify `set_task` to optionally take host or check caller.

        // Wait, looking at `execution.rs` or `main.rs`, how is `set_task` called?
        // It's called from `handle_message`.
        // The `Message` enum likely has `host`.

        self.failed_task = None;
        self.failed_result = None;
        self.waiting_for_proceed = true;
    }

    pub fn set_failed(
        &mut self,
        name: String,
        result: serde_json::Value,
        facts: Option<serde_json::Value>,
    ) {
        self.failed_task = Some(name);
        self.failed_result = Some(result);
        if let Some(f) = facts {
            self.facts = Some(f);
        }
        self.waiting_for_proceed = true;
        self.waiting_for_proceed = true;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_task_result(
        &mut self,
        name: String,
        host: String,
        changed: bool,
        failed: bool,
        error: Option<String>,
        verbose_result: Option<crate::execution::ExecutionDetails>,
        analysis: Option<crate::ai::Analysis>,
    ) {
        // Update History
        self.history.push(TaskHistory {
            name: name.clone(),
            host: host.clone(),
            changed,
            failed,
            error,
            verbose_result,
            analysis,
        });

        // Update Host Stats
        let entry = self.hosts.entry(host.clone()).or_insert(HostStatus {
            name: host.clone(),
            ok_tasks: 0,
            changed_tasks: 0,
            failed_tasks: 0,
        });

        if failed {
            entry.failed_tasks += 1;
        } else if changed {
            entry.changed_tasks += 1;
        } else {
            entry.ok_tasks += 1;
        }
    }

    pub fn find_next_match(&mut self) {
        if self.search_query.is_empty() {
            return;
        }

        let start_index = self.search_index.unwrap_or(0);

        // Search forward from start_index + 1
        for (i, (msg, _)) in self.logs.iter().enumerate().skip(start_index + 1) {
            if msg
                .to_lowercase()
                .contains(&self.search_query.to_lowercase())
            {
                self.search_index = Some(i);
                self.log_scroll = i as u16;
                self.auto_scroll = false;
                return;
            }
        }

        // Wrap around from 0
        for (i, (msg, _)) in self.logs.iter().enumerate().take(start_index + 1) {
            if msg
                .to_lowercase()
                .contains(&self.search_query.to_lowercase())
            {
                self.search_index = Some(i);
                self.log_scroll = i as u16;
                self.auto_scroll = false;
                return;
            }
        }
    }

    pub fn find_prev_match(&mut self) {
        if self.search_query.is_empty() {
            return;
        }

        let start_index = self.search_index.unwrap_or(self.logs.len());

        // Search backwards from start_index - 1
        if start_index > 0 {
            for i in (0..start_index).rev() {
                if let Some((msg, _)) = self.logs.get(i)
                    && msg
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase())
                {
                    self.search_index = Some(i);
                    // Ensure scroll follows
                    // If we found a match, we probably want to scroll to it.
                    // For simplicity, just set log_scroll to i (or close to it)
                    self.log_scroll = if i > 5 { i as u16 - 5 } else { 0 };
                    return;
                }
            }
        }

        // Wrap around to end
        for i in (start_index..self.logs.len()).rev() {
            if let Some((msg, _)) = self.logs.get(i)
                && msg
                    .to_lowercase()
                    .contains(&self.search_query.to_lowercase())
            {
                self.search_index = Some(i);
                self.log_scroll = if i > 5 { i as u16 - 5 } else { 0 };
                return;
            }
        }
    }

    pub fn set_unreachable(
        &mut self,
        task: String,
        host: String,
        error: String,
        result: serde_json::Value,
    ) {
        self.unreachable_hosts.insert(host.clone());
        self.log(
            format!(
                "⚠️  Host {} unreachable during task '{}': {}",
                host, task, error
            ),
            Some(ratatui::style::Color::Red),
        );

        // Record in history
        self.history.push(TaskHistory {
            name: task,
            host: host.clone(),
            changed: false,
            failed: true, // Treat as failed for reporting
            error: Some(serde_json::to_string(&result).unwrap_or_else(|_| error.clone())),
            verbose_result: None,
            analysis: None,
        });
    }

    pub fn copy_to_clipboard(&mut self, text: String) {
        if let Err(e) = self.clipboard.set_text(text) {
            self.notification =
                Some((format!("Clipboard Error: {}", e), std::time::Instant::now()));
        } else {
            self.notification = Some((
                "Copied to Clipboard!".to_string(),
                std::time::Instant::now(),
            ));
        }
    }

    pub fn save_session(&self, filename: &str) -> std::io::Result<()> {
        let session = crate::session::Session::from_app(self);
        session.save(filename)?;
        Ok(())
    }

    pub fn from_session(filename: &str) -> std::io::Result<Self> {
        let session = crate::session::Session::load(filename)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        // We need a dummy Config or load from env?
        // Since session.restore_to_app populates most fields, config is mainly for AI keys etc.
        // Let's try to load config, if fail, panic or dummy.
        // Since this is replay, AI might not work if keys are missing but that's fine.
        let config = Config::new().unwrap_or_else(|_| {
            // Creating a config with empty/default values if load fails
            // We can't easily create Config if fields are private or no Default impl.
            // But Config::new() is what we used before.
            // If we really can't load config, we can't create App.
            panic!("Failed to load configuration for replay.");
        });

        let mut app = App::new(config);

        session.restore_to_app(&mut app);

        app.replay_mode = true;
        app.current_task = Some("REPLAY MODE".to_string());

        Ok(app)
    }
}
