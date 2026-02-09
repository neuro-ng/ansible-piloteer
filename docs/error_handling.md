# Error Handling & Unreachable Hosts

How Ansible Piloteer handles errors, unreachable hosts, and failure scenarios.

## Overview

Piloteer provides comprehensive error handling for various failure scenarios:
- **Task failures**: Interactive debugging with retry/edit/continue options
- **Unreachable hosts**: Automatic detection and tracking
- **Connection errors**: Clear error messages and recovery strategies
- **API failures**: Graceful degradation when AI services unavailable

---

## Task Failures

### Interactive Debugging

When a task fails, Piloteer enters **Inspector Mode** with these options:

| Key | Action | Description |
|-----|--------|-------------|
| `r` | Retry | Re-run the task with current variables |
| `e` | Edit | Modify variables and retry |
| `a` | Ask Pilot | Get AI analysis of the failure |
| `c` | Continue | Skip this failure and proceed |

### Example Workflow

1. Task fails (e.g., package installation)
2. Press `a` to get AI analysis
3. AI suggests the repository is missing
4. Press `e` to edit variables
5. Add the repository URL
6. Press Enter to retry with new variables
7. Task succeeds!

---

## Unreachable Hosts

### Detection

Piloteer automatically detects when hosts are unreachable due to:
- SSH connection failures
- Network timeouts
- Authentication errors
- Host not found

### Tracking

Unreachable hosts are:
- ✅ Tracked in a separate set (`unreachable_hosts`)
- ✅ Logged with clear error messages
- ✅ Recorded in task history as failed
- ✅ Persisted in session saves
- ✅ Included in reports

### Behavior

When a host is unreachable:
- **No interactive debugging** - Can't retry connection failures
- **Logged automatically** - Error message shown in logs
- **Playbook continues** - Other hosts proceed normally
- **Status tracked** - Unreachable count visible in status bar

### Example Log Entry

```
⚠️  Host 192.168.1.100 unreachable during task 'Gather Facts': SSH connection timeout
```

### In Reports

Unreachable hosts appear in the execution report:

```markdown
### Task: Gather Facts [FAILED]
- **Host:** 192.168.1.100
- **Status:** ❌ FAILED
- **Error:**
```
SSH connection timeout after 30s
```
```

---

## Connection Errors

### IPC Connection Failures

**Symptom**: "Connection refused" or "Socket not found"

**Cause**: CLI and Ansible plugin can't communicate

**Recovery**:
1. Check socket path matches on both sides
2. Ensure no firewall blocking (for TCP mode)
3. Verify plugin is loaded correctly

See [Troubleshooting Guide](troubleshooting.md#connection-problems) for details.

### SSH Connection Failures

**Symptom**: "Host unreachable" or "Authentication failed"

**Cause**: Can't connect to managed hosts

**Recovery**:
1. Verify SSH keys are configured
2. Check network connectivity
3. Ensure host is running
4. Verify inventory is correct

**Piloteer Behavior**:
- Marks host as unreachable
- Continues with other hosts
- Logs error for review

---

## AI Service Failures

### API Errors

**Symptom**: "API key not found" or "Rate limit exceeded"

**Cause**: AI service unavailable or misconfigured

**Recovery**:
1. Check API key is set: `echo $OPENAI_API_KEY`
2. Verify quota limits not exceeded
3. Try again later if rate limited

**Piloteer Behavior**:
- Shows error message in TUI
- Debugging continues without AI
- Other features remain functional

### Graceful Degradation

When AI is unavailable:
- ✅ Interactive debugging still works
- ✅ Can manually inspect task details
- ✅ Can edit variables and retry
- ✅ Reports generated without AI analysis

---

## Error Message Formatting

### In TUI

Errors are displayed with:
- ⚠️ Warning icon for unreachable hosts
- ❌ Error icon for task failures
- Clear, concise error messages
- Relevant context (task name, host, etc.)

### In Logs

```
[2024-02-09 12:34:56] ❌ Task 'Install nginx' failed on host web1
[2024-02-09 12:34:56]    Error: Package not found in repository
[2024-02-09 12:35:10] ⚠️  Host db1 unreachable: Connection timeout
```

### In Reports

Errors are formatted in markdown with:
- Status badges (FAILED, OK, CHANGED)
- Code blocks for error messages
- AI analysis (if available)
- Suggested fixes

---

## Session Persistence with Errors

### What's Saved

Session files include:
- ✅ Failed task history
- ✅ Unreachable host list
- ✅ Error messages
- ✅ AI analysis results
- ✅ All logs

### Loading Sessions

When loading a session with failures:
- Replay mode is enabled
- Can review all errors
- Can inspect task details
- Can generate reports

### Example

```bash
# Save session after failure
# Press Ctrl+s in TUI

# Later, load and review
ansible-piloteer --load-session session.json.gz

# Generate report from session
ansible-piloteer --load-session session.json.gz --report report.md
```

---

## Recovery Strategies

### For Task Failures

1. **Analyze**: Use `a` to get AI insights
2. **Inspect**: Use `v` (Analysis Mode) to view full details
3. **Fix**: Use `e` to modify variables
4. **Retry**: Use `r` to try again
5. **Skip**: Use `c` if not critical

### For Unreachable Hosts

1. **Verify connectivity**: Check network, SSH, etc.
2. **Fix infrastructure**: Restart hosts, fix firewall, etc.
3. **Update inventory**: Remove or fix host entries
4. **Re-run playbook**: Start fresh after fixes

### For API Failures

1. **Check configuration**: Verify API key and settings
2. **Check quotas**: Ensure limits not exceeded
3. **Use manual debugging**: Inspect without AI
4. **Try later**: Rate limits reset over time

---

## Best Practices

### 1. Save Sessions Frequently

Use `Ctrl+s` to save snapshots:
- Before making changes
- After important steps
- When encountering errors

### 2. Use AI Sparingly

To avoid quota limits:
- Only use `a` when really stuck
- Review task details first (`v`)
- Set quota limits in config

### 3. Handle Unreachable Hosts

- Verify inventory before running
- Use `--limit` to exclude known bad hosts
- Fix connectivity issues first

### 4. Review Reports

After execution:
- Generate report with `--report`
- Review all failures
- Plan fixes for next run

### 5. Test in Stages

For complex playbooks:
- Use `--tags` to run subsets
- Test connectivity first
- Verify variables before full run

---

## Headless Mode Error Handling

In headless mode (CI/CD):
- Errors logged to stdout
- Exit code indicates failure
- Reports include all errors
- Sessions can be saved for debugging

### Example

```bash
PILOTEER_HEADLESS=1 ansible-piloteer playbook.yml \
  --auto-analyze \
  --report report.md \
  --save-session session.json.gz

# Check exit code
if [ $? -ne 0 ]; then
  echo "Playbook failed, check report.md"
  exit 1
fi
```

---

## See Also

- [Interactive Debugging](interactive_debugging.md) - Debugging workflows
- [Troubleshooting Guide](troubleshooting.md) - Common issues
- [Session Persistence](session_persistence.md) - Save/load sessions
- [Reporting](reporting.md) - Generate reports
