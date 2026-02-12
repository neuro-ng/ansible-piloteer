# OpenZipkin Distributed Tracing

Ansible Piloteer integrates OpenZipkin distributed tracing to provide deep observability into playbook execution, AI interactions, and distributed IPC communication.

## Overview

Distributed tracing allows you to:
- **Visualize execution flow** across playbooks, plays, and tasks
- **Measure performance** with precise timing for each operation
- **Track AI interactions** including token usage and response times
- **Debug distributed systems** by tracing IPC communication between controller and executors
- **Identify bottlenecks** in playbook execution

## Quick Start

### 1. Start Zipkin

Using Docker:
```bash
docker run -d -p 9411:9411 openzipkin/zipkin
```

Using Docker Compose:
```yaml
version: '3'
services:
  zipkin:
    image: openzipkin/zipkin
    ports:
      - "9411:9411"
```

### 2. Configure Ansible Piloteer

Set the Zipkin endpoint via environment variable:

```bash
export PILOTEER_ZIPKIN_ENDPOINT=http://localhost:9411
```

Optional configuration:
```bash
# Custom service name (default: "ansible-piloteer")
export PILOTEER_ZIPKIN_SERVICE_NAME=my-ansible-service

# Sampling rate: 0.0 (never) to 1.0 (always), default: 1.0
export PILOTEER_ZIPKIN_SAMPLE_RATE=0.5  # Sample 50% of traces
```

### 3. Run Ansible Piloteer

```bash
ansible-piloteer my_playbook.yml
```

### 4. View Traces

Open Zipkin UI in your browser:
```
http://localhost:9411
```

## Configuration Reference

| Environment Variable | Description | Default |
|---------------------|-------------|---------|
| `PILOTEER_ZIPKIN_ENDPOINT` | Zipkin server URL | None (tracing disabled) |
| `PILOTEER_ZIPKIN_SERVICE_NAME` | Service name in traces | `ansible-piloteer` |
| `PILOTEER_ZIPKIN_SAMPLE_RATE` | Sampling rate (0.0-1.0) | `1.0` (100%) |

## Sampling Strategies

### Always Sample (Development)
```bash
export PILOTEER_ZIPKIN_SAMPLE_RATE=1.0
```
Captures every trace. Best for development and debugging.

### Never Sample (Disabled)
```bash
unset PILOTEER_ZIPKIN_ENDPOINT
# OR
export PILOTEER_ZIPKIN_SAMPLE_RATE=0.0
```
Disables tracing entirely. No performance overhead.

### Probabilistic Sampling (Production)
```bash
export PILOTEER_ZIPKIN_SAMPLE_RATE=0.1  # 10% of traces
```
Randomly samples a percentage of traces. Reduces overhead in high-volume environments.

## Trace Structure

Ansible Piloteer creates a hierarchical span structure for playbook execution:

```
Playbook Execution (root span)
├── Task 1 (host: web1)
├── Task 1 (host: web2)
├── Task 2 (host: web1)
├── Task 3 (host: db1)
└── Play Recap
```

### Span Lifecycle

**Playbook Span (Root)**:
- **Created**: On IPC handshake (when Ansible connects)
- **Closed**: On Play Recap (when playbook completes)
- **Attributes**:
  - `service.name`: "ansible-piloteer"
  - `service.version`: Package version

**Task Spans (Children)**:
- **Created**: On `TaskStart` message
- **Updated**: On `TaskFail` message (if task fails)
- **Closed**: On `TaskResult` message
- **Attributes**:
  - `task.name`: Task name
  - `task.host`: Target host
  - `task.changed`: Whether task changed system state
  - `task.failed`: Whether task failed
  - `task.status`: "OK", "CHANGED", or "FAILED"
  - `error.message`: Error details (if failed)

**AI Spans (Children of Task Spans)**:
- **Created**: When AI analysis is triggered for a failed task
- **Closed**: After AI response is received and parsed
- **Attributes**:
  - `ai.model`: LLM model name (e.g., "gpt-4")
  - `ai.task_name`: Task being analyzed
  - `ai.api_base`: API endpoint URL
  - `ai.response_time_ms`: API response time in milliseconds
  - `ai.tokens_used`: Total tokens consumed
  - `ai.success`: Whether analysis succeeded
  - `ai.fix_suggested`: Whether a fix was provided
  - `error.message`: Error details (if failed)

### Example Trace

When you run a playbook, you'll see traces like this in Zipkin:

**Playbook Span** (`playbook.execution`):
- Duration: 15.2s
- Service: ansible-piloteer
- Version: 0.1.0

**Task Spans** (children of playbook):
- `task: Install nginx` (host: web1) - 2.3s - OK
- `task: Install nginx` (host: web2) - 2.1s - OK  
- `task: Start nginx` (host: web1) - 0.5s - CHANGED
- `task: Configure firewall` (host: web1) - 1.2s - FAILED (with error details)
  - `ai.analyze_failure` - 1.5s - AI analysis of the failure
    - Model: gpt-4
    - Tokens: 250
    - Fix suggested: true

## Architecture

Ansible Piloteer uses OpenTelemetry with OTLP HTTP exporter to send traces to Zipkin:

```
┌─────────────────────┐
│ Ansible Piloteer    │
│                     │
│  ┌──────────────┐   │
│  │ OpenTelemetry│   │
│  │   SDK        │   │
│  └──────┬───────┘   │
│         │ OTLP/HTTP │
└─────────┼───────────┘
          │
          ▼
┌─────────────────────┐
│  Zipkin Server      │
│  (Port 9411)        │
└─────────────────────┘
```

**Why OTLP instead of native Zipkin exporter?**
- Avoids OpenSSL build dependencies
- Uses rustls for TLS (pure Rust)
- Compatible with Zipkin's OTLP endpoint (`/api/v2/spans`)
- More flexible for future observability backends

## Troubleshooting

### Traces not appearing in Zipkin

1. **Check endpoint configuration:**
   ```bash
   echo $PILOTEER_ZIPKIN_ENDPOINT
   # Should output: http://localhost:9411
   ```

2. **Verify Zipkin is running:**
   ```bash
   curl http://localhost:9411/health
   # Should return: {"status":"UP"}
   ```

3. **Check sampling rate:**
   ```bash
   echo $PILOTEER_ZIPKIN_SAMPLE_RATE
   # Should be > 0.0 (or unset for default 1.0)
   ```

4. **Look for initialization errors:**
   Ansible Piloteer logs tracing initialization failures to stderr:
   ```
   Warning: Failed to initialize tracing: <error details>
   ```

### High overhead in production

Reduce sampling rate:
```bash
export PILOTEER_ZIPKIN_SAMPLE_RATE=0.01  # 1% sampling
```

Or disable tracing entirely:
```bash
unset PILOTEER_ZIPKIN_ENDPOINT
```

## Performance Impact

- **Overhead when disabled**: None (tracing is completely skipped)
- **Overhead with 100% sampling**: ~1-2% CPU, minimal memory
- **Network**: Traces are batched and sent asynchronously
- **Recommended for production**: 1-10% sampling rate

## Next Steps

- **Instrumentation**: Playbook, play, and task spans are planned for the next phase
- **AI Tracing**: LLM API call instrumentation coming soon
- **IPC Tracing**: Distributed mode communication tracing in development

## Related Documentation

- [Getting Started](getting_started.md)
- [Distributed Mode](distributed_mode.md)
- [Troubleshooting](troubleshooting.md)
