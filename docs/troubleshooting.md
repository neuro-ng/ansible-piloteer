# Troubleshooting Guide

Common issues and solutions for Ansible Piloteer.

## Installation Issues

### Problem: `cargo build` fails with compilation errors

**Cause**: Outdated Rust toolchain or missing dependencies

**Solution**:
```bash
# Update Rust toolchain
rustup update

# Clean and rebuild
cargo clean
cargo build --release
```

### Problem: Python module import errors

**Cause**: Ansible can't find the Piloteer strategy plugin

**Solution**:
```bash
# Verify plugin path is set correctly
export ANSIBLE_STRATEGY_PLUGINS=$(pwd)/ansible_plugin/strategies
export ANSIBLE_STRATEGY=piloteer

# Or add to ansible.cfg:
# [defaults]
# strategy_plugins = /path/to/ansible-piloteer/ansible_plugin/strategies
# strategy = piloteer
```

---

## Connection Problems

### Problem: "Connection refused" or "Socket not found"

**Cause**: IPC socket path mismatch between CLI and plugin

**Solution**:
```bash
# Check socket path
echo $PILOTEER_SOCKET

# Ensure both CLI and plugin use the same path
# Default is /tmp/piloteer.sock

# If using custom path:
export PILOTEER_SOCKET=/custom/path/piloteer.sock
```

### Problem: "Handshake failed" error

**Cause**: Secret token mismatch when using TCP mode

**Solution**:
```bash
# Ensure same secret on both sides
export PILOTEER_SECRET="your-secret-token"

# Or use Unix socket (no authentication needed)
export PILOTEER_SOCKET=/tmp/piloteer.sock
```

### Problem: Playbook hangs at "Waiting for connection"

**Cause**: Firewall blocking TCP connection or wrong host/port

**Solution**:
```bash
# Check if port is accessible
nc -zv <host> <port>

# Verify bind address
export PILOTEER_BIND_ADDR="0.0.0.0:8765"

# Check firewall rules
sudo ufw status
```

### Problem: "DISCONNECTED" Status Appears

**Cause**: The Ansible playbook process terminated unexpectedly or the IPC connection dropped.

**Solution**:
- Check if the playbook finished execution.
- Check Ansible logs for crashes.
- The application will automatically attempt to reconnect if the process restarts.

---

## IPC Socket Errors

### Problem: "Address already in use"

**Cause**: Previous Piloteer instance didn't clean up socket

**Solution**:
```bash
# Remove stale socket
rm /tmp/piloteer.sock

# Or use different socket path
export PILOTEER_SOCKET=/tmp/piloteer-$(date +%s).sock
```

### Problem: "Permission denied" on socket

**Cause**: Insufficient permissions to create/access socket

**Solution**:
```bash
# Use directory with write permissions
export PILOTEER_SOCKET=$HOME/.piloteer/piloteer.sock
mkdir -p $HOME/.piloteer

# Or use TCP mode instead
export PILOTEER_SOCKET="localhost:8765"
```

---

## AI API Issues

### Problem: "API key not found" or "Unauthorized"

**Cause**: OpenAI API key not configured

**Solution**:
```bash
# Set API key
export OPENAI_API_KEY="sk-..."

# Verify it's set
echo $OPENAI_API_KEY

# Or add to config file
# ~/.config/ansible-piloteer/piloteer.toml
```

### Problem: "Rate limit exceeded"

**Cause**: Too many API requests

**Solution**:
```bash
# Set quota limits
export PILOTEER_QUOTA_LIMIT_TOKENS=50000
export PILOTEER_QUOTA_LIMIT_USD=2.50

# Use AI analysis sparingly
# Only press 'a' when really needed
```

### Problem: "Model not found" error

**Cause**: Invalid model name or model not available

**Solution**:
```bash
# Use supported model
export PILOTEER_MODEL="gpt-4-turbo-preview"

# For local LLMs:
export PILOTEER_API_BASE="http://localhost:1234/v1"
export PILOTEER_MODEL="local-model"
```

### Problem: AI analysis returns gibberish or errors

**Cause**: Incompatible API endpoint or model

**Solution**:
```bash
# Verify API base URL
curl $PILOTEER_API_BASE/models

# Test with OpenAI first
export PILOTEER_API_BASE="https://api.openai.com/v1"

# For local LLMs, ensure OpenAI-compatible endpoint
```

---

## Performance Problems

### Problem: TUI is slow or laggy

**Cause**: Too many logs or large data structures

**Solution**:
- Use search (`/`) to filter logs
- Clear logs periodically (restart session)
- Reduce verbosity in playbook
- Use Analysis Mode (`v`) for focused inspection

### Problem: High memory usage

**Cause**: Large playbooks with many tasks/hosts

**Solution**:
```bash
# Save session and restart
# Press Ctrl+s to save
# Quit and reload session:
./target/release/ansible-piloteer --load-session session.json.gz
```

### Problem: Session file is very large

**Cause**: Verbose task results stored in history

**Solution**:
- Session files are gzip-compressed
- Limit verbose output in playbooks
- Clean up old sessions periodically

---

## Display Issues

### Problem: TUI looks broken or garbled

**Cause**: Terminal doesn't support required features

**Solution**:
```bash
# Use modern terminal emulator
# Recommended: iTerm2, Alacritty, Windows Terminal

# Check terminal type
echo $TERM

# Try setting TERM
export TERM=xterm-256color
```

### Problem: Colors don't display correctly

**Cause**: Terminal doesn't support 256 colors

**Solution**:
```bash
# Enable 256 color support
export TERM=xterm-256color

# Test colors
curl -s https://gist.githubusercontent.com/HaleTom/89ffe32783f89f403bba96bd7bcd1263/raw/ | bash
```

---

## Debugging Workflow Issues

### Problem: Can't edit variables during task failure

**Cause**: Task doesn't support variable modification

**Solution**:
- Not all task failures can be fixed with variable changes
- Use `a` (AI analysis) to understand the issue
- Use `c` (continue) to skip and proceed
- Fix the playbook and re-run

### Problem: Retry doesn't work as expected

**Cause**: Task has side effects or state changes

**Solution**:
- Some tasks can't be safely retried (e.g., database migrations)
- Use `c` (continue) instead
- Restart playbook from beginning if needed

### Problem: AI analysis not helpful

**Cause**: Insufficient context or complex issue

**Solution**:
- Use Analysis Mode (`v`) to inspect full task details
- Check verbose output in Data Browser
- Review task definition in playbook
- Consult Ansible documentation for the module

---

## Session Management Issues

### Problem: Can't load saved session

**Cause**: Session file corrupted or incompatible version

**Solution**:
```bash
# Check file exists and is readable
ls -lh session.json.gz

# Try decompressing manually
gunzip -c session.json.gz | jq . | head

# If corrupted, session can't be recovered
# Start fresh playbook run
```

### Problem: Session doesn't include all data

**Cause**: Session saved before playbook completed

**Solution**:
- Wait for playbook to complete before saving
- Or save multiple snapshots during execution
- Use `Ctrl+s` at key points

---

## Headless Mode Issues

### Problem: Headless mode doesn't exit on failure

**Cause**: Expected behavior - headless mode continues by default

**Solution**:
```bash
# Use --auto-analyze for AI analysis on failures
PILOTEER_HEADLESS=1 ansible-piloteer playbook.yml --auto-analyze

# Check exit code
echo $?
# 0 = success, non-zero = failures occurred
```

### Problem: No report generated in headless mode

**Cause**: Report flag not specified

**Solution**:
```bash
# Add --report flag
PILOTEER_HEADLESS=1 ansible-piloteer playbook.yml --report report.md
```

---

## Common Error Messages

### "Failed to parse IPC message"

**Cause**: Version mismatch between CLI and plugin

**Solution**: Rebuild both CLI and ensure plugin is up-to-date

### "Task timeout"

**Cause**: Task taking longer than expected

**Solution**: Increase timeout in playbook or wait for completion

### "Clipboard error"

**Cause**: No clipboard support in environment

**Solution**: Copy manually or use different terminal

---

## Getting Help

If you encounter an issue not covered here:

1. **Check logs**: Look for error messages in the TUI
2. **Enable debug logging**:
   ```bash
   export PILOTEER_LOG_LEVEL=debug
   ```
3. **Check GitHub Issues**: [github.com/your-repo/issues](https://github.com/your-repo/issues)
4. **File a bug report**: Include:
   - Piloteer version (`cargo --version`)
   - Ansible version (`ansible --version`)
   - Error messages
   - Steps to reproduce

---

## See Also

- [Getting Started Guide](getting_started.md)
- [Keyboard Shortcuts](keyboard_shortcuts.md)
- [Interactive Debugging](interactive_debugging.md)
