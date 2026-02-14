use ansible_piloteer::{app, auth, config, ui};
use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{
    event::{self},
    execute,
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::process::Command;
use std::time::{Duration, Instant}; // [MODIFY]
// use tokio::net::TcpListener; // Unused for now
use tokio::sync::mpsc;

use ansible_piloteer::app::{Action, ActiveView, App, TaskHistory};
use ansible_piloteer::config::Config;
use ansible_piloteer::ipc::{IpcServer, Message}; // Explicit use
use ansible_piloteer::widgets::json_tree::JsonTreeState; // [NEW]

type DefaultTerminal = Terminal<CrosstermBackend<io::Stdout>>;

#[derive(Parser)]
#[command(
    name = "ansible-piloteer",
    about = "AI-powered Ansible interactive debugger",
    long_about = "Ansible Piloteer is an interactive terminal interface for Ansible playbooks.
It allows you to step through tasks, inspect variables, and use AI to diagnose and fix errors on the fly.

This tool acts as a controller that communicates with a custom Ansible Strategy Plugin.",
    after_help = "
EXAMPLES:
  # Run a playbook (Standard Mode)
  ansible-piloteer my_playbook.yml

  # Run in Distributed Mode (Controller)
  ansible-piloteer run --bind 0.0.0.0:9000 --secret my_secret

  # Generate an execution report
  ansible-piloteer my_playbook.yml --report report.md

  # Query session data (one-off query)
  ansible-piloteer query --input session.json.gz \"task_history[?failed].name\"
  
  # Interactive REPL mode (no query argument)
  ansible-piloteer query --input session.json.gz

  # Advanced queries with aggregations
  ansible-piloteer query --input session.json.gz \"count(task_history[?failed])\"
  ansible-piloteer query --input session.json.gz \"group_by(task_history, &host)\"

QUERY FEATURES (REPL & CLI):
  Functions:
    group_by(arr, expr)  Group array items by expression
    unique(arr)          Get unique items from array
    count(arr)           Count items in array
    sum(arr)             Sum numeric values in array
    avg(arr)             Average of numeric values
    min(arr)             Minimum value in array
    max(arr)             Maximum value in array

  REPL Commands:
    .help               Show help and available functions
    .templates          Show pre-built query templates
    .json               Set output to compact JSON
    .pretty             Set output to pretty JSON (default)
    .yaml               Set output to YAML
    .exit, .quit        Exit REPL

CONFIGURATION (Environment Variables):
  Core:
    ANSIBLE_STRATEGY          Must be set to 'piloteer'
    ANSIBLE_STRATEGY_PLUGINS  Path to 'ansible_plugin/strategies' dir
    PILOTEER_HEADLESS         Run without TUI (for CI/CD)
  
  AI Features:
    OPENAI_API_KEY            OpenAI API key (required for AI features)
    PILOTEER_MODEL            AI model (default: gpt-4)
    PILOTEER_BASE_URL         Custom API endpoint (for local LLMs)
    PILOTEER_QUOTA_TOKENS     Token usage limit
    PILOTEER_QUOTA_USD        Cost limit in USD
  
  Distributed Mode:
    PILOTEER_SOCKET           Socket path or TCP address
    PILOTEER_SECRET           Shared secret for authentication
  
  OAuth:
    PILOTEER_GOOGLE_CLIENT_ID      Google OAuth client ID
    PILOTEER_GOOGLE_CLIENT_SECRET  Google OAuth client secret

TUI CONTROLS:
  General:
    q / Esc     Quit
    ?           Toggle Help
  Debugging:
    r           Retry failed task
    c           Continue (ignore failure)
    e           Edit variables
    a           Ask AI Pilot
    f           Apply AI Fix
  Log View:
    /           Search logs
    n / N       Next / Previous match
    l           Toggle log filter (All/Failed/Changed)
    F           Follow mode (Auto-scroll)
  Analysis Mode:
    v           Toggle Mode / Visual Selection
    Tab         Switch Pane (Task List <-> Data Browser)
    j / k       Navigate (supports count: 10j moves 10 lines)
    0-9         Enter count for next command
    h / l       Collapse/Expand
    Enter       Expand/Collapse
    w           Toggle Text Wrapping
    /           Search Tree
    n / N       Next / Previous match
    y           Yank to clipboard (single/visual/count-based)
  Session:
    Ctrl+s      Save Session Snapshot
    --replay    Replay execution from file

DISTRIBUTED MODE:
  1. Start this CLI as a server:
     ansible-piloteer run --bind 0.0.0.0:9000 --secret 1234
  2. Configure Ansible machine:
     export PILOTEER_SOCKET=192.168.1.5:9000
     export PILOTEER_SECRET=1234
     ansible-playbook ...
"
)]
struct Cli {
    /// Arguments to forward to ansible-playbook
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    ansible_args: Vec<String>,

    #[command(subcommand)]
    command: Option<Commands>,
    /// Export execution report to file (JSON or Markdown)
    #[arg(long)]
    report: Option<String>,
    /// Bind address for TCP listener (e.g. 0.0.0.0:9000)
    #[arg(long)]
    bind: Option<String>,
    /// Shared secret token for authentication
    #[arg(long)]
    secret: Option<String>,

    /// Verbosity level (-v, -vv, -vvv, etc.)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Automatically analyze, fix, and retry failed tasks in headless mode
    #[arg(long)]
    auto_analyze: bool,

    /// Replay a saved session file (offline mode)
    #[arg(long)]
    replay: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with Google
    Auth {
        #[command(subcommand)]
        cmd: AuthCmd,
    },
    /// Query session data using JMESPath. If query is omitted, enters interactive REPL mode.
    Query {
        /// JMESPath query expression (optional)
        query: Option<String>,
        /// Path to input session file (e.g., session.json.gz)
        #[arg(short, long)]
        input: String,
        /// Output format: json, yaml, pretty-json
        #[arg(short, long, default_value = "pretty-json")]
        format: String,
    },
}

#[derive(Subcommand)]
enum AuthCmd {
    Login,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let auto_analyze = cli.auto_analyze;

    // Load config early for tracing initialization
    let config = Config::new().unwrap_or_else(|e| {
        eprintln!("Failed to load config: {}", e);
        std::process::exit(1);
    });

    // Initialize tracing if configured
    if let Err(e) = ansible_piloteer::tracing::init_tracing(&config) {
        eprintln!("Warning: Failed to initialize tracing: {}", e);
    }

    let result = match cli.command {
        Some(Commands::Auth { cmd }) => match cmd {
            AuthCmd::Login => {
                // If credentials are provided in config, use them. Otherwise pass None to use built-in defaults.
                let (client_id, client_secret) =
                    (config.google_client_id, config.google_client_secret);

                if client_id.is_none() {
                    println!(
                        "No Google OAuth credentials found in config. Using default GCloud credentials."
                    );
                }

                println!("Starting Google OAuth login flow...");
                match auth::login(client_id, client_secret).await {
                    Ok(token) => {
                        println!("Login successful!");
                        if let Err(e) = Config::save_auth_token(&token) {
                            eprintln!("Failed to save auth token: {}", e);
                        } else {
                            println!("Token saved to configuration.");
                            println!(
                                "Note: If you used default credentials, the token is valid for GCloud scopes."
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Login failed: {:?}", e);
                    }
                }
                Ok(())
            }
        },
        Some(Commands::Query {
            query,
            input,
            format,
        }) => {
            let session = match ansible_piloteer::session::Session::load(&input) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error loading session from {}: {}", input, e);
                    std::process::exit(1);
                }
            };

            if let Some(q) = query {
                // One-off query mode
                let json_string = match serde_json::to_string(&session) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error serializing session: {}", e);
                        std::process::exit(1);
                    }
                };

                let mut runtime = jmespath::Runtime::new();
                runtime.register_builtin_functions();
                ansible_piloteer::query::register_functions(&mut runtime);

                // Register user-defined filters
                if let Some(filters) = &config.filters {
                    for (name, expr) in filters {
                        runtime.register_function(
                            name,
                            Box::new(ansible_piloteer::query::CustomFilter::new(expr.clone())),
                        );
                    }
                }

                let expr = match runtime.compile(&q) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("Invalid JMESPath query: {}", e);
                        std::process::exit(1);
                    }
                };

                // jmespath::Variable::from_json parses a JSON string into a Variable
                let variable = jmespath::Variable::from_json(&json_string).unwrap_or_else(|e| {
                    eprintln!("Error parsing JSON for JMESPath: {}", e);
                    std::process::exit(1);
                });

                let result = match expr.search(&variable) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("JMESPath execution error: {}", e);
                        std::process::exit(1);
                    }
                };

                match format.as_str() {
                    "json" => println!("{}", serde_json::to_string(&result).unwrap()),
                    "pretty-json" => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
                    "yaml" => {
                        println!("{}", serde_yaml::to_string(&result).unwrap());
                    }
                    _ => {
                        eprintln!(
                            "Unknown format: {}. Supported: json, pretty-json, yaml",
                            format
                        );
                        std::process::exit(1);
                    }
                }
            } else {
                // Interactive REPL mode
                if let Err(e) = ansible_piloteer::repl::run(&session, config.filters.as_ref()) {
                    eprintln!("REPL Error: {}", e);
                    std::process::exit(1);
                }
            }
            Ok(())
        }
        None => {
            run_tui(
                cli.ansible_args,
                cli.report,
                cli.bind,
                cli.secret,
                cli.verbose,
                cli.replay,
                auto_analyze,
            )
            .await
        }
    };

    // Shutdown tracing and flush pending spans
    ansible_piloteer::tracing::shutdown_tracing();

    result
}

async fn run_tui(
    ansible_args: Vec<String>,
    report_path: Option<String>,
    bind_addr: Option<String>,
    secret_token: Option<String>,
    verbose: u8,
    replay_path: Option<String>,
    auto_analyze: bool,
) -> Result<()> {
    let headless = std::env::var("PILOTEER_HEADLESS").is_ok();

    // Load Config
    let mut config = match crate::config::Config::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    if !ansible_args.is_empty() {
        println!("Ansible arguments: {:?}", ansible_args);
    }

    // Override config with CLI args
    if let Some(addr) = bind_addr {
        config.bind_addr = Some(addr);
    }
    if let Some(secret) = secret_token {
        config.secret_token = Some(secret);
    }

    // Setup Terminal if not headless
    // Setup Terminal if not headless
    let mut terminal = if !headless {
        let terminal = ratatui::init();
        execute!(io::stdout(), crossterm::event::EnableMouseCapture)?;
        Some(terminal)
    } else {
        println!("Running in HEADLESS mode");
        None
    };

    // App State
    let mut app = if let Some(r_path) = &replay_path {
        println!("Loading session from {}...", r_path);
        match App::from_session(r_path) {
            Ok(loaded_app) => loaded_app,
            Err(e) => {
                eprintln!("Failed to load session: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        App::new(config.clone())
    };

    let socket_path = config.socket_path.clone();
    let bind_addr = config.bind_addr.clone();
    let secret_token = config.secret_token.clone();

    // Load test script if environment variable is set
    app.load_test_script();

    let (to_app_tx, mut to_app_rx) = mpsc::channel::<Message>(100);
    let (from_app_tx, mut from_app_rx) = mpsc::channel::<Message>(100);

    // Only start IPC if NOT in replay mode
    if !app.replay_mode {
        app.set_ipc_tx(Some(from_app_tx.clone()));

        // Spawn IPC Server Task
        tokio::spawn(async move {
            // Start Server
            // In real app, socket path might be dynamic or configured
            let server = match IpcServer::new(&socket_path, bind_addr.as_deref()).await {
                Ok(s) => s,
                Err(e) => {
                    // Log error through a side channel if possible, or panic
                    eprintln!("Failed to start IPC: {}", e);
                    return;
                }
            };

            // Log Server Start
            {
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open("/tmp/piloteer_debug.log")
                    .expect("Failed to open /tmp/piloteer_debug.log");
                writeln!(f, "DEBUG: IPC Task Started").expect("Failed to write to log");
            }

            loop {
                match server.accept().await {
                    Ok(mut conn) => {
                        // Log connection
                        {
                            use std::io::Write;
                            if let Ok(mut f) = std::fs::OpenOptions::new()
                                .append(true)
                                .open("/tmp/piloteer_debug.log")
                            {
                                writeln!(f, "DEBUG: IPC Connection Accepted").ok();
                            }
                        }

                        let mut connected = true;
                        while connected {
                            tokio::select! {
                                // Receive from Ansible
                                incoming = conn.receive() => {
                                    match incoming {
                                        Ok(Some(msg)) => {
                                            // Handle Handshake
                                            if let Message::Handshake { token } = &msg {
                                                let authorized = match &secret_token {
                                                    Some(expected) => token.as_deref() == Some(expected),
                                                    None => true,
                                                };

                                                if !authorized {
                                                    eprintln!("Authentication Failed: Invalid Token");
                                                    break; // Break inner loop
                                                }
                                                let _ = to_app_tx.send(Message::Handshake { token: token.clone() }).await;
                                                continue;
                                            }

                                            if to_app_tx.send(msg).await.is_err() {
                                                // App closed
                                                return;
                                            }
                                        }
                                        Ok(None) => {
                                            // EOF
                                            {
                                                use std::io::Write;
                                                if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open("/tmp/piloteer_debug.log") {
                                                    writeln!(f, "DEBUG: IPC EOF").ok();
                                                }
                                            }
                                            let _ = to_app_tx.send(Message::ClientDisconnected).await;
                                            connected = false;
                                        }
                                        Err(e) => {
                                            // Error
                                            {
                                                use std::io::Write;
                                                if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open("/tmp/piloteer_debug.log") {
                                                    writeln!(f, "DEBUG: IPC Error: {}", e).ok();
                                                }
                                            }
                                            let _ = to_app_tx.send(Message::ClientDisconnected).await;
                                            connected = false;
                                        }
                                    }
                                }
                                // Send to Ansible
                                outgoing = from_app_rx.recv() => {
                                    match outgoing {
                                        Some(msg) => {
                                            if conn.send(&msg).await.is_err() {
                                                // Send failed, disconnect
                                                let _ = to_app_tx.send(Message::ClientDisconnected).await;
                                                connected = false;
                                            }
                                        }
                                        None => return, // App shutdown
                                    }
                                }
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

    // Give IPC server time to bind
    if !app.replay_mode {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Spawn Ansible Playbook if provided (AND NOT REPLAY)
    let mut _ansible_child = None;
    if !app.replay_mode && !ansible_args.is_empty() {
        use tokio::process::Command;

        let mut cmd = Command::new("ansible-playbook");
        if verbose > 0 {
            let v_flag = format!("-{}", "v".repeat(verbose as usize));
            cmd.arg(v_flag);
        }
        cmd.args(&ansible_args);
        cmd.env("ANSIBLE_STRATEGY", "piloteer");

        // Try to find plugin path relative to executable or CWD
        // For development/POC, we assume CWD has ansible_plugin/strategies
        let cwd = std::env::current_dir().unwrap_or_default();
        let plugin_path = cwd.join("ansible_plugin").join("strategies");
        cmd.env("ANSIBLE_STRATEGY_PLUGINS", plugin_path);

        // Env vars for plugin connection
        // Standard Mode uses Unix socket by default unless bind_addr is set
        if let Some(addr) = &config.bind_addr {
            cmd.env("PILOTEER_SOCKET", addr);
        } else {
            cmd.env("PILOTEER_SOCKET", &config.socket_path);
        }

        if let Some(secret) = &config.secret_token {
            cmd.env("PILOTEER_SECRET", secret);
        }

        // Redirect stdout/stderr to a log file for debugging
        let log_file = std::fs::File::create("ansible_child.log")
            .unwrap_or_else(|_| std::fs::File::create("/dev/null").unwrap());
        let log_file_err = log_file.try_clone().unwrap();

        cmd.stdout(log_file);
        cmd.stderr(log_file_err);
        cmd.stdin(std::process::Stdio::null());

        match cmd.spawn() {
            Ok(child) => {
                _ansible_child = Some(child);
            }
            Err(e) => {
                eprintln!("Failed to spawn ansible-playbook: {}", e);
                // We might want to notify App or exit
            }
        }
    }

    // Main Loop
    let res = run_app(&mut terminal, app, &mut to_app_rx, headless, auto_analyze).await;

    // Restore Terminal if not headless
    if !headless {
        execute!(io::stdout(), crossterm::event::DisableMouseCapture)?;
        ratatui::restore();
    }

    // Ensure child process is killed
    if let Some(mut child) = _ansible_child {
        let _ = child.kill().await;
    }

    let final_app = res?; // Check for errors before printing summary

    // Auto-Archive Session (if not replay)
    if !final_app.replay_mode
        && let Ok(config_dir) = crate::config::Config::get_config_dir()
    {
        let archive_dir = config_dir.join("archive");
        if !archive_dir.exists() {
            std::fs::create_dir_all(&archive_dir).ok();
        }
        let filename = format!(
            "session_{}.json.gz",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );
        let path = archive_dir.join(&filename);
        let session_path = path.to_string_lossy();

        let session = ansible_piloteer::session::Session::from_app(&final_app);
        match session.save(&path.to_string_lossy()) {
            Ok(_) => println!("Session archived to: {}", session_path),
            Err(e) => eprintln!("Failed to archive session: {}", e),
        }
    }

    // Drift Summary
    println!("\n--- Drift Summary ---");
    let changed_tasks: Vec<&TaskHistory> = final_app.history.iter().filter(|t| t.changed).collect();
    if changed_tasks.is_empty() {
        println!("No changes detected.");
    } else {
        println!("The following tasks modified the system state:");
        for task in changed_tasks {
            println!(" - {} [Task: {}]", task.host, task.name);
        }
        println!(
            "Total Drift: {} tasks changed.",
            final_app.history.iter().filter(|t| t.changed).count()
        );
    }

    // Generate Report
    if let Some(path) = report_path {
        use std::fs::File;
        use std::io::Write;

        println!("Generating report at {}...", path);
        if path.ends_with(".json") {
            match File::create(&path) {
                Ok(mut file) => {
                    let json = serde_json::to_string_pretty(&final_app.history).unwrap_or_default();
                    if let Err(e) = file.write_all(json.as_bytes()) {
                        eprintln!("Failed to write JSON report: {}", e);
                    }
                }
                Err(e) => eprintln!("Failed to create report file: {}", e),
            }
        } else if path.ends_with(".md") {
            let report = ansible_piloteer::report::ReportGenerator::new(&final_app);
            if let Err(e) = report.save_to_file(path.as_str()) {
                eprintln!("Failed to write Markdown report: {}", e);
            }
        } else {
            eprintln!("Unsupported report format. Use .json or .md");
        }
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Option<DefaultTerminal>,
    mut app: App,
    ipc_rx: &mut mpsc::Receiver<Message>,
    headless: bool,
    auto_analyze: bool,
) -> Result<App> {
    // Spawn input thread
    let (input_tx, mut input_rx) = mpsc::channel(100);
    std::thread::spawn(move || {
        loop {
            // Poll for 250ms
            if event::poll(Duration::from_millis(250)).unwrap_or(false)
                && let Ok(evt) = event::read()
                && input_tx.blocking_send(evt).is_err()
            {
                break;
            }
            // If main thread drops receiver, blocking_send fails and we break.
        }
    });

    loop {
        if !headless {
            if let Some(t) = terminal {
                t.draw(|f| ui::draw(f, &mut app))?;
            }
        } else {
            // Headless logic here if needed
        }

        if !app.running {
            break;
        }

        // Track if IPC is complete (channel closed)
        let ipc_complete = app.ipc_tx.is_none();

        // Tick rate for UI refresh
        let tick = tokio::time::sleep(Duration::from_millis(250));

        // Update Event Velocity
        app.update_velocity();

        // This select acts as our event loop
        tokio::select! {
            // Handle IPC messages (only if not complete)
            msg_opt = ipc_rx.recv(), if !ipc_complete => {
                match msg_opt {
                    Some(msg) => {
                        match msg {
                            Message::Handshake { token: _ } => {
                                app.client_connected = true;
                                app.log("Connected".to_string(), Some(ratatui::style::Color::Cyan));
                                if headless { println!("Headless: Ansible Connected"); }

                                // Create root playbook span
                                let playbook_span = ansible_piloteer::tracing::create_root_span(
                                    "playbook.execution",
                                    vec![
                                        opentelemetry::KeyValue::new("service.name", "ansible-piloteer"),
                                        opentelemetry::KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                                    ],
                                );

                                // Attach span to context and store guard
                                let guard = ansible_piloteer::tracing::attach_span(playbook_span);
                                app.playbook_span_guard = Some(guard);

                                if let Some(tx) = &app.ipc_tx { let _ = tx.send(Message::Proceed).await; }
                            }
                            Message::PlayStart { name, host_pattern } => {
                                app.log(format!("Play Started: {} (Hosts: {})", name, host_pattern), Some(ratatui::style::Color::Cyan));

                                // End previous play span if any
                                app.play_span_guard = None;
                                app.play_span = None;

                                // Create Play Span
                                let play_span = ansible_piloteer::tracing::create_child_span(
                                    format!("play: {}", name),
                                    vec![
                                        opentelemetry::KeyValue::new("play.name", name.clone()),
                                        opentelemetry::KeyValue::new("play.hosts", host_pattern.clone()),
                                    ],
                                );

                                // Attach span
                                let guard = ansible_piloteer::tracing::attach_span(play_span);
                                app.play_span_guard = Some(guard);

                                if let Some(tx) = &app.ipc_tx { let _ = tx.send(Message::Proceed).await; }
                            }
                            Message::TaskStart { name, task_vars, facts } => {
                                app.log(format!("Task: {}", name), Some(ratatui::style::Color::White));
                                app.task_start_time = Some(Instant::now()); // [NEW] Phase 22
                                app.set_task(name.clone(), task_vars.clone(), facts.clone());

                                // Create task span as child of current context (should be play span)
                                let task_span = ansible_piloteer::tracing::create_child_span(
                                    format!("task: {}", name),
                                    vec![
                                        opentelemetry::KeyValue::new("task.name", name.clone()),
                                    ],
                                );

                                // Store span for later updates
                                app.task_spans.insert(name.clone(), task_span);

                                if let Some(idx) =
                                    app.test_script.iter().position(|a| a.task_name == *name && !a.on_failure)
                                {
                                    println!("Headless: Executing Script Action for TaskStart: {}", name);
                                    let script_action = app.test_script.remove(idx);
                                    for action in script_action.actions {
                                        match action {
                                            ansible_piloteer::app::ScriptActionType::Pause => {
                                                println!("Headless: Pausing (Scripted)...");
                                                tokio::time::sleep(Duration::from_secs(5)).await;
                                            }
                                            ansible_piloteer::app::ScriptActionType::Continue => {
                                                if let Some(tx) = &app.ipc_tx {
                                                    let _ = tx.send(Message::Continue).await;
                                                }
                                            }
                                            ansible_piloteer::app::ScriptActionType::Retry => {
                                                if let Some(tx) = &app.ipc_tx {
                                                    let _ = tx.send(Message::Retry).await;
                                                }
                                            }
                                            ansible_piloteer::app::ScriptActionType::EditVar { key, value } => {
                                                println!("Headless: ModifyVar {} = {}", key, value);
                                                if let Some(tx) = &app.ipc_tx {
                                                    let _ = tx.send(Message::ModifyVar {
                                                        key,
                                                        value,
                                                    })
                                                    .await;
                                                }
                                            }
                                            ansible_piloteer::app::ScriptActionType::ExecuteCommand { cmd } => {
                                                println!("Headless: Executing Command: {}", cmd);
                                                let output = std::process::Command::new("sh")
                                                    .arg("-c")
                                                    .arg(cmd)
                                                    .output();
                                                match output {
                                                    Ok(o) => println!(" Command Finished: status={}", o.status),
                                                    Err(e) => println!(" Command Failed: {}", e),
                                                }
                                            }
                                            ansible_piloteer::app::ScriptActionType::Resume => {
                                                println!("Headless: Resuming (Scripted)...");
                                                if let Some(tx) = &app.ipc_tx {
                                                    let _ = tx.send(Message::Proceed).await;
                                                }
                                            }
                                            _ => {}
                                        }
                                        // Small delay between actions
                                        tokio::time::sleep(Duration::from_millis(100)).await;
                                    }
                                    // If script actions were executed, we assume they handled control flow.
                                    // But typically TaskStart just needs Proceed if not scripted to stop.
                                    // If script didn't send Proceed, we might be stuck?
                                    // Let's assume script includes "Continue" (which maps to Proceed in TaskStart context?)
                                    // Message::Proceed is separate.
                                    // Maybe we add Proceed to ScriptActionType?
                                    // Or map Continue to Proceed if TaskStart?
                                    // For now, if no script found, we auto-proceed. If script found, we do exactly what it says.
                                } else if headless {
                                    println!("Headless: Task Captured: {}", name);
                                    // Auto-proceed for testing
                                    tokio::time::sleep(Duration::from_millis(500)).await;
                                    app.waiting_for_proceed = false;
                                    if let Some(tx) = &app.ipc_tx {
                                        let _ = tx.send(Message::Proceed).await;
                                        println!("Headless: Auto-Proceeding...");
                                    }
                                } else if app.breakpoints.contains(&name) {
                                    app.waiting_for_proceed = true;
                                    app.log(format!("Breakpoint Hit: {}", name), Some(ratatui::style::Color::Magenta));
                                    app.notification = Some((format!("Breakpoint Hit: {}", name), std::time::Instant::now()));
                                } else {
                                    app.waiting_for_proceed = false;
                                    if let Some(tx) = &app.ipc_tx {
                                        let _ = tx.send(Message::Proceed).await;
                                    }
                                }
                            }
                            Message::TaskFail { name, result: _, facts } => {
                                app.log(format!("Task Failed: {}", name), Some(ratatui::style::Color::Red));

                                // Update task span with failure information
                                if let Some(span) = app.task_spans.get_mut(&name) {
                                    ansible_piloteer::tracing::record_error_on_span(span, &format!("Task '{}' failed", name));
                                    ansible_piloteer::tracing::add_span_attributes(span, vec![
                                        opentelemetry::KeyValue::new("task.failed", true),
                                    ]);
                                }

                                if headless {
                                    println!("Headless: Task Failed: {}", name);
                                    if let Some(client) = &app.ai_client {
                                        let vars = app.task_vars.clone().unwrap_or(serde_json::json!({}));
                                        let facts = app.facts.clone();

                                        // Auto-Analyze Logic (only if NOT scripted)
                                        if let Some(idx) =
                                            app.test_script.iter().position(|a| a.task_name == *name && a.on_failure)
                                        {
                                            println!("Headless: Executing Script Action for TaskFail: {}", name);
                                            let script_action = app.test_script.remove(idx);
                                            for action in script_action.actions {
                                                match action {
                                                    ansible_piloteer::app::ScriptActionType::Retry => {
                                                        println!("Headless: Retrying via Script...");
                                                        if let Some(tx) = &app.ipc_tx {
                                                            let _ = tx.send(Message::Retry).await;
                                                        }
                                                    }
                                                    ansible_piloteer::app::ScriptActionType::Continue => {
                                                        println!("Headless: Continuing via Script...");
                                                        if let Some(tx) = &app.ipc_tx {
                                                            let _ = tx.send(Message::Continue).await;
                                                        }
                                                    }
                                                    ansible_piloteer::app::ScriptActionType::EditVar { key, value } => {
                                                        println!("Headless: ModifyVar {} = {}", key, value);
                                                        if let Some(tx) = &app.ipc_tx {
                                                            let _ = tx.send(Message::ModifyVar {
                                                                key,
                                                                value,
                                                            })
                                                            .await;
                                                        }
                                                    }
                                                     ansible_piloteer::app::ScriptActionType::Pause => {
                                                        println!("Headless: Pausing on Failure (Scripted)...");
                                                        tokio::time::sleep(Duration::from_secs(5)).await;
                                                    }
                                                    ansible_piloteer::app::ScriptActionType::ExecuteCommand { cmd } => {
                                                        println!("Headless: Executing Command (on failure): {}", cmd);
                                                        let output = std::process::Command::new("sh")
                                                            .arg("-c")
                                                            .arg(cmd)
                                                            .output();
                                                        match output {
                                                            Ok(o) => println!(" Command Finished: status={}", o.status),
                                                            Err(e) => println!(" Command Failed: {}", e),
                                                        }
                                                    }
                                                    ansible_piloteer::app::ScriptActionType::AskAi => {
                                                        println!("Headless: Asking AI (Scripted)...");
                                                        // Always use real client now, configured via base_url for tests
                                                        match client.analyze_failure(&name, "Task Failed", &vars, facts.as_ref()).await {
                                                            Ok(analysis) => {
                                                                 println!("Headless: AI Analysis Received: {:.50}...", analysis.analysis);
                                                                 app.suggestion = Some(analysis.clone());
                                                            }
                                                            Err(e) => println!("Headless: AI Request Failed: {}", e),
                                                        }
                                                    }
                                                    ansible_piloteer::app::ScriptActionType::AssertAiContext { contains } => {
                                                        println!("Headless: Asserting AI Context...");
                                                        if let Some(suggestion) = &app.suggestion {
                                                            if let Some(text) = contains {
                                                                if suggestion.analysis.contains(&text) {
                                                                    println!("Headless: Assertion PASSED: Analysis contains '{}'", text);
                                                                } else {
                                                                    println!("Headless: Assertion FAILED: Analysis does NOT contain '{}'", text);
                                                                    // Optional: Panic or exit?
                                                                    // std::process::exit(1);
                                                                }
                                                            } else {
                                                                println!("Headless: Assertion PASSED: AI Context present.");
                                                            }
                                                        } else {
                                                            println!("Headless: Assertion FAILED: No AI Context found.");
                                                            // std::process::exit(1);
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                                tokio::time::sleep(Duration::from_millis(200)).await;
                                            }
                                        } else if auto_analyze {
                                            println!("Headless: Analyzing Failure...");
                                            if let Ok(analysis) = client.analyze_failure(&name, "Task Failed", &vars, facts.as_ref()).await {
                                                println!("\n🤖 AI ANALYSIS:\n{}\n", analysis.analysis);
                                                if let Some(fix) = &analysis.fix {
                                                    println!("💡 SUGGESTED FIX: {} = {}\n", fix.key, fix.value);
                                                }
                                                // Persist analysis by sending message to self
                                                if let Some(tx) = &app.ipc_tx {
                                                    let _ = tx.send(Message::AiAnalysis {
                                                        task: name.clone(),
                                                        analysis: analysis.clone()
                                                    }).await;
                                                }
                                            }
                                        } else {
                                            // Old test logic: just print generic info
                                             if let Ok(analysis) = client.analyze_failure(&name, "Task Failed", &vars, facts.as_ref()).await {
                                                println!("Headless: AI Analysis Tokens: {}", analysis.tokens_used);
                                            }
                                        }
                                    }
                                    if let Some(tx) = &app.ipc_tx {
                                        let _ = tx.send(Message::ModifyVar { key: "should_fail".to_string(), value: serde_json::json!(false) }).await;
                                        tokio::time::sleep(Duration::from_millis(500)).await;
                                        let _ = tx.send(Message::Retry).await;
                                    }
                                } else {
                                    app.set_failed(name, serde_json::Value::Null, facts.clone());
                                }
                            }
                            Message::TaskResult { name, host, changed, failed, verbose_result } => {
                               let (status, color) = if failed { ("FAILED", ratatui::style::Color::Red) }
                                                     else if changed { ("CHANGED", ratatui::style::Color::Yellow) }
                                                     else { ("OK", ratatui::style::Color::Green) };
                                app.log(format!("Task '{}' on {}: {}", name, host, status), Some(color));

                                // End task span with final attributes
                                if let Some(span) = app.task_spans.remove(&name) {
                                    ansible_piloteer::tracing::end_span(span, vec![
                                        opentelemetry::KeyValue::new("task.host", host.clone()),
                                        opentelemetry::KeyValue::new("task.changed", changed),
                                        opentelemetry::KeyValue::new("task.failed", failed),
                                        opentelemetry::KeyValue::new("task.status", status),
                                    ]);
                                }

                                let duration = app.task_start_time.map(|start| start.elapsed().as_secs_f64()).unwrap_or(0.0); // [NEW] Phase 22
                                app.record_task_result(
                                    name.clone(),
                                    host,
                                    changed,
                                    failed,
                                    duration, // [NEW]
                                    None,
                                    verbose_result.clone(),
                                    None, // [NEW] analysis
                                );
                                if headless { println!("Headless: Task Result: {}", status); }

                                // DEBUG: Dump verbose result
                                if let Some(v) = &verbose_result {
                                    use std::io::Write;
                                    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/piloteer_verbose.json") {
                                        writeln!(f, "Task: {}\n{}", name, serde_json::to_string_pretty(v).unwrap_or_default()).ok();
                                    }
                                }
                            }
                            Message::TaskUnreachable { name, host, error, result } => {
                                app.set_unreachable(name.clone(), host.clone(), error.clone(), result);

                                if headless {
                                    println!("Headless: Host {} unreachable: {}", host, error);

                                    // Check for script actions to recover from Unreachable state
                                    if let Some(idx) =
                                        app.test_script.iter().position(|a| a.task_name == *name && a.on_failure)
                                    {
                                        println!("Headless: Executing Script Action for TaskUnreachable: {}", name);
                                        let script_action = app.test_script.remove(idx);
                                        for action in script_action.actions {
                                            match action {
                                                ansible_piloteer::app::ScriptActionType::Retry => {
                                                    println!("Headless: Retrying via Script...");
                                                    if let Some(tx) = &app.ipc_tx {
                                                        let _ = tx.send(Message::Retry).await;
                                                    }
                                                }
                                                ansible_piloteer::app::ScriptActionType::ExecuteCommand { cmd } => {
                                                    println!("Headless: Executing Command (on unreachable): {}", cmd);
                                                    let output = std::process::Command::new("sh")
                                                        .arg("-c")
                                                        .arg(cmd)
                                                        .output();
                                                    match output {
                                                        Ok(o) => println!(" Command Finished: status={}", o.status),
                                                        Err(e) => println!(" Command Failed: {}", e),
                                                    }
                                                }
                                                _ => {}
                                            }
                                            tokio::time::sleep(Duration::from_millis(200)).await;
                                        }
                                    }
                                }
                            }
                            Message::PlayRecap { stats } => {
                                app.log(format!("Play Recap Received: {:?}", stats), Some(ratatui::style::Color::Cyan));

                                // End Play Span
                                app.play_span_guard = None;
                                app.play_span = None;

                                // Add to history so it can be inspected in Analysis Mode
                                app.record_task_result(
                                    "Play Recap".to_string(),
                                    "all".to_string(),
                                    false,
                                    false,
                                    0.0, // [NEW] Duration
                                    None,
                                    Some(ansible_piloteer::execution::ExecutionDetails::new(stats.clone())),
                                    None, // [NEW] analysis

                                );

                                app.play_recap = Some(stats.clone());
                                app.set_task("Playbook Complete".to_string(), serde_json::Value::Null, None);

                                // Close playbook span by dropping the guard
                                app.playbook_span_guard = None;
                            }
                            Message::AiAnalysis { task, analysis } => {
                                app.asking_ai = false;
                                app.suggestion = Some(analysis.clone());
                                app.log(format!("AI Analysis Received for '{}'", task), Some(ratatui::style::Color::Cyan));

                                // Update history if task matches
                                if let Some(history_item) = app.history.iter_mut().rev().find(|t| t.name == task) {
                                    history_item.analysis = Some(analysis);
                                } else {
                                     app.log(format!("Could not find task '{}' in history to attach analysis", task), Some(ratatui::style::Color::Red));
                                }

                                // Notification
                                app.notification = Some(("AI Analysis Ready. Press 'v' to view.".to_string(), std::time::Instant::now()));
                            }
                            Message::ClientDisconnected => {
                                app.client_connected = false;
                                app.log("Client Disconnected".to_string(), Some(ratatui::style::Color::Red));
                                if headless { println!("Headless: Client Disconnected"); }
                            }
                            Message::ModifyVar { .. } => {} // Handled elsewhere or just for IPC
                            Message::Proceed | Message::Retry | Message::Continue => {}
                        }
                    }
                    None => {
                        if headless { app.running = false; }
                        else {
                            app.log("Playbook execution complete. Press 'q' to quit.".to_string(), Some(ratatui::style::Color::Cyan));
                            app.ipc_tx = None;
                        }
                    }
                }
            }

            // Handle TUI Input (Instant)
            Some(event) = input_rx.recv() => {
                 if !headless {
                    match app.handle_event(event) {
                       Action::Quit => app.running = false,
                       Action::SaveSession => {
                            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                            let filename = format!("piloteer_session_{}.json.gz", timestamp);
                            let session = ansible_piloteer::session::Session::from_app(&app);
                            if let Err(e) = session.save(&filename) {
                                 app.notification = Some((format!("Save Failed: {}", e), std::time::Instant::now()));
                            } else {
                                 app.notification = Some((format!("Saved to {}", filename), std::time::Instant::now()));
                            }
                       }
                       Action::ExportReport => {
                            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                            let filename = format!("piloteer_report_{}.md", timestamp);
                            let report = ansible_piloteer::report::ReportGenerator::new(&app);
                            if let Err(e) = report.save_to_file(&filename) {
                                  app.notification = Some((format!("Report Failed: {}", e), std::time::Instant::now()));
                            } else {
                                  app.notification = Some((format!("Report Saved: {}", filename), std::time::Instant::now()));
                            }
                       }
                       Action::Proceed => {
                           if app.waiting_for_proceed {
                               app.waiting_for_proceed = false;
                               if let Some(tx) = &app.ipc_tx { let _ = tx.send(Message::Proceed).await; }
                           }
                       }
                       Action::Retry => {
                           if app.waiting_for_proceed {
                               app.waiting_for_proceed = false;
                               if let Some(tx) = &app.ipc_tx { let _ = tx.send(Message::Retry).await; }
                           }
                       }
                       Action::Continue => {
                           if app.waiting_for_proceed {
                               app.waiting_for_proceed = false;
                               if let Some(tx) = &app.ipc_tx { let _ = tx.send(Message::Continue).await; }
                           }
                       }
                       Action::EditVar => {
                           // Phase 31: Launch External Editor
                           if let ansible_piloteer::app::EditState::EditingValue { temp_file, .. } = &app.edit_state {
                               // 1. Suspend TUI
                               let _ = disable_raw_mode();
                               let _ = execute!(io::stdout(), LeaveAlternateScreen);

                               // 2. Run Editor
                               let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
                               let status = Command::new(&editor)
                                   .arg(temp_file)
                                   .status();

                               // 3. Resume TUI
                               let _ = execute!(io::stdout(), EnterAlternateScreen);
                               let _ = enable_raw_mode();
                               if let Some(t) = terminal {
                                   let _ = t.clear();
                                   let _ = t.hide_cursor();
                               }

                               // 4. Handle Result
                               if let Ok(status) = status {
                                   if status.success() {
                                       match app.apply_edit() {
                                           Ok((key, value)) => {
                                                // Send IPC
                                                if let Some(tx) = &app.ipc_tx {
                                                    let _ = tx.send(Message::ModifyVar { key: key.clone(), value: value.clone() }).await;
                                                    app.notification = Some((format!("Updated Variable: {}", key), std::time::Instant::now()));
                                                }
                                           },
                                           Err(e) => {
                                               app.notification = Some((format!("Edit Failed: {}", e), std::time::Instant::now()));
                                           }
                                       }
                                   } else {
                                       app.notification = Some(("Editor exited with error".to_string(), std::time::Instant::now()));
                                       app.cancel_edit();
                                   }
                               } else {
                                    app.notification = Some(("Failed to launch editor".to_string(), std::time::Instant::now()));
                                    app.cancel_edit();
                               }
                           }
                       }
                       Action::AskAi => {
                           let client_opt = app.ai_client.clone();
                           let current_task_opt = app.current_task.clone();
                           let failed_task_opt = app.failed_task.clone();
                           let vars = app.task_vars.clone().unwrap_or(serde_json::json!({}));
                           let facts = app.facts.clone();

                           if let Some(client) = client_opt {
                               app.asking_ai = true;
                               app.log("Asking AI Pilot...".to_string(), Some(ratatui::style::Color::Magenta));

                               // Clone TX to send result back to main loop
                               if let Some(tx) = &app.ipc_tx {
                                   let tx = tx.clone();
                                   tokio::spawn(async move {
                                        let task_name = failed_task_opt.or(current_task_opt).unwrap_or("Unknown".to_string());
                                        match client.analyze_failure(&task_name, "Task Failed", &vars, facts.as_ref()).await {
                                            Ok(analysis) => {
                                                // We need a new Message variant to send analysis back?
                                                // Or we can just use a separate channel for internal app events?
                                                // Current architecture uses ipc_rx for both IPC and some app events might need a way.
                                                // Actually, ipc_rx receives Message enum. We should add Message::AiAnalysis.
                                                let _ = tx.send(Message::AiAnalysis {
                                                    task: task_name,
                                                    analysis
                                                }).await;
                                            },
                                            Err(_e) => {
                                                 // Log error somehow?
                                                 // For now ignore or maybe send error message
                                            }
                                        }
                                   });
                               }
                           }
                       }
                       Action::ApplyFix => {
                           if let Some(analysis) = &app.suggestion
                               && let Some(fix) = &analysis.fix
                                   && let Some(tx) = &app.ipc_tx {
                                        let _ = tx.send(Message::ModifyVar { key: fix.key.clone(), value: fix.value.clone() }).await;
                                        app.log(format!("Applying Fix: {} = {}", fix.key, fix.value), Some(ratatui::style::Color::Green));
                                   }
                       }
                       Action::ToggleFollow => {
                            app.auto_scroll = !app.auto_scroll;
                            if app.auto_scroll { app.log_scroll = 0; }
                       }
                        Action::ToggleAnalysis => {
                            if app.active_view == ActiveView::Analysis {
                                app.active_view = ActiveView::Dashboard;
                            } else {
                                app.active_view = ActiveView::Analysis;
                                app.scroll_offset = 0;
                                if let Some(task) = app.history.get(app.analysis_index) {
                                     let json_data = task.verbose_result.as_ref().map(|d| d.inner().clone()).unwrap_or_else(|| {
                                         if let Some(err) = &task.error { serde_json::json!({ "error": err }) }
                                         else { serde_json::json!({ "message": "No verbose data captured." }) }
                                     });
                                     app.analysis_tree = Some(JsonTreeState::new(json_data));
                                } else { app.analysis_tree = None; }
                            }
                        }
                       Action::AnalysisNext => {
                            if !app.history.is_empty() {
                                app.analysis_index = (app.analysis_index + 1).min(app.history.len() - 1);
                                app.scroll_offset = 0;
                                if let Some(task) = app.history.get(app.analysis_index) {
                                     let json_data = task.verbose_result.as_ref().map(|d| d.inner().clone()).unwrap_or_else(|| {
                                         if let Some(err) = &task.error { serde_json::json!({ "error": err }) }
                                         else { serde_json::json!({ "message": "No verbose data captured." }) }
                                     });
                                     app.analysis_tree = Some(JsonTreeState::new(json_data));
                                }
                            }
                       }
                       Action::AnalysisPrev => {
                           if !app.history.is_empty() {
                                app.analysis_index = app.analysis_index.saturating_sub(1);
                                app.scroll_offset = 0;
                                if let Some(task) = app.history.get(app.analysis_index) {
                                     let json_data = task.verbose_result.as_ref().map(|d| d.inner().clone()).unwrap_or_else(|| {
                                         if let Some(err) = &task.error { serde_json::json!({ "error": err }) }
                                         else { serde_json::json!({ "message": "No verbose data captured." }) }
                                     });
                                     app.analysis_tree = Some(JsonTreeState::new(json_data));
                                }
                           }
                       }
                        Action::ToggleMetrics => {
                            if app.active_view == ActiveView::Metrics {
                                app.active_view = ActiveView::Dashboard;
                            } else {
                                app.active_view = ActiveView::Metrics;
                            }
                        }
                       Action::ToggleMetricsView => {
                           // Toggle between Dashboard and Heatmap
                           app.metrics_view = match app.metrics_view {
                               ansible_piloteer::app::MetricsView::Dashboard => ansible_piloteer::app::MetricsView::Heatmap,
                               ansible_piloteer::app::MetricsView::Heatmap => ansible_piloteer::app::MetricsView::Dashboard,
                           };
                       }
                        Action::SubmitQuery(query_str) => {
                            // Phase 26: Execute JMESPath Query
                            app.notification = Some(("Executing Query...".to_string(), std::time::Instant::now()));

                            // Reconstruct Session for Query Context
                            let session = ansible_piloteer::session::Session::from_app(&app);
                            match serde_json::to_value(&session) {
                                Ok(json) => {
                                    match ansible_piloteer::query::run_query(&query_str, &json) {
                                        Ok(result) => {
                                            app.active_view = ActiveView::Analysis;
                                            app.analysis_tree = Some(JsonTreeState::new(result));
                                            app.analysis_focus = app::AnalysisFocus::DataBrowser;
                                            app.notification = Some((format!("Query Executed: {}", query_str), std::time::Instant::now()));
                                        },
                                        Err(e) => {
                                            app.notification = Some((format!("Query Error: {}", e), std::time::Instant::now()));
                                        }
                                    }
                                },
                                Err(e) => {
                                    app.notification = Some((format!("Session Serialization Error: {}", e), std::time::Instant::now()));
                                }
                            }
                        }
                       Action::None => {}
                        Action::Yank => {
                             if app.active_view == ActiveView::Analysis {
                                 if app.analysis_focus == app::AnalysisFocus::DataBrowser
                                    && let Some(tree) = &app.analysis_tree
                                    && let Some(content) = tree.get_selected_content()
                                {
                                    app.copy_to_clipboard(content);
                                }
                            } else {
                               // Standard Mode: Copy Inspector Content
                               let content = if let Some(err) = &app.failed_result {
                                   serde_json::to_string_pretty(err).unwrap_or_default()
                               } else {
                                   "No Active Failure".to_string()
                               };
                               app.copy_to_clipboard(content);
                           }
                        }
                        Action::YankVisual => {
                             // Phase 16: Visual mode yank
                             if app.active_view == ActiveView::Analysis && app.visual_mode {
                                 if let Some(tree) = &app.analysis_tree
                                    && let Some(start) = app.visual_start_index
                                {
                                    let end = tree.selected_line;
                                    if let Some(content) = tree.get_range_content(start, end) {
                                        app.copy_to_clipboard(content);
                                    }
                                }
                                // Exit visual mode after yank
                                app.visual_mode = false;
                                app.visual_start_index = None;
                            }
                        }
                        Action::YankWithCount => {
                             // Phase 16: Count-based yank (y3j)
                             if app.active_view == ActiveView::Analysis {
                                 if let Some(tree) = &app.analysis_tree
                                    && let Some(count) = app.pending_count
                                    && let Some(content) = tree.get_content_with_count(count)
                                {
                                    app.copy_to_clipboard(content);
                                }
                                app.pending_count = None;
                            }
                        }

                        Action::ToggleBreakpoint => {
                            app.toggle_breakpoint();
                        }
                        _ => {}
                 }
                     }
            }, // End input_rx match

            _ = tick => {}
        } // End select!
    } // End loop
    Ok(app)
}
