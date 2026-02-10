# Query & REPL Guide

This guide covers the data query and transformation features in Ansible Piloteer, including the interactive REPL and aggregation functions.

## Overview

Ansible Piloteer includes a powerful query engine based on JMESPath that allows you to analyze session data from playbook executions. You can use it in two modes:

1. **One-off queries**: Execute a single query and get results
2. **Interactive REPL**: Explore data interactively with a Read-Eval-Print Loop

## Quick Start

### One-Off Query

```bash
ansible-piloteer query --input session.json.gz "task_history[?failed].name"
```

### Interactive REPL

```bash
ansible-piloteer query --input session.json.gz
```

## REPL Commands

| Command | Description |
|---------|-------------|
| `.help` | Show help and available functions |
| `.templates` | Show pre-built query templates |
| `.json` | Set output to compact JSON |
| `.pretty` | Set output to pretty JSON (default) |
| `.yaml` | Set output to YAML |
| `.exit`, `.quit` | Exit REPL |

## Built-in Functions

### Aggregation Functions

| Function | Description | Example |
|----------|-------------|---------|
| `count(array)` | Count items in array | `count(task_history[?failed])` |
| `sum(array)` | Sum numeric values | `sum(task_history[*].duration)` |
| `avg(array)` | Calculate average | `avg(task_history[*].duration)` |
| `min(array)` | Find minimum value | `min(task_history[*].duration)` |
| `max(array)` | Find maximum value | `max(task_history[*].duration)` |

### Data Manipulation Functions

| Function | Description | Example |
|----------|-------------|---------|
| `group_by(array, expr)` | Group items by expression | `group_by(task_history, &host)` |
| `unique(array)` | Get unique items | `unique(task_history[*].host)` |

## Query Templates

The REPL includes pre-built templates for common queries. Access them with `.templates`:

### 1. Failed Tasks
```
task_history[?failed == `true`]
```

### 2. Changed Hosts
```
task_history[?changed == `true`].host | unique(@)
```

### 3. Unreachable Hosts
```
hosts[?status == 'unreachable'].name
```

### 4. Task Execution Count
```
count(task_history[*])
```

### 5. Failed Tasks by Host
```
group_by(task_history[?failed == `true`], &host)
```

### 6. Tasks with Errors
```
task_history[?error != null].{name: name, error: error}
```

## Example Queries

### Basic Filtering

```bash
# Get all failed task names
task_history[?failed].name

# Get tasks that changed state
task_history[?changed]

# Get tasks on a specific host
task_history[?host == 'web01']
```

### Aggregations

```bash
# Count failed tasks
count(task_history[?failed])

# Average task duration
avg(task_history[*].duration)

# Find slowest task
max(task_history[*].duration)
```

### Advanced Queries

```bash
# Group tasks by status
group_by(task_history, &failed)

# Get unique hosts that had failures
unique(task_history[?failed].host)

# Complex projection
task_history[?failed].{task: name, host: host, error: error}
```

## Output Formats

### JSON (Compact)
```bash
ansible-piloteer query --input session.json.gz --format json "task_history[0]"
```

### JSON (Pretty)
```bash
ansible-piloteer query --input session.json.gz --format pretty-json "task_history[0]"
```

### YAML
```bash
ansible-piloteer query --input session.json.gz --format yaml "task_history[0]"
```

## Session Data Structure

The session file contains:

```json
{
  "task_history": [
    {
      "name": "Task name",
      "host": "hostname",
      "changed": false,
      "failed": false,
      "duration": 1.23,
      "error": null,
      "analysis": null
    }
  ],
  "hosts": [
    {
      "name": "hostname",
      "status": "ok"
    }
  ],
  "logs": ["..."],
  "play_recap": {...}
}
```

## Tips & Tricks

1. **Start with templates**: Use `.templates` to see common patterns
2. **Test incrementally**: Build complex queries step by step in the REPL
3. **Use projections**: Extract only the fields you need with `{name: name, host: host}`
4. **Combine functions**: Chain operations like `unique(task_history[?failed].host)`
5. **Check types**: Aggregation functions work on numeric arrays

## JMESPath Reference

For full JMESPath syntax, see: https://jmespath.org/

Common operators:
- `[]` - Array indexing
- `[*]` - Array projection
- `[?expr]` - Filter expression
- `.` - Sub-expression
- `|` - Pipe (chain operations)
- `&` - Expression reference
- `@` - Current node
