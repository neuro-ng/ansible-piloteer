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
    Search,
    SubmitSearch,
    SubmitQuery(String), // [NEW] Return query string to main loop for execution
    NextMatch,           // [NEW]
    PrevMatch,           // [NEW]
    ToggleFollow,
    ToggleFilter,   // [NEW]
    ToggleAnalysis, // [NEW] - Enter/Exit Data Browser
    AnalysisNext,
    AnalysisPrev,
    Yank,
    YankVisual,    // [NEW] Phase 16: Visual mode yank
    YankWithCount, // [NEW] Phase 16: Count-based yank (y3j)
    SaveSession,
    ExportReport,      // [NEW]
    ToggleMetrics,     // [NEW] Phase 22
    ToggleMetricsView, // [NEW] Phase 22
    ToggleBreakpoint,  // [NEW] Phase 31
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogFilter {
    All,
    Failed,
    Changed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditState {
    Idle,
    SelectingVariable {
        filter: String,
        selected_index: usize,
    },
    EditingValue {
        key: String,
        temp_file: std::path::PathBuf,
    },
}

#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
pub enum ScriptActionType {
    Pause,
    Continue,
    Resume, // [NEW] Phase 28
    Retry,
    EditVar {
        key: String,
        value: serde_json::Value,
    },
    ExecuteCommand {
        cmd: String,
    }, // [NEW] Phase 27
    AssertAiContext {
        contains: Option<String>,
    }, // [NEW] Phase 28
    AskAi,
    ApplyFix,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ScriptAction {
    pub task_name: String,
    pub on_failure: bool,
    pub actions: Vec<ScriptActionType>,
}

use crate::ai::{AiClient, Analysis};
use crate::clipboard::ClipboardHandler;
use crate::highlight::SyntaxHighlighter;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveView {
    Dashboard,
    Analysis,
    Metrics,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DashboardFocus {
    Logs,
    Inspector,
} // [NEW] Phase 27

pub struct App {
    pub running: bool,
    pub logs: VecDeque<(String, ratatui::style::Color)>,
    pub current_task: Option<String>,
    pub task_vars: Option<serde_json::Value>,
    pub facts: Option<serde_json::Value>,            // [NEW]
    pub task_start_time: Option<std::time::Instant>, // [NEW] Phase 22
    pub failed_task: Option<String>,
    pub failed_result: Option<serde_json::Value>,
    pub waiting_for_proceed: bool,
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
    pub active_view: ActiveView,
    pub dashboard_focus: DashboardFocus, // [NEW] Phase 27
    // show_analysis replaced by active_view == Analysis
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
    // show_metrics replaced by active_view == Metrics
    pub metrics_view: MetricsView,      // [NEW] Phase 22
    pub test_script: Vec<ScriptAction>, // [NEW] Phase 27
    // Communication
    pub ipc_tx: Option<mpsc::Sender<Message>>,
    pub client_connected: bool, // [NEW] Track active client connection
    pub unreachable_hosts: std::collections::HashSet<String>, // [NEW]
    // Visual Mode State (Phase 16: Multi-Line Copy)
    pub visual_mode: bool,
    pub visual_start_index: Option<usize>,
    pub pending_count: Option<usize>,
    pub pending_command: Option<char>,
    // Tracing State (Phase 21: OpenZipkin)
    pub playbook_span: Option<opentelemetry::global::BoxedSpan>,
    pub playbook_span_guard: Option<opentelemetry::ContextGuard>,
    pub task_spans: std::collections::HashMap<String, opentelemetry::global::BoxedSpan>,
    pub play_span: Option<opentelemetry::global::BoxedSpan>,
    pub play_span_guard: Option<opentelemetry::ContextGuard>,
    // Event Velocity
    pub event_velocity: VecDeque<u64>,
    pub event_counter: u64,
    pub last_velocity_update: std::time::Instant,
    pub breakpoints: std::collections::HashSet<String>, // [NEW] Phase 31
    pub edit_state: EditState,                          // [NEW] Phase 31
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
    pub duration: f64, // [NEW] Seconds
    pub error: Option<String>,
    pub verbose_result: Option<crate::execution::ExecutionDetails>,
    pub analysis: Option<crate::ai::Analysis>, // [NEW]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MetricsView {
    Dashboard,
    Heatmap,
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
            task_start_time: None, // [NEW]
            task_vars: None,
            facts: None,
            failed_task: None,
            failed_result: None,
            waiting_for_proceed: false,
            ipc_tx: None,
            client_connected: false, // [NEW]
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
            active_view: ActiveView::Dashboard,
            dashboard_focus: DashboardFocus::Logs, // [NEW]
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
            metrics_view: MetricsView::Dashboard, // [NEW] Phase 22
            test_script: Vec::new(),              // [NEW] Phase 27
            unreachable_hosts: std::collections::HashSet::new(),
            // Visual Mode State
            visual_mode: false,
            visual_start_index: None,
            pending_count: None,
            pending_command: None,
            // Tracing State
            playbook_span: None,
            playbook_span_guard: None,
            task_spans: std::collections::HashMap::new(),
            play_span: None,
            play_span_guard: None,
            // Event Velocity
            event_velocity: VecDeque::new(),
            event_counter: 0,
            last_velocity_update: std::time::Instant::now(),
            breakpoints: std::collections::HashSet::new(),
            edit_state: EditState::Idle,
        }
    }

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

    pub fn load_test_script(&mut self) {
        if let Ok(path) = std::env::var("PILOTEER_TEST_SCRIPT") {
            // println!("Headless: Loading test script from: {}", path);
            if let Ok(content) = std::fs::read_to_string(&path) {
                match serde_json::from_str::<Vec<ScriptAction>>(&content) {
                    Ok(actions) => {
                        self.test_script = actions;
                        // println!("Headless: Loaded {} script actions", self.test_script.len());
                    }
                    Err(e) => {
                        eprintln!("Headless Error: Failed to parse test script: {}", e);
                    }
                }
            } else {
                eprintln!("Headless Error: Failed to read test script file: {}", path);
            }
        }
    }

    pub fn set_ipc_tx(&mut self, tx: Option<mpsc::Sender<Message>>) {
        self.ipc_tx = tx;
    }

    pub fn is_connected(&self) -> bool {
        self.client_connected && self.ipc_tx.is_some()
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

                // [NEW] Variable Selection Logic
                // We need to handle this carefully to avoid double borrow of self
                let mut selection_action = Action::None;

                if let EditState::SelectingVariable {
                    filter,
                    selected_index,
                } = &mut self.edit_state
                {
                    match key.code {
                        KeyCode::Esc => {
                            // We will handle state change after the block
                            selection_action = Action::None; // Just marker
                        }
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

                // Handle the action triggered by Enter
                if let Action::EditVar = selection_action {
                    // Now we are out of the borrow, we can use self methods
                    if let EditState::SelectingVariable {
                        filter,
                        selected_index,
                    } = &self.edit_state
                    {
                        let all_vars = self.get_flattened_vars();
                        let filtered: Vec<&String> = all_vars
                            .iter()
                            .filter(|v| v.to_lowercase().contains(&filter.to_lowercase()))
                            .collect();

                        if let Some(selected_key) =
                            filtered.get(*selected_index % filtered.len().max(1))
                        {
                            let key_clone = selected_key.to_string();
                            if let Err(e) = self.prepare_edit(key_clone) {
                                self.notification =
                                    Some((format!("Error: {}", e), std::time::Instant::now()));
                                self.edit_state = EditState::Idle;
                                return Action::None;
                            }
                            return Action::EditVar;
                        }
                    }
                    return Action::None;
                }

                // If we are selecting, we consumed the key
                if matches!(self.edit_state, EditState::SelectingVariable { .. }) {
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

                            // Check for query prefix
                            let query = self.search_query.trim();
                            if query.starts_with("::query::") {
                                let query_str =
                                    query.trim_start_matches("::query::").trim().to_string();
                                if !query_str.is_empty() {
                                    return Action::SubmitQuery(query_str);
                                }
                                return Action::None;
                            }

                            // Trigger search
                            if self.active_view == ActiveView::Analysis {
                                if let Some(tree) = &mut self.analysis_tree {
                                    tree.set_search(self.search_query.clone());
                                }
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
                                        self.active_view = ActiveView::Analysis;
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

                // Global Navigation (Tab Cycling)
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

                // Analysis Mode Navigation
                if self.active_view == ActiveView::Analysis {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('v') => {
                            // Exit Analysis
                            self.active_view = ActiveView::Dashboard;
                            return Action::None;
                            // return Action::ToggleAnalysis; // Removed Action
                        }
                        // Switch Focus
                        // NOTE: Tab is now Global for View Switching.
                        // We use Shift+Tab or another key for cycling focus inside Analysis?
                        // OR: We check for Tab, if we are in Analysis, we MIGHT cycle focus IF user wants that behavior.
                        // BUT Requirement says "Tab to switch main views".
                        // So we should remove Tab from Analysis focus switching OR use something else.
                        // Let's use 'w' or just arrows/Shift+Arrows.
                        // Wait, 'w' is wrap.
                        // Let's use 'o' or just keep Shift+Arrows.
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
                if self.active_view == ActiveView::Analysis {
                    match key.code {
                        KeyCode::Esc => {
                            if self.show_detail_view {
                                self.show_detail_view = false;
                                return Action::None;
                            }
                            self.active_view = ActiveView::Dashboard;
                            self.analysis_focus = AnalysisFocus::TaskList;
                            return Action::None;
                        }
                        KeyCode::Char('y') => {
                            // Check if we're in visual mode or have a pending count
                            if self.visual_mode {
                                return Action::YankVisual;
                            } else if self.pending_count.is_some() {
                                return Action::YankWithCount;
                            } else {
                                return Action::Yank;
                            }
                        }
                        KeyCode::Char('V') => {
                            // Toggle visual mode (Shift+v)
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
                                KeyCode::Char('b') => return Action::ToggleBreakpoint, // [NEW] Phase 31
                                _ => {}
                            },
                            AnalysisFocus::DataBrowser => {
                                if let Some(tree) = &mut self.analysis_tree {
                                    match key.code {
                                        // Number input for counts
                                        KeyCode::Char(c @ '0'..='9') => {
                                            let digit = c.to_digit(10).unwrap() as usize;
                                            self.pending_count =
                                                Some(self.pending_count.unwrap_or(0) * 10 + digit);
                                            return Action::None;
                                        }
                                        KeyCode::Up => {
                                            let count = self.pending_count.unwrap_or(1);
                                            self.pending_count = None;
                                            for _ in 0..count {
                                                tree.select_prev();
                                            }
                                            return Action::None;
                                        }
                                        KeyCode::Down => {
                                            let count = self.pending_count.unwrap_or(1);
                                            self.pending_count = None;
                                            for _ in 0..count {
                                                tree.select_next();
                                            }
                                            return Action::None;
                                        }
                                        KeyCode::Char('k') => {
                                            let count = self.pending_count.unwrap_or(1);
                                            self.pending_count = None;
                                            for _ in 0..count {
                                                tree.select_prev();
                                            }
                                            return Action::None;
                                        }
                                        KeyCode::Char('j') => {
                                            let count = self.pending_count.unwrap_or(1);
                                            self.pending_count = None;
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
                    KeyCode::Char('e') => {
                        // Enter selection mode
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
                    _ => {} // Default for this match block
                }

                if self.active_view == ActiveView::Dashboard {
                    // Dashboard Focus & Navigation
                    match key.code {
                        KeyCode::Right => {
                            self.dashboard_focus = DashboardFocus::Inspector;
                            return Action::None;
                        }
                        KeyCode::Left => {
                            self.dashboard_focus = DashboardFocus::Logs;
                            return Action::None;
                        }
                        // Up: Scroll Logs or Inspector
                        KeyCode::Up => {
                            match self.dashboard_focus {
                                DashboardFocus::Logs => {
                                    self.auto_scroll = false;
                                    self.log_scroll = self.log_scroll.saturating_sub(1);
                                }
                                DashboardFocus::Inspector => {
                                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                                }
                            }
                            return Action::None;
                        }
                        // Down: Scroll Logs or Inspector
                        KeyCode::Down => {
                            match self.dashboard_focus {
                                DashboardFocus::Logs => {
                                    self.log_scroll = self.log_scroll.saturating_add(1);
                                }
                                DashboardFocus::Inspector => {
                                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                                }
                            }
                            return Action::None;
                        }
                        // Log Scroll (PageUp/PageDown) or Inspector (PageUp/PageDown)
                        KeyCode::PageUp => {
                            match self.dashboard_focus {
                                DashboardFocus::Logs => {
                                    self.auto_scroll = false;
                                    self.log_scroll = self.log_scroll.saturating_sub(10);
                                }
                                DashboardFocus::Inspector => {
                                    self.scroll_offset = self.scroll_offset.saturating_sub(10);
                                }
                            }
                            return Action::None;
                        }
                        KeyCode::PageDown => {
                            match self.dashboard_focus {
                                DashboardFocus::Logs => {
                                    self.log_scroll = self.log_scroll.saturating_add(10);
                                }
                                DashboardFocus::Inspector => {
                                    self.scroll_offset = self.scroll_offset.saturating_add(10);
                                }
                            }
                            return Action::None;
                        }
                        _ => {}
                    }
                }

                // Normal Mode Keys
                match key.code {
                    KeyCode::Char('a') => return Action::AskAi, // [FIX] AI Chat binding
                    KeyCode::Char('y') => return Action::Yank,

                    // Specific View Toggles (Shortcuts)
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

        self.event_counter += 1;

        if self.logs.len() > 1000 {
            self.logs.pop_front();
        }
        // Note: auto_scroll with log_scroll = logs.len() would scroll past the content.
        // For now, keep scroll at 0 to show all logs from the top.
        // A proper implementation would calculate based on viewport height.
        if self.auto_scroll {
            // FIX: Don't reset to 0. Set to length to ensure we scroll to bottom.
            // Paragraph widget handles over-scrolling by showing the end.
            self.log_scroll = self.logs.len() as u16;
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
        self.task_start_time = Some(std::time::Instant::now());

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
        duration: f64, // [NEW]
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
            duration, // [NEW]
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

        // If in Analysis mode - handled by tree (but search triggers SubmitSearch action which we might handle?)
        // Currently search in Analysis is handled via `Action::SubmitSearch` -> `main.rs`?
        // No, `handle_event` returns `SubmitSearch`, main loop ignores it?
        // Wait, `handle_event` calls `tree.set_search`.
        // So for Dashboard we need logic here.

        if self.active_view == ActiveView::Dashboard {
            match self.dashboard_focus {
                DashboardFocus::Logs => {
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
                DashboardFocus::Inspector => {
                    // Implement Inspector Search
                    // We need the content.
                    // For now, let's reconstruct it effectively.
                    let content = if let Some(err) = &self.failed_result {
                        serde_json::to_string_pretty(err).unwrap_or_default()
                    } else {
                        String::new()
                    };

                    // Simple search: find byte offset match
                    if let Some(idx) = content
                        .to_lowercase()
                        .find(&self.search_query.to_lowercase())
                    {
                        // Rough scrolling approximation: count newlines before match
                        let lines_before = content[..idx].chars().filter(|&c| c == '\n').count();
                        self.scroll_offset = lines_before as u16;
                    }
                }
            }
        }
    }

    pub fn find_prev_match(&mut self) {
        if self.search_query.is_empty() {
            return;
        }

        if self.active_view == ActiveView::Dashboard {
            match self.dashboard_focus {
                DashboardFocus::Logs => {
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
                DashboardFocus::Inspector => {
                    // Previous match in Inspector not implemented yet for simple string.
                    // Just finding first match is a start.
                }
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
            failed: true,  // Treat as failed for reporting
            duration: 0.0, // [NEW] Unreachable tasks have no duration
            error: Some(serde_json::to_string(&result).unwrap_or_else(|_| error.clone())),
            verbose_result: None,
            analysis: None,
        });
    }

    pub fn update_velocity(&mut self) {
        let elapsed = self.last_velocity_update.elapsed();

        if elapsed >= std::time::Duration::from_secs(1) {
            self.event_velocity.push_back(self.event_counter);
            self.event_counter = 0;
            if self.event_velocity.len() > 100 {
                self.event_velocity.pop_front();
            }
            self.last_velocity_update = std::time::Instant::now();
        }
    }

    pub fn toggle_breakpoint(&mut self) {
        if self.analysis_index < self.history.len() {
            let task_name = self.history[self.analysis_index].name.clone();
            if self.breakpoints.contains(&task_name) {
                self.breakpoints.remove(&task_name);
                self.notification = Some((
                    format!("Breakpoint removed: {}", task_name),
                    std::time::Instant::now(),
                ));
            } else {
                self.notification = Some((
                    format!("Breakpoint set: {}", task_name),
                    std::time::Instant::now(),
                ));
                self.breakpoints.insert(task_name);
            }
        }
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

    pub fn prepare_edit(&mut self, key: String) -> std::io::Result<()> {
        if let Some(val) = self.get_var_value(&key) {
            let content = if let Ok(s) = serde_json::to_string_pretty(&val) {
                s
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Failed to serialize variable",
                ));
            };

            // Create temp file
            let mut temp_file = tempfile::Builder::new()
                .prefix("piloteer_edit_")
                .suffix(".json")
                .tempfile()?;

            use std::io::Write;
            write!(temp_file.as_file_mut(), "{}", content)?;
            let (_file, path) = temp_file.keep()?;

            self.edit_state = EditState::EditingValue {
                key,
                temp_file: path,
            };
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Variable not found",
            ))
        }
    }

    pub fn apply_edit(&mut self) -> Result<(String, serde_json::Value), String> {
        if let EditState::EditingValue { key, temp_file } = &self.edit_state {
            let key_clone = key.clone();
            let content = std::fs::read_to_string(temp_file)
                .map_err(|e| format!("Failed to read temp file: {}", e))?;

            // Clean up
            let _ = std::fs::remove_file(temp_file);

            let val: serde_json::Value =
                serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {}", e))?;

            self.edit_state = EditState::Idle;
            Ok((key_clone, val))
        } else {
            Err("Not in editing state".to_string())
        }
    }

    pub fn cancel_edit(&mut self) {
        if let EditState::EditingValue { temp_file, .. } = &self.edit_state {
            let _ = std::fs::remove_file(temp_file);
        }
        self.edit_state = EditState::Idle;
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

#[cfg(test)]
mod tests_app {
    use super::*;

    #[test]
    fn test_update_velocity() {
        let config = crate::config::Config {
            // Correct Fields from src/config.rs
            openai_api_key: None,
            socket_path: "/tmp/piloteer.sock".to_string(),
            model: "gpt-4".to_string(),
            api_base: "https://api.openai.com/v1".to_string(),
            log_level: "info".to_string(),
            auth_token: None,
            bind_addr: None,
            secret_token: None,
            quota_limit_tokens: None,
            quota_limit_usd: None,
            google_client_id: None,
            google_client_secret: None,
            zipkin_endpoint: None,
            zipkin_service_name: "ansible-piloteer".to_string(),
            zipkin_sample_rate: 1.0,
            filters: None,
            provider: None,
        };

        let mut app = App::new(config);

        // Initial state
        assert_eq!(app.event_counter, 0);
        assert!(app.event_velocity.is_empty());

        // Simulate events
        app.event_counter = 50;

        // Explicitly set time to avoid test flakiness
        app.last_velocity_update = std::time::Instant::now();
        let initial_time = app.last_velocity_update;

        app.update_velocity();

        // Assert no update yet
        assert_eq!(app.last_velocity_update, initial_time);
        assert_eq!(app.event_counter, 50);
        assert!(app.event_velocity.is_empty());

        // Sleep to test the update trigger
        std::thread::sleep(std::time::Duration::from_millis(1050));
        app.event_counter = 50;
        app.update_velocity();

        // Now it MUST have updated
        assert_eq!(app.event_counter, 0);
        assert!(!app.event_velocity.is_empty());
        assert_eq!(*app.event_velocity.back().unwrap(), 50);
    }

    #[test]
    fn test_set_task() {
        let config = crate::config::Config {
            openai_api_key: None,
            socket_path: "/tmp/piloteer.sock".to_string(),
            model: "gpt-4".to_string(),
            api_base: "https://api.openai.com/v1".to_string(),
            log_level: "info".to_string(),
            auth_token: None,
            bind_addr: None,
            secret_token: None,
            quota_limit_tokens: None,
            quota_limit_usd: None,
            google_client_id: None,
            google_client_secret: None,
            zipkin_endpoint: None,
            zipkin_service_name: "ansible-piloteer".to_string(),
            zipkin_sample_rate: 1.0,
            filters: None,
            provider: None,
        };

        let mut app = App::new(config);

        assert_eq!(app.current_task, None);
        assert_eq!(app.task_vars, None);

        let task_name = "Test Task".to_string();
        let vars = serde_json::json!({"foo": "bar"});
        let facts = Some(serde_json::json!({"ansible_os_family": "Debian"}));

        app.set_task(task_name.clone(), vars.clone(), facts.clone());

        assert_eq!(app.current_task, Some(task_name));
        assert_eq!(app.task_vars, Some(vars));
        assert_eq!(app.facts, facts);
        assert!(app.task_start_time.is_some());
    }

    #[test]
    fn test_toggle_breakpoint() {
        let config = crate::config::Config {
            openai_api_key: None,
            socket_path: "/tmp/piloteer.sock".to_string(),
            model: "gpt-4".to_string(),
            api_base: "https://api.openai.com/v1".to_string(),
            log_level: "info".to_string(),
            auth_token: None,
            bind_addr: None,
            secret_token: None,
            quota_limit_tokens: None,
            quota_limit_usd: None,
            google_client_id: None,
            google_client_secret: None,
            zipkin_endpoint: None,
            zipkin_service_name: "ansible-piloteer".to_string(),
            zipkin_sample_rate: 1.0,
            filters: None,
            provider: None,
        };

        let mut app = App::new(config);

        // Add dummy history
        app.history.push(TaskHistory {
            name: "Task 1".to_string(),
            host: "localhost".to_string(),
            changed: false,
            failed: false,
            duration: 0.0,
            error: None,
            verbose_result: None,
            analysis: None,
        });

        // Test Toggling
        app.analysis_index = 0;
        assert!(!app.breakpoints.contains("Task 1"));

        app.toggle_breakpoint();
        assert!(app.breakpoints.contains("Task 1"));
        assert!(app.notification.is_some());

        app.toggle_breakpoint();
        assert!(!app.breakpoints.contains("Task 1"));
    }

    #[test]
    fn test_variable_editing_flow() {
        let config = crate::config::Config {
            openai_api_key: None,
            socket_path: "/tmp/piloteer_test_edit.sock".to_string(),
            model: "gpt-4".to_string(),
            api_base: "https://api.openai.com/v1".to_string(),
            log_level: "info".to_string(),
            auth_token: None,
            bind_addr: None,
            secret_token: None,
            quota_limit_tokens: None,
            quota_limit_usd: None,
            google_client_id: None,
            google_client_secret: None,
            zipkin_endpoint: None,
            zipkin_service_name: "ansible-piloteer".to_string(),
            zipkin_sample_rate: 1.0,
            filters: None,
            provider: None,
        };
        let mut app = App::new(config);

        // Setup Task Vars
        let vars = serde_json::json!({"test_var": "initial_value", "number": 42});
        app.set_task("Test Task".to_string(), vars, None);

        // 1. Prepare Edit
        assert!(app.prepare_edit("test_var".to_string()).is_ok());

        // Verify State
        let temp_path = if let EditState::EditingValue { key, temp_file } = &app.edit_state {
            assert_eq!(key, "test_var");
            temp_file.clone()
        } else {
            panic!("App not in EditingValue state");
        };

        // 2. Simulate External Edit
        let new_content =
            serde_json::to_string_pretty(&serde_json::json!("modified_value")).unwrap();
        std::fs::write(&temp_path, new_content).expect("Failed to write to temp file");

        // 3. Apply Edit
        let result = app.apply_edit();
        assert!(result.is_ok());
        let (key, value) = result.unwrap();

        assert_eq!(key, "test_var");
        assert_eq!(value, serde_json::json!("modified_value"));
        assert!(matches!(app.edit_state, EditState::Idle));

        // Ensure temp file is gone
        assert!(!temp_path.exists());
    }
}
