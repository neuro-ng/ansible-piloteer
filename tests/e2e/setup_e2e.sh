#!/bin/bash
set -e

KEY_NAME="id_ed25519"

if [ ! -f "$KEY_NAME" ]; then
    echo "Generating SSH key pair ($KEY_NAME)..."
    ssh-keygen -t ed25519 -f "$KEY_NAME" -N "" -C "ansible_piloteer@e2e"
    chmod 600 "$KEY_NAME"
    chmod 644 "$KEY_NAME.pub"
    echo "Keys generated."
else
    echo "SSH keys already exist."
fi
