# Distributed Tracing with OpenZipkin

Ansible Piloteer supports distributed tracing using OpenZipkin (via OpenTelemetry). This allows you to visualize the execution flow of your playbooks, analyze performance bottlenecks, and debug failures with detailed context.

## Prerequisites

You need a running Zipkin server to collect and view traces. The easiest way to run Zipkin is using Docker:

```bash
docker run -d -p 9411:9411 --name zipkin openzipkin/zipkin
```

Zipkin will be available at `http://localhost:9411`.

## Configuration

Tracing is configured via environment variables.

| Variable | Description | Default |
|----------|-------------|---------|
| `PILOTEER_ZIPKIN_ENDPOINT` | URL of the Zipkin server | `None` (Tracing Disabled) |
| `PILOTEER_ZIPKIN_SERVICE_NAME` | Service name in traces | `ansible-piloteer` |
| `PILOTEER_ZIPKIN_SAMPLE_RATE` | Sampling rate (0.0 to 1.0) | `1.0` (100%) |

### Enabling Tracing

To enable tracing, set the endpoint:

```bash
export PILOTEER_ZIPKIN_ENDPOINT=http://localhost:9411
ansible-piloteer my_playbook.yml
```

To disable tracing, simply unset `PILOTEER_ZIPKIN_ENDPOINT`.

## Viewing Traces

1. Open the Zipkin UI: [http://localhost:9411](http://localhost:9411)
2. Click **Run Query** to see recent traces.
3. Click on a trace to view details.

## Trace Structure

The trace hierarchy mirrors the Ansible execution structure:

- **playbook.execution** (Root Span)
  - **play: <play_name>** (Child of Playbook)
    - **task: <task_name>** (Child of Play)
      - **ipc.send** / **ipc.receive** (IPC Communication)

### Span Attributes

Spans include rich metadata:

- **task.name**: Name of the task
- **task.host**: Host where the task ran
- **task.changed**: Boolean indicating if state changed
- **task.failed**: Boolean indicating failure
- **play.name**: Name of the play
- **play.hosts**: Host pattern for the play
- **error**: Present if the span failed (red in UI)

## Troubleshooting

- **No traces appear?** Ensure `PILOTEER_ZIPKIN_ENDPOINT` is set correctly and the Zipkin server is reachable.
- **Missing spans?** Check `PILOTEER_ZIPKIN_SAMPLE_RATE`. Set it to `1.0` to capture everything.
