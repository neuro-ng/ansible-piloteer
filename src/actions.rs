use crate::app::{Action, ActiveView, AnalysisFocus, App, EditState, MetricsView};
use crate::ipc::Message;
use crate::widgets::json_tree::JsonTreeState;
use anyhow::Result;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io;
use std::io::Stdout;
use std::process::Command;
use tokio::sync::mpsc;

// ── App methods moved here because they're called only by action dispatch ────

impl App {
    pub fn toggle_breakpoint(&mut self) {
        if self.analysis_index < self.history.len() {
            let task_name = self.history[self.analysis_index].name.clone();
            if self.breakpoints.remove(&task_name) {
                self.notify(format!("Breakpoint removed: {}", task_name));
            } else {
                self.breakpoints.insert(task_name.clone());
                self.notify(format!("Breakpoint set: {}", task_name));
            }
        }
    }

    pub fn copy_to_clipboard(&mut self, text: String) {
        if let Err(e) = self.clipboard.set_text(text) {
            self.notify(format!("Clipboard Error: {}", e));
        } else {
            self.notify("Copied to Clipboard!".to_string());
        }
    }

    pub fn prepare_edit(&mut self, key: String) -> io::Result<()> {
        let val = self
            .get_var_value(&key)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Variable not found"))?;
        let content = serde_json::to_string_pretty(&val).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Failed to serialize variable")
        })?;
        let mut temp = tempfile::Builder::new()
            .prefix("piloteer_edit_")
            .suffix(".json")
            .tempfile()?;
        use io::Write;
        write!(temp.as_file_mut(), "{}", content)?;
        let (_, path) = temp.keep()?;
        self.edit_state = EditState::EditingValue {
            key,
            temp_file: path,
        };
        Ok(())
    }

    pub fn apply_edit(&mut self) -> Result<(String, serde_json::Value), String> {
        let EditState::EditingValue { key, temp_file } = &self.edit_state else {
            return Err("Not in editing state".to_string());
        };
        let key = key.clone();
        let content = std::fs::read_to_string(temp_file)
            .map_err(|e| format!("Failed to read temp file: {}", e))?;
        let _ = std::fs::remove_file(temp_file);
        let val: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {}", e))?;
        self.edit_state = EditState::Idle;
        Ok((key, val))
    }

    pub fn cancel_edit(&mut self) {
        if let EditState::EditingValue { temp_file, .. } = &self.edit_state {
            let _ = std::fs::remove_file(temp_file);
        }
        self.edit_state = EditState::Idle;
    }
}

// ── Action dispatcher ────────────────────────────────────────────────────────

pub async fn dispatch(
    action: Action,
    app: &mut App,
    terminal: &mut Option<Terminal<CrosstermBackend<Stdout>>>,
    ai_tx: &mpsc::Sender<anyhow::Result<crate::ai::ChatMessage>>,
) {
    match action {
        Action::Quit => app.running = false,

        Action::SaveSession => {
            let filename = format!(
                "piloteer_session_{}.json.gz",
                chrono::Local::now().format("%Y%m%d_%H%M%S")
            );
            match crate::session::Session::from_app(app).save(&filename) {
                Ok(_) => app.notify(format!("Saved to {}", filename)),
                Err(e) => app.notify(format!("Save Failed: {}", e)),
            }
        }

        Action::ExportReport => {
            let filename = format!(
                "piloteer_report_{}.md",
                chrono::Local::now().format("%Y%m%d_%H%M%S")
            );
            match crate::report::ReportGenerator::new(app).save_to_file(&filename) {
                Ok(_) => app.notify(format!("Report Saved: {}", filename)),
                Err(e) => app.notify(format!("Report Failed: {}", e)),
            }
        }

        Action::Proceed => {
            if app.waiting_for_proceed {
                app.waiting_for_proceed = false;
                send_ipc(app, Message::Proceed).await;
            }
        }
        Action::Retry => {
            if app.waiting_for_proceed {
                app.waiting_for_proceed = false;
                send_ipc(app, Message::Retry).await;
            }
        }
        Action::Continue => {
            if app.waiting_for_proceed {
                app.waiting_for_proceed = false;
                send_ipc(app, Message::Continue).await;
            }
        }

        Action::EditVar => launch_editor(app, terminal).await,

        Action::AskAi => ask_ai(app).await,

        Action::SubmitChat => submit_chat(app, ai_tx).await,

        Action::ApplyFix => {
            if let Some(analysis) = &app.suggestion.clone()
                && let Some(fix) = &analysis.fix
            {
                send_ipc(
                    app,
                    Message::ModifyVar {
                        key: fix.key.clone(),
                        value: fix.value.clone(),
                    },
                )
                .await;
                app.log(
                    format!("Applying Fix: {} = {}", fix.key, fix.value),
                    Some(ratatui::style::Color::Green),
                );
            }
        }

        Action::ToggleFollow => {
            app.auto_scroll = !app.auto_scroll;
            if app.auto_scroll {
                app.log_scroll = 0;
            }
        }

        Action::ToggleAnalysis => {
            if app.active_view == ActiveView::Analysis {
                app.active_view = ActiveView::Dashboard;
            } else {
                app.active_view = ActiveView::Analysis;
                app.scroll_offset = 0;
                refresh_analysis_tree(app);
            }
        }

        Action::AnalysisNext => {
            if !app.history.is_empty() {
                app.analysis_index = (app.analysis_index + 1).min(app.history.len() - 1);
                app.scroll_offset = 0;
                refresh_analysis_tree(app);
            }
        }

        Action::AnalysisPrev => {
            if !app.history.is_empty() {
                app.analysis_index = app.analysis_index.saturating_sub(1);
                app.scroll_offset = 0;
                refresh_analysis_tree(app);
            }
        }

        Action::ToggleMetrics => {
            app.active_view = if app.active_view == ActiveView::Metrics {
                ActiveView::Dashboard
            } else {
                ActiveView::Metrics
            };
        }

        Action::ToggleMetricsView => {
            app.metrics_view = match app.metrics_view {
                MetricsView::Dashboard => MetricsView::Heatmap,
                MetricsView::Heatmap => MetricsView::Dashboard,
            };
        }

        Action::SubmitQuery(query_str) => {
            let session = crate::session::Session::from_app(app);
            match serde_json::to_value(&session) {
                Ok(json) => match crate::query::run_query(&query_str, &json) {
                    Ok(result) => {
                        app.active_view = ActiveView::Analysis;
                        app.analysis_tree = Some(JsonTreeState::new(result));
                        app.analysis_focus = AnalysisFocus::DataBrowser;
                        app.notify(format!("Query: {}", query_str));
                    }
                    Err(e) => app.notify(format!("Query Error: {}", e)),
                },
                Err(e) => app.notify(format!("Serialization Error: {}", e)),
            }
        }

        Action::Yank => {
            let content = if app.active_view == ActiveView::Analysis
                && app.analysis_focus == AnalysisFocus::DataBrowser
                && let Some(tree) = &app.analysis_tree
            {
                tree.get_selected_content()
            } else {
                app.failed_result
                    .as_ref()
                    .and_then(|r| serde_json::to_string_pretty(r).ok())
            };
            if let Some(c) = content {
                app.copy_to_clipboard(c);
            }
        }

        Action::YankVisual => {
            if app.active_view == ActiveView::Analysis
                && app.visual_mode
                && let Some(tree) = &app.analysis_tree
                && let Some(start) = app.visual_start_index
            {
                let end = tree.selected_line;
                if let Some(content) = tree.get_range_content(start, end) {
                    app.copy_to_clipboard(content);
                }
            }
            app.visual_mode = false;
            app.visual_start_index = None;
        }

        Action::YankWithCount => {
            if app.active_view == ActiveView::Analysis
                && let Some(tree) = &app.analysis_tree
                && let Some(count) = app.pending_count
                && let Some(content) = tree.get_content_with_count(count)
            {
                app.copy_to_clipboard(content);
            }
            app.pending_count = None;
        }

        Action::ToggleBreakpoint => app.toggle_breakpoint(),

        Action::None
        | Action::Search
        | Action::SubmitSearch
        | Action::NextMatch
        | Action::PrevMatch
        | Action::ToggleFilter => {}
    }
}

// ── Private helpers ──────────────────────────────────────────────────────────

async fn send_ipc(app: &App, msg: Message) {
    if let Some(tx) = &app.ipc_tx {
        let _ = tx.send(msg).await;
    }
}

fn refresh_analysis_tree(app: &mut App) {
    if let Some(task) = app.history.get(app.analysis_index) {
        let json_data = task
            .verbose_result
            .as_ref()
            .map(|d| d.inner().clone())
            .unwrap_or_else(|| {
                if let Some(err) = &task.error {
                    serde_json::json!({ "error": err })
                } else {
                    serde_json::json!({ "message": "No verbose data captured." })
                }
            });
        app.analysis_tree = Some(JsonTreeState::new(json_data));
    } else {
        app.analysis_tree = None;
    }
}

async fn launch_editor(app: &mut App, terminal: &mut Option<Terminal<CrosstermBackend<Stdout>>>) {
    let EditState::EditingValue { temp_file, .. } = &app.edit_state else {
        return;
    };
    let temp_file = temp_file.clone();

    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
    let status = Command::new(&editor).arg(&temp_file).status();

    let _ = execute!(io::stdout(), EnterAlternateScreen);
    let _ = enable_raw_mode();
    if let Some(t) = terminal {
        let _ = t.clear();
        let _ = t.hide_cursor();
    }

    match status {
        Ok(s) if s.success() => match app.apply_edit() {
            Ok((key, value)) => {
                send_ipc(
                    app,
                    Message::ModifyVar {
                        key: key.clone(),
                        value,
                    },
                )
                .await;
                app.notify(format!("Updated Variable: {}", key));
            }
            Err(e) => app.notify(format!("Edit Failed: {}", e)),
        },
        Ok(_) => {
            app.notify("Editor exited with error".to_string());
            app.cancel_edit();
        }
        Err(_) => {
            app.notify("Failed to launch editor".to_string());
            app.cancel_edit();
        }
    }
}

async fn ask_ai(app: &mut App) {
    let Some(client) = app.ai_client.clone() else {
        return;
    };
    let Some(tx) = app.ipc_tx.clone() else { return };

    app.asking_ai = true;
    app.log(
        "Asking AI Pilot...".to_string(),
        Some(ratatui::style::Color::Magenta),
    );

    let task_name = app
        .failed_task
        .clone()
        .or_else(|| app.current_task.clone())
        .unwrap_or_else(|| "Unknown".to_string());
    let vars = app.task_vars.clone().unwrap_or(serde_json::json!({}));
    let facts = app.facts.clone();

    tokio::spawn(async move {
        if let Ok(analysis) = client
            .analyze_failure(&task_name, "Task Failed", &vars, facts.as_ref())
            .await
        {
            let _ = tx
                .send(Message::AiAnalysis {
                    task: task_name,
                    analysis,
                })
                .await;
        }
    });
}

async fn submit_chat(app: &mut App, ai_tx: &mpsc::Sender<anyhow::Result<crate::ai::ChatMessage>>) {
    let Some(client) = app.ai_client.clone() else {
        app.notify("AI Client not configured.".to_string());
        return;
    };

    let input = app.chat_input.trim().to_string();
    if input.is_empty() {
        return;
    }
    app.chat_input.clear();

    let lower = input.to_lowercase();
    match lower.as_str() {
        "p" | "proceed" => handle_chat_ipc(app, Message::Proceed, "⏩ Proceeding...").await,
        "c" | "continue" => {
            handle_chat_ipc(
                app,
                Message::Continue,
                "▶️ Continuing (skip failed task)...",
            )
            .await
        }
        "r" | "retry" => handle_chat_ipc(app, Message::Retry, "🔄 Retrying task...").await,
        _ if input.starts_with('/') => handle_slash_command(app, &input, &client).await,
        _ => {
            app.chat_loading = true;
            let user_msg = crate::ai::ChatMessage {
                role: "user".to_string(),
                content: input,
                collapsed: false,
            };
            app.chat_history.push(user_msg);
            app.chat_scroll = app.chat_history.len().saturating_sub(1) as u16;

            let history = app.chat_history.clone();
            let tx = ai_tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(client.chat(history).await).await;
            });
        }
    }
}

async fn handle_chat_ipc(app: &mut App, msg: Message, feedback: &str) {
    let content = if app.waiting_for_proceed {
        app.waiting_for_proceed = false;
        send_ipc(app, msg).await;
        feedback.to_string()
    } else {
        "Not waiting for proceed.".to_string()
    };
    push_system_msg(app, content);
}

async fn handle_slash_command(app: &mut App, input: &str, client: &crate::ai::AiClient) {
    let parts: Vec<&str> = input.split_whitespace().collect();
    let content = match parts.first().copied() {
        Some("/model") => {
            if parts.len() == 1 {
                let models = client.list_models().await;
                let current = client.get_model();
                let mut s = String::from("Available Models:\n");
                for (i, m) in models.iter().enumerate() {
                    let marker = if *m == current { " ← current" } else { "" };
                    s.push_str(&format!("  {}. {}{}\n", i + 1, m, marker));
                }
                s.push_str("\nUse /model <name> to switch.");
                s
            } else if let Some(name) = parts.get(1) {
                if let Some(c) = &mut app.ai_client {
                    c.set_model(name);
                }
                format!("Switched to model: {}", name)
            } else {
                "Usage: /model [name]".to_string()
            }
        }
        Some("/context") => {
            let ctx = crate::ai::AiClient::build_context_summary(
                app.current_task.as_deref(),
                app.task_vars.as_ref(),
                app.failed_task.as_deref(),
                app.failed_result.as_ref(),
            );
            format!("📋 Current Context:\n\n{}", ctx)
        }
        Some("/help") => "Chat Commands:\n\
            /model          — List available models\n\
            /model <name>   — Switch to a model\n\
            /context        — Show current task context\n\
            /help           — Show this help\n\
            \nQuick Actions:\n\
            p / proceed     — Proceed to next task\n\
            c / continue    — Continue past failure\n\
            r / retry       — Retry failed task"
            .to_string(),
        _ => format!(
            "Unknown command: {}. Type /help for available commands.",
            input
        ),
    };
    push_system_msg(app, content);
}

fn push_system_msg(app: &mut App, content: String) {
    app.chat_history.push(crate::ai::ChatMessage {
        role: "system".to_string(),
        content,
        collapsed: false,
    });
    app.chat_scroll = app.chat_history.len().saturating_sub(1) as u16;
}
