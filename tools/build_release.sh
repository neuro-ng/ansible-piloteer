#!/bin/bash
set -e

# Setup directories
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST_DIR="$PROJECT_ROOT/dist"
mkdir -p "$DIST_DIR"

echo "Building Ansible Piloteer (Release)..."
cd "$PROJECT_ROOT"
cargo build --release

echo "Assembling artifacts..."
# Copy Binary
cp "$PROJECT_ROOT/target/release/ansible-piloteer" "$DIST_DIR/"

# Copy Plugin
mkdir -p "$DIST_DIR/ansible_plugin"
cp -r "$PROJECT_ROOT/ansible_plugin/"* "$DIST_DIR/ansible_plugin/"

# Copy Config Template (if we had one, creating a dummy one for now)
cat <<EOF > "$DIST_DIR/piloteer.toml.example"
# Ansible Piloteer Configuration

# AI Model settings
# openai_api_key = "sk-..."# model = "gemini"
# api_base = "http://localhost:11434/v1" # Use for LocalAI/Ollama etc.
# log_level = "info"cket_path = "/tmp/piloteer.sock"

# Logging
log_level = "info"
EOF

# Copy Install Script
cp "$PROJECT_ROOT/tools/install.sh" "$DIST_DIR/" 2>/dev/null || true

echo "Build complete! Artifacts in $DIST_DIR"
ls -l "$DIST_DIR"
