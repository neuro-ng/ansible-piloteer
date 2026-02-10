#!/bin/bash
set -e

# Build the project
cargo build

# Start the Rust server in the background
export PILOTEER_HEADLESS=1
./target/debug/ansible-piloteer > piloteer_output.log 2>&1 &
SERVER_PID=$!

# Give it a moment to bind the socket
sleep 1

# Run Ansible
export ANSIBLE_STRATEGY_PLUGINS=$(pwd)/ansible_plugin/strategies
export ANSIBLE_STRATEGY=piloteer
export PILOTEER_SOCKET=/tmp/piloteer.sock
export ANSIBLE_STDOUT_CALLBACK=default

echo "Running Ansible Playbook (Fail on Var)..."
./venv/bin/ansible-playbook tests/playbooks/fail_on_var.yml

echo "Ansible finished."
kill $SERVER_PID
