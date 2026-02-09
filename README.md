# Ansible Piloteer

**Ansible Piloteer** is an interactive, AI-powered debugger and execution wrapper for Ansible playbooks. It transforms Ansible's "black box" execution into a transparent, controllable process with a TUI (Text User Interface) for step-through debugging, live state inspection, and runtime failure recovery.

## 🚀 Features

-   **Interactive TUI**:
    -   **Dashboard**: View live task status, progress bars, and execution statistics.
    -   **Analysis Mode**:
        -   Deep inspection of JSON facts and task results.
        -   **Search**: Interactive search with highlighting (try `/`).
        -   **Deep Navigation**: Recursive expand/collapse (`Shift+h`/`Shift+l`).
        -   **Detail View**: Inspect long values in a popup (`w`).
    -   **Host Targeting**: List hosts, filter history, and inspect facts per host (`H` to list, `f` for facts).
-   **AI-Powered Debugging**:
    -   **Ask Pilot**: Analyze failures using LLMs (OpenAI, Local LLMs) to get plain-English explanations.
    -   **Auto-Fix**: Automatically apply AI-suggested variable fixes with a single keystroke.
    -   **Retry**: Re-run failed tasks with modified variables.
    -   **Modify Variables**: Inject new variables or override existing ones at runtime (`extra_vars` precedence).
    -   **Continue**: Skip failures and proceed with execution.
-   **Session Persistence**:
    -   **Save/Load**: Snapshot your session (`Ctrl+s`) and replay it later (`--replay`).
    -   **Reporting**: Export detailed Markdown reports of your debugging session (`Ctrl+e`).
-   **Deep Execution Analysis**:
    -   **Play Recap**: View aggregated execution statistics.
    -   **Clipboard**: Copy data with `y`.
-   **Architecture**: Built with a high-performance Rust CLI and a custom Ansible Strategy Plugin (Python).

## 🛠️ Architecture

The system consists of two main components communicating via Unix Domain Sockets:

1.  **Piloteer CLI (Rust)**: The user-facing TUI and process controller. It manages the Ansible process and renders the UI.
2.  **Ansible Strategy Plugin (Python)**: A custom execution strategy that hooks into Ansible internals to capture state, pause execution, and apply runtime modifications.

## 📦 Installation & Setup

### Prerequisites
-   **Rust**: Stable toolchain (`cargo`).
-   **Python**: 3.8+ with Ansible installed.

### Build
 Clone the repository and build the Rust CLI:

```bash
cargo build --release
```

### Environment Setup
The Piloteer requires a specific environment to load its custom Ansible plugin.

1.  **Activate your Python environment** (if using venv):
    ```bash
    source venv/bin/activate
    ```

2.  **Set Ansible Strategy Path**:
    Point Ansible to the `ansible_plugin` directory in this repo.
    ```bash
    export ANSIBLE_STRATEGY_PLUGINS=$(pwd)/ansible_plugin/strategies
    ```

3.  **Configure Strategy**:
    Tell Ansible to use the `piloteer` strategy.
    ```bash
    export ANSIBLE_STRATEGY=piloteer
    ```

## ⚙️ Configuration

Piloteer can be configured via `config.toml`, environment variables, or CLI arguments.

### Authentication (Optional)
To use the hosted AI features with Google Login:

```bash
./target/release/ansible-piloteer auth login
```

### Environment Variables

| Variable | Description | Default |
| :--- | :--- | :--- |
| `OPENAI_API_KEY` | API Key for OpenAI (or compatible providers) | None |
| `PILOTEER_MODEL` | LLM Model to use | `gpt-4-turbo-preview` |
| `PILOTEER_API_BASE` | Base URL for LLM API (e.g. for Local LLMs) | `https://api.openai.com/v1` |
| `PILOTEER_SOCKET` | Path to the IPC socket or `host:port` | `/tmp/piloteer.sock` |
| `PILOTEER_SECRET` | Shared secret for TCP authentication | None |


## 🏃 Usage
 
 ### Local Execution
 
 To run a playbook with Piloteer:
 
 ```bash
 # Start the Piloteer CLI, which wraps the ansible-playbook command
 ./target/release/ansible-piloteer playbook.yml

 # Run in CI/Headless mode with Auto-Analyze
 PILOTEER_HEADLESS=1 ./target/release/ansible-piloteer playbook.yml --auto-analyze
 ```
 
## 🧪 Testing

The project includes comprehensive unit and integration tests:

```bash
# Run all tests
cargo test

# Run specific test suite
cargo test unreachable
cargo test report_generation

# Run with output
cargo test -- --nocapture
```

**Test Coverage**:
- Unit tests: Configuration, AI parsing, execution details
- Integration tests: IPC communication, session persistence, unreachable hosts, report generation
- Total: 21 tests covering critical workflows

 ### Distributed Execution
 
 To run Piloteer as a remote debugger (e.g. for a container or remote host):
 
 ```bash
 # On the Controller (TUI)
 ./target/release/ansible-piloteer run --bind 0.0.0.0:9000 --secret mytoken
 
 # On the Executor (Ansible)
 export ANSIBLE_STRATEGY=piloteer
 export PILOTEER_SOCKET=192.168.1.50:9000
 export PILOTEER_SECRET=mytoken
 ansible-playbook playbook.yml
 ```
 
 See [docs/distributed_mode.md](docs/distributed_mode.md) for details.
 
 ### Reporting
 
 Generate an execution report after the run:
 
 ```bash
 ./target/release/ansible-piloteer playbook.yml --report report.md
 # or
 ./target/release/ansible-piloteer playbook.yml --report report.json
 ```

*(Note: Currently, during development, you may need to run the components manually or use a helper script like `run_poc.sh`)*

### Interactive Controls
When a task fails, the Piloteer enters **Debug Mode**:

-   **`r`**: **Retry** the failed task.
-   **`e`**: **Edit/Inject Variable** (Currently simulates injecting `should_fail=false`).
-   **`a`**: **Ask Pilot** (Send failure context to AI for analysis).
-   **`f`**: **Apply Fix** (Apply the variable fix suggested by AI).
-   **`c`**: **Continue** (Accept failure and move to next task).
-   **`q`**: **Quit** the application.

### TUI Controls
-   **Search**: `/` to search logs, `n`/`N` to find next/previous match.
-   **Filter**: `l` to toggle log filtering (All -> Failed -> Changed).
-   **Follow**: `F` (Shift+f) to toggle auto-scrolling of logs.
-   **Analysis Mode**:
    -   `v`: Toggle Analysis Mode (Enter/Exit)
    -   `j` / `k`: Navigate Task List or Data Tree
    -   `Enter`: Toggle expand/collapse
    -   `/`: Search
    -   `n` / `N`: Next/Prev match
    -   `h` / `l`: Collapse/Parent / Expand/Child
    -   `y`: Copy to Clipboard
-   **Session**:
    -   `Ctrl+s`: Manually Save Session

## 📚 Documentation

### User Guides
- [Getting Started](docs/getting_started.md) - Installation and first steps
- [Keyboard Shortcuts](docs/keyboard_shortcuts.md) - Complete shortcut reference
- [Troubleshooting](docs/troubleshooting.md) - Common issues and solutions

### Features
- [Interactive Debugging](docs/interactive_debugging.md) - Debug failed tasks interactively
- [Session Persistence](docs/session_persistence.md) - Save, load, and replay sessions
- [Reporting](docs/reporting.md) - Generate execution reports
- [Deep Analysis](docs/deep_analysis.md) - Data browser and analysis mode
- [Error Handling](docs/error_handling.md) - Unreachable hosts and error recovery

### Advanced Topics
- [Distributed Mode](docs/distributed_mode.md) - Remote debugging setup
- [CI/CD Integration](docs/ci_cd_integration.md) - Headless mode and automation
- [Interface Reference](docs/interface_reference.md) - TUI layout and components
See [docs/interactive_debugging.md](docs/interactive_debugging.md) for a detailed guide on debugging workflows, [docs/deep_analysis.md](docs/deep_analysis.md) for details on using the Data Browser, and [docs/session_persistence.md](docs/session_persistence.md) for session management.

## 🧪 Verification

You can verify the interactive debugging capabilities using the included test script:

```bash
./verify_debug.sh
```
This script runs a test playbook (`fail_on_var.yml`) that conditionally fails, modifies the variable, and successfully retries.

## 🔮 Roadmap
-   **Advanced TUI**: Full variable editor and breakpoint management.
-   **History**: Request/Response logs for AI interactions.
