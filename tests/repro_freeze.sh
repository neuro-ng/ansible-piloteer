#!/bin/bash
set -e

# Build release
cargo build --release

# Run in HEADLESS mode with the sample playbook
# This should print "Headless: Ansible Connected" etc. to stdout
# We capture output and check for success indicators

echo "Running Ansible Piloteer in headless mode..."
export PILOTEER_HEADLESS=1
./target/release/ansible-piloteer ./tests/playbooks/hello.yml > piloteer_headless.log 2>&1 &
PID=$!

# Wait for a bit
sleep 5

# Kill process
kill $PID || true

echo "Checking logs..."
cat piloteer_headless.log

if grep -q "Headless: Ansible Connected" piloteer_headless.log; then
    echo "SUCCESS: Connected"
else
    echo "FAILURE: Not connected"
    exit 1
fi

if grep -q "Task Result: OK" piloteer_headless.log; then
    echo "SUCCESS: Task executed"
else
    echo "FAILURE: Task not executed"
    exit 1
fi
