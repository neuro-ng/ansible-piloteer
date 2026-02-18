use crate::app::{App, ScriptActionType};
use crate::ipc::{IpcServer, Message};
use std::time::Duration;
use tokio::sync::mpsc;

// ── IPC server task ──────────────────────────────────────────────────────────

pub fn spawn_ipc_server(
    socket_path: String,
    bind_addr: Option<String>,
    secret_token: Option<String>,
    to_app_tx: mpsc::Sender<Message>,
    mut from_app_rx: mpsc::Receiver<Message>,
) {
    tokio::spawn(async move {
        let server = match IpcServer::new(&socket_path, bind_addr.as_deref()).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to start IPC: {}", e);
                return;
            }
        };

        loop {
            match server.accept().await {
                Ok(mut conn) => {
                    let mut connected = true;
                    while connected {
                        tokio::select! {
                            incoming = conn.receive() => match incoming {
                                Ok(Some(msg)) => {
                                    if let Message::Handshake { token } = &msg {
                                        let ok = match &secret_token {
                                            Some(expected) => token.as_deref() == Some(expected),
                                            None => true,
                                        };
                                        if !ok {
                                            eprintln!("Authentication Failed: Invalid Token");
                                            break;
                                        }
                                        let _ = to_app_tx.send(Message::Handshake { token: token.clone() }).await;
                                        continue;
                                    }
                                    if to_app_tx.send(msg).await.is_err() {
                                        return;
                                    }
                                }
                                Ok(None) => {
                                    let _ = to_app_tx.send(Message::ClientDisconnected).await;
                                    connected = false;
                                }
                                Err(_) => {
                                    let _ = to_app_tx.send(Message::ClientDisconnected).await;
                                    connected = false;
                                }
                            },
                            outgoing = from_app_rx.recv() => match outgoing {
                                Some(msg) => {
                                    if conn.send(&msg).await.is_err() {
                                        let _ = to_app_tx.send(Message::ClientDisconnected).await;
                                        connected = false;
                                    }
                                }
                                None => return,
                            },
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Accept error: {}", e);
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                }
            }
        }
    });
}

// ── IPC message handler ──────────────────────────────────────────────────────

pub async fn handle_message(app: &mut App, msg: Message, headless: bool, auto_analyze: bool) {
    match msg {
        Message::Handshake { .. } => {
            app.client_connected = true;
            app.log("Connected".to_string(), Some(ratatui::style::Color::Cyan));
            if headless {
                println!("Headless: Ansible Connected");
            }

            let span = crate::telemetry::create_root_span(
                "playbook.execution",
                vec![
                    opentelemetry::KeyValue::new("service.name", "ansible-piloteer"),
                    opentelemetry::KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                ],
            );
            app.playbook_span_guard = Some(crate::telemetry::attach_span(span));
            send_ipc(app, Message::Proceed).await;
        }

        Message::PlayStart { name, host_pattern } => {
            app.log(
                format!("Play Started: {} (Hosts: {})", name, host_pattern),
                Some(ratatui::style::Color::Cyan),
            );
            app.play_span_guard = None;
            app.play_span = None;

            let span = crate::telemetry::create_child_span(
                format!("play: {}", name),
                vec![
                    opentelemetry::KeyValue::new("play.name", name.clone()),
                    opentelemetry::KeyValue::new("play.hosts", host_pattern.clone()),
                ],
            );
            app.play_span_guard = Some(crate::telemetry::attach_span(span));
            send_ipc(app, Message::Proceed).await;
        }

        Message::TaskStart {
            name,
            task_vars,
            facts,
        } => {
            app.log(
                format!("Task: {}", name),
                Some(ratatui::style::Color::White),
            );
            app.task_start_time = Some(std::time::Instant::now());
            app.set_task(name.clone(), task_vars.clone(), facts.clone());

            let task_span = crate::telemetry::create_child_span(
                format!("task: {}", name),
                vec![opentelemetry::KeyValue::new("task.name", name.clone())],
            );
            app.task_spans.insert(name.clone(), task_span);

            if let Some(idx) = app
                .test_script
                .iter()
                .position(|a| a.task_name == *name && !a.on_failure)
            {
                println!("Headless: Executing Script Action for TaskStart: {}", name);
                let script = app.test_script.remove(idx);
                run_script_actions(app, script.actions).await;
            } else if headless {
                println!("Headless: Task Captured: {}", name);
                tokio::time::sleep(Duration::from_millis(500)).await;
                app.waiting_for_proceed = false;
                send_ipc(app, Message::Proceed).await;
                println!("Headless: Auto-Proceeding...");
            } else if app.breakpoints.contains(&name) {
                app.waiting_for_proceed = true;
                app.log(
                    format!("Breakpoint Hit: {}", name),
                    Some(ratatui::style::Color::Magenta),
                );
                app.notify(format!("Breakpoint Hit: {}", name));
            } else {
                app.waiting_for_proceed = false;
                send_ipc(app, Message::Proceed).await;
            }
        }

        Message::TaskFail {
            name,
            result: _,
            facts,
        } => {
            app.log(
                format!("Task Failed: {}", name),
                Some(ratatui::style::Color::Red),
            );

            if let Some(span) = app.task_spans.get_mut(&name) {
                crate::telemetry::record_error_on_span(span, &format!("Task '{}' failed", name));
                crate::telemetry::add_span_attributes(
                    span,
                    vec![opentelemetry::KeyValue::new("task.failed", true)],
                );
            }

            if headless {
                println!("Headless: Task Failed: {}", name);
                handle_headless_failure(app, &name, auto_analyze).await;
            } else {
                app.set_failed(name, serde_json::Value::Null, facts.clone());
            }
        }

        Message::TaskResult {
            name,
            host,
            changed,
            failed,
            verbose_result,
        } => {
            let (status, color) = task_status(failed, changed);
            app.log(
                format!("Task '{}' on {}: {}", name, host, status),
                Some(color),
            );

            if let Some(span) = app.task_spans.remove(&name) {
                crate::telemetry::end_span(
                    span,
                    vec![
                        opentelemetry::KeyValue::new("task.host", host.clone()),
                        opentelemetry::KeyValue::new("task.changed", changed),
                        opentelemetry::KeyValue::new("task.failed", failed),
                        opentelemetry::KeyValue::new("task.status", status),
                    ],
                );
            }

            let duration = app
                .task_start_time
                .map(|t| t.elapsed().as_secs_f64())
                .unwrap_or(0.0);
            app.record_task_result(
                name.clone(),
                host,
                changed,
                failed,
                duration,
                None,
                verbose_result,
                None,
            );

            if headless {
                println!("Headless: Task Result: {}", status);
            }
        }

        Message::TaskUnreachable {
            name,
            host,
            error,
            result,
        } => {
            app.set_unreachable(name.clone(), host.clone(), error.clone(), result);

            if headless {
                println!("Headless: Host {} unreachable: {}", host, error);

                if let Some(idx) = app
                    .test_script
                    .iter()
                    .position(|a| a.task_name == *name && a.on_failure)
                {
                    println!(
                        "Headless: Executing Script Action for TaskUnreachable: {}",
                        name
                    );
                    let script = app.test_script.remove(idx);
                    run_script_actions(app, script.actions).await;
                }
            }
        }

        Message::PlayRecap { stats } => {
            app.log(
                format!("Play Recap Received: {:?}", stats),
                Some(ratatui::style::Color::Cyan),
            );
            app.play_span_guard = None;
            app.play_span = None;
            app.record_task_result(
                "Play Recap".to_string(),
                "all".to_string(),
                false,
                false,
                0.0,
                None,
                Some(crate::execution::ExecutionDetails::new(stats.clone())),
                None,
            );
            app.play_recap = Some(stats.clone());
            app.set_task(
                "Playbook Complete".to_string(),
                serde_json::Value::Null,
                None,
            );
            app.playbook_span_guard = None;
        }

        Message::AiAnalysis { task, analysis } => {
            app.asking_ai = false;
            app.suggestion = Some(analysis.clone());
            app.log(
                format!("AI Analysis Received for '{}'", task),
                Some(ratatui::style::Color::Cyan),
            );
            if let Some(item) = app.history.iter_mut().rev().find(|t| t.name == task) {
                item.analysis = Some(analysis);
            }
            app.notify("AI Analysis Ready. Press 'v' to view.".to_string());
        }

        Message::ClientDisconnected => {
            app.client_connected = false;
            app.log(
                "Client Disconnected".to_string(),
                Some(ratatui::style::Color::Red),
            );
            if headless {
                println!("Headless: Client Disconnected");
            }
        }

        Message::ModifyVar { .. } | Message::Proceed | Message::Retry | Message::Continue => {}
    }
}

// ── Private helpers ──────────────────────────────────────────────────────────

async fn send_ipc(app: &App, msg: Message) {
    if let Some(tx) = &app.ipc_tx {
        let _ = tx.send(msg).await;
    }
}

fn task_status(failed: bool, changed: bool) -> (&'static str, ratatui::style::Color) {
    if failed {
        ("FAILED", ratatui::style::Color::Red)
    } else if changed {
        ("CHANGED", ratatui::style::Color::Yellow)
    } else {
        ("OK", ratatui::style::Color::Green)
    }
}

async fn run_script_actions(app: &mut App, actions: Vec<ScriptActionType>) {
    for action in actions {
        match action {
            ScriptActionType::Pause => {
                println!("Headless: Pausing (Scripted)...");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            ScriptActionType::Continue => {
                send_ipc(app, Message::Continue).await;
            }
            ScriptActionType::Resume => {
                println!("Headless: Resuming (Scripted)...");
                send_ipc(app, Message::Proceed).await;
            }
            ScriptActionType::Retry => {
                send_ipc(app, Message::Retry).await;
            }
            ScriptActionType::EditVar { key, value } => {
                println!("Headless: ModifyVar {} = {}", key, value);
                send_ipc(app, Message::ModifyVar { key, value }).await;
            }
            ScriptActionType::ExecuteCommand { cmd } => {
                println!("Headless: Executing Command: {}", cmd);
                let result = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .output();
                match result {
                    Ok(o) => println!(" Command Finished: status={}", o.status),
                    Err(e) => println!(" Command Failed: {}", e),
                }
            }
            ScriptActionType::AskAi => {
                println!("Headless: Asking AI (Scripted)...");
                if let Some(client) = &app.ai_client.clone() {
                    let vars = app.task_vars.clone().unwrap_or(serde_json::json!({}));
                    let facts = app.facts.clone();
                    let task = app.current_task.clone().unwrap_or_default();
                    match client
                        .analyze_failure(&task, "Task Failed", &vars, facts.as_ref())
                        .await
                    {
                        Ok(analysis) => {
                            println!(
                                "Headless: AI Analysis Received: {:.50}...",
                                analysis.analysis
                            );
                            app.suggestion = Some(analysis);
                        }
                        Err(e) => println!("Headless: AI Request Failed: {}", e),
                    }
                }
            }
            ScriptActionType::ApplyFix => {}
            ScriptActionType::AssertAiContext { contains } => {
                println!("Headless: Asserting AI Context...");
                match (&app.suggestion, contains) {
                    (Some(s), Some(text)) => {
                        if s.analysis.contains(&text) {
                            println!("Headless: Assertion PASSED: Analysis contains '{}'", text);
                        } else {
                            println!(
                                "Headless: Assertion FAILED: Analysis does NOT contain '{}'",
                                text
                            );
                        }
                    }
                    (Some(_), None) => println!("Headless: Assertion PASSED: AI Context present."),
                    (None, _) => println!("Headless: Assertion FAILED: No AI Context found."),
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn handle_headless_failure(app: &mut App, name: &str, auto_analyze: bool) {
    let client = app.ai_client.clone();
    let vars = app.task_vars.clone().unwrap_or(serde_json::json!({}));
    let facts = app.facts.clone();

    if let Some(idx) = app
        .test_script
        .iter()
        .position(|a| a.task_name == *name && a.on_failure)
    {
        println!("Headless: Executing Script Action for TaskFail: {}", name);
        let script = app.test_script.remove(idx);
        run_script_actions(app, script.actions).await;
    } else if let Some(client) = client {
        if auto_analyze {
            println!("Headless: Analyzing Failure...");
            if let Ok(analysis) = client
                .analyze_failure(name, "Task Failed", &vars, facts.as_ref())
                .await
            {
                println!("\n🤖 AI ANALYSIS:\n{}\n", analysis.analysis);
                if let Some(fix) = &analysis.fix {
                    println!("💡 SUGGESTED FIX: {} = {}\n", fix.key, fix.value);
                }
                if let Some(tx) = &app.ipc_tx {
                    let _ = tx
                        .send(Message::AiAnalysis {
                            task: name.to_string(),
                            analysis,
                        })
                        .await;
                }
            }
        } else if let Ok(analysis) = client
            .analyze_failure(name, "Task Failed", &vars, facts.as_ref())
            .await
        {
            println!("Headless: AI Analysis Tokens: {}", analysis.tokens_used);
        }
    }

    // Default headless recovery: reset var and retry
    send_ipc(
        app,
        Message::ModifyVar {
            key: "should_fail".to_string(),
            value: serde_json::json!(false),
        },
    )
    .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    send_ipc(app, Message::Retry).await;
}
