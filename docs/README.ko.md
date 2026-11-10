# nfcd

[![Release](https://img.shields.io/github/v/release/chanungdev/nfcd)](https://github.com/chanungdev/nfcd/releases)
[![CI](https://github.com/chanungdev/nfcd/actions/workflows/ci.yml/badge.svg)](https://github.com/chanungdev/nfcd/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/chanungdev/nfcd)](https://github.com/chanungdev/nfcd#license)

[![English](https://img.shields.io/badge/lang-English_↗-blue)](../README.md)

> [!NOTE]
> nfcd는 [elgar328/nfd2nfc](https://github.com/elgar328/nfd2nfc)의 포크입니다. 설정 경로(`~/.config/nfcd/`)와 LaunchAgent 라벨이 달라 원본과 함께 설치해도 충돌하지 않습니다.

NFD 파일명을 실시간으로 감지해 NFC로 변환하는 macOS 도구입니다. 다른 OS와 파일을 주고받을 때 생기는 파일명 깨짐 문제를 해결합니다.

## NFD/NFC란?

유니코드에는 한글, 악센트 문자(é, ü, ñ), 일본어(が, ぱ) 같은 조합 문자를 표현하는 두 가지 정규화 방식이 있습니다:

- **NFC** (조합형): 하나의 코드포인트 — `가` = `U+AC00`
- **NFD** (분해형): 기본 문자 + 결합 문자 — `가` = `U+1100 U+1161`

macOS는 파일명을 NFD로 저장하지만, Windows와 Linux는 NFC를 사용합니다. 이 차이 때문에 다른 OS와 파일을 주고받으면 다음과 같은 문제가 생깁니다:

- 한글이 자모로 풀어져서 표시됨 (예: 한글 → ㅎㅏㄴㄱㅡㄹ)
- 악센트 문자가 잘못 표시되거나 정렬이 깨짐
- Git이 같은 파일을 변경된 것으로 인식
- 클라우드 동기화나 압축 해제 시 경로를 찾지 못함

**nfcd**는 TUI와 CLI, 그리고 백그라운드 감시 서비스를 통해 이 문제를 자동으로 해결합니다.

## 주요 기능

- **실시간 감시** — NFD 파일명이 생성되면 즉시 NFC로 변환
- **감시 경로 설정** — `recursive`/`children` 모드와 gitignore 형식의 ignore 패턴으로 변환 범위를 세밀하게 조절
- **수동 변환** — 파일명을 NFD/NFC 간 변환, 폴더 단위 재귀 변환 지원
- **로그 뷰어** — 과거 감시 로그 조회 및 실시간 로그 스트리밍
- **직관적인 TUI** — 화면에 모든 단축키가 표시되며 마우스 조작도 가능
- **CLI 모드** — 스크립트와 AI 에이전트 친화적, `--json` 출력과 `--dry-run` 지원

## Home

<img src="https://github.com/elgar328/nfd2nfc/releases/download/assets/home.png" alt="Home" width="750" />

Home 탭에서 watcher 서비스의 현재 상태를 확인하고, 시작/중지/재시작할 수 있습니다.

## Config

<img src="https://github.com/elgar328/nfd2nfc/releases/download/assets/config.png" alt="Config" width="750" />

Config 탭에서 감시할 디렉토리를 관리합니다. 각 경로의 모드(`recursive` 또는 `children`), ignore 패턴 개수, 유효성 상태를 확인할 수 있습니다.

### Ignore 패턴

`~/.config/nfcd/config.toml`에 gitignore 형식의 패턴으로 제외할 파일과 폴더를 지정합니다. Config 탭에서 `i`로 편집할 수도 있습니다 (`G` 행이 global 패턴):

```toml
[global]                        # 모든 경로에 적용
ignore = [".git/", "node_modules/", "*.app/"]

[[paths]]
path = "~/Downloads"
mode = "recursive"
ignore = ["*.crdownload", "/torrents/*", "!/torrents/keep/"]
```

`*`, `**`, `!` 예외, 끝의 `/`(폴더 전용), 앞의 `/`(해당 경로 기준 고정)를 지원합니다. 한글 이름은 NFD/NFC 형태와 관계없이 매칭됩니다.

다른 항목 안에 있는 경로를 따로 등록하면, 그 아래는 global 패턴과 자기 패턴만 적용됩니다. 상위 항목이 ignore한 폴더를 다시 감시할 때 사용합니다.

감시 폴더 안에 `.nfcignore` 파일을 두면 `.gitignore`처럼 그 폴더와 하위에 패턴이 적용됩니다. 설정 파일보다 우선하고, 깊은 폴더의 파일이 얕은 폴더의 파일보다 우선합니다. ignore된 폴더 안의 파일은 읽지 않습니다.

`nfcd check-ignore <path>`로 경로가 ignore되는지, 어느 패턴 때문인지 확인할 수 있습니다.

watcher는 설정 변경을 자동으로 반영합니다. 잘못된 설정은 무시하고 이전 설정을 유지합니다.

## Logs

<img src="https://github.com/elgar328/nfd2nfc/releases/download/assets/logs.png" alt="Logs" width="750" />

Logs 탭에서 과거 watcher 로그를 조회하거나 실시간으로 확인할 수 있습니다. 로그는 macOS 시스템 로그에 기록되며, 보관 기간은 OS가 관리합니다.

## Browser

<img src="https://github.com/elgar328/nfd2nfc/releases/download/assets/browser.png" alt="Browser" width="750" />

Browser 탭에서 파일과 디렉토리의 정규화 형식(NFD/NFC)을 확인하고 직접 변환할 수 있습니다. Watcher는 새로 생성/수정된 파일만 감지하므로, 기존 NFD 파일명은 이 탭에서 변환하세요.

## CLI 모드

인수 없이 `nfcd`를 실행하면 TUI가 실행됩니다. 커맨드를 추가하면 CLI 모드로 동작합니다. 각 커맨드의 세부 옵션은 `nfcd <command> --help`로 확인할 수 있습니다.

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

사용 예시:

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

## 제한 사항

nfcd는 macOS가 파일명을 NFD로 저장하여 다른 OS에서 파일명이 깨지는 문제를 근본적으로 해결합니다. 다만 브라우저, 이메일 서비스, 클라우드 스토리지, 메시징 앱, 압축 도구 등은 파일을 주고받는 과정에서 자체적으로 파일명을 NFD로 변환할 수 있습니다. 이는 각 앱 고유의 동작으로, nfcd가 관여할 수 없는 영역입니다.

## 설치 방법

소스에서 빌드합니다 ([Rust](https://rustup.rs) 필요):

```bash
git clone https://github.com/chanungdev/nfcd.git
cd nfcd
cargo install --path nfcd
cargo install --path nfcd-watcher
```

두 바이너리는 같은 디렉터리에 설치되어야 합니다 (기본값 `~/.cargo/bin`).

실행:

```bash
nfcd
```

처음 실행하면 watcher 서비스가 LaunchAgent로 자동 등록됩니다. 이후 TUI나 CLI에서 시작/중지할 수 있습니다.

### 권한 설정

Watcher가 파일을 변환할 때 macOS가 폴더 접근 권한을 반복적으로 요청한다면, watcher 바이너리에 **전체 디스크 접근 권한**을 부여하세요:

1. `which nfcd-watcher`를 실행하여 바이너리 경로 확인
2. **시스템 설정 → 개인정보 보호 및 보안 → 전체 디스크 접근 권한** 열기
3. 이미 `nfcd-watcher`가 목록에 있다면 **토글을 끄는 것이 아니라 완전히 제거** (항목 선택 후 `-` 버튼)
4. `+` 클릭 → `Cmd+Shift+G`를 눌러 1단계의 경로를 입력하고 추가
5. `nfcd status`를 실행하여 **Full Disk Access**가 `granted`로 표시되는지 확인

새 버전으로 업데이트한 후에는 위 단계를 다시 진행하여 전체 디스크 접근 권한을 다시 설정해야 합니다.

> **참고:** macOS Tahoe 26.1~26.2에서는 시스템 버그로 인해 커맨드라인 바이너리에 전체 디스크 접근 권한이 적용되지 않습니다. 이 문제는 macOS 26.3에서 해결되었습니다.

### 업그레이드

```bash
git pull
cargo install --path nfcd
cargo install --path nfcd-watcher
```

전체 디스크 접근 권한을 사용 중이라면, 업그레이드 후 해당 권한을 다시 설정해야 합니다.

## 제거 방법

```bash
cargo uninstall nfcd nfcd-watcher
```

## License

[MIT](../LICENSE)
