# wezterm 폴더 기능 명세

본 문서는 `E:\Project\wezterm\wezterm` 폴더에 포함된 단일 크레이트 `wezterm`의 상세 기능 명세다. 이 저장소는 WezTerm의 Windows 전용 영구 분기이며, 비Windows 플랫폼 코드는 제거되었거나 죽은 경로다. 모든 코드 인용은 `파일경로:라인` 형식으로 표기한다.

---

## 1. 개요 및 책임

`wezterm` 크레이트는 워크스페이스의 **CLI 진입점 실행 파일**(`wezterm.exe`)을 생성한다. 단일 책임은 다음과 같다.

- 명령줄 인자를 파싱해 서브커맨드를 분기한다(`clap` 기반).
- GUI를 띄워야 하는 서브커맨드(`start`, `ssh`, `serial`, `connect`, `ls-fonts`, `show-keys`)는 **자기 자신이 처리하지 않고** 같은 디렉터리의 `wezterm-gui.exe`로 위임한다(`wezterm/src/main.rs:763-787`).
- GUI가 필요 없는 순수 CLI 동작은 자신이 직접 수행한다: 멀티플렉서(mux) 서버와의 RPC 상호작용(`cli` 서브커맨드 군), 이미지 출력(`imgcat`), OSC 7 작업 디렉터리 통지(`set-working-directory`), asciicast 녹화/재생(`record`/`replay`), 셸 자동완성 생성(`shell-completion`).

즉, 이 크레이트는 두 가지 역할을 겸한다.

1. **디스패처(launcher)**: 인자를 검사해 GUI 바이너리로 프로세스를 위임하는 얇은 셸.
2. **헤드리스 CLI 도구**: 터미널/이미지/녹화 유틸리티와, 실행 중인 wezterm mux 서버를 원격 제어하는 클라이언트.

Windows fork에서의 실제 책임은 본질적으로 동일하다. 단, GUI 위임 대상이 `wezterm-gui.exe`로 고정되어 있고(`wezterm/src/main.rs:769`), 위임 시 `--attach-parent-console` 인자를 강제로 전달해 콘솔 핸들을 GUI 프로세스에 연결한다(`wezterm/src/main.rs:777`). 이는 Windows에서 콘솔 서브시스템과 GUI 서브시스템이 분리되어 있는 특성에 대한 대응이다.

---

## 2. 워크스페이스 내 위치

### 의존성(deps)

| 크레이트 | 사용 목적 |
| --- | --- |
| codec | mux 서버와 주고받는 RPC 메시지(`Pdu` 및 각종 요청/응답 구조체) 직렬화 |
| config | 설정 로딩(`common_init`), `ConfigHandle`, 키 할당 타입(`SpawnTabDomain`, `PaneDirection`) |
| env-bootstrap | 프로세스 기동 시 환경 부트스트랩(`bootstrap()`) |
| filedescriptor | Windows 콘솔 핸들(`CONIN$`/`CONOUT$`)을 `FileDescriptor`로 감싸 다루기 |
| mux | 멀티플렉서 코어 타입(`Mux`, `WindowId`, `TabId`, `PaneId`, `Activity`, `connui`) |
| portable-pty | asciicast 녹화 시 PTY 생성(`native_pty_system`, `PtySize`, `CommandBuilder`) |
| promise | CLI 비동기 실행을 위한 `ScopedExecutor`/`block_on` |
| tabout | CLI `list`/`list-clients`의 테이블 출력 정렬 |
| termwiz | 이스케이프 시퀀스 파싱·생성, 터미널 capability, 입력 이벤트, 표면(surface) 렌더링 |
| termwiz-funcs | `lines_to_escapes`(셀 라인을 이스케이프 포함 문자열로 변환) |
| umask | `UmaskSaver`로 umask 저장/복원 |
| wezterm-client | mux 서버 접속 클라이언트(`Client`), 소켓 재시도 연결(`unix_connect_with_retry`) |
| wezterm-gui-subcommands | GUI로 위임되는 서브커맨드 정의(`StartCommand` 등) 및 공용 헬퍼(`name_equals_value`, `DEFAULT_WINDOW_CLASS`) |
| wezterm-term | 터미널 셀/색상 타입(`ColorPalette`, `TerminalSize`, `StableRowIndex`) |

부가 외부 의존(직접): `anyhow`, `chrono`, `clap`/`clap_complete`/`clap_complete_fig`, `hostname`, `humantime`, `image`, `log`, `serde`/`serde_json`, `shell-words`, `smol`, `tempfile`, `url`, `winapi`(Windows 한정).

### 피의존(usedBy)

| 크레이트 | 비고 |
| --- | --- |
| (없음) | 최상위 바이너리 크레이트. 다른 크레이트가 라이브러리로 참조하지 않는다. |

### 계층상의 위치

이 크레이트는 의존성 그래프의 **최상위(루트 바이너리)**에 위치한다. 아래로는 mux/codec/client/termwiz 계층에 의존하지만, 자신을 의존하는 크레이트는 없다. 파이프라인 관점에서는 "사용자 명령줄 → `wezterm.exe`(이 크레이트) → (GUI 위임) `wezterm-gui.exe` 또는 (직접) mux 서버 RPC"의 첫 단계에 해당한다.

---

## 3. 공개 API 표면

이 크레이트는 바이너리이므로 외부에 라이브러리 API를 노출하지 않는다. `pub` 항목 대부분은 `clap` 파생 매크로가 요구하는 가시성 때문에 존재하며, 실질적 진입점은 `main()` 하나다.

크레이트 경계 안에서 의미 있는 공개 표면은 다음과 같다.

- `fn main()` — 프로세스 진입점. `config` 메인 스레드 지정, 에러 콜백 등록, `run()` 호출, `Mux::shutdown()`(`wezterm/src/main.rs:704-711`).
- `pub struct Opt` — 최상위 인자 구조체. `--skip-config`, `--config-file`, `--config`(반복 가능 오버라이드), 서브커맨드(`wezterm/src/main.rs:29-54`).
- `pub(crate) struct ImageInfo { pub width: u32, pub height: u32, pub format: image::ImageFormat }` — `imgcat`가 사용하는 이미지 메타(`wezterm/src/main.rs:308-313`).
- `cli::run_cli(opts: &Opt, cli: CliCommand) -> anyhow::Result<()>` — `cli` 서브커맨드 동기 진입점. 내부에서 `promise` 스코프드 익스큐터로 비동기 본체를 블로킹 실행(`wezterm/src/cli/mod.rs:205-211`).
- `cli::resolve_relative_cwd(cwd: Option<OsString>) -> anyhow::Result<Option<String>>` — 상대 cwd를 현재 디렉터리 기준 절대 경로 문자열로 정규화. `spawn`/`split-pane`이 공유(`wezterm/src/cli/mod.rs:213-224`).
- `pub struct CliCommand` / `pub struct CliCommand.sub: CliSubCommand` — `cli` 서브커맨드 트리. `--no-auto-start`, `--prefer-mux`, `--class`(`wezterm/src/cli/mod.rs:53-74`).
- `asciicast::{Header, Theme, Event, RecordCommand, PlayCommand}` — asciicast v2 데이터 모델과 녹화/재생 명령(`wezterm/src/asciicast.rs:22-130, 253-559`).
- 각 `cli::*` 서브모듈의 `pub struct <Command>` 와 `pub async fn run(...)` — 개별 CLI 명령 구현. 시그니처 패턴은 두 가지다.
  - 대부분: `async fn run(self|&self, client: Client) -> anyhow::Result<()>`
  - 설정 필요 명령: `spawn`은 `run(self, client, &ConfigHandle)`, `proxy`는 `run(&self, client, &ConfigHandle)`(`wezterm/src/cli/mod.rs:189-190`).

`clap` `CompletionGenerator` 트레이트 구현(`CompletionShell`)은 `bash/elvish/fish/powershell/zsh/fig` 6종 셸 자동완성을 위임 생성한다(`wezterm/src/main.rs:66-88`).

---

## 4. 내부 구조

모듈 분해는 다음과 같다.

```
src/
  main.rs       진입점·인자 파싱·서브커맨드 분기·GUI 위임·imgcat·set-cwd·tmux passthru
  asciicast.rs  asciicast v2 녹화(record)/재생(replay), Windows 콘솔 TTY 래퍼
  cli/
    mod.rs                  cli 서브커맨드 트리 정의, 비동기 실행 디스패처, 공용 헬퍼
    <command>.rs (18개)      개별 mux RPC 명령 구현
```

### 제어 흐름

1. `main()` → `run()`: `env_bootstrap::bootstrap()` 호출, `UmaskSaver` 생성, `Opt::parse()`(`wezterm/src/main.rs:728-733`).
2. 서브커맨드가 없으면 `SubCommand::Start(StartCommand::default())`로 기본 분기(`wezterm/src/main.rs:739`).
3. 분기(`wezterm/src/main.rs:741-760`):
   - `Start/BlockingStart/LsFonts/ShowKeys/Ssh/Serial/Connect` → `delegate_to_gui(saver)` (GUI 바이너리로 프로세스 위임).
   - `ImageCat/SetCwd` → 직접 `cmd.run()`.
   - `Cli` → `cli::run_cli`.
   - `Record` → `init_config` 후 `cmd.run(config)`; `Replay` → `cmd.run()`.
   - `ShellCompletion` → `clap_complete::generate`.

### `cli` 비동기 흐름

`run_cli`(`mod.rs:205`)는 `promise::spawn::ScopedExecutor`를 만들고 `run_cli_async`를 블로킹 실행한다. `run_cli_async`(`mod.rs:168`)는 헤드리스 `ConnectionUI`로 `Client::new_default_unix_domain`을 생성한 뒤(`mod.rs:172-180`), 18개 서브커맨드를 `match`로 디스패치한다(`mod.rs:182-202`). 클라이언트 생성 시 `--class`가 미지정이면 `DEFAULT_WINDOW_CLASS`를 사용한다.

### 데이터 흐름 패턴(CLI 명령)

다수 명령(`activate-tab`, `set-tab-title`, `set-window-title`, `rename-workspace`, `zoom-pane`, `spawn`, `move-pane-to-new-tab`)이 동일 패턴을 반복한다: `client.list_panes()`로 전체 패널 트리를 받아 `into_tree().cursor()`로 preorder 순회하며 `pane_id → tab_id/window_id/workspace` 매핑을 `HashMap`에 채운 뒤, `WEZTERM_PANE` 기반 현재 패널을 기준으로 대상 ID를 해소한다. 이 순회 코드는 6개 파일에 거의 동일하게 중복되어 있다.

### 비대 모듈

- `main.rs`(787행)가 가장 크다. 대부분은 `imgcat`(`ImgCatCommand`, 약 290행: 인자 정의·이미지 리사이즈/리샘플·셀 치수 계산·OSC 1337 출력)와 `set-working-directory`/`TmuxPassthru` 로직이다. 진입점 본체(`run`/`delegate_to_gui`/`init_config`)는 작다.
- `asciicast.rs`(587행)는 헤더/이벤트 직렬화, Windows 콘솔 TTY 래퍼(`win::WinTty`), 녹화 루프(스레드 3개 + mpsc 채널), 재생/설명 로직을 모두 포함한다.

---

## 5. 핵심 데이터 구조·타입

- `Opt`(`main.rs:29`): 최상위 인자. `config_override: Vec<(String, String)>`는 `name=value` 파서로 검증되며 `skip_config`와 상호 배타(`conflicts_with`).
- `SubCommand`(`main.rs:90`)/`CliSubCommand`(`mod.rs:77`): `clap` 파생 enum. 각 variant가 하나의 서브커맨드. `BlockingStart`는 `-e` 단축 별칭 처리를 위한 숨김 항목(clap 이슈 #1335 회피, `main.rs:98-102`).
- `ImgCatCommand`(`main.rs:151`): 너비/높이(`ITermDimension`), 종횡비 보존, 커서 위치, 최대 픽셀 수(`max_pixels`, 기본 25,000,000), 리샘플 필터/포맷 등. 불변식: `--resize`와 `--percent`류 셀 지정이 독립적으로 적용되며, `compute_image_cell_dimensions`는 종횡비를 유지하면서 화면 픽셀 한계 내로 후보를 산출해 면적 최대 후보를 선택한다(`main.rs:340-380`).
- `TmuxPassthru`(`main.rs:656`): `Disable/Enable/Detect(기본)`. `Detect`는 `TMUX` 환경변수 존재로 판정. `encode`는 tmux passthru 시 이스케이프(`\x1b`)를 이중화하고 `\x1bPtmux;...\x1b\\`로 감싼다(`main.rs:677-692`).
- `asciicast::Header`(`asciicast.rs:23`): asciicast v2 헤더. `version >= 2` 불변식, 색상 테마(`Theme`)는 resolved 팔레트의 첫 16색을 `:`로 결합. `serde`의 `skip_serializing_if`로 선택 필드 생략.
- `asciicast::Event(pub f32, pub String, pub String)`(`asciicast.rs:123`): `(경과초, 타입, 데이터)` 튜플. 타입 `"o"`만 출력 이벤트로 간주(`asciicast.rs:451, 466, 513`).
- `asciicast::win::WinTty`(`asciicast.rs:141`): 저장된 콘솔 모드(`saved_input/saved_output`)와 코드페이지(`saved_cp`)를 보관. **불변식**: `Drop` 시 반드시 `set_cooked()`로 원래 콘솔 상태 복원(`asciicast.rs:236-240`). 생성 시 출력 코드페이지를 `CP_UTF8`로 강제(`asciicast.rs:163`).
- `asciicast::Message`(`asciicast.rs:244`): 녹화/재생 스레드 간 채널 메시지. `Stdin/Stdout/Terminated`.
- `CliListResultItem`/`CliListResultPtySize`(`list.rs:102-142`), `CliListClientsResultItem`(`list_clients.rs:121`): **JSON 출력 안정성 계약**이 명시된 직렬화 구조체. 주석상 필드/타입 변경이 곧 출력 포맷 변경이므로 변경에 주의해야 한다.

UTF-8 경계 처리 불변식(녹화): 자식 출력은 버퍼 경계에서 불완전 UTF-8 시퀀스가 걸칠 수 있으므로, `str::from_utf8` 검증으로 유효 구간만 `.cast`에 기록하고 잔여를 버퍼에 남긴다(`asciicast.rs:380-399`).

---

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
| --- | --- |
| clap / clap_complete / clap_complete_fig | 인자 파싱 및 6종 셸 자동완성 생성 |
| image | `imgcat`의 이미지 디코딩·치수 추정·리사이즈/리샘플·인코딩 |
| termwiz | 이스케이프 시퀀스(OSC 1337 iTerm 이미지, OSC 7 cwd, CSI 커서) 생성·파싱, 터미널 raw/cooked 제어, 입력 폴링 |
| termwiz-funcs | `get-text --escapes`에서 셀 라인을 색상/스타일 이스케이프 포함 문자열로 환원 |
| portable-pty | asciicast 녹화 시 PTY 생성 및 자식 프로세스 spawn |
| smol | `proxy`의 영구 대기(`future::pending`) 및 비동기 IO |
| promise | CLI 비동기 본체를 동기 컨텍스트에서 블로킹 실행 |
| chrono / humantime | `list-clients`의 접속/유휴 시간 계산 및 사람 친화적 표기 |
| tabout | CLI 테이블 출력 컬럼 정렬 |
| serde / serde_json | asciicast 직렬화, CLI `--format json` 출력 |
| shell-words | asciicast 헤더의 `command` 필드를 셸 인용 규칙으로 결합 |
| url | `set-working-directory`의 `file://` URL 구성 |
| hostname | OSC 7 URL의 호스트명 기본값 획득 |
| tempfile | 녹화 파일명 미지정 시 임시 파일 생성 |
| winapi | Windows 콘솔 모드/코드페이지/스크린버퍼 API(아래 8장) |

---

## 7. 설정·기능 플래그

- **Cargo feature**: 이 크레이트는 자체 feature flag를 정의하지 않는다(`Cargo.toml`에 `[features]` 없음).
- **타깃 조건부 의존**: `winapi`는 `cfg(windows)`에서만 활성(`Cargo.toml:41-52`), 빌드 의존 `cc`/`embed-resource`도 `cfg(windows)` 한정(`Cargo.toml:54-56`).
- **런타임 설정 연동**(`config` 크레이트 경유):
  - `init_config`(`main.rs:713-726`)가 `config::common_init`으로 설정을 로드한 뒤 `update_ulimit()`을 호출하고, `default_ssh_auth_sock`가 있으면 `SSH_AUTH_SOCK` 환경변수를 설정한다.
  - asciicast 녹화는 `config.term`, `config.default_prog`, `config.default_cwd`, `config.resolved_palette`를 참조해 헤더/실행 프로그램을 구성한다(`asciicast.rs:59-97, 304-312`).
  - `spawn`은 `config.default_workspace`(없으면 `mux::DEFAULT_WORKSPACE`)와 `config.initial_size(...)`를 사용(`spawn_command.rs:86-97`).
- **CLI 옵션 플래그**(주요): `--no-auto-start`, `--prefer-mux`, `--class`(`mod.rs:55-70`); `imgcat`의 `--width/--height/--max-pixels/--resample-*` 등; `--tmux-passthru` 3상태.
- 환경변수: `WEZTERM_PANE`(CLI 명령의 기본 대상 패널 결정), `TMUX`(tmux passthru 감지), `SHELL`/`LANG`(녹화 헤더에 캡처).

---

## 8. Windows 전용 고려사항

- **GUI 위임 대상 고정**: `delegate_to_gui`의 실행 파일명이 `"wezterm-gui.exe"`로 하드코딩(`main.rs:769`). 현재 실행 파일의 부모 디렉터리에서 형제 바이너리를 찾는다. 위임 시 `--attach-parent-console`를 항상 부여(`main.rs:777`)하고, 자식 종료 코드를 그대로 전파하며 프로세스를 종료한다(`main.rs:782-785`).
- **Windows 콘솔 TTY**(`asciicast.rs:132-241`): `win` 모듈이 `winapi`로 콘솔을 직접 제어한다.
  - `CONIN$`/`CONOUT$`를 읽기/쓰기로 열어 핸들 확보(`asciicast.rs:151-154`).
  - `GetConsoleMode`/`SetConsoleMode`로 입력·출력 모드 저장/설정, `GetConsoleOutputCP`/`SetConsoleOutputCP`로 코드페이지를 `CP_UTF8`로 전환(`asciicast.rs:159-164`).
  - raw 모드: 입력에 `ENABLE_VIRTUAL_TERMINAL_INPUT`, 출력에 `ENABLE_PROCESSED_OUTPUT | ENABLE_WRAP_AT_EOL_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN`(`asciicast.rs:185-196`).
  - 화면 크기: `GetConsoleScreenBufferInfo`의 윈도우 영역으로 cols/rows 계산, 픽셀 치수는 0으로 보고(`asciicast.rs:201-225`).
  - 이 `WinTty`가 `use win::WinTty as Tty`로 **무조건 사용**된다(`asciicast.rs:17`). 비Windows 대체 구현은 존재하지 않으므로, 이 크레이트는 Windows에서만 컴파일된다.
- **conpty 가정**: `imgcat`에서 `let is_conpty = true;`로 고정(`main.rs:515`). Windows는 항상 conpty 경유라는 전제하에, 픽셀 기하가 알려진 경우 이미지 출력 후 커서를 강제 이동한다.
- **리소스 매니페스트 임베드**: `build.rs`가 `cfg(windows)`에서 `assets/windows/console.manifest`를 `RT_MANIFEST`로 임베드하는 `.rc`를 생성하고, MSVC 환경(`cl.exe`)을 찾아 환경변수를 설정한 뒤 `embed_resource::compile`로 컴파일한다(`build.rs:4-37`). 빌드에 MSVC 툴체인 탐지가 필수.
- **죽은 코드/비Windows 분기**: 이 크레이트의 `src/` 내부에는 `cfg(unix)` 등 비Windows 조건부 코드가 없다(검색 결과 0건). asciicast의 `win` 모듈만 `cfg` 없이 Windows 전용 API를 직접 호출한다. 단, `proxy`/`tls-creds` 등 명령은 "unix domain socket" 용어를 사용하나(`proxy.rs:23-31`, `wezterm_client`의 `unix_connect_with_retry`), 이는 Windows named pipe를 추상화한 동일 명칭의 도메인 개념이다.

---

## 9. 리팩토링 주의점

- **패널 트리 순회 중복**: `list_panes()` 후 `into_tree().cursor()` preorder 순회로 `HashMap`을 채우는 패턴이 `activate_tab.rs`, `set_tab_title.rs`, `set_window_title.rs`, `rename_workspace.rs`, `zoom_pane.rs`, `spawn_command.rs`, `move_pane_to_new_tab.rs`, `list.rs`에 사실상 복제되어 있다. 공용 헬퍼로 추출 가능하나, 각 명령이 채우는 매핑 키가 다르므로 일반화 시 클로저/제네릭 설계가 필요하다.
- **JSON 출력 안정성 계약**: `CliListResultItem`(`list.rs:118`)·`CliListResultItem`의 `CliListResultPtySize`·`CliListClientsResultItem`(`list_clients.rs:121`)은 외부 도구가 파싱하는 안정 포맷이다. 필드명/타입 변경은 하위 호환을 깬다(코드 주석에 명시됨). 리팩토링으로 내부 mux 타입과 직접 결합시키지 말 것.
- **GUI 위임 결합**: 디스패처는 `wezterm-gui.exe`가 같은 디렉터리에 존재한다는 배치 전제에 강하게 결합되어 있다(`main.rs:769-774`). 패키징 구조 변경 시 위임 실패 위험.
- **activate-tab 로직 이중 유지**: `activate_tab.rs`의 인덱스/상대 이동 계산은 GUI 측 `TermWindow::activate_tab`/`activate_tab_relative`와 동일 로직을 의도적으로 복제하며, 주석으로 "한쪽 수정 시 다른 쪽도 수정"을 명시한다(`activate_tab.rs:101-114`). 동기화 누락 시 CLI와 GUI 동작 불일치 발생.
- **콘솔 상태 복원 불변식**: `WinTty::Drop`이 콘솔 모드/코드페이지를 복원한다. raw 모드 진입 후 패닉이나 조기 반환이 일어나도 `Drop`이 보장하지만, `set_raw`/`set_cooked`를 직접 호출하는 경로(녹화/재생)에서 짝이 어긋나면 콘솔이 raw 상태로 남는다.
- **`config::designate_this_as_the_main_thread()` 호출 시점**: `main()` 최초 라인(`main.rs:705`). 이 호출 전 다른 스레드에서 config 접근 시 패닉 위험. 진입점 재배치 시 주의.
- **순환 의존**: 라이브러리로서 피의존이 없어 순환 의존은 없다. 다만 `cli` 명령들이 `codec`/`mux`/`wezterm_client`의 구체 타입에 직접 의존하므로, 그 계층의 시그니처 변경이 다수 CLI 파일로 직접 파급된다.
- **`summarize`의 `Summarized::Action` 미사용 필드**: `asciicast.rs:561`에 `#[allow(dead_code)]`가 달려 있어, 데이터 보존 목적의 죽은 변형이 존재한다.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `Cargo.toml` | 57 | 크레이트 매니페스트. 의존성, Windows 한정 `winapi`/빌드 의존 정의 |
| `build.rs` | 38 | Windows 콘솔 매니페스트(`RT_MANIFEST`) 임베드, MSVC 환경 탐지 |
| `src/main.rs` | 787 | 진입점·인자 파싱·서브커맨드 분기·GUI 위임·`imgcat`·`set-working-directory`·tmux passthru·셸 자동완성 |
| `src/asciicast.rs` | 587 | asciicast v2 녹화(`record`)/재생(`replay`), Windows 콘솔 TTY 래퍼(`win::WinTty`), UTF-8 경계 처리 |
| `src/cli/mod.rs` | 224 | `cli` 서브커맨드 트리, 비동기 디스패처(`run_cli`/`run_cli_async`), `resolve_relative_cwd` 헬퍼 |
| `src/cli/list.rs` | 202 | `cli list`: 패널/탭/윈도우 목록을 테이블 또는 안정 JSON으로 출력 |
| `src/cli/list_clients.rs` | 170 | `cli list-clients`: 접속 클라이언트 목록, 접속/유휴 시간 표기 |
| `src/cli/activate_tab.rs` | 159 | `cli activate-tab`: 탭 ID/인덱스/상대 오프셋(랩 옵션)으로 탭 활성화 |
| `src/cli/spawn_command.rs` | 123 | `cli spawn`: 새 탭/윈도우에 명령 spawn, 패널 ID 출력 |
| `src/cli/split_pane.rs` | 113 | `cli split-pane`: 방향/크기 지정 분할, 패널 이동 지원 |
| `src/cli/get_text.rs` | 94 | `cli get-text`: 패널 텍스트(선택적 이스케이프 포함) 추출 |
| `src/cli/zoom_pane.rs` | 88 | `cli zoom-pane`: 줌/언줌/토글 |
| `src/cli/proxy.rs` | 85 | `cli proxy`: stdin/stdout과 mux 소켓을 잇는 netcat 유사 RPC 파이프 |
| `src/cli/move_pane_to_new_tab.rs` | 75 | `cli move-pane-to-new-tab`: 패널을 새 탭(선택적 새 윈도우)으로 이동 |
| `src/cli/set_window_title.rs` | 64 | `cli set-window-title`: 윈도우 제목 변경 |
| `src/cli/rename_workspace.rs` | 64 | `cli rename-workspace`: 워크스페이스 이름 변경 |
| `src/cli/set_tab_title.rs` | 63 | `cli set-tab-title`: 탭 제목 변경 |
| `src/cli/activate_pane_direction.rs` | 59 | `cli activate-pane-direction`: 방향 인접 패널 활성화 + `PaneDirectionParser` |
| `src/cli/send_text.rs` | 52 | `cli send-text`: 텍스트를 붙여넣기/직접 전송 |
| `src/cli/adjust_pane_size.rs` | 38 | `cli adjust-pane-size`: 방향별 패널 크기 조정 |
| `src/cli/get_pane_direction.rs` | 34 | `cli get-pane-direction`: 지정 방향 인접 패널 ID 조회 |
| `src/cli/tls_creds.rs` | 31 | `cli tlscreds`: mux 서버 TLS 자격증명(PEM 또는 PDU) 출력 |
| `src/cli/activate_pane.rs` | 22 | `cli activate-pane`: 패널 포커스 |
| `src/cli/kill_pane.rs` | 20 | `cli kill-pane`: 패널 종료 |

> 비고: 이 크레이트에는 생성(generated) 데이터 테이블 파일이 없다.
