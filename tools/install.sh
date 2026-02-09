#!/bin/bash
set -e

# Default paths
BIN_DIR="${HOME}/.local/bin"
PLUGIN_DIR="${HOME}/.ansible/plugins/strategy"
CONFIG_DIR="${HOME}/.config/ansible-piloteer"

# Create directories
mkdir -p "$BIN_DIR"
mkdir -p "$PLUGIN_DIR"
mkdir -p "$CONFIG_DIR"

# Install Binary
echo "Installing binary to $BIN_DIR..."
if [ -f "ansible-piloteer" ]; then
    cp ansible-piloteer "$BIN_DIR/"
    chmod +x "$BIN_DIR/ansible-piloteer"
else
    echo "Error: ansible-piloteer binary not found in current directory."
    echo "Run this script from the dist/ folder after building."
    exit 1
fi

# Install Plugin
echo "Installing plugin to $PLUGIN_DIR..."
if [ -d "ansible_plugin" ]; then
    cp -r ansible_plugin/* "$PLUGIN_DIR/"
else
    echo "Error: ansible_plugin directory not found."
    exit 1
fi

# Install Config
if [ ! -f "$CONFIG_DIR/piloteer.toml" ]; then
    if [ -f "piloteer.toml.example" ]; then
        echo "Installing default config to $CONFIG_DIR..."
        cp piloteer.toml.example "$CONFIG_DIR/piloteer.toml"
    fi
else
    echo "Config exists at $CONFIG_DIR/piloteer.toml, skipping..."
fi

echo "Installation complete!"
echo "Make sure $BIN_DIR is in your PATH."
echo "Add to ansible.cfg:"
echo "[defaults]"
echo "strategy_plugins = ${PLUGIN_DIR}"
echo "strategy = piloteer"
