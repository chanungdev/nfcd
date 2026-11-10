# nfcd

[![Release](https://img.shields.io/github/v/release/chanungdev/nfcd)](https://github.com/chanungdev/nfcd/releases)
[![CI](https://github.com/chanungdev/nfcd/actions/workflows/ci.yml/badge.svg)](https://github.com/chanungdev/nfcd/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/chanungdev/nfcd)](https://github.com/chanungdev/nfcd#license)

[![한국어](https://img.shields.io/badge/lang-한국어_↗-blue)](docs/README.ko.md)

> [!NOTE]
> nfcd is a fork of [elgar328/nfd2nfc](https://github.com/elgar328/nfd2nfc). It uses its own config path (`~/.config/nfcd/`) and LaunchAgent label, so it does not conflict with the original.

A macOS tool that watches directories and converts NFD filenames to NFC in real time, ensuring cross-platform compatibility.

## What is NFD/NFC?

Unicode has two main ways to represent composed characters such as Korean Hangul, accented Latin (é, ü, ñ), Japanese kana (が, ぱ), and more:

- **NFC** (Composed): One codepoint per character — `가` = `U+AC00`
- **NFD** (Decomposed): Base + combining marks — `가` = `U+1100 U+1161`

macOS stores filenames in NFD, while Windows and Linux use NFC. This mismatch causes problems when sharing files across platforms:

- Hangul syllables appear as decomposed jamo (e.g., 한글 → ㅎㅏㄴㄱㅡㄹ)
- Accented characters may render or sort incorrectly
- Git sees identical files as untracked or modified
- Cloud sync and archive tools fail to match paths

**nfcd** provides an interactive TUI, a scriptable CLI, and a background watcher service that handles conversion automatically.

## Features

- **Real-time monitoring** — Converts NFD filenames to NFC as soon as they appear
- **Flexible watch paths** — `recursive`/`children` modes plus gitignore-style ignore patterns control exactly what is converted
- **Manual conversion** — Convert filenames between NFD and NFC, individually or recursively
- **Log viewer** — Review past watcher logs or follow new entries live
- **Intuitive TUI** — Every keybinding is shown on screen, with full mouse support
- **CLI mode** — Script and AI-agent friendly, with `--json` output and `--dry-run` previews

## Home

<img src="https://github.com/elgar328/nfd2nfc/releases/download/assets/home.png" alt="Home" width="750" />

The Home tab displays the current watcher service status and provides controls to start, stop, or restart it.

## Config

<img src="https://github.com/elgar328/nfd2nfc/releases/download/assets/config.png" alt="Config" width="750" />

The Config tab manages which directories the watcher monitors. Each path entry has a mode (`recursive` or `children`), its ignore pattern count, and validation status.

### Ignore patterns

Exclude files and directories with gitignore-style patterns in `~/.config/nfcd/config.toml` (the TUI can't edit patterns yet):

```toml
[global]                        # applied to every path
ignore = [".git/", "node_modules/", "*.app/"]

[[paths]]
path = "~/Downloads"
mode = "recursive"
ignore = ["*.crdownload", "/torrents/*", "!/torrents/keep/"]
```

Patterns support `*`, `**`, `!` negation, trailing `/` for directories, and leading `/` to anchor at the entry's path. Korean (or any NFD/NFC) names match regardless of normalization form.

A path nested inside another entry takes over its own subtree, using only the global patterns and its own. Use this to watch a directory that a parent entry ignores.

Run `nfcd check-ignore <path>` to see whether a path is ignored and which pattern decides it.

Restart the watcher after editing the config.

## Logs

<img src="https://github.com/elgar328/nfd2nfc/releases/download/assets/logs.png" alt="Logs" width="750" />

The Logs tab lets you browse past watcher logs and follow new entries in real time. Logs are stored in macOS system logs, and retention is managed by the OS.

## Browser

<img src="https://github.com/elgar328/nfd2nfc/releases/download/assets/browser.png" alt="Browser" width="750" />

The Browser tab lets you inspect files and directories for their Unicode normalization form and convert them directly. The watcher only picks up newly created or modified files, so use this tab to convert any existing NFD names.

## CLI Mode

Running `nfcd` without arguments launches the TUI. Add a command to use CLI mode. Run `nfcd <command> --help` for detailed options.

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

Example:

```
$ nfcd status
Watcher:          running
Version:          2.1.0
Binary:           ~/.cargo/bin/nfcd
Watcher binary:   ~/.cargo/bin/nfcd-watcher
Config:           ~/.config/nfcd/config.toml
Plist:            ~/Library/LaunchAgents/io.github.chanungdev.nfcd.plist
Registered paths: 6 total (4 active, 1 redundant, 1 not found)
Full Disk Access: granted
Update:           up to date
```

## Limitations

nfcd solves the root cause of filenames breaking across operating systems — macOS storing them in NFD. However, other software (browsers, email services, cloud storage, messaging apps, archive tools, etc.) may independently convert filenames to NFD when transferring files. This is application-level behavior outside the scope of nfcd.

## Installation

Build from source (requires [Rust](https://rustup.rs)):

```bash
git clone https://github.com/chanungdev/nfcd.git
cd nfcd
cargo install --path nfcd
cargo install --path nfcd-watcher
```

Both binaries must be installed in the same directory (`~/.cargo/bin` by default).

Then run it:

```bash
nfcd
```

On first launch, the watcher service is automatically registered as a LaunchAgent. After that, you can start and stop it from the TUI or CLI.

### Permissions

If macOS repeatedly prompts for folder access when the watcher converts files, grant **Full Disk Access** to the watcher binary:

1. Run `which nfcd-watcher` to find the binary path
2. Open **System Settings → Privacy & Security → Full Disk Access**
3. If `nfcd-watcher` is already listed, remove it completely (click `-`, not just toggle off)
4. Click `+`, press `Cmd+Shift+G`, paste the path from step 1, and add it
5. Run `nfcd status` and verify that **Full Disk Access** shows `granted`

After upgrading to a new version, you must re-do the steps above to grant Full Disk Access again.

> **Note:** On macOS Tahoe 26.1–26.2, Full Disk Access does not take effect for command-line binaries due to a system bug. This is resolved in macOS 26.3.

### Upgrading

```bash
git pull
cargo install --path nfcd
cargo install --path nfcd-watcher
```

If you have granted Full Disk Access, you must reconfigure it after upgrading.

## Uninstallation

```bash
cargo uninstall nfcd nfcd-watcher
```

## License

[MIT](LICENSE)
