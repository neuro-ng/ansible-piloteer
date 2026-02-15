# E2E Testing Roadmap

This document outlines the testing strategy for Ansible Piloteer, specifically focusing on the End-to-End (E2E) Docker environment.

## Initial Spec: Multi-OS Docker Environment (Phase 1)
**Goal**: Create a reproducible environment simulating a heterogeneous fleet of servers.

- [x] **Environment Setup**:
    - **Ubuntu 24.04** (3 hosts): Standard Debian-based target.
    - **Alpine 3.19** (2 hosts): Minimal Musl-libc target (tests python/ssh quirks).
    - **CentOS Stream 9** (2 hosts): RPM-based / Enterprise Linux target.
    - ~~**NixOS** (2 hosts): Declarative OS target.~~ *(Deferred due to container limitations)*
- [x] **Orchestration**:
    - `docker-compose.yml` to spin up all containers with SSH exposed on mapped ports.
    - `Dockerfile` customization for each distro to ensure SSHD and Python3 are present.
- [x] **Inventory Integration**:
    - `inventory.ini` pre-configured with correct ports, users, and python interpreter paths.
- [x] **Connectivity Verification**:
    - `test_playbook.yml` to `ping` all hosts and gather facts.

## Phase 2: Core Functionality Tests
**Goal**: Verify that Piloteer correctly handles basic Ansible operations across all OS types.

- [x] **Fact Gathering**: Ensure `setup` module works on all variants (verified via `ansible_os_family`).
- [x] **Privilege Escalation**: Verify `become: yes` works with passwordless sudo (verified in setup).
- [x] **Package Management**: Test `apt`, `apk`, `dnf`, etc., using `generic` package module abstraction or conditional tasks.
- [x] **File Operations**: Test `copy`, `file`, `lineinfile` (permissions, ownership, content).
- [x] **Service Management**: Test `service` / `systemd` interactions (Note: Docker containers may need specific init configurations for this).

## Phase 3: Interactive Debugging Scenarios
**Goal**: Verify Piloteer's interactive features in a real environment.

- [x] **Pause/Resume**: Test pausing execution on a specific host/task.
- [x] **Variable Injection**:
    - Scenario: Playbook fails due to wrong path/value.
    - Action: Modify variable in Piloteer UI.
    - Verification: Playbook proceeds and succeeds on retry.
- [x] **Retry Logic**:
    - Scenario: Transient failure (e.g., file lock).
    - Action: Fix environment or wait, then Retry.
    - Verification: Task runs again successfully.

## Phase 4: Failure Handling & Edge Cases
**Goal**: Ensure Piloteer behaves robustly under failure.

- [x] **Unreachable Hosts**:
    - Scenario: Stop a container (`docker stop ubuntu-1`).
    - Verification: Piloteer flags host as "Unreachable" but continues with others (if strategy allows).
- [x] **Sudden Disconnect**:
    - Scenario: Kill SSH connection mid-task.
- [x] **Bad Output**:
    - Scenario: Task outputting non-UTF8 or massive data blobs.

## Phase 5: AI Integration Tests
**Goal**: Verify AI analysis on real failures.

- [x] **Simulated Failures**:
    - Create a playbook with "obvious" errors (e.g., installing non-existent package).
    - Capture AI advice and verify relevance for the specific OS (e.g., suggesting `apk` for Alpine, `apt` for Ubuntu).

## Phase 6: Drift Detection & Reporting
**Goal**: Verify tracking of changes.

- [x] **Drift Scenario**:
    - Run play 2: Idempotency check (Result: "Ok").
    - Verification: Piloteer correctly reports drift/no-drift.

## Phase 7: UI & UX Verification
**Goal**: Validate the interactive components of the TUI across specific screens.

### 1. Main Screen (Dashboard)
- [ ] **Log Streaming**:
    - Scenario: Run a long playbook.
    - Verification: Logs scroll automatically (Follow Mode), and can be paused/scrolled manually.
- [ ] **Inspector Pane (Detail Verification)**:
    - Scenario 1 (Normal Execution):
        - Status: `RUNNING` (Green)
        - Current Task: Name matches the executing task.
        - Drift: Updates if "Changed" tasks occur.
    - Scenario 2 (Task Failure):
        - Status: `TASK FAILED` (Red)
        - Error Details:
            - `msg`: The primary error message is clearly visible.
            - `stderr/stdout`: Raw output is scrollable and colored.
            - `module_args`: Input arguments are visible for debugging.
        - AI Pilot:
            - Analysis pane appears (if enabled).
            - "Suggested Fix" is actionable (e.g., `pkg_name: nginx`).
    - Scenario 3 (State Changes):
        - Variable changes from CLI (`piloteer set`) are reflected in "Active Failure" context.
- [ ] **Status Indicators**:
    - Scenario: Disconnect/Reconnect.
    - Verification: Status indicator correctly reflects `DISCONNECTED` vs `RUNNING`.

### 2. View Detail (Analysis Mode)
- [ ] **Navigation**:
    - Action: Press `v` to enter Analysis Mode.
    - Verification: Layout splits into Task List (left) and Data Browser (right).
- [ ] **Data Exploration**:
    - Action: Select a task -> Navigate JSON tree -> Expand/Collapse nodes.
    - Verification: Deep nested structures (variables, facts) match exact output of `-vvv` (accurate keys/values).
- [ ] **Search**:
    - Action: Press `/`, type query (e.g., "failed"), press `Enter`.
    - Verification: Focus jumps to matching key/value in the JSON tree.

### 3. Metrics Screen
- [ ] **Dashboard Access**:
    - Action: Press `m` to toggle Metrics view.
    - Verification: Overlay displays "Task Duration" histogram and "Success Rate" pie chart.
- [ ] **Event Velocity**:
    - Scenario: High-speed log output.
    - Verification: Sparkline graph accurately spikes during verbose tasks (e.g., 50+ logs/sec).
- [ ] **Heatmap**:
    - Action: Toggle to Heatmap view (if implemented).
    - Verification: Grid cells correctly color-code duration (Green < 1s, Red > 10s).
- [ ] **Data Accuracy**:
    - Scenario: Compare "Success Rate" with Play Recap.
    - Verification: Pie chart segments (Ok, Changed, Failed) match final recap numbers.

## Phase 8: Full Application Deployment Scenario
**Goal**: Verify Piloteer against a multi-tier application stack (Web, App, DB).

### 1. Web Server (Ubuntu Hosts)
- [x] **Nginx Installation**:
    - Scenario: Install `nginx` package.
    - Insight: Check if `apt` module works correctly.
- [x] **Configuration**:
    - Scenario: Deploy custom `nginx.conf` via `template` module.
    - Insight: Verify file transfer and jinja2 templating.
- [x] **Service Management**:
    - Scenario: Start and enable `nginx` service.
    - Insight: Verify service state management.

### 2. App Server (CentOS Hosts)
- [x] **Runtime Setup**:
    - Scenario: Install `python3` and `pip`.
    - Insight: Check `dnf` package manager interaction.
- [x] **Application Code**:
    - Scenario: git clone a dummy Flask/FastAPI app.
    - Insight: Verify `git` module and network access from container.
- [x] **Dependency Management**:
    - Scenario: `pip install -r requirements.txt`.
    - Insight: Check command execution and environment variables.

### 3. Database Server (Alpine Hosts)
- [x] **PostgreSQL/Redis**:
    - Scenario: Install `redis` (simpler for Alpine) or `postgresql`.
    - Insight: Verify `apk` module and service init scripts on Alpine.
- [x] **Connectivity**:
    - Scenario: App server connects to Database server.
    - Insight: Verify internal Docker network DNS resolution (`ping alpine-1`).

### 4. Integration Test
- [x] **End-to-End Flow**:
    - Scenario: Curl the Web Server -> Proxies to App Server -> Reads from DB -> Returns JSON.
    - Insight: Full stack connectivity verification.

## Phase 9: CLI Verification
**Goal**: Verify that Piloteer correctly passes arguments to the underlying Ansible process.

- [x] **Inventory Flag**:
    - Scenario: Run `piloteer -i inventory.ini playbook.yml`.
    - Verification: Playbook runs against the specified inventory, not default.
- [x] **Extra Vars**:
    - Scenario: Run `piloteer -e "my_var=123" playbook.yml`.
    - Verification: Playbook picks up the variable (verify via debug task or TUI inspector).
- [x] **Tags/Skip Tags**:
    - Scenario: Run `piloteer --tags "web" playbook.yml`.
    - Verification: Only tasks tagged with "web" are executed.
- [x] **Check Mode**:
    - Scenario: Run `piloteer --check playbook.yml`.
    - Verification: Ansible runs in check mode (no changes made), and TUI reflects this state.
- [x] **Connection Flags**:
    - Scenario: Run `piloteer -u ansible_piloteer --private-key ./id_ed25519 playbook.yml`.
    - Verification: Connection succeeds using provided credentials overriding defaults.
- [x] **Privilege Escalation**:
    - Scenario: Run `piloteer -b --become-user root playbook.yml`.
    - Verification: Tasks run with sudo/root privileges.
- [x] **Environment Variables**:
    - Scenario: `ANSIBLE_FORCE_COLOR=true piloteer playbook.yml`.
    - Verification: Child process receives the variable (verify via debug task showing `lookup('env', 'ANSIBLE_FORCE_COLOR')`).
- [x] **Verbosity Passthrough**:
    - Scenario: `piloteer -vvv playbook.yml`.
    - Verification: Ansible execution is verbose, and Piloteer captures the debug output.
