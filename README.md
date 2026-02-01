# yed - YAML Editor TUI

A fast, terminal-based YAML editor with vim-like keybindings. Navigate, edit, and manage YAML files directly from your terminal.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

## Features

- **Tree View Navigation** - Hierarchical display of YAML structure with expand/collapse
- **Vim-like Keybindings** - Familiar navigation for vim users
- **In-place Editing** - Edit values, rename keys, add/delete nodes
- **Search** - Filter nodes by path or key name
- **Mouse Support** - Click to select and expand/collapse nodes
- **Clipboard Integration** - Copy node paths with `y` key
- **Type-aware Editing** - Supports strings, numbers, booleans, null values

## Installation

### From Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/yourusername/yed.git
cd yed

# Build release binary
cargo build --release

# The binary will be at target/release/yed (or yed.exe on Windows)
```

### Using Cargo

```bash
cargo install yed
```

### Pre-built Binaries

Download the latest release from the [Releases](https://github.com/yourusername/yed/releases) page.

| Platform | Architecture | Download | Typical Use |
|----------|--------------|----------|-------------|
| Linux | x86_64 (musl) | `yed-linux-x86_64-musl` | PC, servers |
| Linux | aarch64 (musl) | `yed-linux-aarch64-musl` | Raspberry Pi 4/5, ARM servers |
| Linux | aarch64 (gnu) | `yed-linux-aarch64-gnu` | ARM64 with glibc |
| Linux | armv7 (musl) | `yed-linux-armv7-musl` | Raspberry Pi 3, 32-bit ARM |
| Windows | x86_64 | `yed-windows-x86_64.exe` | Windows PC |
| macOS | x86_64 | `yed-macos-x86_64` | Intel Mac |
| macOS | aarch64 (Apple Silicon) | `yed-macos-aarch64` | M1/M2/M3 Mac |

## Usage

```bash
# Open a YAML file
yed config.yaml

# Open with full path
yed /path/to/your/file.yaml
```

## Keybindings

### Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `gg` | Jump to top |
| `G` | Jump to bottom |
| `Ctrl+u` | Page up |
| `Ctrl+d` | Page down |

### Tree Operations

| Key | Action |
|-----|--------|
| `h` / `←` | Collapse node |
| `l` / `→` | Expand node |
| `Enter` | Toggle expand/collapse (or edit if scalar) |

### Editing

| Key | Action |
|-----|--------|
| `e` | Edit value |
| `r` | Rename key |
| `a` | Add child (key for maps, value for sequences) |
| `d` | Delete node (with confirmation) |

### Other

| Key | Action |
|-----|--------|
| `y` | Copy current node path to clipboard |
| `/` | Start search |
| `n` | Next search match |
| `N` | Previous search match |
| `Ctrl+s` | Save file |
| `q` | Quit (with confirmation) |
| `Esc` | Cancel current operation |

### Input Mode

When editing values or keys:

| Key | Action |
|-----|--------|
| `Enter` | Confirm input |
| `Esc` | Cancel |
| `←` / `→` | Move cursor |
| `Home` / `End` | Jump to start/end |
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character at cursor |

## Value Input Format

When editing or adding values, use the following formats:

| Type | Format | Example |
|------|--------|---------|
| String | Wrap in double quotes | `"hello world"` |
| Integer | Plain number | `42` |
| Float | Decimal number | `3.14` |
| Boolean | `true` or `false` (case-insensitive) | `true`, `FALSE` |
| Null | `null` | `null` |

**Note:** Unquoted text that doesn't match other types will result in an error. Always wrap strings in double quotes.

## ARM Support

yed runs on ARM devices:

| Device | Target | Download |
|--------|--------|----------|
| Raspberry Pi 4 / 5 | aarch64 | `yed-linux-aarch64-musl` |
| Raspberry Pi 3 | armv7 | `yed-linux-armv7-musl` |
| Apple Silicon (M1/M2/M3) | aarch64 | `yed-macos-aarch64` |
| ARM servers (AWS Graviton, etc.) | aarch64 | `yed-linux-aarch64-musl` or `yed-linux-aarch64-gnu` |

**Install on Raspberry Pi:**

```bash
# Download latest release (replace VERSION and choose correct file)
curl -sL https://github.com/yourusername/yed/releases/download/v0.1.0/yed-linux-aarch64-musl -o yed
chmod +x yed
sudo mv yed /usr/local/bin/
```

**Or build from source on the device:**

```bash
# Install Rust (if not installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/yourusername/yed.git
cd yed
cargo build --release
sudo cp target/release/yed /usr/local/bin/
```

## Mouse Support

| Action | Effect |
|--------|--------|
| Left Click | Select row + toggle expand (if container) |
| Scroll Up/Down | Scroll the tree view |

## Interface Layout

```
┌─────────────────────────────────────────────────────────────────┐
│ PATH server.port  DEPTH 2  TYPE number  VALUE 8080              │  ← Status Bar
├─────────────────────────────────────┬───────────────────────────┤
│ Tree                                │ Details                   │
│ ▾ server                            │ Path: server.port         │
│     host = "localhost"              │ Depth: 2                  │
│     port = 8080                     │ Type: number              │
│   ▸ tls                             │ Value: 8080               │
│ ▸ items                             │                           │
│ ▸ feature_flags                     │                           │
│                                     │                           │
├─────────────────────────────────────┴───────────────────────────┤
│ [NORMAL] j/k:move h/l:fold Enter:toggle e:edit r:rename ...     │  ← Help Bar
└─────────────────────────────────────────────────────────────────┘
```

## Building for Different Platforms

### Linux (musl - static binary)

```bash
# Install musl target
rustup target add x86_64-unknown-linux-musl

# Install musl-tools (Ubuntu/Debian)
sudo apt-get install musl-tools

# Build static binary
cargo build --release --target x86_64-unknown-linux-musl
```

### Linux ARM (Raspberry Pi, ARM servers)

```bash
# 64-bit ARM (Raspberry Pi 4/5, AWS Graviton, etc.)
rustup target add aarch64-unknown-linux-musl
cross build --release --target aarch64-unknown-linux-musl

# 32-bit ARM (Raspberry Pi 3, etc.)
rustup target add armv7-unknown-linux-musleabihf
cross build --release --target armv7-unknown-linux-musleabihf
```

**On ARM device directly (no cross-compilation):**

```bash
# On Raspberry Pi 4 (aarch64)
cargo build --release

# Or specify target
cargo build --release --target aarch64-unknown-linux-gnu
```

### Windows

```bash
cargo build --release --target x86_64-pc-windows-msvc
```

### macOS

```bash
# Intel Mac
cargo build --release --target x86_64-apple-darwin

# Apple Silicon (M1/M2/M3)
cargo build --release --target aarch64-apple-darwin
```

### Cross-compilation with Docker (Linux / macOS host)

For building Linux musl binaries **from a Linux or macOS machine**:

```bash
# Update Rust first (cross requires rustc 1.92+)
rustup update

# Using cross (requires Docker)
cargo install cross

cross build --release --target x86_64-unknown-linux-musl
cross build --release --target aarch64-unknown-linux-musl
```

If you cannot update Rust, use an older cross version:

```bash
cargo install cross --version 0.2.4
```

### Building Linux/ARM binaries from Windows

**`cross` on Windows requires Docker Desktop** (or another Docker engine) to be installed and running. Without Docker, you will see errors like `toolchain 'stable-x86_64-unknown-linux-gnu' may not be able to run on this system`.

**Options:**

1. **Use GitHub Actions (recommended)**  
   Push a tag (e.g. `v0.1.0`); the workflow will build all targets (including Linux and ARM) and attach them to the Release. No local cross-compilation needed.

2. **Use WSL2 (Windows Subsystem for Linux)**  
   Install WSL2, open a Linux shell, install Rust and Docker there, then run `cross build ...` as in the “Cross-compilation with Docker” section above.

3. **Use Docker Desktop on Windows**  
   Install [Docker Desktop](https://www.docker.com/products/docker-desktop/), start it, then in PowerShell or CMD:
   ```powershell
   cargo install cross
   cross build --release --target aarch64-unknown-linux-musl
   ```

4. **Build on the ARM device**  
   On a Raspberry Pi or other ARM machine, clone the repo and run `cargo build --release` (see “ARM Support” below).

## Configuration

Currently, yed does not require a configuration file. All settings are controlled via command-line arguments and keyboard shortcuts.

## Troubleshooting

### "Failed to copy path" error

Clipboard functionality requires:
- **Linux**: `xclip` or `xsel` installed, or running in a Wayland session with `wl-copy`
- **Windows**: Works out of the box
- **macOS**: Works out of the box

```bash
# Ubuntu/Debian
sudo apt-get install xclip

# Fedora
sudo dnf install xclip

# Arch Linux
sudo pacman -S xclip
```

### Terminal compatibility

yed requires a terminal that supports:
- 256 colors (recommended)
- Mouse events
- Alternate screen buffer

Tested terminals:
- Windows Terminal ✓
- iTerm2 ✓
- Alacritty ✓
- Kitty ✓
- GNOME Terminal ✓
- Konsole ✓

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [Ratatui](https://github.com/ratatui-org/ratatui) - Rust TUI library
- YAML parsing by [yaml-rust2](https://github.com/Ethiraric/yaml-rust2)
- Inspired by vim and other TUI tools
