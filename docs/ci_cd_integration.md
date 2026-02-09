# CI/CD Integration Guide

How to use Ansible Piloteer in CI/CD pipelines for automated testing and debugging.

## Overview

Piloteer supports **headless mode** for non-interactive execution in CI/CD environments. In this mode:
- No TUI is displayed
- Failures are logged to stdout
- Optional AI analysis can be triggered automatically
- Reports can be generated for review
- Exit codes indicate success/failure

---

## Headless Mode

### Basic Usage

```bash
# Enable headless mode
PILOTEER_HEADLESS=1 ansible-piloteer playbook.yml
```

### With Auto-Analysis

Automatically analyze failures with AI:

```bash
PILOTEER_HEADLESS=1 ansible-piloteer playbook.yml --auto-analyze
```

### With Report Generation

Generate a markdown report of the execution:

```bash
PILOTEER_HEADLESS=1 ansible-piloteer playbook.yml --report report.md
```

### Complete Example

```bash
PILOTEER_HEADLESS=1 ansible-piloteer playbook.yml \
  --auto-analyze \
  --report execution_report.md \
  --save-session session.json.gz
```

---

## Exit Codes

Piloteer uses standard exit codes:

| Exit Code | Meaning |
|-----------|---------|
| `0` | All tasks succeeded |
| `1` | One or more tasks failed |
| `2` | Playbook error (syntax, connection, etc.) |

### Example Usage

```bash
if PILOTEER_HEADLESS=1 ansible-piloteer playbook.yml; then
  echo "Playbook succeeded"
else
  echo "Playbook failed with exit code $?"
  exit 1
fi
```

---

## GitHub Actions

### Basic Workflow

```yaml
name: Ansible Playbook CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Set up Python
        uses: actions/setup-python@v4
        with:
          python-version: '3.10'
      
      - name: Install Ansible
        run: pip install ansible
      
      - name: Set up Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1
      
      - name: Build Piloteer
        run: cargo build --release
      
      - name: Run Playbook with Piloteer
        env:
          PILOTEER_HEADLESS: "1"
          ANSIBLE_STRATEGY_PLUGINS: ${{ github.workspace }}/ansible_plugin/strategies
          ANSIBLE_STRATEGY: piloteer
        run: |
          ./target/release/ansible-piloteer tests/playbooks/test.yml \
            --report report.md
      
      - name: Upload Report
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: execution-report
          path: report.md
```

### With AI Analysis

```yaml
      - name: Run Playbook with AI Analysis
        env:
          PILOTEER_HEADLESS: "1"
          OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
          ANSIBLE_STRATEGY_PLUGINS: ${{ github.workspace }}/ansible_plugin/strategies
          ANSIBLE_STRATEGY: piloteer
        run: |
          ./target/release/ansible-piloteer playbook.yml \
            --auto-analyze \
            --report report.md
```

### With Session Artifacts

```yaml
      - name: Save Session on Failure
        if: failure()
        uses: actions/upload-artifact@v3
        with:
          name: debug-session
          path: session.json.gz
```

---

## GitLab CI

### Basic Pipeline

```yaml
stages:
  - test

ansible_test:
  stage: test
  image: ubuntu:22.04
  
  before_script:
    - apt-get update && apt-get install -y python3 python3-pip curl build-essential
    - pip3 install ansible
    - curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    - source $HOME/.cargo/env
    - cargo build --release
  
  script:
    - export PILOTEER_HEADLESS=1
    - export ANSIBLE_STRATEGY_PLUGINS=$(pwd)/ansible_plugin/strategies
    - export ANSIBLE_STRATEGY=piloteer
    - ./target/release/ansible-piloteer playbook.yml --report report.md
  
  artifacts:
    when: always
    paths:
      - report.md
    expire_in: 1 week
```

### With AI Analysis

```yaml
ansible_test_with_ai:
  stage: test
  image: ubuntu:22.04
  
  variables:
    PILOTEER_HEADLESS: "1"
    ANSIBLE_STRATEGY_PLUGINS: "${CI_PROJECT_DIR}/ansible_plugin/strategies"
    ANSIBLE_STRATEGY: "piloteer"
  
  script:
    - ./target/release/ansible-piloteer playbook.yml \
        --auto-analyze \
        --report report.md \
        --save-session session.json.gz
  
  artifacts:
    when: always
    paths:
      - report.md
      - session.json.gz
```

---

## Jenkins

### Declarative Pipeline

```groovy
pipeline {
    agent any
    
    environment {
        PILOTEER_HEADLESS = '1'
        ANSIBLE_STRATEGY_PLUGINS = "${WORKSPACE}/ansible_plugin/strategies"
        ANSIBLE_STRATEGY = 'piloteer'
    }
    
    stages {
        stage('Setup') {
            steps {
                sh 'pip install ansible'
                sh 'cargo build --release'
            }
        }
        
        stage('Run Playbook') {
            steps {
                sh '''
                    ./target/release/ansible-piloteer playbook.yml \
                        --report report.md
                '''
            }
        }
    }
    
    post {
        always {
            archiveArtifacts artifacts: 'report.md', allowEmptyArchive: true
        }
    }
}
```

### With AI Analysis

```groovy
        stage('Run Playbook with AI') {
            environment {
                OPENAI_API_KEY = credentials('openai-api-key')
            }
            steps {
                sh '''
                    ./target/release/ansible-piloteer playbook.yml \
                        --auto-analyze \
                        --report report.md \
                        --save-session session.json.gz
                '''
            }
        }
```

---

## Docker Integration

### Dockerfile

```dockerfile
FROM ubuntu:22.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    python3 python3-pip curl build-essential

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install Ansible
RUN pip3 install ansible

# Copy Piloteer
COPY . /ansible-piloteer
WORKDIR /ansible-piloteer

# Build Piloteer
RUN cargo build --release

# Set environment
ENV PILOTEER_HEADLESS=1
ENV ANSIBLE_STRATEGY_PLUGINS=/ansible-piloteer/ansible_plugin/strategies
ENV ANSIBLE_STRATEGY=piloteer

ENTRYPOINT ["./target/release/ansible-piloteer"]
```

### Usage

```bash
# Build image
docker build -t ansible-piloteer .

# Run playbook
docker run -v $(pwd)/playbooks:/playbooks ansible-piloteer /playbooks/test.yml --report /playbooks/report.md
```

---

## Best Practices

### 1. Use Report Artifacts

Always generate and save reports for later review:

```bash
--report report.md
```

Upload as CI artifacts for easy access.

### 2. Save Sessions on Failure

Capture full session data when things go wrong:

```bash
--save-session session.json.gz
```

This allows local debugging of CI failures.

### 3. Set AI Quotas

Prevent runaway costs in CI:

```bash
export PILOTEER_QUOTA_LIMIT_TOKENS=10000
export PILOTEER_QUOTA_LIMIT_USD=1.00
```

### 4. Use Secrets for API Keys

Never hardcode API keys:

```yaml
# GitHub Actions
env:
  OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}

# GitLab CI
variables:
  OPENAI_API_KEY: $CI_OPENAI_API_KEY
```

### 5. Cache Build Artifacts

Speed up CI by caching Rust builds:

```yaml
# GitHub Actions
- uses: actions/cache@v3
  with:
    path: |
      ~/.cargo/registry
      ~/.cargo/git
      target
    key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
```

### 6. Test in Parallel

Run multiple playbooks in parallel:

```yaml
strategy:
  matrix:
    playbook: [test1.yml, test2.yml, test3.yml]
steps:
  - run: ./target/release/ansible-piloteer ${{ matrix.playbook }}
```

---

## Troubleshooting CI Issues

### Problem: Build takes too long

**Solution**: Use pre-built Docker image or cache dependencies

### Problem: AI analysis fails in CI

**Solution**: Check API key is set correctly and quota limits

### Problem: Reports not generated

**Solution**: Ensure `--report` flag is used and path is writable

### Problem: Exit code always 0

**Solution**: Verify Piloteer is actually running (check for PILOTEER_HEADLESS)

---

## See Also

- [Getting Started Guide](getting_started.md)
- [Troubleshooting Guide](troubleshooting.md)
- [Reporting Documentation](reporting.md)
