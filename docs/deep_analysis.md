# Deep Execution Analysis

Ansible Piloteer includes a powerful **Deep Execution Analysis** feature that allows you to inspect the verbose results of every task and view a comprehensive play recap, all within the TUI.

## Features

-   **Verbose Task Inspection**: View the full JSON return value of any task, including `stdout`, `stderr`, and internal Ansible variables.
-   **Play Recap**: See the aggregated statistics (OK, Changed, Failed, Skipped) for all hosts at the end of the run.
-   **Drift Analysis**: Quickly identify which tasks caused changes to the system.
-   **Reporting**: Export the analysis to Markdown (`Ctrl+e`) for sharing or archiving.

## Usage

### Entering Analysis Mode

At any time during or after execution, press **`v`** to toggle **Analysis Mode**.

Where:
-   **Left Pane**: Displays the list of executed tasks, color-coded by status.
    -   **Green**: Success (OK)
    -   **Yellow**: Changed
    -   **Red**: Failed
-   **Right Pane**: Displays the detailed JSON output for the selected task.
    -   **AI Analysis**: If the task failed and was analyzed by AI, the explanation and suggested fix will be visible here.

### Clipboard Support
Press **`y`** in either pane to copy content to your system clipboard.
-   **Task List**: Copies the task summary.
-   **Data Browser**: Copies the selected JSON value or subtree.

### Navigation Controls

| Key | Action |
| :--- | :--- |
| **`v`** / **`Esc`** | Exit Analysis Mode and return to Live Logs. |
| **`Up`** / **`k`** | Select the previous task in the history list. |
| **`Down`** / **`j`** | Select the next task in the history list. |
| **`Left`** / **`h`** | Collapse node, Jump to Parent, or Scroll Left. |
| **`Right`** / **`l`** | Expand node, Jump to Child, or Scroll Right. |
| **`y`** | **Yank** (Copy) selected content to clipboard. |

#### Data Browser (Right Pane)
The right pane features a hierarchical, interactive JSON tree:

| Key | Action |
| :--- | :--- |
| **`Up`** / **`k`** | Move selection up. |
| **`Down`** / **`j`** | Move selection down. |
| **`Enter`** / **`Space`** | **Collapse / Expand** the selected object or array. |
| **`/`** | Open **Search** input. Type query and press Enter. |
| **`n`** | Jump to **Next Match**. |
| **`n`** | Jump to **Next Match**. |
| **`N`** | Jump to **Previous Match**. |
| **`h`** | **Collapse** node or jump to Parent. |
| **`l`** | **Expand** node or jump to Child. |
| **`y`** | **Yank** (Copy) selected value. |

### Analyzing Failures

When a task fails, Piloteer normally pauses in **Debug Mode**. You can switch to **Analysis Mode** (`v`) to inspect the raw error output from Ansible before deciding to Retry or Ask Pilot.

1.  Task Fails.
2.  Press `v` to open Analysis.
3.  Scroll to the bottom of the JSON output in the right pane to see the `msg` or `stderr`.
4.  Press `v` again to return to the Debug prompt.
5.  Press `a` to ask AI for help, or `r` to retry.

## Play Recap

At the end of a playbook execution, Piloteer captures the standard Ansible Play Recap. This summary is displayed in the Live Log view, but the raw statistics are also available for inspection in the Analysis view (typically as the final entry or via the header stats).
