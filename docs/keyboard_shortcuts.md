# Keyboard Shortcuts Reference

Complete reference of all keyboard shortcuts in Ansible Piloteer.

## Main View

### Navigation
| Key | Action |
|-----|--------|
| `↑`/`k` | Scroll logs up |
| `↓`/`j` | Scroll logs down |
| `PgUp` | Page up in logs |
| `PgDn` | Page down in logs |

### View Controls
| Key | Action |
|-----|--------|
| `v` | Toggle Analysis Mode (detailed task inspection) |
| `H` | Toggle Host List |
| `?` | Toggle Help Modal |
| `q` | Quit application |

### Search
| Key | Action |
|-----|--------|
| `/` | Start search (in logs or data browser) |
| `n` | Next search match |
| `N` | Previous search match |
| `Esc` | Cancel search |

### Session Management
| Key | Action |
|-----|--------|
| `Ctrl+s` | Save session snapshot |
| `Ctrl+e` | Export report (Markdown) |

---

## Analysis Mode

### Focus Control
| Key | Action |
|-----|--------|
| `Tab` | Switch focus between Task List and Data Browser |
| `v` | Return to Main View |
| `?` | Toggle Help Modal |

### Task List (when focused)
| Key | Action |
|-----|--------|
| `↑`/`k` | Previous task |
| `↓`/`j` | Next task |
| `Enter` | Select task and view details |

### Data Browser (when focused)
| Key | Action |
|-----|--------|
| `↑`/`k` | Move up in tree |
| `↓`/`j` | Move down in tree |
| `h` | Collapse current node |
| `l` | Expand current node |
| `Shift+h` | Deep collapse (recursive) |
| `Shift+l` | Deep expand (recursive) |
| `w` | Toggle text wrapping (truncate long lines with '...') |
| `v` | Toggle visual selection mode |
| `0-9` | Enter count for next command |
| `y` | Copy current value / selection to clipboard (supports count e.g. `5y`) |
| `/` | Search in data |
| `n` | Next search result |
| `N` | Previous search result |

---

## Inspector (Task Failure)

When a task fails, the Inspector shows these controls:

| Key | Action |
|-----|--------|
| `r` | Retry task (with current variables) |
| `e` | Edit variables (modify and retry) |
| `c` | Continue (skip failure and proceed) |
| `a` | Ask Pilot (AI analysis) |

---

## Host List Modal

| Key | Action |
|-----|--------|
| `↑`/`k` | Previous host |
| `↓`/`j` | Next host |
| `Enter` | Filter tasks by selected host |
| `f` | View facts for selected host |
| `Esc` | Close host list |
| `H` | Close host list |

---

## Variable Editor

When editing variables (`e` during task failure):

| Key | Action |
|-----|--------|
| `Type` | Enter variable value |
| `Enter` | Confirm and retry task |
| `Esc` | Cancel edit |

---

## Quick Reference by Context

### When viewing logs:
- `v` - Switch to Analysis Mode for detailed inspection
- `/` - Search logs
- `H` - View host list
- `Ctrl+s` - Save session

### When task fails:
- `a` - Get AI analysis
- `e` - Edit variables
- `r` - Retry
- `c` - Continue anyway

### When in Analysis Mode:
- `Tab` - Switch between task list and data browser
- `y` - Copy data to clipboard
- `/` - Search in data
- `w` - Toggle text wrapping

### When in Data Browser:
- `h`/`l` - Collapse/expand nodes
- `Shift+h`/`Shift+l` - Deep collapse/expand
- `w` - Toggle text wrapping
- `y` - Copy value

---

## Tips

1. **Search is context-aware**: `/` searches logs in Main View, but searches data in Analysis Mode
2. **Deep navigation**: Use `Shift+h`/`Shift+l` to recursively collapse/expand entire subtrees
3. **Text wrapping**: Press `w` to toggle wrapping. When off, long lines are truncated with '...'
4. **Clipboard**: The `y` key works in both Inspector and Data Browser
5. **Help is always available**: Press `?` to see context-specific help
6. **Session snapshots**: Use `Ctrl+s` frequently to save your debugging session

---

## See Also

- [Getting Started Guide](getting_started.md) - Learn the basics
- [Interactive Debugging](interactive_debugging.md) - Debugging workflows
- [Deep Analysis](deep_analysis.md) - Data Browser features
