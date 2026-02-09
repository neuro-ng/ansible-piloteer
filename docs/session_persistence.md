# Session Persistence

Ansible Piloteer supports saving execution sessions to disk, allowing for later analysis, sharing of debug logs, and archival purposes.

## Auto-Archive
By default, every execution session is automatically saved to a compressed JSON file upon exit.
- **Location**: `~/.config/ansible-piloteer/archive/`
- **Format**: `session_YYYYMMDD_HHMMSS.json.gz`

## Manual Save
You can manually save the current session snapshot at any time during execution.
- **Key**: `Ctrl+s`
- **Location**: `~/.config/ansible-piloteer/archive/` (same as auto-archive)

## Replay Mode
You can load a saved session to inspect it in the TUI without running Ansible. This is useful for:
- Analyzing a playbook run that happened on a different machine (e.g., CI/CD).
- Reviewing past failures.
- Sharing execution context with a colleague.

### Usage
```bash
ansible-piloteer --replay ~/.config/ansible-piloteer/archive/session_20231027_100000.json.gz
```

In Replay Mode:
- **Navigation**: All TUI navigation works as normal (Log view, Data Browser, Search).
- **Interactive Controls**: Execution controls (`Retry`, `Continue`, `Edit`) are disabled.
- **AI Pilot**: You can still ask the AI to analyze failures if you have an API key configured, as the context is preserved.

## Data Format
The session file captures:
- **Task History**: Full list of executed tasks with status and timing.
- **Logs**: The raw stdout/stderr logs from Ansible.
- **Facts**: Host facts (`ansible_facts`) if gathered.
- **Verbose Data**: Full `-vvvvv` debug output for every task.
