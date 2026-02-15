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
Reliable usage requires creating your own **Google OAuth 2.0 Desktop App** credentials. The default credentials may be revoked or rate-limited.

**How to Create Credentials:**
1.  Go to the [Google Cloud Console Credentials Page](https://console.cloud.google.com/apis/credentials).
2.  Click **Create Credentials** -> **OAuth client ID**.
3.  Select Application type: **Desktop app**.
4.  Name it "Ansible Piloteer".
5.  Add **Authorized Redirect URI**: `http://localhost:8085`.
6.  Click **Create** and copy your **Client ID** and **Client Secret**.

**Configure Piloteer:**
You can pass these credentials via environment variables or strictly in the config file.

```bash
export PILOTEER_GOOGLE_CLIENT_ID="your-new-client-id"
export PILOTEER_GOOGLE_CLIENT_SECRET="your-new-client-secret"
```

**Login:**
```bash
./target/release/ansible-piloteer auth login
```

Once successful, the token is saved to `~/.config/ansible-piloteer/auth.json`.

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
