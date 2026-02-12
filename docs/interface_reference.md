# Interface Reference

This document provides a detailed reference for the various panes and components of the Ansible Piloteer TUI.

## Main Screen Layout

The main screen is divided into three primary areas:
1.  **Log View** (Left): Streams Ansible output.
2.  **Inspector Pane** (Right): Displays status, details, and AI analysis.
3.  **Help Modal** (Overlay): Accessible via `?`.

---

## Inspector Pane

The Inspector Pane on the right side of the screen provides context-aware information about the current execution state. It is divided into three sections:

### 1. Status Section (Top)
Always visible at the top of the pane.

*   **Status Indicator**:
    *   `RUNNING` (Green): Playbook is executing normally.
    *   `FROZEN` (Yellow): Execution is paused (e.g., waiting for user input or at a breakpoint).
    *   `TASK FAILED` (Red): A task has failed and requires attention.
    *   `DISCONNECTED` (Yellow): Communication with the Ansible controller has been lost.
*   **Current Task**: Displays the name of the active task.
*   **Drift**: Shows the number of tasks that have changed the system state vs. total tasks (e.g., `2 changed / 10 total`).
*   **AI Quota**: Displays current session token usage and estimated cost (visible only if AI is enabled).
*   **Controls**: Context-sensitive hints for available keyboard shortcuts (e.g., `[r]etry`, `[c]ontinue`).

### 2. Inspector Section (Middle)
The main content area of the pane.

*   **Active Failure**: When a task fails, this section displays the raw JSON result from Ansible (keys like `msg`, `stderr`, `stdout`).
*   **Syntax Highlighting**: content is color-coded for readability.
*   **Scrolling**: Use `Up`/`Down` keys (or mouse) to scroll through long output.
*   **State**: If no failure is active, it displays "No Active Failure".

### 3. Pilot Section (Bottom)
Visible only when AI features are active or a suggestion is available.

*   **Analysis**: A natural language explanation of the error and potential root causes provided by the AI model.
*   **Proposed Fix**:
    *   Displays specific key-value pairs (e.g., `deploy_mode: active`) recommended to fix the issue.
    *   **Apply**: Press `f` to automatically inject these variables and retry the task.
*   **Token Usage**: Displays the cost of the analysis for this specific failure.

---

## Analysis Mode (Data Browser)

Accessible via `v` key. Splits the screen into:

*   **Task List** (Left): A history of executed tasks.
*   **Data Browser** (Right): A navigable tree view of verbose task data.
    *   **Navigation**: `j`/`k` to move, `Enter` to expand/collapse.
    *   **Search**: `/` to search within the JSON structure.

---

## Connection Status

The Inspector Pane displays a **"DISCONNECTED"** status (in yellow) if communication with the Ansible controller is interrupted. The application will automatically attempt to reconnect, and the status pane will show a waiting message.

---

## Query CLI

The `ansible-piloteer` CLI supports data querying via the `query` subcommand.

*   `query`: Execute JMESPath queries against session data.
    *   `--input`: Path to session file (e.g., `session.json.gz`).
    *   `--format`: Output format (`json`, `yaml`, `pretty-json`).
