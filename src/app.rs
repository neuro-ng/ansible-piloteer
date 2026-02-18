use crate::ipc::Message;
use std::collections::VecDeque;
use tokio::sync::mpsc;

// ── Action returned by handle_event ─────────────────────────────────────────

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
    SubmitQuery(String),
    NextMatch,
    PrevMatch,
    ToggleFollow,
    ToggleFilter,
    ToggleAnalysis,
    AnalysisNext,
    AnalysisPrev,
    Yank,
    YankVisual,
    YankWithCount,
    SaveSession,
    ExportReport,
    ToggleMetrics,
    ToggleMetricsView,
    ToggleBreakpoint,
    SubmitChat,
    None,
}

// ── Enums shared across modules ──────────────────────────────────────────────

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
    Resume,
    Retry,
    EditVar {
        key: String,
        value: serde_json::Value,
    },
    ExecuteCommand {
        cmd: String,
    },
    AssertAiContext {
        contains: Option<String>,
    },
    AskAi,
    ApplyFix,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ScriptAction {
    pub task_name: String,
    pub on_failure: bool,
    pub actions: Vec<ScriptActionType>,
}

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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnalysisFocus {
    TaskList,
    DataBrowser,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MetricsView {
    Dashboard,
    Heatmap,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChatMode {
    Insert,
    Normal,
    Search,
}

// ── Supporting data types ────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskHistory {
    pub name: String,
    pub host: String,
    pub changed: bool,
    pub failed: bool,
    pub duration: f64,
    pub error: Option<String>,
    pub verbose_result: Option<crate::execution::ExecutionDetails>,
    pub analysis: Option<crate::ai::Analysis>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HostStatus {
    pub name: String,
    pub ok_tasks: usize,
    pub changed_tasks: usize,
    pub failed_tasks: usize,
}

// ── App state ────────────────────────────────────────────────────────────────

use crate::ai::AiClient;
use crate::clipboard::ClipboardHandler;
use crate::highlight::SyntaxHighlighter;

pub struct App {
    pub running: bool,
    pub logs: VecDeque<(String, ratatui::style::Color)>,
    pub current_task: Option<String>,
    pub task_vars: Option<serde_json::Value>,
    pub facts: Option<serde_json::Value>,
    pub task_start_time: Option<std::time::Instant>,
    pub failed_task: Option<String>,
    pub failed_result: Option<serde_json::Value>,
    pub waiting_for_proceed: bool,
    pub ai_client: Option<AiClient>,
    pub suggestion: Option<crate::ai::Analysis>,
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
    pub log_filter: LogFilter,
    pub play_recap: Option<serde_json::Value>,
    // View state
    pub active_view: ActiveView,
    pub dashboard_focus: DashboardFocus,
    pub analysis_index: usize,
    pub analysis_focus: AnalysisFocus,
    pub analysis_tree: Option<crate::widgets::json_tree::JsonTreeState>,
    pub clipboard: ClipboardHandler,
    pub notification: Option<(String, std::time::Instant)>,
    pub replay_mode: bool,
    // Host tracking
    pub host_facts: std::collections::HashMap<String, serde_json::Value>,
    pub host_filter: Option<String>,
    pub show_host_list: bool,
    pub hosts: std::collections::HashMap<String, HostStatus>,
    pub host_list_index: usize,
    pub show_detail_view: bool,
    pub metrics_view: MetricsView,
    // Scripted testing
    pub test_script: Vec<ScriptAction>,
    // IPC
    pub ipc_tx: Option<mpsc::Sender<Message>>,
    pub client_connected: bool,
    pub unreachable_hosts: std::collections::HashSet<String>,
    // AI Chat
    pub chat_active: bool,
    pub chat_input: String,
    pub chat_history: Vec<crate::ai::ChatMessage>,
    pub chat_scroll: u16,
    pub chat_loading: bool,
    pub chat_mode: ChatMode,
    pub chat_selected_index: Option<usize>,
    pub chat_search_query: String,
    pub chat_auto_scroll: bool,
    // Visual / vi-mode
    pub visual_mode: bool,
    pub visual_start_index: Option<usize>,
    pub pending_count: Option<usize>,
    pub pending_command: Option<char>,
    // Tracing
    pub playbook_span: Option<opentelemetry::global::BoxedSpan>,
    pub playbook_span_guard: Option<opentelemetry::ContextGuard>,
    pub task_spans: std::collections::HashMap<String, opentelemetry::global::BoxedSpan>,
    pub play_span: Option<opentelemetry::global::BoxedSpan>,
    pub play_span_guard: Option<opentelemetry::ContextGuard>,
    // Event velocity metrics
    pub event_velocity: VecDeque<u64>,
    pub event_counter: u64,
    pub last_velocity_update: std::time::Instant,
    // Breakpoints / edit
    pub breakpoints: std::collections::HashSet<String>,
    pub edit_state: EditState,
}

// ── App methods ──────────────────────────────────────────────────────────────

use crate::config::Config;

impl App {
    pub fn new(config: Config) -> Self {
        let enable_ai = config.openai_api_key.is_some()
            || config.api_base != "https://api.openai.com/v1"
            || config.auth_token.is_some()
            || config.provider.as_deref() == Some("google");

        let ai_client = enable_ai.then(|| AiClient::new(config.clone()));

        Self {
            running: true,
            logs: VecDeque::new(),
            history: Vec::new(),
            current_task: None,
            task_start_time: None,
            task_vars: None,
            facts: None,
            failed_task: None,
            failed_result: None,
            waiting_for_proceed: false,
            ipc_tx: None,
            client_connected: false,
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
            dashboard_focus: DashboardFocus::Logs,
            analysis_index: 0,
            analysis_focus: AnalysisFocus::TaskList,
            analysis_tree: None,
            clipboard: ClipboardHandler::new(),
            notification: None,
            replay_mode: false,
            host_facts: std::collections::HashMap::new(),
            host_filter: None,
            show_host_list: false,
            hosts: std::collections::HashMap::new(),
            host_list_index: 0,
            show_detail_view: false,
            metrics_view: MetricsView::Dashboard,
            test_script: Vec::new(),
            unreachable_hosts: std::collections::HashSet::new(),
            chat_active: false,
            chat_input: String::new(),
            chat_history: Vec::new(),
            chat_scroll: 0,
            chat_loading: false,
            chat_mode: ChatMode::Insert,
            chat_selected_index: None,
            chat_search_query: String::new(),
            chat_auto_scroll: true,
            visual_mode: false,
            visual_start_index: None,
            pending_count: None,
            pending_command: None,
            playbook_span: None,
            playbook_span_guard: None,
            task_spans: std::collections::HashMap::new(),
            play_span: None,
            play_span_guard: None,
            event_velocity: VecDeque::new(),
            event_counter: 0,
            last_velocity_update: std::time::Instant::now(),
            breakpoints: std::collections::HashSet::new(),
            edit_state: EditState::Idle,
        }
    }

    pub fn set_ipc_tx(&mut self, tx: Option<mpsc::Sender<Message>>) {
        self.ipc_tx = tx;
    }

    pub fn is_connected(&self) -> bool {
        self.client_connected && self.ipc_tx.is_some()
    }

    pub fn notify(&mut self, msg: String) {
        self.notification = Some((msg, std::time::Instant::now()));
    }

    pub fn log(&mut self, msg: String, color: Option<ratatui::style::Color>) {
        self.logs
            .push_back((msg, color.unwrap_or(ratatui::style::Color::White)));
        self.event_counter += 1;
        if self.logs.len() > 1000 {
            self.logs.pop_front();
        }
        if self.auto_scroll {
            self.log_scroll = self.logs.len() as u16;
        }
    }

    pub fn load_test_script(&mut self) {
        let Ok(path) = std::env::var("PILOTEER_TEST_SCRIPT") else {
            return;
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            eprintln!("Headless Error: Failed to read test script file: {}", path);
            return;
        };
        match serde_json::from_str::<Vec<ScriptAction>>(&content) {
            Ok(actions) => self.test_script = actions,
            Err(e) => eprintln!("Headless Error: Failed to parse test script: {}", e),
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
        self.task_start_time = Some(std::time::Instant::now());

        if let Some(f) = &facts
            && let Some(host) = f.get("inventory_hostname").and_then(|h| h.as_str())
        {
            let host_name = host.to_string();
            self.host_facts.insert(host_name.clone(), f.clone());
            self.hosts.entry(host_name.clone()).or_insert(HostStatus {
                name: host_name,
                ok_tasks: 0,
                changed_tasks: 0,
                failed_tasks: 0,
            });
        }

        self.facts = facts;
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
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_task_result(
        &mut self,
        name: String,
        host: String,
        changed: bool,
        failed: bool,
        duration: f64,
        error: Option<String>,
        verbose_result: Option<crate::execution::ExecutionDetails>,
        analysis: Option<crate::ai::Analysis>,
    ) {
        self.history.push(TaskHistory {
            name: name.clone(),
            host: host.clone(),
            changed,
            failed,
            duration,
            error,
            verbose_result,
            analysis,
        });

        let entry = self.hosts.entry(host.clone()).or_insert(HostStatus {
            name: host,
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
        self.history.push(TaskHistory {
            name: task,
            host,
            changed: false,
            failed: true,
            duration: 0.0,
            error: Some(serde_json::to_string(&result).unwrap_or(error)),
            verbose_result: None,
            analysis: None,
        });
    }

    pub fn update_velocity(&mut self) {
        if self.last_velocity_update.elapsed() >= std::time::Duration::from_secs(1) {
            self.event_velocity.push_back(self.event_counter);
            self.event_counter = 0;
            if self.event_velocity.len() > 100 {
                self.event_velocity.pop_front();
            }
            self.last_velocity_update = std::time::Instant::now();
        }
    }

    pub fn save_session(&self, filename: &str) -> std::io::Result<()> {
        crate::session::Session::from_app(self).save(filename)
    }

    pub fn from_session(filename: &str) -> std::io::Result<Self> {
        let session = crate::session::Session::load(filename)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let config = Config::new().unwrap_or_else(|_| {
            panic!("Failed to load configuration for replay.");
        });
        let mut app = App::new(config);
        session.restore_to_app(&mut app);
        app.replay_mode = true;
        app.current_task = Some("REPLAY MODE".to_string());
        Ok(app)
    }
}
