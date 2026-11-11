# nfcd

[![Release](https://img.shields.io/github/v/release/chanungdev/nfcd)](https://github.com/chanungdev/nfcd/releases)
[![CI](https://github.com/chanungdev/nfcd/actions/workflows/ci.yml/badge.svg)](https://github.com/chanungdev/nfcd/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/chanungdev/nfcd)](https://github.com/chanungdev/nfcd#license)

[![한국어](https://img.shields.io/badge/lang-한국어_↗-blue)](docs/README.ko.md)

A macOS watcher that renames NFD filenames to NFC in real time, so Korean (and other composed) filenames survive the trip to Windows, Linux, Git, and the cloud.

> [!NOTE]
> nfcd is a fork of [elgar328/nfd2nfc](https://github.com/elgar328/nfd2nfc) 2.1.0 (MIT). It adds gitignore-style ignore rules and a watcher that doesn't miss files. It uses its own binaries, config path, and LaunchAgent label, so it can be installed next to nfd2nfc. See [Coming from nfd2nfc](#coming-from-nfd2nfc).

## Why

Unicode can write the same character two ways:

- **NFC** (composed): `가` = `U+AC00`
- **NFD** (decomposed): `가` = `U+1100 U+1161`

macOS often stores filenames in NFD, while Windows, Linux, and most web services expect NFC. When files cross that line:

- Hangul shows up as separated jamo (한글 → ㅎㅏㄴㄱㅡㄹ)
- Git reports identical files as untracked or modified
- Cloud sync and archive tools fail to match paths

nfcd runs in the background and fixes names as they appear.

## Features

- **Real-time conversion** — A LaunchAgent watches your directories through FSEvents and renames NFD names to NFC.
- **Doesn't miss files**
  - A folder moved or extracted into a watched directory is converted with everything inside it.
  - Names created while the watcher was off are converted at startup.
  - If macOS drops file system events, the watched directories are rescanned.
- **gitignore-style ignore rules** — Global, per-path, and per-directory `.nfcignore` patterns, with `!` negation. `nfcd check-ignore` shows which rule decides a path.
- **Live config** — Edits to `config.toml` take effect without restarting the watcher.
- **Menu bar app** — Watcher status and start/stop in the menu bar, and a settings window for watched folders and ignore patterns (English/Korean).
- **TUI and CLI** — A keyboard- and mouse-driven TUI, plus a scriptable CLI with `--json` and `--dry-run`.

## Installation

### Homebrew

```bash
brew install chanungdev/tap/nfcd               # CLI, TUI, and watcher
brew install --cask chanungdev/tap/nfcd-app    # menu bar app (also installs the above)
```

Homebrew loads casks from third-party taps only after you trust them. If the cask install fails with `untrusted tap`, run `brew trust --cask chanungdev/tap/nfcd-app` and try again.

### Menu bar app

The app shows the watcher status in the menu bar. From its menu you can start or stop the watcher and open **Settings…**, which edits watched folders and ignore patterns, checks a path against the ignore rules, and shows the Full Disk Access status. It opens at login; turn that off in the Status tab. **Quit menu bar app** closes only the app; the watcher keeps running until you choose **Stop watcher**.

The app isn't signed with an Apple Developer ID yet, so macOS blocks it the first time:

1. Open `nfcd.app` from Applications and close the warning.
2. Open **System Settings → Privacy & Security**, scroll down, and click **Open Anyway** next to the nfcd message.

The app controls the watcher through the `nfcd` CLI. Full Disk Access is needed for `nfcd-watcher` only, not for the app.

### From source

Requires [Rust](https://rustup.rs).

```bash
git clone https://github.com/chanungdev/nfcd.git
cd nfcd
cargo install --path nfcd --locked
cargo install --path nfcd-watcher --locked
```

`nfcd` looks for `nfcd-watcher` in its own directory, so install both binaries to the same place (`~/.cargo/bin` by default).

### Full Disk Access

Folders such as `~/Documents` and `~/Downloads` are protected by macOS. If the watcher keeps triggering folder access prompts, or files aren't converted, grant **Full Disk Access** to the watcher binary:

1. Run `which nfcd-watcher` to find the binary path.
2. Open **System Settings → Privacy & Security → Full Disk Access**.
3. If `nfcd-watcher` is already listed, remove it completely (click `-`, not just toggle it off).
4. Click `+`, press `Cmd+Shift+G`, paste the path from step 1, and add it.
5. Run `nfcd status` and check that **Full Disk Access** shows `granted`.

Repeat these steps after every upgrade, because the new binary needs to be granted again.

> **Note:** On macOS Tahoe 26.1–26.2, Full Disk Access does not take effect for command-line binaries due to a system bug. This is fixed in macOS 26.3.

## Quick start

```bash
nfcd config add ~/Downloads                          # watch a directory (recursive by default)
nfcd convert ~/Downloads --mode recursive --dry-run  # preview existing NFD names
nfcd watcher start
nfcd status
```

Or run `nfcd` with no arguments to open the TUI. On first launch, the watcher is registered as a LaunchAgent and starts at login.

> [!IMPORTANT]
> On startup the watcher converts every existing NFD name under the watched paths, except ignored ones. Check them first with `nfcd convert <path> --mode recursive --dry-run`.

## Configuration

The config file is `~/.config/nfcd/config.toml`. You can edit it by hand, from the TUI Config tab, or with `nfcd config`. The watcher reloads it automatically. If the file is invalid, the watcher keeps the previous config and logs a warning.

On a new install, the config starts with default global patterns for version control, dependency, and IDE folders (`.git/`, `node_modules/`, `.venv/`, `.idea/`, `.vscode/`, …) and bundles (`*.app/`, `*.photoslibrary/`). Edit or remove them like any other pattern.

```toml
[global]                        # applied to every path
ignore = [".git/", "node_modules/", "*.app/", "*.photoslibrary/"]

[[paths]]
path = "~/Downloads"
mode = "recursive"
ignore = ["*.crdownload", "/torrents/*", "!/torrents/keep/"]

[[paths]]
path = "~/Desktop"
mode = "children"
```

### Modes

| Mode | Converts |
|---|---|
| `recursive` | The directory and everything below it |
| `children` | Direct children of the directory only |

### Ignore patterns

Patterns follow `.gitignore` syntax:

- `*` and `**` wildcards
- `!` negation
- a trailing `/` for directories only
- a leading `/` to anchor at the entry's path

Korean names match whether they are stored as NFD or NFC.

Rules are checked in this order. The first match wins:

1. `.nfcignore` files, deepest directory first
2. The path's own `ignore` patterns
3. `[global] ignore`

Like Git, a file inside an ignored directory can't be re-included with `!`, and `.nfcignore` files inside ignored directories are not read.

### `.nfcignore`

Put a `.nfcignore` file in any watched directory to add patterns for that directory and below. It uses the same syntax as `.gitignore`, and patterns are relative to the directory the file is in.

```gitignore
# ~/Downloads/projects/.nfcignore
build/
*.log
```

The watcher rescans when a `.nfcignore` file changes, so files that are no longer ignored get converted.

### Nested paths

A path registered inside another path owns its subtree. Only the global patterns and its own patterns apply there, not the parent's. Use this to watch a directory that a parent path ignores.

A nested path that has no patterns of its own and isn't ignored by its parent is shown as **redundant**.

### Checking a path

```
$ nfcd check-ignore ~/Downloads/a.crdownload ~/Downloads/projects/app.log ~/Downloads/note.txt
~/Downloads/a.crdownload: ignored by "*.crdownload" from #1 ~/Downloads
~/Downloads/projects/app.log: ignored by "*.log" from ~/Downloads/projects/.nfcignore
~/Downloads/note.txt: not ignored (watched by #1 ~/Downloads)
```

The command exits with status 1 if none of the given paths is ignored, like `git check-ignore`.

## TUI

Run `nfcd` with no arguments to open the TUI. Every shortcut is shown on screen, and the mouse works too.

| Tab | Purpose |
|---|---|
| **Home** | Watcher status. Start, stop, or restart the watcher. |
| **Config** | Watch paths and ignore patterns (see the keys below). |
| **Logs** | Browse past watcher logs or follow new entries live. Logs are kept in the macOS system log. |
| **Browser** | See the normalization form of files and convert them manually. Ignore rules apply here too. |

Keys in the Config tab:

| Key | Action |
|---|---|
| `a` | Add a path |
| `d` | Delete a path |
| `i` | Edit ignore patterns. The `G` row at the top holds global patterns. |
| `m` | Toggle the mode (`recursive` / `children`) |
| `o` | Sort paths |
| `+` / `-` | Move a path up or down |
| `s` | Save |
| `Esc` | Discard changes |

## CLI

Add a command to use CLI mode. Run `nfcd <command> --help` for all options.

```
nfcd status [--json]
nfcd watcher start|stop|restart
nfcd config list [--json]
nfcd config add <path> [--mode recursive|children] [--dry-run] [--json]
nfcd config remove <index> [--dry-run] [--json]
nfcd config sort
nfcd convert <path> [--mode name|children|recursive] [--target nfc|nfd] [--dry-run] [--json]
nfcd check-ignore <path>... [--json]
nfcd log show [--last 30m] [--json]
nfcd log stream [--json]
```

```
$ nfcd status
Watcher:          running
Version:          0.1.0
Binary:           /opt/homebrew/bin/nfcd
Watcher binary:   /opt/homebrew/bin/nfcd-watcher
Config:           ~/.config/nfcd/config.toml
Plist:            ~/Library/LaunchAgents/io.github.chanungdev.nfcd.plist
Registered paths: 3 total (2 active, 1 redundant)
Full Disk Access: granted
Update:           up to date
```

## Coming from nfd2nfc

nfcd doesn't read nfd2nfc's config, and its own config format is different. `action = "ignore"` entries are replaced by patterns on the parent path:

```toml
# nfd2nfc (~/.config/nfd2nfc/config.toml)
[[paths]]
path = "~/Documents"
action = "watch"
mode = "recursive"

[[paths]]
path = "~/Documents/Obsidian/.git"
action = "ignore"
mode = "recursive"

# nfcd (~/.config/nfcd/config.toml)
[[paths]]
path = "~/Documents"
mode = "recursive"
ignore = ["/Obsidian/.git/"]
```

nfcd rejects a config that still contains `action`.

Steps:

1. Stop nfd2nfc: `nfd2nfc watcher stop`. Don't run both on the same directories.
2. Write `~/.config/nfcd/config.toml`.
3. Install nfcd and start it: `nfcd watcher start`.
4. Once you're happy with nfcd, uninstall nfd2nfc (for example `brew uninstall nfd2nfc`) and delete `~/Library/LaunchAgents/io.github.elgar328.nfd2nfc.plist`. Otherwise its LaunchAgent starts again at login.

## Limitations

nfcd fixes names on your Mac. Other software, such as browsers, mail services, cloud storage, messengers, and archive tools, may still convert names to NFD on its own when it transfers files. That is outside what nfcd can control.

## Uninstallation

```bash
nfcd watcher stop
rm ~/Library/LaunchAgents/io.github.chanungdev.nfcd.plist
brew uninstall --zap --cask nfcd-app     # if you installed the menu bar app
brew uninstall nfcd                      # or: cargo uninstall nfcd nfcd-watcher
rm -r ~/.config/nfcd                     # optional: remove your config
```

## License

[MIT](LICENSE). Based on [nfd2nfc](https://github.com/elgar328/nfd2nfc) by elgar328.
