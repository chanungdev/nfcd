# nfcd

[![Release](https://img.shields.io/github/v/release/chanungdev/nfcd)](https://github.com/chanungdev/nfcd/releases)
[![CI](https://github.com/chanungdev/nfcd/actions/workflows/ci.yml/badge.svg)](https://github.com/chanungdev/nfcd/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/chanungdev/nfcd)](https://github.com/chanungdev/nfcd#license)

[![English](https://img.shields.io/badge/lang-English_↗-blue)](../README.md)

NFD 파일 이름을 실시간으로 NFC로 바꾸는 macOS용 감시 도구입니다. 한글 파일 이름이 Windows, Linux, Git, 클라우드에서 깨지지 않게 해 줍니다.

> [!NOTE]
> nfcd는 [elgar328/nfd2nfc](https://github.com/elgar328/nfd2nfc) 2.1.0을 바탕으로 한 포크입니다(MIT). gitignore 형식의 ignore 규칙을 넣었고, 파일을 놓치지 않도록 감시 기능을 보강했습니다. 바이너리, 설정 경로, LaunchAgent 라벨이 모두 달라서 nfd2nfc와 함께 설치해도 됩니다. [nfd2nfc에서 옮겨 오기](#nfd2nfc에서-옮겨-오기)를 참고하세요.

## 왜 필요한가

유니코드는 같은 글자를 두 가지 방식으로 적을 수 있습니다.

- **NFC** (조합형): `가` = `U+AC00`
- **NFD** (분해형): `가` = `U+1100 U+1161`

macOS는 파일 이름을 NFD로 저장하는 경우가 많고, Windows, Linux, 대부분의 웹 서비스는 NFC를 씁니다. 그래서 파일을 주고받으면 다음 문제가 생깁니다.

- 한글이 자모로 풀려서 보입니다 (한글 → ㅎㅏㄴㄱㅡㄹ).
- Git이 같은 파일을 untracked나 modified로 표시합니다.
- 클라우드 동기화나 압축 도구가 경로를 찾지 못합니다.

nfcd는 백그라운드에서 돌면서 파일 이름이 생기는 대로 고칩니다.

## 기능

- **실시간 변환:** LaunchAgent가 FSEvents로 폴더를 감시하다가 NFD 이름을 NFC로 바꿉니다.
- **파일을 놓치지 않음**
  - 감시 폴더에 폴더를 옮기거나 압축을 풀어 넣으면, 그 안에 있는 파일까지 모두 바꿉니다.
  - 감시 데몬이 꺼져 있던 동안 생긴 파일은 다음에 시작할 때 바꿉니다.
  - macOS가 파일 시스템 이벤트를 놓쳤다고 알려 오면 감시 폴더를 다시 스캔합니다.
- **gitignore 형식의 ignore 규칙:** global 패턴, 경로별 패턴, 폴더별 `.nfcignore` 파일을 쓸 수 있고 `!`로 예외를 둘 수 있습니다. 어떤 규칙 때문에 제외됐는지는 `nfcd check-ignore`로 확인합니다.
- **설정 즉시 반영:** `config.toml`을 고치면 감시 데몬을 재시작하지 않아도 바로 적용됩니다.
- **메뉴 막대 앱:** 메뉴 막대에서 watcher 상태를 보고 시작하거나 중지할 수 있습니다. 설정 창에서 감시 폴더와 ignore 패턴을 편집합니다(한국어/영어).
- **TUI와 CLI:** 키보드와 마우스로 쓰는 TUI가 있고, 스크립트에서 쓰기 좋은 CLI는 `--json`과 `--dry-run`을 지원합니다.

## 설치

### Homebrew

```bash
brew install chanungdev/tap/nfcd               # CLI, TUI, watcher
brew install --cask chanungdev/tap/nfcd-app    # 메뉴 막대 앱 (위 항목도 함께 설치)
```

### 메뉴 막대 앱

메뉴 막대에 watcher 상태가 표시됩니다. 메뉴에서 watcher를 시작하거나 중지할 수 있고, **Settings…**를 열면 감시 폴더와 ignore 패턴을 편집하고, 경로가 ignore되는지 확인하고, 전체 디스크 접근 권한 상태를 볼 수 있습니다. 로그인할 때 자동으로 실행되며, 상태 탭에서 끌 수 있습니다.

앱이 아직 Apple Developer ID로 서명되지 않아서 처음 실행할 때 macOS가 막습니다.

1. 응용 프로그램 폴더에서 `nfcd.app`을 열고 경고 창을 닫습니다.
2. **시스템 설정 → 개인정보 보호 및 보안**을 열고 아래로 내려서, nfcd 관련 메시지 옆의 **그래도 열기**를 누릅니다.

앱은 `nfcd` CLI를 통해 watcher를 제어합니다. 전체 디스크 접근 권한은 앱이 아니라 `nfcd-watcher`에만 필요합니다.

### 소스로 빌드

[Rust](https://rustup.rs)가 필요합니다.

```bash
git clone https://github.com/chanungdev/nfcd.git
cd nfcd
cargo install --path nfcd --locked
cargo install --path nfcd-watcher --locked
```

`nfcd`는 같은 폴더에서 `nfcd-watcher`를 찾습니다. 그래서 두 바이너리를 같은 위치에 설치해야 합니다(기본 위치는 `~/.cargo/bin`).

### 전체 디스크 접근 권한

`~/Documents`, `~/Downloads` 같은 폴더는 macOS가 보호합니다. 폴더 접근 허용 창이 계속 뜨거나 파일 이름이 바뀌지 않으면 감시 바이너리에 **전체 디스크 접근 권한**을 주세요.

1. `which nfcd-watcher`로 바이너리 경로를 확인합니다.
2. **시스템 설정 → 개인정보 보호 및 보안 → 전체 디스크 접근 권한**을 엽니다.
3. 목록에 `nfcd-watcher`가 이미 있으면 완전히 지웁니다. 토글을 끄지 말고 `-` 버튼으로 지워야 합니다.
4. `+`를 누르고 `Cmd+Shift+G`로 1단계의 경로를 붙여 넣어 추가합니다.
5. `nfcd status`에서 **Full Disk Access**가 `granted`인지 확인합니다.

업그레이드할 때마다 바이너리가 바뀌므로 이 과정을 다시 해야 합니다.

> **참고:** macOS Tahoe 26.1–26.2에서는 시스템 버그 때문에 명령줄 바이너리에 전체 디스크 접근 권한이 적용되지 않습니다. macOS 26.3에서 고쳐졌습니다.

## 빠른 시작

```bash
nfcd config add ~/Downloads                          # 감시 폴더 추가 (기본 모드는 recursive)
nfcd convert ~/Downloads --mode recursive --dry-run  # 이미 있는 NFD 이름 미리 보기
nfcd watcher start
nfcd status
```

인자 없이 `nfcd`만 실행하면 TUI가 열립니다. 처음 실행하면 감시 데몬이 LaunchAgent로 등록되고, 그다음부터는 로그인할 때 자동으로 시작됩니다.

> [!IMPORTANT]
> 감시 데몬은 시작할 때 감시 폴더 안에 이미 있는 NFD 이름을 모두 바꿉니다. ignore된 파일은 건드리지 않습니다. 먼저 `nfcd convert <path> --mode recursive --dry-run`으로 어떤 파일이 바뀔지 확인하세요.

## 설정

설정 파일은 `~/.config/nfcd/config.toml`입니다. 직접 편집해도 되고, TUI의 Config 탭이나 `nfcd config` 명령을 써도 됩니다. 감시 데몬이 바뀐 내용을 알아서 다시 읽습니다. 파일에 오류가 있으면 이전 설정을 그대로 쓰고 경고를 로그에 남깁니다.

처음 설치하면 설정에 기본 global 패턴이 들어 있습니다. 버전 관리, 의존성, IDE 폴더(`.git/`, `node_modules/`, `.venv/`, `.idea/`, `.vscode/` 등)와 번들(`*.app/`, `*.photoslibrary/`)입니다. 다른 패턴처럼 고치거나 지울 수 있습니다.

```toml
[global]                        # 모든 경로에 적용
ignore = [".git/", "node_modules/", "*.app/", "*.photoslibrary/"]

[[paths]]
path = "~/Downloads"
mode = "recursive"
ignore = ["*.crdownload", "/torrents/*", "!/torrents/keep/"]

[[paths]]
path = "~/Desktop"
mode = "children"
```

### 모드

| 모드 | 변환 범위 |
|---|---|
| `recursive` | 폴더와 그 아래 전체 |
| `children` | 폴더 바로 아래 항목만 |

### Ignore 패턴

`.gitignore`와 같은 문법을 씁니다.

- `*`와 `**`: 와일드카드
- `!`: 예외
- 끝에 붙인 `/`: 폴더에만 적용
- 앞에 붙인 `/`: 감시 경로를 기준으로 위치 고정

한글 이름은 NFD로 저장돼 있든 NFC로 저장돼 있든 똑같이 매칭됩니다.

규칙은 아래 순서로 확인하고, 처음 매칭된 규칙을 따릅니다.

1. `.nfcignore` 파일 (깊은 폴더에 있는 파일부터)
2. 해당 경로의 `ignore` 패턴
3. `[global] ignore`

Git과 마찬가지로, ignore된 폴더 안에 있는 파일은 `!`로 다시 포함시킬 수 없습니다. ignore된 폴더 안의 `.nfcignore`도 읽지 않습니다.

### `.nfcignore`

감시 폴더 안 어디에든 `.nfcignore` 파일을 두면, 그 폴더와 하위 폴더에 패턴을 더할 수 있습니다. 문법은 `.gitignore`와 같고, 패턴은 파일이 있는 폴더를 기준으로 합니다.

```gitignore
# ~/Downloads/projects/.nfcignore
build/
*.log
```

`.nfcignore`가 바뀌면 감시 데몬이 다시 스캔합니다. 그래서 더 이상 제외되지 않는 파일도 바뀝니다.

### 다른 경로 안에 등록한 경로

어떤 경로를 다른 경로 안에 따로 등록하면, 그 아래는 새로 등록한 경로가 맡습니다. 거기에는 global 패턴과 그 경로 자신의 패턴만 적용되고, 바깥 경로의 패턴은 적용되지 않습니다. 바깥 경로에서 ignore한 폴더를 다시 감시하고 싶을 때 이렇게 등록하면 됩니다.

이렇게 안쪽에 등록한 경로에 자기 패턴이 없고 바깥 경로가 그 경로를 ignore하지도 않으면, 따로 등록할 의미가 없으므로 **redundant**로 표시합니다.

### 경로 확인

```
$ nfcd check-ignore ~/Downloads/a.crdownload ~/Downloads/projects/app.log ~/Downloads/note.txt
~/Downloads/a.crdownload: ignored by "*.crdownload" from #1 ~/Downloads
~/Downloads/projects/app.log: ignored by "*.log" from ~/Downloads/projects/.nfcignore
~/Downloads/note.txt: not ignored (watched by #1 ~/Downloads)
```

`git check-ignore`처럼, 넘긴 경로 중 ignore된 것이 하나도 없으면 종료 코드 1을 돌려줍니다.

## TUI

인자 없이 `nfcd`를 실행하면 TUI가 열립니다. 쓸 수 있는 단축키가 모두 화면에 표시되고, 마우스도 쓸 수 있습니다.

| 탭 | 용도 |
|---|---|
| **Home** | 감시 데몬 상태를 보고, 시작·중지·재시작합니다. |
| **Config** | 감시 경로와 ignore 패턴을 관리합니다 (아래 키 참고). |
| **Logs** | 지난 로그를 보거나 새 로그를 실시간으로 따라갑니다. 로그는 macOS 시스템 로그에 저장됩니다. |
| **Browser** | 파일 이름이 NFC인지 NFD인지 보고 직접 변환합니다. 여기서도 ignore 규칙이 적용됩니다. |

Config 탭 키:

| 키 | 동작 |
|---|---|
| `a` | 경로 추가 |
| `d` | 경로 삭제 |
| `i` | ignore 패턴 편집. 맨 위 `G` 행은 global 패턴입니다. |
| `m` | 모드 전환 (`recursive` / `children`) |
| `o` | 경로 정렬 |
| `+` / `-` | 경로를 위나 아래로 이동 |
| `s` | 저장 |
| `Esc` | 변경 취소 |

## CLI

명령을 붙여 실행하면 CLI로 동작합니다. 전체 옵션은 `nfcd <command> --help`로 볼 수 있습니다.

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

## nfd2nfc에서 옮겨 오기

nfcd는 nfd2nfc의 설정 파일을 읽지 않고, 설정 형식도 다릅니다. nfd2nfc의 `action = "ignore"` 항목은 바깥 경로의 패턴으로 바꿔 적습니다.

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

설정 파일에 `action`이 남아 있으면 nfcd가 읽지 않습니다.

옮기는 순서:

1. nfd2nfc를 멈춥니다: `nfd2nfc watcher stop`. 같은 폴더를 두 프로그램이 함께 감시하지 않게 하세요.
2. `~/.config/nfcd/config.toml`을 작성합니다.
3. nfcd를 설치하고 시작합니다: `nfcd watcher start`.
4. nfcd를 계속 쓰기로 했다면 nfd2nfc를 지웁니다. 예를 들어 `brew uninstall nfd2nfc`를 실행하고 `~/Library/LaunchAgents/io.github.elgar328.nfd2nfc.plist`도 지웁니다. plist를 지우지 않으면 로그인할 때 nfd2nfc가 다시 시작됩니다.

## 한계

nfcd는 내 Mac에 있는 파일 이름을 고칩니다. 하지만 브라우저, 메일 서비스, 클라우드 저장소, 메신저, 압축 도구 같은 다른 프로그램이 파일을 보내거나 받을 때 이름을 NFD로 바꿀 수 있습니다. 이건 nfcd가 막을 수 없습니다.

## 삭제

```bash
nfcd watcher stop
rm ~/Library/LaunchAgents/io.github.chanungdev.nfcd.plist
brew uninstall --zap --cask nfcd-app     # 메뉴 막대 앱을 설치했다면
brew uninstall nfcd                      # 또는: cargo uninstall nfcd nfcd-watcher
rm -r ~/.config/nfcd                     # 선택: 설정까지 지울 때
```

## 라이선스

[MIT](../LICENSE). elgar328의 [nfd2nfc](https://github.com/elgar328/nfd2nfc)를 바탕으로 합니다.
