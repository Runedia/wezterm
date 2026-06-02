# mux 폴더 기능 명세

본 문서는 `E:\Project\wezterm\mux` 폴더(크레이트 `mux`)에 대한 상세 기능 명세다. 코드 인용은 `파일경로:라인` 형식으로 표기한다. 본 저장소는 WezTerm의 Windows 전용 영구 분기이며, upstream 동기화를 포기했고 비Windows 플랫폼 코드는 제거된 상태다.

---

## 1. 개요 및 책임

`mux`(multiplexer) 크레이트는 WezTerm의 **세션 상태 모델 핵심**이다. 단일 책임은 다음과 같이 요약된다.

- 터미널 세션의 논리 계층 — **윈도우(Window) → 탭(Tab) → 페인(Pane)** — 을 정의하고, 이들의 생성·소멸·활성화·크기 조정·분할(split)을 관리한다.
- 페인을 제공하는 **도메인(Domain)** 추상을 정의한다. 도메인은 로컬 ConPTY, WSL, SSH 원격, tmux 제어 모드, termwiz 내장 UI 등 페인의 실제 출처를 캡슐화한다.
- 전역 단일 인스턴스 `Mux`(lib.rs:100)를 통해 모든 페인/탭/윈도우/도메인/클라이언트 상태를 보관하고, 상태 변화를 `MuxNotification`(lib.rs:53) 이벤트로 구독자(GUI, mux 서버 등)에게 방송한다.
- PTY에서 바이트를 읽어 termwiz 이스케이프 파서로 `Action`을 만들고, 이를 페인의 터미널 모델에 적용하는 **출력 파이프라인**을 운영한다(lib.rs:138 `parse_buffered_data`, lib.rs:277 `read_from_pane_pty`).

`mux`는 렌더링·윈도잉(`wezterm-gui`)이나 직렬화 프로토콜(`codec`)을 알지 못한다. 순수하게 "터미널 세션이 무엇으로 구성되며 어떻게 변하는가"만 모델링한다. 화면 묘사는 `Pane` 트레이트(pane.rs:166)가 추상화하고, 실제 셀 데이터는 `wezterm-term`의 `Terminal`이 보유한다.

Windows fork에서의 실제 책임:
- 기본 도메인 `LocalDomain`(domain.rs:202)은 `native_pty_system()` → ConPTY를 사용한다. `is_conpty()`(domain.rs:256)가 참이면 `terminal.enable_conpty_quirks()`(domain.rs:604)를 켠다.
- WSL 도메인은 별도 PTY 백엔드가 아니라 `wsl.exe --distribution ... --exec ...` 명령으로 **재작성**되어 LocalDomain 위에서 실행된다(domain.rs:264~306, `fixup_command`).
- SSH 에이전트 프록시는 Windows 심볼릭 링크(`std::os::windows::fs::symlink_file`, ssh_agent.rs:5)로 구현된다.

---

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 사용 이유 |
|---|---|
| `bintree` | 탭 내부 페인 레이아웃을 이진 트리(`Tree`, tab.rs:17)로 표현 |
| `config` | 전역 설정, `WslDomain`/`ExecDomain`/`SerialDomain`/`SshDomain`, `ExitBehavior`, Lua 콜백 호출 |
| `filedescriptor` | `socketpair`/`poll`, `AsRawSocketDescriptor` (출력 파이프라인·SSH·tmux pty) |
| `luahelper` | Lua `Value` ↔ Rust 변환(`from_lua_value_dynamic`) |
| `portable-pty` | PTY 시스템 추상(`PtySystem`/`MasterPty`/`Child`), ConPTY·serial 백엔드 |
| `procinfo` | 전경 프로세스/CWD 탐지(`LocalProcessInfo`) |
| `promise` | `spawn_into_main_thread` 비동기 메인 스레드 디스패치 |
| `rangeset` | 변경 라인 집합(`RangeSet<StableRowIndex>`) |
| `termwiz` | 이스케이프 파서, `Action`/`CSI`, lineedit, terminfo 렌더러, tmux_cc 파서 |
| `termwiz-funcs` | termwiz 보조 함수 |
| `wezterm-dynamic` | `Value`, `FromDynamic`/`ToDynamic` 직렬화 |
| `wezterm-ssh` | SSH 세션(`Session`/`SshPty`/`SshChildProcess`) |
| `wezterm-term` | 터미널 셀 모델(`Terminal`/`Line`/`Screen`), 클립보드·다운로드·알림 핸들러 |

추가 직접 의존(Cargo.toml): `anyhow`, `async-trait`(객체 안전 비동기 트레이트), `chrono`(ClientInfo 타임스탬프), `crossbeam`(connui 채널), `downcast-rs`(트레이트 객체 다운캐스트), `fancy-regex`(검색), `finl_unicode`(grapheme), `hostname`/`names`(ClientId·워크스페이스 이름), `metrics`(히스토그램), `mlua`(Lua 값), `parking_lot`(락), `percent-encoding`(CWD URL), `serde`/`thiserror`/`url`/`shell-words`/`textwrap`/`terminfo`/`smol`/`lazy_static`/`log`/`libc`. Windows 전용: `ntapi`, `winapi`(handleapi/memoryapi/psapi/processthreadsapi/tlhelp32).

### 피의존(usedBy)

| 크레이트 | 사용 형태 |
|---|---|
| `codec` | `PaneNode`/`PaneEntry`/`SerdeUrl`/`StableCursorPosition` 등 직렬화 타입 공유 (버전 호환 주의 대상) |
| `mux-lua` | `Mux`/`Pane`/`Tab` 등을 Lua API로 노출 |
| `wezterm` | CLI가 mux 상태를 조회·조작 |
| `wezterm-client` | 원격 mux 서버에 붙는 클라이언트 측 도메인이 `Domain`/`Pane` 구현 |
| `wezterm-gui` | `MuxNotification` 구독, 페인 렌더, 입력 라우팅 |
| `wezterm-mux-server`, `wezterm-mux-server-impl` | 서버가 `Mux`를 호스팅 |

**계층상 위치**: `mux`는 터미널 모델 계층(`wezterm-term`)과 표현/네트워크 계층(`wezterm-gui`/`codec`/서버) 사이의 **상태 허브**다. 모든 프런트엔드는 `Mux::get()`(lib.rs:752) 전역을 통해 동일한 세션 상태를 공유한다.

---

## 3. 공개 API 표면

### 전역 진입점 — `Mux` (lib.rs:100)
프로세스 전역 단일 인스턴스. `lazy_static MUX: Mutex<Option<Arc<Mux>>>`(lib.rs:364)에 저장되며 `set_mux`/`get`/`try_get`/`shutdown`(lib.rs:744~750)으로 접근한다.

주요 메서드 군:
- 등록/조회: `add_pane`/`add_tab_and_active_pane`/`add_tab_no_panes`/`add_tab_to_window`/`new_empty_window`, `get_pane`/`get_tab`/`get_window`/`get_window_mut`/`get_active_tab_for_window`, `iter_panes`/`iter_windows`/`iter_domains`/`iter_clients`/`iter_workspaces`.
- 제거/정리: `remove_pane`/`remove_tab`/`kill_window`/`prune_dead_windows`(lib.rs:891)/`domain_was_detached`.
- 도메인: `add_domain`/`get_domain`/`get_domain_by_name`/`default_domain`/`set_default_domain`/`resolve_spawn_tab_domain`(lib.rs:1109).
- 스폰/분할(async): `spawn_tab_or_window`(lib.rs:1303), `split_pane`(lib.rs:1181), `move_pane_to_new_tab`(lib.rs:1242).
- 포커스/식별: `record_focus_for_client`/`resolve_focused_pane`/`focus_pane_and_containing_tab`/`with_identity`(lib.rs:667)/`active_identity`.
- 워크스페이스: `active_workspace`/`set_active_workspace`/`rename_workspace`/`generate_workspace_name`/`is_workspace_empty`.
- 구독: `subscribe<F: Fn(MuxNotification)->bool>`(lib.rs:686), `notify`/`notify_from_any_thread`(lib.rs:701).
- 해석: `resolve_pane_id`(lib.rs:1065) → `(DomainId, WindowId, TabId)`, `window_containing_tab`.

연관 타입: `MuxNotification`(lib.rs:53, 22개 변형의 이벤트 열거형), `MuxWindowBuilder`(lib.rs:368, Drop 시 `WindowCreated` 통지), `IdentityHolder`(lib.rs:1398, Drop 시 식별자 복원).

### `Pane` 트레이트 (pane.rs:166)
`#[async_trait(?Send)]` + `Downcast`. 페인의 화면·입력·수명·메타데이터를 추상화하는 핵심 인터페이스. 필수: `pane_id`/`get_cursor_position`/`get_current_seqno`/`get_changed_since`/`get_lines`/`with_lines_mut`/`get_dimensions`/`get_title`/`send_paste`/`reader`/`writer`/`resize`/`key_down`/`key_up`/`mouse_event`/`is_dead`/`palette`/`domain_id`/`is_mouse_grabbed`/`is_alt_screen_active`/`get_current_working_dir`. 기본 구현 제공: `search`(async, 기본 빈 결과)/`get_semantic_zones`/`perform_actions`/`kill`/`erase_scrollback`/`can_close_without_prompting`/`exit_behavior` 등. 보조 함수 `impl_get_lines_via_with_lines`/`impl_get_logical_lines_via_get_lines`/`impl_with_lines_via_get_lines`(pane.rs:389~533)로 구현 부담을 줄인다.

부속 타입: `PaneId=usize`(pane.rs:25), `alloc_pane_id`, `SearchResult`/`Pattern`/`PatternType`, `LogicalLine`(pane.rs:118, 물리/논리 줄 좌표 변환), `CachePolicy`(pane.rs:342), `CloseReason`, `PerformAssignmentResult`, 콜백 트레이트 `WithPaneLines`/`ForEachPaneLogicalLine`.

### `Domain` 트레이트 (domain.rs:50)
`#[async_trait(?Send)]` + `Downcast`. `spawn_pane`(필수)/`spawn`/`split_pane`/`move_pane_to_new_tab`/`attach`/`detach`/`state`/`detachable`/`spawnable`/`domain_id`/`domain_name`/`domain_label`. `spawn`·`split_pane`는 기본 구현이 `Mux`와 협력해 탭/페인을 등록한다.
부속: `DomainId=usize`, `DomainState{Detached,Attached}`(domain.rs:31), `SplitSource{Spawn,MovePane}`(domain.rs:40).
구현체: `LocalDomain`(domain.rs:202), `RemoteSshDomain`(ssh.rs:177), `TmuxDomain`(tmux.rs:81), `TermWizTerminalDomain`(termwiztermtab.rs:38).

### `Tab` (tab.rs:52)
페인 컨테이너. `Mutex<TabInner>` 래퍼. 공개 메서드(tab.rs:511~756): `new`/`tab_id`/`get/set_title`/`get_size`/`resize`/`get_active_pane`/`set_active_pane`/`set_active_idx`/`iter_panes`/`iter_panes_ignoring_zoom`/`iter_splits`/`split_and_insert`/`compute_split_size`/`assign_pane`/`remove_pane`/`kill_pane`/`kill_panes_in_domain`/`prune_dead_panes`/`is_dead`/`set_zoomed`/`toggle_zoom`/`get_zoomed_pane`/`rotate_clockwise`/`rotate_counter_clockwise`/`activate_pane_direction`/`get_pane_direction`/`adjust_pane_size`/`resize_split_by`/`swap_active_with_index`/`count_panes`/`sync_with_pane_tree`/`codec_pane_tree`.
부속 타입: `TabId=usize`, `Tree`/`Cursor`(bintree 별칭), `PositionedPane`(tab.rs:58)/`PositionedSplit`(tab.rs:193), `SplitDirection`/`SplitSize`/`SplitRequest`/`SplitDirectionAndSize`(tab.rs:95~131).

### 직렬화 트리 (codec 공유)
`PaneNode`(tab.rs:2114, `Empty`/`Split`/`Leaf`), `PaneEntry`(tab.rs:2160), `SerdeUrl`(tab.rs:2179). 주석에 "codec가 직접 사용하므로 변경 시 codec 버전 올릴 것"(tab.rs:2111, 2157) 명시.

### `Window` (window.rs:9)
`WindowId=usize`. 탭 벡터·활성 인덱스·직전 활성·워크스페이스·제목·초기 위치 보유. `push`/`insert`/`remove_by_idx`/`remove_by_id`/`get_active`/`save_and_then_set_active`/`set_active_without_saving`/`get_workspace`/`set_workspace`/`get/set_title`/`prune_dead_tabs`/`can_close_without_prompting`.

### 보조 공개 타입
- `renderable`: `StableCursorPosition`(renderable.rs:14), `RenderableDimensions`(renderable.rs:25), `terminal_*` 어댑터 함수(get_lines/with_lines/dimensions 등).
- `client`: `ClientId`(client.rs:17, hostname/username/pid/epoch/id/ssh_auth_sock), `ClientInfo`(client.rs:49).
- `activity`: `Activity`(activity.rs:13, RAII 활동 카운터).
- `connui`: `ConnectionUI`/`UIRequest`(connui.rs:40), SSH 연결 중 사용자 입력 UI.
- `termwiztermtab`: `TermWizTerminalPane`/`TermWizTerminal`, 내장 applet 페인.
- `ssh`: `ssh_connect_with_ui`(ssh.rs:60), `ssh_domain_to_ssh_config`(ssh.rs:184).
- 상수 `DEFAULT_WORKSPACE="default"`(lib.rs:50).

---

## 4. 내부 구조

### 모듈 분해
| 모듈 | 역할 |
|---|---|
| `lib.rs` | `Mux` 전역, `MuxNotification`, 출력 파이프라인(PTY 읽기·파싱·코얼레싱), 클립보드/다운로드 핸들러 |
| `pane.rs` | `Pane` 트레이트와 논리/물리 줄 변환 헬퍼 |
| `tab.rs` | `Tab`/`TabInner`, bintree 기반 레이아웃·분할·줌·리사이즈·방향 이동 (최대 모듈) |
| `window.rs` | `Window`, 탭 목록·활성 관리·dead 정리 |
| `domain.rs` | `Domain` 트레이트, `LocalDomain`(ConPTY/WSL/Exec/Serial/flatpak) |
| `localpane.rs` | `LocalPane`(ConPTY/SSH/tmux 공통 로컬 페인), 프로세스 수명·exit_behavior·검색·CWD 탐지 |
| `renderable.rs` | `Terminal`을 `Pane` 렌더 API에 연결하는 어댑터 |
| `ssh.rs` | `RemoteSshDomain`, SSH 핸드셰이크·`WrappedSshPty`/`WrappedSshChildKiller` |
| `ssh_agent.rs` | `AgentProxy`, 활성 클라이언트의 SSH agent 소켓을 심볼릭 링크로 추적 |
| `tmux.rs` | `TmuxDomain`/`TmuxDomainState`, tmux 제어 모드(-CC) 상태 기계 |
| `tmux_commands.rs` | tmux 명령(ListAllPanes/Windows/NewWindow/SplitPane/Resize/SendKeys 등) 인코딩·결과 처리 (대형) |
| `tmux_pty.rs` | `TmuxPty`/`TmuxChild`, tmux 페인을 PTY처럼 보이게 하는 어댑터 |
| `connui.rs` | 연결 UI 이벤트 루프(`ConnectionUIImpl`) |
| `termwiztermtab.rs` | termwiz applet을 페인/도메인으로 호스팅 |
| `activity.rs` | 전역 활동 카운터(프런트엔드 keep-alive) |
| `client.rs` | 클라이언트 식별·세션 메타데이터 |

### 제어/데이터 흐름

**출력 경로 (PTY → 화면)**: `Mux::add_pane`(lib.rs:768)가 `pane.reader()`로 reader를 얻어 전용 스레드에서 `read_from_pane_pty`(lib.rs:277) 실행 → BUFSIZE(1MiB) 블로킹 read → 내부 socketpair로 전달 → 별도 스레드 `parse_buffered_data`(lib.rs:138)가 termwiz 파서로 `Action` 생성. `Action`을 누적하되, `mux_output_parser_coalesce_delay_ms` 동안 `poll`로 추가 데이터를 기다려 **프레임 코얼레싱**한다. 단, `SynchronizedOutput`(DEC private mode) 진입 시 hold, 해제/SoftReset 시 flush(lib.rs:160~189). `send_actions_to_mux`(lib.rs:120)가 메인 스레드에서 `pane.perform_actions`를 호출하고 `PaneOutput` 통지.

**입력 경로**: 프런트엔드가 `Pane::key_down`/`mouse_event`/`send_paste`/`writer`를 호출 → `LocalPane`이 `Mux::record_input_for_current_identity()`(localpane.rs:333 등)로 활성 클라이언트 갱신 후 `Terminal`/PTY writer로 전달.

**스폰 경로**: `Mux::spawn_tab_or_window`(lib.rs:1303) → `resolve_spawn_tab_domain` → 필요 시 `domain.attach` → `resolve_cwd` → `domain.spawn` → `LocalDomain::spawn_pane`(domain.rs:566): `build_command`+`fixup_command` → `openpty` → `wezterm_term::Terminal` 생성 → `LocalPane::new` → `Mux::add_pane`.

**알림 경로**: 상태 변경 시 `Mux::notify`(lib.rs:696)가 모든 구독자에게 `MuxNotification`을 클론 전달. 구독자가 `false`를 반환하면 `retain`으로 제거된다.

### 비대 모듈
- **`tab.rs` (2540행)**: `Tab`은 얇은 래퍼이고, 실제 로직 대부분이 `TabInner`(tab.rs:758~2109, 약 1350행)에 집중된다. bintree 커서 순회, 분할 좌표 계산(`compute_split_size`), 줌, 리사이즈 전파, 방향 기반 페인 이동(`activate_pane_direction`), 회전 등이 모두 한 impl 블록에 있다. 테스트 모듈(tab.rs:2209~)도 길다.
- **`tmux_commands.rs` (1177행)·`tmux.rs` (435행)·`tmux_pty.rs`**: tmux -CC 제어 모드 서브시스템. 명령 큐·레이아웃 동기화·원격 페인 매핑이 분산되어 있어 응집도가 낮다.
- **`ssh.rs` (1129행)**: SSH 핸드셰이크 UI 루프와 PTY 래핑이 한 파일에 있다.

---

## 5. 핵심 데이터 구조·타입과 불변식

### `Mux` (lib.rs:100)
- 모든 컬렉션이 `RwLock<HashMap<...>>`. `main_thread_id`로 메인 스레드 여부 판정(`is_main_thread`, lib.rs:461). **불변식**: 통지(`notify`)는 메인 스레드에서 수행되어야 하며, 다른 스레드에서는 `notify_from_any_thread`(lib.rs:701)가 메인으로 디스패치한다.
- `default_domain`은 `add_domain` 시 비어 있으면 자동 설정(domain.rs 호출 측). `panes`에 등록된 페인은 클립보드·다운로드 핸들러가 주입된다(lib.rs:773~779).
- `num_panes_by_workspace`는 `recompute_pane_count`(lib.rs:465)로 갱신되며, 어느 탭이 busy(`count_panes()==None`)면 재계산을 **중단**한다. → 캐시가 일시적으로 부정확할 수 있다(불변식 약함).

### `Pane`/`LocalPane` (localpane.rs:77)
- `ProcessState`(localpane.rs:40): `Running{child_waiter, pid, signaller, killed}` → `DeadPendingClose{killed}` → `Dead`의 단방향 전이. `is_dead`(localpane.rs:199)가 상태 기계를 구동하며 `exit_behavior`·`clean_exit_codes`에 따라 hold/close를 결정한다. **불변식**: `Dead`가 되어야 mux가 페인을 정리한다.
- `CachedProcInfo`(localpane.rs:55): 전경 프로세스·CWD를 `PROC_INFO_CACHE_TTL=300ms`(localpane.rs:38) 동안 캐시. `CachePolicy::FetchImmediate`는 캐시 무시.
- `tmux_domain: Mutex<Option<...>>`가 Some이면 해당 페인은 tmux -CC 모드 호스트이며 키 입력·마우스·alt 화면이 특수 처리된다(localpane.rs:100~115, 335~344, 430~444).

### `TabInner` (tab.rs:40)
- `pane: Option<Tree>`(bintree). 리프=페인, 노드=`SplitDirectionAndSize`. **불변식**: `SplitDirectionAndSize::width/height`(tab.rs:159)는 first+second+분할선 1셀로 계산되며, 분할선이 항상 1셀을 차지한다고 가정한다.
- `zoomed: Option<Arc<dyn Pane>>` Some이면 활성 페인이 탭 전체를 차지하고 `size_before_zoom`에 원복용 크기를 보존한다.
- `Recency`(tab.rs:24): 페인 인덱스별 최근 활성 점수. 방향 이동/포커스 복원에 사용.

### `Window` (window.rs:9)
- **불변식**: `active < tabs.len()`(set_active_without_saving의 assert, window.rs:205). 탭 제거 후 `fixup_active_tab_after_removal`(window.rs:121)가 활성 인덱스를 보정한다. 같은 탭을 두 번 추가하면 panic(`check_that_tab_isnt_already_in_window`, window.rs:68).

### `ClientId` (client.rs:17)
- `epoch`는 프로세스 시작 시각 1회 계산(`EPOCH` lazy_static). `Hash`/`Eq` 파생 → `Mux::clients` 키. 직렬화 가능하여 codec/서버에서 공유.

### `MuxNotification` (lib.rs:53)
- 22개 변형. `Clone`이며 구독자 수만큼 복제된다. 대용량 데이터는 `Arc`로 감싼다(`SaveToDownloads{data: Arc<Vec<u8>>}`).

---

## 6. 외부 의존성 (주목 대상)

- **`portable-pty`** = PTY 추상의 핵심. Windows에서는 `win::conpty::ConPtySystem`(domain.rs:260)을 구체 타입으로 다운캐스트해 ConPTY 여부를 판정한다. serial 도메인은 `portable_pty::serial::SerialTty`(domain.rs:248)를 사용. `serde_support` feature 활성(Cargo.toml:31).
- **`wezterm-term`** = 실제 셀/스크린 모델·이스케이프 적용·검색 대상. `use_serde` feature 활성(Cargo.toml:46). `LocalPane`은 `Terminal`을 `Mutex`로 보유하고 `renderable.rs`가 어댑터 역할.
- **`termwiz`** = 이스케이프 파서(`escape::parser::Parser`, lib.rs:140), `tmux_cc`(tmux 제어 모드 이벤트 파서), `lineedit`(SSH/연결 UI 입력), terminfo 렌더러(connui/termwiztermtab applet 출력).
- **`wezterm-ssh`** = `Session`/`SshPty`/`SshChildProcess`. SSH 백엔드는 `ssh2` 또는 `libssh` 중 선택(ssh.rs:205, config의 `ssh_backend`).
- **`procinfo`** = 전경 프로세스·CWD·프로세스 트리. Windows에는 세션/잡 개념이 없어, 콘솔에서 가장 최근 시작된 프로세스를 전경으로 추론한다(localpane.rs:949~967 `find_youngest`).
- **`bintree`** = 탭 레이아웃 트리. `Tree`/`Cursor` 별칭으로 직접 사용(tab.rs:17).
- **`promise`** = `spawn::spawn_into_main_thread`로 비메인 스레드의 작업을 메인으로 마샬링(전 모듈에서 빈번).
- **`metrics`** = `histogram!`로 파싱 지연·바이트율 계측(lib.rs:125, 328).
- **`names`** = 워크스페이스 자동 명명(`generate_workspace_name`, lib.rs:590).
- **`fancy-regex`** = 페인 내 정규식 검색(localpane.rs:566).
- **`mlua`/`luahelper`/`config`** = Lua 콜백(`mux-is-process-stateful` 훅 localpane.rs:484, ExecDomain `fixup_command`/`label` domain.rs:330).

---

## 7. 설정·기능 플래그

### Cargo feature
`mux` 자체에는 선언된 feature가 없다(Cargo.toml). 의존 측 feature만 켠다: `portable-pty/serde_support`, `wezterm-term/use_serde`, `serde/rc`·`derive`. Windows 타깃에서만 `ntapi`/`winapi`를 링크한다(Cargo.toml:48).

### 동작에 영향을 주는 config 항목 (소스에서 직접 참조)
- `mux_output_parser_buffer_size`, `mux_output_parser_coalesce_delay_ms` (출력 코얼레싱, lib.rs:139~229)
- `mux_enable_ssh_agent` (AgentProxy 생성 여부, lib.rs:429)
- `exit_behavior`, `exit_behavior_messaging`, `clean_exit_codes` (페인 종료 처리, localpane.rs:227~293)
- `skip_close_confirmation_for_processes_named` (닫기 확인 생략, localpane.rs:507)
- `default_workspace` (lib.rs:454)
- `switch_to_last_active_tab_when_closing_tab` (window.rs:155)
- `default_prog`, `default_cwd`, `wsl_domains()`, `exec_domains`, `ssh_backend`, `log_unknown_escape_sequences` (domain.rs/localpane.rs/ssh.rs)
- 환경 변수 주입: `WEZTERM_PANE`, `WEZTERM_UNIX_SOCKET`, `SSH_AUTH_SOCK`(domain.rs:470~476)

설정 철학상, 이들 기본값은 `wezterm.lua`보다 `config` 크레이트 소스 수정으로 고정하는 것을 우선한다.

---

## 8. Windows 전용 고려사항

- **PTY 백엔드**: 기본은 ConPTY. `is_conpty()`(domain.rs:256)가 `ConPtySystem` 다운캐스트로 판정하고 `enable_conpty_quirks`(domain.rs:604)를 활성화한다. WSL은 별도 PTY가 아니라 `wsl.exe` 명령 재작성(domain.rs:264~306).
- **winapi/libc 사용 지점**:
  - 소켓 버퍼 크기 조정(`SO_SNDBUF`/`SO_RCVBUF`)을 `libc::setsockopt`로 설정하고 `winapi::um::winsock2`의 상수를 사용한다(lib.rs:30, 243~270). 출력 socketpair의 1MiB 버퍼 확보용.
  - `libc::getpid`로 PID 취득(client.rs:40, ssh_agent.rs:86).
  - `filedescriptor::AsRawSocketDescriptor`로 Winsock 디스크립터를 `poll`에 넘김(lib.rs:207, ssh.rs:539).
- **SSH 에이전트**: `std::os::windows::fs::symlink_file`로 `agent.<pid>` 심볼릭 링크를 만들어 활성 클라이언트의 SSH agent 소켓을 가리킨다(ssh_agent.rs:5, 50~76). 링크가 이미 있으면 제거 후 재생성. Windows에서 심볼릭 링크 생성은 권한 의존이므로 실패 경로가 존재한다.
- **전경 프로세스 추론**: Windows에는 프로세스 그룹/세션 리더 개념이 없어, 콘솔(`console != 0`)에서 시작 시각이 가장 늦은 프로세스를 전경으로 추정한다(localpane.rs:945~967). 이는 휴리스틱이며 정확하지 않을 수 있다.
- **Windows 파일 URL 보정**: OSC 7 등으로 받은 `file:///C:\...` 경로의 선행 슬래시를 제거한다(lib.rs:1166~1174).
- **죽은 코드 (비Windows 경로)**:
  - `domain.rs:365` flatpak 분기(`/.flatpak-info` 존재 검사 및 `flatpak-spawn`). Windows에서는 경로가 존재하지 않아 도달 불가. **리눅스 잔재**.
  - `ExitBehavior`/exit 메시지의 일부 유닉스 가정. `tty_name()`은 항상 `None`(localpane.rs:454).
  - `procinfo` 주석의 tcgetpgrp 언급(localpane.rs:61)은 유닉스 비용 설명이며 Windows 경로와 무관.
  - 본 크레이트 `src`에는 `cfg(unix)`/`cfg(windows)` 분기가 **존재하지 않는다**(grep 무결과). 플랫폼 분기는 의존 크레이트(`portable-pty`/`procinfo`/`filedescriptor`)에 위임되어 있고, mux 코드는 런타임 검사(flatpak 파일 존재, ConPTY 다운캐스트)로 분기한다. 즉 플랫폼 제거가 mux 레벨에서 완결되지 않았고, flatpak 경로 같은 죽은 코드가 컴파일된 채 남아 있다.

---

## 9. 리팩토링 주의점

1. **codec와의 ABI 결합**: `PaneNode`/`PaneEntry`/`SerdeUrl`/`StableCursorPosition`/`RenderableDimensions`는 `codec`가 직접 직렬화한다(tab.rs:2111, 2157 주석). 필드 추가·순서 변경은 mux 서버↔클라이언트 프로토콜을 깨뜨린다. 변경 시 codec 버전을 함께 올려야 한다.
2. **전역 단일톤 `Mux`**: `Mux::get().unwrap()`(lib.rs:752)이 다수 경로에서 호출되어, mux 미초기화 시 panic 위험이 있다. 대부분 `try_get`을 쓰지만 일관적이지 않다. 전역 의존을 줄이려면 광범위한 시그니처 변경이 필요하다.
3. **스레드/락 모델**: 출력 파이프라인은 페인당 2개 스레드(reader, parser)를 띄운다(lib.rs:308, 786). 통지는 메인 스레드 전용이며 `notify_from_any_thread`로만 우회 가능. `RwLock` 다수 보유로, `prune_dead_windows`(lib.rs:891)·`remove_tab_internal`(lib.rs:821)은 `try_write`로 재진입 락을 피하는 방어 코드를 둔다 — 이 가정을 깨면 데드락/누락 정리가 발생한다.
4. **`tab.rs` 비대 + 단일 `Mutex<TabInner>`**: 모든 탭 연산이 하나의 락 뒤에 직렬화된다. `count_panes()`가 `None`(busy)을 반환하면 `recompute_pane_count`가 중단되어 워크스페이스 카운트가 부정확해진다(lib.rs:465~480). 분할/리사이즈 알고리즘을 분리할 때 bintree 좌표 불변식(분할선 1셀)을 보존해야 한다.
5. **tmux 서브시스템 순환·분산**: `tmux.rs`↔`tmux_commands.rs`↔`tmux_pty.rs`가 `TmuxDomainState`를 공유하며 상호 참조한다. 명령 큐는 `VecDeque<Box<dyn TmuxCommand>>`와 상태 기계(`State`)에 의존하며, `localpane.rs`의 DCS 핸들러(localpane.rs:745)가 tmux 도메인을 동적으로 생성·연결한다. 결합도가 높아 단위 분리가 어렵다.
6. **`Pane`↔`Mux` 양방향 결합**: `LocalPane`이 `Mux::get()`을 직접 호출(입력 기록·dead 통지·tmux 도메인 등록)한다. 페인을 mux로부터 독립시키려면 콜백/이벤트 주입으로 역전이 필요하다.
7. **죽은 flatpak 경로**: domain.rs:365 분기는 Windows fork에서 도달 불가하므로 제거 후보. 단, `fixup_command`의 제어 흐름(else-if 체인)을 건드리므로 WSL/Exec 분기와 분리해 신중히 제거해야 한다.
8. **휴리스틱의 취약성**: 전경 프로세스 추론(시작 시각 기반)과 SSH agent 디바운스(100ms, ssh_agent.rs:30~33)는 의도적 근사다. 변경 시 사용자 체감(닫기 확인, agent 라우팅)에 직접 영향.
9. **`async_trait(?Send)`**: `Pane`/`Domain`이 비-Send 비동기 트레이트다. 멀티스레드 런타임 도입이나 trait 객체를 스레드 간 이동시키려는 리팩토링은 컴파일 단계에서 광범위하게 막힌다.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `src/tab.rs` | 2540 | `Tab`/`TabInner`, bintree 레이아웃·분할·줌·리사이즈·방향이동·회전, 직렬화 트리(`PaneNode`/`PaneEntry`/`SerdeUrl`). 최대 모듈 |
| `src/lib.rs` | 1462 | `Mux` 전역·`MuxNotification`, PTY 출력 파이프라인(읽기/파싱/코얼레싱), 스폰·분할·이동 API, 클립보드/다운로드 핸들러 |
| `src/tmux_commands.rs` | 1177 | tmux -CC 명령(List/New/Split/Resize/SendKeys) 인코딩·결과 처리. 대형 |
| `src/ssh.rs` | 1129 | `RemoteSshDomain`, SSH 핸드셰이크·UI 연동, `WrappedSshPty`/`WrappedSshChildKiller`/`PtyReader`/`PtyWriter` |
| `src/pane.rs` | 1078 | `Pane` 트레이트, `LogicalLine` 좌표 변환, 구현 보조 함수, 단위 테스트 |
| `src/localpane.rs` | 999 | `LocalPane`(ConPTY/WSL/SSH/tmux 공통), 프로세스 수명 상태기계·exit_behavior·검색·CWD/전경프로세스 탐지·tmux DCS 핸들러 |
| `src/domain.rs` | 708 | `Domain` 트레이트, `LocalDomain`(ConPTY·WSL 재작성·ExecDomain·serial·flatpak[죽은코드]), `WriterWrapper`/`FailedSpawnPty` |
| `src/termwiztermtab.rs` | 586 | termwiz applet을 페인/도메인으로 호스팅(`TermWizTerminalPane`/`TermWizTerminalDomain`), SSH 입력 프롬프트 등 |
| `src/connui.rs` | 460 | 연결 UI 이벤트 루프(`ConnectionUI`/`UIRequest`/`ConnectionUIImpl`), 비밀번호 마스킹 |
| `src/tmux.rs` | 435 | `TmuxDomain`/`TmuxDomainState`, tmux 제어 모드 상태 기계·이벤트 디스패치 |
| `src/window.rs` | 268 | `Window`, 탭 목록·활성/직전활성 관리·dead 정리·워크스페이스/제목 |
| `src/ssh_agent.rs` | 220 | `AgentProxy`, 활성 클라이언트 SSH agent 소켓을 심볼릭 링크로 추적(Windows symlink) |
| `src/tmux_pty.rs` | 152 | `TmuxPty`/`TmuxChild`, tmux 페인을 PTY로 가장하는 어댑터(쓰기→SendKeys 명령) |
| `src/renderable.rs` | 142 | `StableCursorPosition`/`RenderableDimensions`, `Terminal`↔`Pane` 렌더 API 어댑터 |
| `src/client.rs` | 81 | `ClientId`/`ClientInfo`, 클라이언트 식별·세션 메타데이터 |
| `src/activity.rs` | 42 | `Activity`, 전역 활동 카운터(프런트엔드 keep-alive, Drop 시 dead 정리 트리거) |

생성 파일 없음 — 본 크레이트는 전부 수기 작성 소스다.
