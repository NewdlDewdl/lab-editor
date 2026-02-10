# Lab Editor

Terminal editor for assembling university lab submission files.

## Install

### macOS

```
curl -fsSL https://raw.githubusercontent.com/NewdlDewdl/lab-editor/main/install.sh | bash
```

Or download `Lab-Editor.dmg` from [Releases](https://github.com/NewdlDewdl/lab-editor/releases) and copy the binary manually:
```
sudo cp "/Volumes/Lab Editor/lab-editor" /usr/local/bin/
```

### Windows

1. Download `lab-editor.exe` from [Releases](https://github.com/NewdlDewdl/lab-editor/releases)
2. Run from CMD or PowerShell

### Linux

```
cargo install --git https://github.com/NewdlDewdl/lab-editor.git
```

## Usage

```
lab-editor                    # interactive setup
lab-editor -a1 -c2 -l1 -s6   # quick start (activity 1, chapter 2, lab 1, 6 steps)
```

## Controls

| Key | Action |
|-----|--------|
| Ctrl+N / Ctrl+P | Next / previous step |
| Ctrl+L | Clear current step |
| Ctrl+S | Save |
| Ctrl+Q | Quit |

## Build from Source

```
git clone https://github.com/NewdlDewdl/lab-editor.git
cd lab-editor
cargo build --release
```

Binary will be at `target/release/lab-editor`.
