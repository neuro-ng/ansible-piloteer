# Reporting and Export

Ansible Piloteer allows you to export the details of your debugging session into a human-readable format. This is useful for:
- Creating post-mortem reports.
- Sharing execution details with team members.
- Archiving playbook runs for compliance or audit.

## Generating Reports

### 1. From the Interactive TUI
While running the TUI, you can export a report at any time:

- Press `Ctrl+e`.
- A file named `piloteer_report_YYYYMMDD_HHMMSS.md` will be created in your current directory.
- A notification will appear in the TUI confirming the file path.

### 2. From the CLI
You can instruct Ansible Piloteer to generate a report automatically after execution:

```bash
# Run a playbook and generate a Markdown report
ansible-piloteer my_playbook.yml --report my_report.md

# Run a playbook and generate a JSON dump
ansible-piloteer my_playbook.yml --report my_report.json
```

## Report Formats

### Markdown (`.md`)
The Markdown report includes:
- **Execution Summary**: Date, time, and overall status.
- **Host Summary**: Table showing OK, Changed, and Failed task counts per host.
- **Task History**: Chronological list of all executed tasks with their status (✅ OK, ⚠️ Changed, ❌ Failed).
- **Failure Details**: If a task failed, the error message and any captured stdout/stderr are included.
- **AI Analysis**: (Coming Soon) If AI features were used, the analysis and suggested fixes will be included.

### JSON (`.json`)
The JSON export is a raw dump of the task history array. It is useful for programmatic processing or ingesting into other tools.
