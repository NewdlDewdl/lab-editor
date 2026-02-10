#!/bin/bash
set -euo pipefail

REPO="NewdlDewdl/lab-editor"
BINARY_NAME="lab-editor"

echo "Installing $BINARY_NAME..."

OS="$(uname -s)"

case "$OS" in
    Darwin)
        ASSET="lab-editor-macos"
        ;;
    Linux)
        echo ""
        echo "Linux detected. Please install from source:"
        echo ""
        echo "  cargo install --git https://github.com/$REPO.git"
        echo ""
        exit 0
        ;;
    MINGW*|MSYS*|CYGWIN*)
        echo ""
        echo "Windows detected. Please download lab-editor.exe from:"
        echo ""
        echo "  https://github.com/$REPO/releases/latest"
        echo ""
        exit 0
        ;;
    *)
        echo "Error: Unsupported OS: $OS"
        exit 1
        ;;
esac

URL="https://github.com/$REPO/releases/latest/download/$ASSET"

TMPFILE="$(mktemp)"
trap 'rm -f "$TMPFILE"' EXIT

echo "Downloading from $URL..."
curl -fsSL -o "$TMPFILE" "$URL"
chmod +x "$TMPFILE"

if sudo cp "$TMPFILE" "/usr/local/bin/$BINARY_NAME" 2>/dev/null; then
    sudo chmod +x "/usr/local/bin/$BINARY_NAME"
    INSTALL_DIR="/usr/local/bin"
else
    mkdir -p "$HOME/bin"
    cp "$TMPFILE" "$HOME/bin/$BINARY_NAME"
    chmod +x "$HOME/bin/$BINARY_NAME"
    INSTALL_DIR="$HOME/bin"

    if [[ ":$PATH:" != *":$HOME/bin:"* ]]; then
        echo ""
        echo "Note: $HOME/bin is not in your PATH. Add it with:"
        echo ""
        echo "  export PATH=\"\$HOME/bin:\$PATH\""
        echo ""
    fi
fi

echo ""
echo "Installed $BINARY_NAME to $INSTALL_DIR/$BINARY_NAME"
echo ""
echo "Run it with:"
echo ""
echo "  $BINARY_NAME"
echo ""
