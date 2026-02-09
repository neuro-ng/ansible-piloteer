#!/bin/bash
set -e

echo "Starting Piloteer (Headless)..."
# Start Piloteer in background
ansible-piloteer &
PILOTEER_PID=$!

# Wait for socket
sleep 2

echo "Running Ansible Playbook..."
# Run Playbook
if ansible-playbook test_playbook.yml; then
    echo "Playbook finished successfully!"
    EXIT_CODE=0
else
    echo "Playbook failed!"
    EXIT_CODE=1
fi

echo "Stopping Piloteer..."
kill $PILOTEER_PID || true

exit $EXIT_CODE
