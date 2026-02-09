# Getting Started with Ansible Piloteer

This guide will help you set up Ansible Piloteer, an interactive AI-powered debugger for your Ansible playbooks.

## Prerequisites

Before you begin, ensure you have the following installed:

1.  **Rust Toolchain**: You need `cargo` to build the CLI.
    -   Install via [rustup.rs](https://rustup.rs).
2.  **Python 3.8+**: Ansible requires Python.
3.  **Ansible**: `pip install ansible`

## Installation

### 1. Build the Piloteer CLI

The core interface is a high-performance Rust application. Build it from source:

```bash
# Clone the repository
git clone https://github.com/your-username/ansible-piloteer.git
cd ansible-piloteer

# Build release binary
cargo build --release
```

The executable will be located at `./target/release/ansible-piloteer`.

### 2. Prepare the Python Environment

Piloteer uses a custom Ansible Strategy Plugin. You must ensure Ansible can find it.

```bash
# If you are using a virtual environment (Recommended)
python3 -m venv venv
source venv/bin/activate
pip install ansible
```

## Configuration

Ansible needs to know two things:
1.  Where to find the Piloteer plugin.
2.  That it should *use* the Piloteer plugin instead of the default `linear` strategy.

You can set this up via environment variables for a specific run, or in your `ansible.cfg`.

### Optional: Authentication (Google Login)

If you plan to use hosted AI features that require authentication, you can log in using your Google account.

**Prerequisites:**
You can use the built-in default credentials (mimicking GCloud CLI) for a quick start, OR provide your own Google OAuth 2.0 Client credentials if you need specific scopes or quotas.

1.  **Run Login (Quick Start)**:
    ```bash
    ./target/release/ansible-piloteer auth login
    ```
    *This uses the default public Client ID. It is convenient for testing.*

2.  **Optional: Configure Custom Credentials**:
    *   **Option A (Env Vars)**:
        ```bash
        export PILOTEER_GOOGLE_CLIENT_ID="your-client-id"
        export PILOTEER_GOOGLE_CLIENT_SECRET="your-client-secret"
        ```
    *   **Option B (Config File)**: Add to `~/.config/ansible-piloteer/piloteer.toml`:
        ```toml
        google_client_id = "your-client-id"
        google_client_secret = "your-client-secret"
        ```

3.  The login command will open your browser. Sign in and authorize the application.
4.  Once successful, the token is saved to `~/.config/ansible-piloteer/auth.json`.

### Method A: Environment Variables (Quick Start)

```bash
# 1. Point to the plugin directory in this repository
export ANSIBLE_STRATEGY_PLUGINS=$(pwd)/ansible_plugin/strategies

# 2. Instruct Ansible to use 'piloteer' strategy
export ANSIBLE_STRATEGY=piloteer
```

### Method B: ansible.cfg (Persistent)

Add the following to your `ansible.cfg` file in your project root:

```ini
[defaults]
strategy_plugins = /path/to/ansible-piloteer/ansible_plugin/strategies
strategy = piloteer
strategy = piloteer

[piloteer]
# Optional: OpenAI Configuration
# openai_api_key = sk-... (or set OPENAI_API_KEY env var)
# model = gpt-4-turbo-preview
# api_base = https://api.openai.com/v1

# Optional: Google OAuth Credentials (defaults provided)
# google_client_id = "your-client-id"
# google_client_secret = "your-client-secret"

# Optional: AI Quota Limits
# quota_limit_tokens = 100000
# quota_limit_usd = 5.00
```

### Method C: Environment Variables for AI
- `OPENAI_API_KEY`: Your API key.
- `PILOTEER_MODEL`: Model name (default: `gpt-4-turbo-preview`).
- `PILOTEER_API_BASE`: Custom API endpoint (for local LLMs).
- `PILOTEER_QUOTA_LIMIT_TOKENS`: Daily token limit (e.g. `50000`).
- `PILOTEER_QUOTA_LIMIT_USD`: Daily cost limit (e.g. `2.50`).

## Running Your First Playbook

Once configured, use the Piloteer CLI to run your playbook. The CLI acts as a wrapper around `ansible-playbook`.

```bash
./target/release/ansible-piloteer my_playbook.yml
```

You should see the Piloteer TUI launch, connecting to Ansible and streaming logs in real-time.

You can now:
-   **Search** logs with `/`.
-   **Cluster** failures with `l` (Filter).
-   **Interact** with failures using the keys displayed in the Status bar (`r`, `e`, `a`, `f`, `c`).

## Advanced Usage

### Distributed Mode

You can run the TUI on a different machine than where Ansible executes (e.g. debugging a container).
See [Distributed Mode Guide](distributed_mode.md) for setup instructions.

### Reporting

To generate a post-execution report:

```bash
./target/release/ansible-piloteer my_playbook.yml --report execution_report.md
```
