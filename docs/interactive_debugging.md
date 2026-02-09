# Interactive Debugging Guide

Ansible Piloteer transforms execution into an interactive session. This guide explains how to use the debugger when things go wrong.

## The Interface

When a task fails, Piloteer pauses execution and enters **Debug Mode**. The TUI will display:
-   **Status Bar**: Indicates "TASK FAILED" in red.
-   **Task Info**: The failed task name and host.
-   **Logs**: The error message returned by Ansible.

For a detailed breakdown of all screen components, see the [Interface Reference](interface_reference.md).


### Controls

| Key | Action | Description |
| :--- | :--- | :--- |
| `r` | **Retry** | Re-queues the failed task immediately. Use this after making external changes or modifying variables. |
| `e` | **Edit Variable** | Opens a prompt to inject a variable (e.g., to flip a boolean flag or correct a path). |
| `a` | **Ask Pilot** | Sends the failure details and task variables to the configured AI model for analysis. |
| `f` | **Apply Fix** | Applies the `fix` suggested by the AI (if available). Usually injects a variable and triggers a retry. |
| `c` | **Continue** | Accepts the failure (marking the host as failed) and proceeds to the next available task/host. |
| `Ctrl+e` | **Export Report** | Save the current session analysis to a Markdown file. |
| `Ctrl+s` | **Save Session** | Save the full session state for later replay. |
| `q` | **Quit** | Terminates the entire playbook execution. |

### TUI Navigation & Filtering

| Key | Action | Description |
| :--- | :--- | :--- |
| `/` | **Search** | Open search bar. Type query and press Enter. |
| `n` | **Next Match** | Jump to next search result. |
| `N` | **Prev Match** | Jump to previous search result. |
| `l` | **Toggle Filter** | Cycle log filters: All -> Failed -> Changed -> All. |
| `F` | **Follow Mode** | Toggle "Follow Mode" (auto-scrolling). Default: On. |

## Example: Fixing a "Should Fail" Task

Imagine a playbook with a conditional failure:

```yaml
vars:
  deploy_mode: "maintenance"

tasks:
  - name: Check Deployment Mode
    fail:
      msg: "Cannot deploy during maintenance mode!"
    when: deploy_mode == "maintenance"
```

### Scenario
1.  Ansible runs the task.
2.  Task **FAILS** because `deploy_mode` is "maintenance".
3.  Piloteer pauses and alerts you.

### Resolution Steps
Instead of editing the YAML file and restarting the entire play:

1.  **Press `e`** (Edit Variable).
2.  Input the new value: `{"deploy_mode": "active"}`.
    -   *Note: This injects `deploy_mode="active"` as an `extra_var`, overriding the playbook text.*
3.  **Press `r`** (Retry).
4.  Piloteer clears the failure state and runs the task again.
5.  **Success!** The task is skipped (since the condition `deploy_mode == "maintenance"` is now false), and the playbook continues.

## AI Pilot Workflow

When configured with an API Key or Local LLM, you can use the AI Pilot to diagnose issues.

1.  **Failure Occurs**: The task fails.
2.  **Press `a`** (Ask Pilot): The Piloteer sends the task failure message and captured variables to the AI.
3.  **Review Analysis**: The AI provides an explanation and optionally suggests a fix. This analysis is saved and will appear in exported reports.
4.  **Press `f`** (Apply Fix): If a fix is suggested (e.g., changing a variable), pressing `f` will automatically inject that variable and retry the task.

> **Note**: Piloteer monitors your AI usage. The status bar displays the current daily token usage and estimated cost. Limits can be configured to prevent overspending.

## Limitations

-   Variable injection currently supports simple key-value pairs via the prompt.
-   Modifications are applied via `extra_vars` and persist for the duration involving that host.
