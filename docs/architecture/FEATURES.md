# WezTerm 기능 명세서 (Feature Specification)

## 목차

1. [프로젝트 개요](#1-프로젝트-개요)
2. [컴포넌트 아키텍처 맵](#2-컴포넌트-아키텍처-맵)
3. [핵심 기능 목록](#3-핵심-기능-목록)
4. [CLI 명령어 매핑](#4-cli-명령어-매핑)
5. [Lua API 매핑](#5-lua-api-매핑)
6. [설정 시스템](#6-설정-시스템)

---

## 1. 프로젝트 개요

**WezTerm**은 Rust로 작성된 GPU 가속 터미널 에뮬레이터입니다.
다음 플랫폼을 지원합니다: Linux, macOS, Windows 10+, FreeBSD, NetBSD.

### 주요 특징

| 기능 | 설명 |
|------|------|
| 멀티플렉싱 | 도메인별 멀티플렉서 서버, 원격 호스트 연결 지원 |
| 렌더링 | wgpu 기반 GPU 가속 (Linux/macOS), SDL2 백엔드 옵션 |
| 폰트 | True Type, Harfbuzz 글자 모양(shape) 렌더링, 이모지 & 리가처 지원 |
| 그래픽 | iTerm2 imgcat, Kitty graphics, Sixel 지원 |
| 연결 | SSH, TLS over TCP/IP, 직렬 포트(serial port) 지원 |
| 설정 | Lua 기반 설정 파일, 핫 리로딩 (hot reloading) |

---

## 2. 컴포넌트 아키텍처 맵

### 2.1 메인 애플리케이션 크레이트 (`wezterm/`)

| 경로 | 역할 |
|------|------|
| `wezterm/src/main.rs` | CLI 진입점, 서브커맨드 dispatcher |
| `wezterm/src/cli/` | multiplexer 제어 CLI 명령어 모음 (20+ 개) |
| `wezterm/src/asciicast.rs` | asciinema 레코드/재생 기능 |

### 2.2 GUI 애플리케이션 (`wezterm-gui/`)

| 경로 | 역할 |
|------|------|
| `wezterm-gui/src/main.rs` | GUI 진입점 |
| `wezterm-gui/src/frontend.rs` | GUI 프론트엔드 추상화 (백엔드별 렌더링 추상) |
| `wezterm-gui/src/inputmap.rs` | 키 바인딩 매핑 시스템 |
| `wezterm-gui/src/commands.rs` | GUI 명령어 dispatcher |
| `wezterm-gui/src/spawn.rs` | 프로세스 스폰 관리 |
| `wezterm-gui/src/tabbar.rs` | 탭 바 렌더링 |
| `wezterm-gui/src/selection.rs` | 텍스트 선택 시스템 |
| `wezterm-gui/src/scrollbar.rs` | 스크롤바 렌더링 |
| `wezterm-gui/src/glyphcache.rs` | 글리프 캐싱 (성능 최적화) |
| `wezterm-gui/src/colorease.rs` | 색보정 이징(easing) 효과 |
| `wezterm-gui/src/download.rs` | 다운로드 관리 |
| `wezterm-gui/src/update.rs` | 자동 업데이트 체크 |
| `wezterm-gui/src/stats.rs` | 성능 통계 모니터링 |

### 2.3 멀티플렉서 (`mux/`)

가장 핵심적인 컴포넌트. **터미널 세션, 창, 탭, 패ان(panes), 도메인**을 관리합니다.

| 파일 | 역할 |
|------|------|
| `mux/src/lib.rs` | MuxServer 핵심: 클라이언트/세션 관리, 알림 라우팅 |
| `mux/src/pane.rs` | Pane (개별 터미널 영역) 모델 |
| `mux/src/tab.rs` | Tab 모델, split 요청 처리 |
| `mux/src/window.rs` | Window 모델 (GUI 창) |
| `mux/src/client.rs` | MuxClient (CLI/remote 클라이언트) |
| `mux/src/domain.rs` | Domain: 로컬/SSH/WSL 실행 도메인 추상화 |
| `mux/src/ssh.rs` | SSH 연결 및隧道 기능 |
| `mux/src/ssh_agent.rs` | SSH agent 프록시 |
| `mux/src/tmux.rs` | tmux 호환성 레이어 |
| `mux/src/renderable.rs` | 렌더링 가능한 탭/윈도우 모델 |

### 2.4 터미널 에뮬레이션 (`termwiz/`)

**터미널 에뮬레이션의 핵심 라이브러리**. 자체 escape sequence parser 보유.

| 파일/디렉토리 | 역할 |
|---------------|------|
| `termwiz/src/lib.rs` | Surface, Terminal trait, Widget 추상화 |
| `termwiz/src/terminal.rs` | 터미널 디바이스 트레이트 ( 입출력 추상) |
| `termwiz/src/keymap.rs` | 키 매핑 시스템 |
| `termwiz/src/input.rs` | 입력 처리 (키보드/마우스) |
| `termwiz/src/caps/` | 터미널 기능 능력(caps) probing |
| `termwiz/src/render/` | 렌더링 관련 유틸리티 |
| `termwiz/src/lineedit/` | 라인 에디터 (bash/zsh 스타일) |
| `termwiz/src/widgets/` | UI 위젯 레이아웃 시스템 |

### 2.5 터미널 커널 (`term/`)

| 파일 | 역할 |
|------|------|
| `term/src/` | vt100 터미널 모델, ANSI escape 처리 |
| `term/src/lib.rs` | Terminal 구조체: 셀 배열, 상태 관리, 스크롤백 |

### 2.6 설정 시스템 (`config/`)

Lua 기반 설정의 핵심. **모든 설정 옵션**이 여기에 정의됩니다.

| 파일 | 역할 |
|------|------|
| `config/src/lib.rs` | Configuration 단일tons, Lua 상태 관리 |
| `config/src/config.rs` | 메인 Configuration 구조체 |
| `config/src/keys.rs` | 키 바인딩 파싱/처리 |
| `config/src/keyassignment.rs` | 키 할당 타입 (이동, 전환 등) |
| `config/src/window.rs` | 창 관련 설정 |
| `config/src/font.rs` | 폰트 설정 |
| `config/src/frontend.rs` | GUI 프론트엔드 설정 |
| `config/src/terminal.rs` | 터미널 기능 설정 |
| `config/src/unix.rs` | Unix 플랫폼 설정 |
| `config/src/windows.rs` | Windows 플랫폼 설정 (있다면) |
| `config/src/wsl.rs` | WSL 도메인 설정 |
| `config/src/ssh.rs` | SSH 관련 설정 |
| `config/src/tls.rs` | TLS 설정 |
| `config/src/color.rs` | 색상 팔레트 |
| `config/src/bell.rs` | 벨(알림) 효과 |
| `config/src/cell.rs` | 셀 렌더링 설정 |
| `config/src/daemon.rs` | 데몬 모드 설정 |
| `config/src/exec_domain.rs` | 실행 도메인 설정 |
| `config/src/serial.rs` | 직렬 포트 설정 |

### 2.7 Lua API 크레이트 (`lua-api-crates/`)

설정 및 런타임에서 접근 가능한 **Lua 확장 기능**들.

| 크레이트 | 역할 |
|----------|------|
| `lua-api-crates/battery/` | 배터리 상태 정보 API |
| `lua-api-crates/color-funcs/` | 색상 변환/팔레트 API, built-in scheme 제공 |
| `lua-api-crates/filesystem/` | 파일 시스템 API (Lua용) |
| `lua-api-crates/logging/` | 로깅 API |
| `lua-api-crates/mux/` | mux 서버 제어 API (탭/창/패너 조작) |
| `lua-api-crates/plugin/` | 플러그인 API |
| `lua-api-crates/procinfo-funcs/` | 프로세스 정보 API |
| `lua-api-crates/serde-funcs/` | 직렬화 유틸리티 |
| `lua-api-crates/share-data/` | Lua-간 데이터 공유 API |
| `lua-api-crates/spawn-funcs/` | 프로세스 스폰 API |
| `lua-api-crates/time-funcs/` | 시간 관련 API |
| `lua-api-crates/url-funcs/` | URL 유틸리티 API |
| `lua-api-crates/window-funcs/` | 창 조작 API |

### 2.8 하위 크레이트 (Library)

| 경로 | 기능 |
|------|------|
| `bidi/` | BIDI(좌우반전 텍스트) 처리 |
| `vtparse/` | VT100/ANSI escape sequence 파서 |
| `wezterm-cell/` | 터미널 셀 구조체 |
| `wezterm-surface/` | Surface (표면) 모델, CellCluster |
| `wezterm-input-types/` | 키보드/마우스 입력 타입 정의 |
| `wezterm-escape-parser/` | Escape sequence 파싱 (tmux_cc 등) |
| `wezterm-dynamic/` | 동적 타입 직렬화 시스템 (Lua ↔ Rust) |
| `wezterm-font/` | 폰트 로딩/매칭 |
| `wezterm-gui-subcommands/` | GUI 서브커맨드 |
| `wezterm-ssh/` | SSH 클라이언트 실행 파일 |
| `wezterm-open-url/` | URL 열기 유틸리티 |
| `wezterm-toast-notification/` | 데스크톱 알림 |
| `wezterm-uds/` | Unix Domain Socket 통신 |
| `wezterm-blob-leases/` |blob(이진 데이터) 임대 관리 (폰트 등) |
| `config/derive/` | 설정 구조체 derive macro |
| `luahelper/` | Rust ↔ Lua 바인딩 헬퍼 |
| `portable-pty/` (`pty/`) | 포트이블 PTY (가상 터미널) |

### 2.9 의존성 레이어 (`deps/`)

| 경로 | 설명 |
|------|------|
| `deps/cairo/` | vendored Cairo 그래픽스 라이브러리 |
| `deps/fontconfig/` | 폰트 구성 시스템 |
| `deps/freetype/` | FreeType 폰트 렌더링 엔진 |
| `deps/harfbuzz/` | HarfBuzz 글자 모양(shape) 엔진 |

### 2.10 도구 및 유틸리티

| 경로 | 설명 |
|------|------|
| `async_ossl/` | 비동기 OpenSSL 래퍼 |
| `base91/` | Base91 인코딩/디코딩 |
| `codec/` | 직렬화 코드크 |
| `color-types/` | 색상 타입 라이브러리 |
| `frecency/` | 피시엔시(frequency+recency) 캐싱 |
| `lfucache/` | LFU(Least Frequently Used) 캐시 |
| `promise/` | Promise 패턴 구현 |
| `rangeset/` | 범위 집합 자료구조 |
| `ratelim/` | 속도 제한(rate limiter) |
| `strip-ansi-escapes/` | ANSI escape stripper |
| `umask/` | 파일 권한 마스크 유틸리티 |

---

## 3. 핵심 기능 목록

### 3.1 터미널 에뮬레이션

| 기능 ID | 기능명 | 설명 | 주요 파일 |
|---------|--------|------|----------|
| T-001 | VT100/ANSI 처리 | vt100 터미널 시뮬레이션 | `term/src/`, `vtparse/` |
| T-002 | 이스케이프シ퀀스 파싱 | ANSI escape sequence 파서/생성기 | `wezterm-escape-parser/`, `termwiz/src/` |
| T-003 | True Color | 24-bit 색상 렌더링 | `config/src/color.rs`, `termwiz/` |
| T-004 | 리가처 | Fira Code 등 프로그래밍 리가처 | `wezterm-font/`, `deps/harfbuzz/` |
| T-005 | BIDI 텍스트 | 히브리어/아랍어 좌우반전 | `bidi/` |
| T-006 | 이모지 렌더링 | 컬러 이모지 폰트 폴백 | `wezterm-font/`, `deps/freetype/` |

### 3.2 그래픽/이미지

| 기능 ID | 기능명 | 설명 | 주요 파일 |
|---------|--------|------|----------|
| G-001 | iTerm2 imgcat | iTerm2 이미지 프로토콜 | `wezterm-gui/src/`, docs/imgcat.md |
| G-002 | Kitty graphics | Kitty 이미지만 프로토콜 | `wezterm-gui/src/` |
| G-003 | Sixel | Sixel 그래픽 (실험적) | `termwiz/` |

### 3.3 멀티플렉싱 및 원격

| 기능 ID | 기능명 | 설명 | 주요 파일 |
|---------|--------|------|----------|
| M-001 | 로컬 mux 서버 | Unix domain socket으로 multiplexer 연결 | `mux/src/lib.rs`, `wezterm-uds/` |
| M-002 | SSH 원격 연결 | SSH를 통해 원격 호스트 터미널 | `mux/src/ssh.rs`, `wezterm-ssh/` |
| M-003 | TLS 원격 연결 | TCP/IP over TLS 연결 | `config/src/tls.rs` |
| M-004 | 도메인 모델 | 로컬/SSH/WSL 등 실행 컨텍스트 추상화 | `mux/src/domain.rs` |
| M-005 | tmux 호환성 | tmux 명령어 지원 | `mux/src/tmux*` |
| M-006 | 세션 관리 | 창, 탭, 패너 계층 구조 | `mux/src/window.rs`, `mux/src/tab.rs`, `mux/src/pane.rs` |
| M-007 | 클라이언트 연결 | CLI와 remote client 연결 | `mux/src/client.rs` |

### 3.4 사용자 인터페이스

| 기능 ID | 기능명 | 설명 | 주요 파일 |
|---------|--------|------|----------|
| U-001 | 윈도우 관리 | 다중 창 지원, 핫키 생성 | `config/src/window.rs`, `mux/src/window.rs` |
| U-002 | 탭 시스템 | 탭 생성/전환/이동/드래그 | `mux/src/tab.rs`, `wezterm-gui/src/tabbar.rs` |
| U-003 | 분할 (Split) | 수평/수직 패너 분할 | `mux/src/tab.rs` |
| U-004 | 키 바인딩 | Lua 기반 커스텀 키맵 | `config/src/keys.rs`, `config/src/keyassignment.rs`, `wezterm-gui/src/inputmap.rs` |
| U-005 | 스크롤백 탐색 | Shift+PageUp/PageDown, 마우스 휠 | `term/src/`, `wezterm-gui/src/scrollbar.rs` |
| U-006 | 텍스트 선택 | xterm 호환 마우스/키보드 선택 | `wezterm-gui/src/selection.rs` |
| U-007 | 검색 모드 | Ctrl+Shift+F 스크롤백 검색 | `term/src/` |
| U-008 | UI 애니메이션 | 색보정 이징, 창 전환 효과 | `wezterm-gui/src/colorease.rs` |

### 3.5 설정 시스템

| 기능 ID | 기능명 | 설명 | 주요 파일 |
|---------|--------|------|----------|
| S-001 | Lua 설정 파일 | `~/.config/wezterm/wezterm.lua` | `config/src/config.rs`, `config/src/lua.rs` |
| S-002 | 핫 리로딩 | 설정 변경 시 즉시 반영 | `config/src/lib.rs` |
| S-003 | 색상 스킴 내장 | 20+ 가지 built-in color scheme | `lua-api-crates/color-funcs/`, `docs/colorschemes/` |
| S-004 | 커스텀 플러그인 | Lua 플러그인 시스템 | `lua-api-crates/plugin/`, `docs/config/plugins.md` |

### 3.6 네트워크 및 연결

| 기능 ID | 기능명 | 설명 | 주요 파일 |
|---------|--------|------|----------|
| N-001 | SSH 클라이언트 | 내장 SSH로 원격 터미널 연결 | `wezterm-ssh/`, `mux/src/ssh.rs` |
| N-002 | SSH agent | SSH agent forward | `mux/src/ssh_agent.rs` |
| N-003 | 직렬 포트 | Arduino/엔베디드 시리얼 통신 | `config/src/serial.rs`, docs/serial.md |
| N-004 | URL 열기 | 외부 브라우저로 URL 열기 | `wezterm-open-url/` |
| N-005 | 데스크톱 알림 | OS 네이티브 토스트 알림 | `wezterm-toast-notification/` |

### 3.7 asciinema (녹화/재생)

| 기능 ID | 기능명 | 설명 | 주요 파일 |
|---------|--------|------|----------|
| A-001 | 레코드 (`record`) | 세션 녹음 (.cast) | `wezterm/src/cli/record.rs` |
| A-002 | 재생 (`replay`) | 녹화된 세션 재생 | `wezterm/src/cli/replay.rs` |

---

## 4. CLI 명령어 매핑

### 진입점: `wezterm` 메인 명령어

```
wezterm [SUBCOMMAND] [OPTIONS]
```

### 서브커맨드 상세

#### 세션 관리

| 명령어 | 파일 | 설명 |
|--------|------|------|
| `wezterm start` | `wezterm/src/cli/start.rs` (docs) | 새 세션 시작, 새 창/탭 생성 |
| `wezterm spawn-command` | `wezterm/src/cli/spawn_command.rs` | 새 프로세스 스폰 |

#### 클라이언트 관리

| 명령어 | 파일 | 설명 |
|--------|------|------|
| `wezterm list-clients` | `wezterm/src/cli/list_clients.rs` | 연결된 클라이언트 목록 |

#### 탭/창 조작

| 명령어 | 파일 | 설명 |
|--------|------|------|
| `wezterm activate-tab <N>` | `wezterm/src/cli/activate_tab.rs` | 탭 활성화 (1-9) |
| `wezterm set-tab-title` | `wezterm/src/cli/set_tab_title.rs` | 탭 제목 설정 |
| `wezterm set-window-title` | `wezterm/src/cli/set_window_title.rs` | 창 제목 설정 |
| `wezterm rename-workspace` | `wezterm/src/cli/rename_workspace.rs` | 워크스페이스 이름 변경 |

#### 패너 조작

| 명령어 | 파일 | 설명 |
|--------|------|------|
| `wezterm activate-pane <direction>` | `wezterm/src/cli/activate_pane.rs` | 패너 이동 (좌/우/상/하) |
| `wezterm activate-pane-direction` | `wezterm/src/cli/activate_pane_direction.rs` | 방향별 패너 활성화 |
| `wezterm get-pane-direction` | `wezterm/src/cli/get_pane_direction.rs` | 방향별 인접 패너 조회 |
| `wezterm adjust-pane-size` | `wezterm/src/cli/adjust_pane_size.rs` | 패너 크기 조절 |
| `wezmove-pane-to-new-tab` | `wezterm/src/cli/move_pane_to_new_tab.rs` | 패너를 새 탭으로 이동 |

#### 분할

| 명령어 | 파일 | 설명 |
|--------|------|------|
| `wezterm split-pane` | `wezterm/src/cli/split_pane.rs` | 현재 패너 분할 |

#### 상태 조회

| 명령어 | 파일 | 설명 |
|--------|------|------|
| `wezterm list` | `wezterm/src/cli/list.rs` | 세션/탭/패너 정보 |
| `wezterm get-text` | `wezterm/src/cli/get_text.rs` | 패너 텍스트 추출 |

#### 기타

| 명령어 | 파일 | 설명 |
|--------|------|------|
| `wezterm kill-pane` | `wezterm/src/cli/kill_pane.rs` | 패너 종료 |
| `wezterm zoom-pane` | `wezterm/src/cli/zoom_pane.rs` | 패너 확대/축소 |
| `wezterm send-text` | `wezterm/src/cli/send_text.rs` | 패너에 텍스트 전송 |
| `wezterm proxy` | `wezterm/src/cli/proxy.rs` | 프록시 시작 |
| `wezterm tls-creds` | `wezterm/src/cli/tls_creds.rs` | TLS 인증서 관리 |

---

## 5. Lua API 매핑

### 설정 시 제공되는 전역 객체 (`wezterm.api`):

#### mux 제어 (lua-api-crates/mux)

```lua
-- 세션/탭/창/패너 탐색 및 조작
wezterm.mux.get_domains()
wezterm.mux.get_panes()
wezterm.mux.get_tabs(domain)
wezterm.mux.get_windows(domain)
wezterm.mux.activate_tab{ domain, tab_id }
wezterm.mux.activate_window{ domain }
wezterm.mux.spawn_tab{ domain, ... }
wezterm.mux.split_horizontal{ pane_id }
wezterm.mux.close_tab{ domain, tab_id }
```

#### 색상 기능 (lua-api-crates/color-funcs)

```lua
wezterm.colors.get_builtin_names()
wezterm.colors.get_palette(name)
-- Hue/saturation/luminance 변환 유틸리티
```

#### 배터리 상태 (lua-api-crates/battery)

```lua
wezterm.batteries.get()
```

#### 파일 시스템 (lua-api-crates/filesystem)

```lua
wezterm.fs.read_file(path)
wezterm.fs.write_file(path, data)
wezterm.fs.list_dir(path)
```

#### 프로세스 정보 (lua-api-crates/procinfo-funcs)

```lua
-- 로컬 시스템 프로세스 정보 조회
```

#### 창 조작 (lua-api-crates/window-funcs)

```lua
wezterm.gui.get_windows()
wezterm.gui.active_window()
-- 창 크기, 위치, 상태 조작
```

#### 플러그인 API (lua-api-crates/plugin)

```lua
-- 플러그인 등록 및 관리
-- LuaHook 시스템: draw, update 이벤트 훅
```

---

## 6. 설정 시스템

### 6.1 설정 파일 위치

```
Unix: ~/.config/wezterm/wezterm.lua
macOS: ~/.config/wezterm/wezterm.lua
Windows: %APPDATA%\wezterm\config.lua
```

### 6.2 설정 모듈 매핑

| 설정 모듈 | 주요 옵션 | 파일 |
|----------|----------|------|
| 창 설정 | 크기, 위치, 레이어, 모서리 둥글기 | `config/src/window.rs` |
| 폰트 설정 | 글꼴 이름, 크기, 리가처 | `config/src/font.rs`, `wezterm-font/` |
| 키 바인딩 | keybinds, keytables | `config/src/keys.rs`, `config/src/keyassignment.rs` |
| 터미널 설정 | 커서 스타일, 벨 효과, scrollback | `config/src/terminal.rs`, `config/src/bell.rs` |
| 렌더링 설정 | 렌더러, 안티앨리어싱 | `config/src/frontend.rs` |
| 셀 설정 | 셀 크기, 여백 | `config/src/cell.rs` |
| 색상 스킴 | foreground, background, palette | `config/src/color.rs` |
| SSH 설정 | 도메인별 SSH 호스트/키/옵션 | `config/src/ssh.rs`, `mux/src/domain.rs` |
| TLS 설정 | 도메인별 TLS 호스트/인증서 | `config/src/tls.rs` |
| WSL 설정 | WSL 도메인 구성 | `config/src/wsl.rs` |
| 데몬 설정 | 백그라운드 실행 옵션 | `config/src/daemon.rs` |
| 직렬 포트 | 시리얼 포트 구성 | `config/src/serial.rs` |

### 6.3 내장 색상 스킴

`lua-api-crates/color-funcs/src/schemes/` 에 20+ 가지 color scheme 내장:
- base16, gogh, iterm2, sexy 등

### 6.4 설정 파이프라인

```
wezterm.lua (Lua 코드)
    ↓
LuaConfigState (config/src/lib.rs)
    ↓
ToDynamic → serde_json / toml
    ↓
Configuration (config/src/config.rs) - 타입 검증
    ↓
MuxServer - 실시간 적용
```
