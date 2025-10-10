#!/bin/bash

# EDB - Ethereum Debugger
# Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU Affero General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU Affero General Public License for more details.
#
# You should have received a copy of the GNU Affero General Public License
# along with this program. If not, see <https://www.gnu.org/licenses/>.
#
# This script installs EDB from source
# Run with: curl -sSL https://install.edb.sh | bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Print colored messages
print_error() {
    echo -e "${RED}Error: $1${NC}" >&2
}

print_success() {
    echo -e "${GREEN}$1${NC}"
}

print_info() {
    echo -e "${YELLOW}$1${NC}"
}

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)     echo "Linux";;
        Darwin*)    echo "macOS";;
        CYGWIN*)    echo "Windows";;
        MINGW*)     echo "Windows";;
        *)          echo "Unknown";;
    esac
}

OS=$(detect_os)
print_info "Detected OS: $OS"

# Check if git is installed
check_git() {
    if ! command -v git &> /dev/null; then
        print_error "git is not installed"
        echo ""
        echo "Please install git first:"
        case "$OS" in
            Linux)
                echo "  Ubuntu/Debian: sudo apt-get install git"
                echo "  Fedora/RHEL:   sudo dnf install git"
                echo "  Arch:          sudo pacman -S git"
                ;;
            macOS)
                echo "  Using Homebrew: brew install git"
                echo "  Or download from: https://git-scm.com/download/mac"
                ;;
            Windows)
                echo "  Download from: https://git-scm.com/download/win"
                ;;
            *)
                echo "  Visit: https://git-scm.com/downloads"
                ;;
        esac
        exit 1
    fi
    print_success "✓ git is installed"
}

# Check if cargo is installed
check_cargo() {
    if ! command -v cargo &> /dev/null; then
        print_error "cargo (Rust toolchain) is not installed"
        echo ""
        echo "Please install Rust and Cargo first:"
        case "$OS" in
            Linux|macOS)
                echo "  Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
                ;;
            Windows)
                echo "  Download from: https://rustup.rs/"
                ;;
            *)
                echo "  Visit: https://rustup.rs/"
                ;;
        esac
        echo ""
        echo "After installation, restart your terminal and run this script again."
        exit 1
    fi
    print_success "✓ cargo is installed"
}

# Create ~/.edb directory if it doesn't exist
create_edb_dir() {
    EDB_DIR="$HOME/.edb"
    if [ ! -d "$EDB_DIR" ]; then
        print_info "Creating directory: $EDB_DIR"
        mkdir -p "$EDB_DIR"
        print_success "✓ Created $EDB_DIR"
    else
        print_success "✓ Directory $EDB_DIR already exists"
    fi
}

# Clone the repository
clone_repo() {
    EDB_DIR="$HOME/.edb"
    REPO_DIR="$EDB_DIR/edb"

    if [ -d "$REPO_DIR" ]; then
        print_info "Repository already exists at $REPO_DIR"
        read -p "Do you want to pull the latest changes? (y/n) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            print_info "Pulling latest changes..."
            cd "$REPO_DIR"
            git pull
            print_success "✓ Updated repository"
        fi
    else
        print_info "Cloning repository to $REPO_DIR..."
        git clone https://github.com/edb-rs/edb "$REPO_DIR"
        print_success "✓ Cloned repository"
    fi
}

# Install EDB components
install_edb() {
    REPO_DIR="$HOME/.edb/edb"
    cd "$REPO_DIR"

    print_info "Building and installing EDB components..."
    print_info "This may take a few minutes..."
    echo ""

    # Install main edb binary
    print_info "Installing edb..."
    cargo install --path crates/edb
    print_success "✓ Installed edb"

    # Install rpc-proxy
    print_info "Installing edb-rpc-proxy..."
    cargo install --path crates/rpc-proxy
    print_success "✓ Installed edb-rpc-proxy"

    # Install tui
    print_info "Installing edb-tui..."
    cargo install --path crates/tui
    print_success "✓ Installed edb-tui"
}

# Main installation flow
main() {
    echo ""
    print_info "=========================================="
    print_info "  EDB Installation Script"
    print_info "=========================================="
    echo ""

    # Step 1: Check prerequisites
    print_info "Step 1/4: Checking prerequisites..."
    check_git
    check_cargo
    echo ""

    # Step 2: Create ~/.edb directory
    print_info "Step 2/4: Setting up installation directory..."
    create_edb_dir
    echo ""

    # Step 3: Clone repository
    print_info "Step 3/4: Cloning EDB repository..."
    clone_repo
    echo ""

    # Step 4: Install components
    print_info "Step 4/4: Building and installing EDB..."
    install_edb
    echo ""

    # Success message
    echo ""
    print_success "=========================================="
    print_success "  EDB Installation Complete!"
    print_success "=========================================="
    echo ""
    print_info "You can now use EDB by running:"
    echo "  edb --help"
    echo ""
    print_info "To debug a transaction:"
    echo "  edb --rpc-urls <RPC_ENDPOINT> replay <TX_HASH>"
    echo ""
    print_info "For more information, visit:"
    echo "  https://github.com/edb-rs/edb"
    echo ""
}

# Run main installation
main
