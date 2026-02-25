use ansible_piloteer::{actions, auth, ipc_handler, ui};
use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::event;
use crossterm::execute;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

use ansible_piloteer::app::{App, TaskHistory};
use ansible_piloteer::config::Config;
use ansible_piloteer::ipc::Message;

type DefaultTerminal = Terminal<CrosstermBackend<io::Stdout>>;

// ── CLI ──────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "ansible-piloteer",
    about = "AI-powered Ansible interactive debugger",
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
    /// Arguments forwarded to ansible-playbook
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    ansible_args: Vec<String>,

    #[command(subcommand)]
    command: Option<Commands>,

    /// Export execution report (.json or .md)
    #[arg(long)]
    report: Option<String>,

    /// Bind address for TCP listener (e.g. 0.0.0.0:9000)
    #[arg(long)]
    bind: Option<String>,

    /// Shared secret token for authentication
    #[arg(long)]
    secret: Option<String>,

    /// Verbosity level (-v, -vv, -vvv, …)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Automatically analyze and retry failed tasks (headless)
    #[arg(long)]
    auto_analyze: bool,

    /// Replay a saved session file
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
    /// Query session data with JMESPath (omit query for interactive REPL)
    Query {
        query: Option<String>,
        #[arg(short, long)]
        input: String,
        #[arg(short, long, default_value = "pretty-json")]
        format: String,
    },
    /// Start MCP stdio server for IDE integration
    Mcp,
    /// Install the Piloteer Ansible strategy plugin to ~/.ansible/plugins/strategy/
    Init {
        /// Force overwrite even if the plugin is already installed
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum AuthCmd {
    Login {
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long, default_value = "google")]
        backend: String,
    },
    Gcloud {
        #[arg(long, default_value = "default")]
        profile: String,
    },
    Adc {
        #[arg(long, default_value = "default")]
        profile: String,
    },
    List,
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let auto_analyze = cli.auto_analyze;

    let config = Config::new().unwrap_or_else(|e| {
        eprintln!("Failed to load config: {}", e);
        std::process::exit(1);
    });

    if let Err(e) = ansible_piloteer::telemetry::init_tracing(&config) {
        eprintln!("Warning: Failed to initialize tracing: {}", e);
    }

    let result = match cli.command {
        Some(Commands::Auth { cmd }) => handle_auth(cmd, config).await,
        Some(Commands::Query {
            query,
            input,
            format,
        }) => handle_query(query, input, format, config),
        Some(Commands::Mcp) => ansible_piloteer::mcp::run_stdio_server().await,
        Some(Commands::Init { force }) => match ansible_piloteer::plugin::install_plugin(force) {
            Ok(path) => {
                println!("✓ Strategy plugin installed to: {}", path.display());
                println!("\n  Ansible will auto-discover it. Just set:");
                println!("    export ANSIBLE_STRATEGY=piloteer");
                Ok(())
            }
            Err(e) => Err(e),
        },
        None => {
            // Auto-install the strategy plugin on every TUI startup
            ansible_piloteer::plugin::ensure_plugin();
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

    ansible_piloteer::telemetry::shutdown_tracing();
    result
}

// ── Subcommand handlers ──────────────────────────────────────────────────────

async fn handle_auth(cmd: AuthCmd, config: Config) -> Result<()> {
    match cmd {
        AuthCmd::Login { profile, backend } => {
            if backend != "google" {
                eprintln!("Only 'google' backend supports OAuth login.");
                std::process::exit(1);
            }
            println!("Starting Google OAuth login for profile '{}'...", profile);
            match auth::login(config.google_client_id, config.google_client_secret).await {
                Ok(token) => save_token(&profile, &backend, &token),
                Err(e) => eprintln!("Login Failed: {:?}", e),
            }
        }
        AuthCmd::Gcloud { profile } => {
            println!("Fetching token from gcloud ADC...");
            match auth::get_gcloud_token().await {
                Ok(token) => save_token(&profile, "google", &token),
                Err(e) => eprintln!("gcloud auth failed: {:?}", e),
            }
        }
        AuthCmd::Adc { profile } => {
            println!("Reading Application Default Credentials file...");
            match auth::get_adc_token().await {
                Ok(token) => save_token(&profile, "google", &token),
                Err(e) => eprintln!("ADC auth failed: {:?}", e),
            }
        }
        AuthCmd::List => {
            let data = Config::load_auth_data().unwrap_or_default();
            if data.is_empty() {
                println!("No cached credentials found.");
                return Ok(());
            }
            println!(
                "{:<15} | {:<10} | {:<40}",
                "Profile", "Backend", "Token (Masked)"
            );
            println!("{:-<15}-+-{:-<10}-+-{:-<40}", "", "", "");
            for (profile, backends) in data {
                for (backend, token) in backends {
                    let masked = if token.len() > 8 {
                        format!("{}...{}", &token[..4], &token[token.len() - 4..])
                    } else {
                        "****".to_string()
                    };
                    println!("{:<15} | {:<10} | {:<40}", profile, backend, masked);
                }
            }
        }
    }
    Ok(())
}

fn save_token(profile: &str, backend: &str, token: &str) {
    match Config::save_auth_token(profile, backend, token) {
        Ok(_) => println!("Token saved to profile '{}'.", profile),
        Err(e) => eprintln!("Failed to save auth token: {}", e),
    }
}

fn handle_query(
    query: Option<String>,
    input: String,
    format: String,
    config: Config,
) -> Result<()> {
    let session = ansible_piloteer::session::Session::load(&input)
        .map_err(|e| anyhow::anyhow!("Error loading session from {}: {}", input, e))?;

    let Some(q) = query else {
        ansible_piloteer::repl::run(&session, config.filters.as_ref())
            .map_err(|e| anyhow::anyhow!("REPL Error: {}", e))?;
        return Ok(());
    };

    let json_string = serde_json::to_string(&session)?;
    let mut runtime = jmespath::Runtime::new();
    runtime.register_builtin_functions();
    ansible_piloteer::query::register_functions(&mut runtime);
    if let Some(filters) = &config.filters {
        for (name, expr) in filters {
            runtime.register_function(
                name,
                Box::new(ansible_piloteer::query::CustomFilter::new(expr.clone())),
            );
        }
    }
    let expr = runtime
        .compile(&q)
        .map_err(|e| anyhow::anyhow!("Invalid query: {}", e))?;
    let variable = jmespath::Variable::from_json(&json_string)
        .map_err(|e| anyhow::anyhow!("Error parsing JSON: {}", e))?;
    let result = expr
        .search(&variable)
        .map_err(|e| anyhow::anyhow!("JMESPath error: {}", e))?;

    match format.as_str() {
        "json" => println!("{}", serde_json::to_string(&result).unwrap()),
        "pretty-json" => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
        "yaml" => println!("{}", serde_yaml::to_string(&result).unwrap()),
        other => anyhow::bail!(
            "Unknown format: {}. Supported: json, pretty-json, yaml",
            other
        ),
    }
    Ok(())
}

// ── TUI runner ───────────────────────────────────────────────────────────────

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

    let mut config = Config::new().unwrap_or_else(|e| {
        eprintln!("Failed to load config: {}", e);
        std::process::exit(1);
    });
    if let Some(addr) = bind_addr {
        config.bind_addr = Some(addr);
    }
    if let Some(secret) = secret_token {
        config.secret_token = Some(secret);
    }

    let mut terminal: Option<DefaultTerminal> = if !headless {
        let t = ratatui::init();
        execute!(io::stdout(), crossterm::event::EnableMouseCapture)?;
        Some(t)
    } else {
        println!("Running in HEADLESS mode");
        None
    };

    let mut app = match &replay_path {
        Some(path) => App::from_session(path).unwrap_or_else(|e| {
            eprintln!("Failed to load session: {}", e);
            std::process::exit(1);
        }),
        None => App::new(config.clone()),
    };
    app.load_test_script();

    if !app.replay_mode {
        let (to_app_tx, to_app_rx) = mpsc::channel::<Message>(100);
        let (from_app_tx, from_app_rx) = mpsc::channel::<Message>(100);
        app.set_ipc_tx(Some(from_app_tx));

        ipc_handler::spawn_ipc_server(
            config.socket_path.clone(),
            config.bind_addr.clone(),
            config.secret_token.clone(),
            to_app_tx,
            from_app_rx,
        );

        tokio::time::sleep(Duration::from_millis(500)).await;

        if !ansible_args.is_empty() {
            spawn_ansible(&ansible_args, verbose, &config);
        }

        let mut to_app_rx = to_app_rx;
        let final_app = run_app(&mut terminal, app, &mut to_app_rx, headless, auto_analyze).await?;
        cleanup(&mut terminal, headless, final_app, report_path).await
    } else {
        let (_, mut dummy_rx) = mpsc::channel::<Message>(1);
        let final_app = run_app(&mut terminal, app, &mut dummy_rx, headless, auto_analyze).await?;
        cleanup(&mut terminal, headless, final_app, report_path).await
    }
}

fn spawn_ansible(ansible_args: &[String], verbose: u8, config: &Config) {
    use tokio::process::Command;
    let mut cmd = Command::new("ansible-playbook");
    if verbose > 0 {
        cmd.arg(format!("-{}", "v".repeat(verbose as usize)));
    }
    cmd.args(ansible_args);
    cmd.env("ANSIBLE_STRATEGY", "piloteer");
    let plugin_path = std::env::current_dir()
        .unwrap_or_default()
        .join("ansible_plugin")
        .join("strategies");
    cmd.env("ANSIBLE_STRATEGY_PLUGINS", plugin_path);
    if let Some(addr) = &config.bind_addr {
        cmd.env("PILOTEER_SOCKET", addr);
    } else {
        cmd.env("PILOTEER_SOCKET", &config.socket_path);
    }
    if let Some(secret) = &config.secret_token {
        cmd.env("PILOTEER_SECRET", secret);
    }
    let log = std::fs::File::create("ansible_child.log")
        .unwrap_or_else(|_| std::fs::File::create("/dev/null").unwrap());
    let log_err = log.try_clone().unwrap();
    cmd.stdout(log)
        .stderr(log_err)
        .stdin(std::process::Stdio::null());
    if let Err(e) = cmd.spawn() {
        eprintln!("Failed to spawn ansible-playbook: {}", e);
    }
}

async fn cleanup(
    _terminal: &mut Option<DefaultTerminal>,
    headless: bool,
    app: App,
    report_path: Option<String>,
) -> Result<()> {
    if !headless {
        execute!(io::stdout(), crossterm::event::DisableMouseCapture)?;
        ratatui::restore();
    }

    if !app.replay_mode
        && let Ok(config_dir) = Config::get_config_dir()
    {
        let archive_dir = config_dir.join("archive");
        std::fs::create_dir_all(&archive_dir).ok();
        let filename = format!(
            "session_{}.json.gz",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );
        let path = archive_dir.join(&filename);
        match ansible_piloteer::session::Session::from_app(&app).save(&path.to_string_lossy()) {
            Ok(_) => println!("Session archived to: {}", path.display()),
            Err(e) => eprintln!("Failed to archive session: {}", e),
        }
    }

    print_drift_summary(&app.history);

    if let Some(path) = report_path {
        generate_report(&app, &path);
    }

    Ok(())
}

fn print_drift_summary(history: &[TaskHistory]) {
    println!("\n--- Drift Summary ---");
    let changed: Vec<_> = history.iter().filter(|t| t.changed).collect();
    if changed.is_empty() {
        println!("No changes detected.");
    } else {
        println!("The following tasks modified the system state:");
        for t in &changed {
            println!(" - {} [Task: {}]", t.host, t.name);
        }
        println!("Total Drift: {} tasks changed.", changed.len());
    }
}

fn generate_report(app: &App, path: &str) {
    use std::io::Write;
    println!("Generating report at {}...", path);
    if path.ends_with(".json") {
        match std::fs::File::create(path) {
            Ok(mut f) => {
                let json = serde_json::to_string_pretty(&app.history).unwrap_or_default();
                if let Err(e) = f.write_all(json.as_bytes()) {
                    eprintln!("Failed to write JSON report: {}", e);
                }
            }
            Err(e) => eprintln!("Failed to create report file: {}", e),
        }
    } else if path.ends_with(".md") {
        if let Err(e) = ansible_piloteer::report::ReportGenerator::new(app).save_to_file(path) {
            eprintln!("Failed to write Markdown report: {}", e);
        }
    } else {
        eprintln!("Unsupported report format. Use .json or .md");
    }
}

// ── Event loop ───────────────────────────────────────────────────────────────

async fn run_app(
    terminal: &mut Option<DefaultTerminal>,
    mut app: App,
    ipc_rx: &mut mpsc::Receiver<Message>,
    headless: bool,
    auto_analyze: bool,
) -> Result<App> {
    let (input_tx, mut input_rx) = mpsc::channel(100);
    std::thread::spawn(move || {
        loop {
            if event::poll(Duration::from_millis(250)).unwrap_or(false)
                && let Ok(evt) = event::read()
                && input_tx.blocking_send(evt).is_err()
            {
                break;
            }
        }
    });

    let (ai_tx, mut ai_rx) = mpsc::channel::<anyhow::Result<ansible_piloteer::ai::ChatMessage>>(10);

    loop {
        if !headless && let Some(t) = terminal {
            t.draw(|f| ui::draw(f, &mut app))?;
        }
        if !app.running {
            break;
        }

        app.update_velocity();
        let ipc_done = app.ipc_tx.is_none();

        tokio::select! {
            Some(res) = ai_rx.recv() => handle_ai_response(&mut app, res),

            msg_opt = ipc_rx.recv(), if !ipc_done => match msg_opt {
                Some(msg) => {
                    ipc_handler::handle_message(&mut app, msg, headless, auto_analyze).await;
                }
                None => {
                    if headless {
                        app.running = false;
                    } else {
                        app.log(
                            "Playbook execution complete. Press 'q' to quit.".to_string(),
                            Some(ratatui::style::Color::Cyan),
                        );
                        app.ipc_tx = None;
                    }
                }
            },

            Some(event) = input_rx.recv() => {
                if !headless {
                    let action = app.handle_event(event);
                    actions::dispatch(action, &mut app, terminal, &ai_tx).await;
                }
            },

            _ = tokio::time::sleep(Duration::from_millis(250)) => {},
        }
    }

    Ok(app)
}

fn handle_ai_response(app: &mut App, res: anyhow::Result<ansible_piloteer::ai::ChatMessage>) {
    app.chat_loading = false;
    let msg = match res {
        Ok(m) => m,
        Err(e) => ansible_piloteer::ai::ChatMessage {
            role: "system".to_string(),
            content: format!("Error: {}", e),
            collapsed: false,
        },
    };
    app.chat_history.push(msg);
    if app.chat_auto_scroll {
        app.chat_scroll = app.chat_history.len().saturating_sub(1) as u16;
    }
}
