# rest-snapview

Terminal UI for browsing restic snapshots.

## Features

- Browse snapshots in a restic repository (sorted by date, newest first)
- Navigate directory trees within snapshots
- Search/filter files by name with `/`
- Download files/folders with directory picker
- Tab completion for paths
- Remembers last download directory

## Requirements

- Rust 1.74+ (tested with 1.93.0)
- restic CLI installed and in PATH
- Repository access configured via environment variables

## Environment Variables

Required:
- `RESTIC_REPOSITORY` - Repository location
- `RESTIC_PASSWORD` or `RESTIC_PASSWORD_FILE` or `RESTIC_PASSWORD_COMMAND` - Repository password

## Usage

```bash
export RESTIC_REPOSITORY="rest:https://your-server/repo"
export RESTIC_PASSWORD_FILE="$HOME/.restic-password"
cargo run
```

## Keyboard Controls

| Key | Action |
|-----|--------|
| `↑` / `k` | Move cursor up |
| `↓` / `j` | Move cursor down |
| `Ctrl-F` | Page down (full screen) |
| `Ctrl-B` | Page up (full screen) |
| `Ctrl-D` | Scroll down (half screen) |
| `Ctrl-U` | Scroll up (half screen) |
| `g` / `Home` | Go to first item |
| `G` / `End` | Go to last item |
| `Tab` | Switch panel |
| `Enter` | Open directory / Select snapshot |
| `Backspace` / `h` | Go to parent directory |
| `/` | Search/filter files (Files panel) |
| `d` | Download selected file/folder |
| `?` | Show help |
| `q` / `Esc` | Quit |

### File Search (press `/` in Files panel)

| Key | Action |
|-----|--------|
| Type | Filter files by name |
| `↑` / `↓` | Navigate filtered list |
| `Enter` | Confirm filter (stay filtered) |
| `Esc` | Clear filter and exit search |

### Download Dialog

| Key | Action |
|-----|--------|
| `Tab` | Auto-complete path |
| `↑` / `↓` | Select directory |
| `←` | Go to parent directory |
| `→` / `Enter` | Enter selected directory / confirm |
| `Esc` | Cancel |
