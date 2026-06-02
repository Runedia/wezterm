# wezterm-gui 폴더 기능 명세

본 문서는 `E:\Project\wezterm\wezterm-gui` 폴더에 포함된 단일 크레이트 `wezterm-gui`의 상세 기능 명세다. 이 저장소는 WezTerm의 Windows 전용 영구 분기이며, upstream 동기화를 포기하고 비Windows 플랫폼 코드를 제거한 상태다. 본 명세는 추후 리팩토링을 위한 근거 자료로서 소스 코드를 직접 확인하여 작성했다.

---

## 1. 개요 및 책임

`wezterm-gui`는 WezTerm의 **GUI 실행 바이너리(최상위 크레이트)**다. 워크스페이스 전체에서 가장 크고(약 177,000행, 단 약 140,000행은 생성된 유니코드 이름 테이블) 결합도가 가장 높은 크레이트이며, fan-out 29의 의존 관계를 가진다.

이 크레이트의 단일 책임은 **터미널 멀티플렉서(mux)가 관리하는 가상 터미널 상태를 사용자가 보는 윈도우 픽셀로 변환하고, 입력(키보드·마우스)을 mux로 전달하는 것**이다. 구체적으로 다음을 수행한다.

- 프로세스 부트스트랩 및 CLI 서브커맨드(`start`, `ssh`, `serial`, `connect`, `ls-fonts`, `show-keys`) 처리(`main.rs`).
- mux 백엔드 초기화 및 GUI 프런트엔드(`frontend.rs`)와의 연결, 워크스페이스↔윈도우 조정.
- 윈도우별 렌더 루프·입력 처리·탭/팬 레이아웃·오버레이·명령 팔레트를 담당하는 `TermWindow`(`termwindow/` 모듈, 핵심).
- 폰트 셰이핑 결과를 GPU 텍스처 아틀라스에 캐싱하고(OpenGL/Glium 또는 WebGPU/wgpu) 정점 버퍼로 렌더(`glyphcache.rs`, `renderstate.rs`, `quad.rs`, `customglyph.rs`).
- Lua 설정 시스템에 GUI 전용 함수(`wezterm.gui.*`)와 `GuiWin` 사용자 데이터를 노출(`scripting/`).

`main.rs`는 `#![windows_subsystem = "windows"]`(테스트 제외)로 빌드되어 콘솔 창을 생성하지 않으며, `run()` 진입점은 Windows의 `SetCurrentProcessExplicitAppUserModelID`를 호출해 토스트 알림 귀속을 설정한다(`main.rs:1175`).

---

## 2. 워크스페이스 내 위치

이 크레이트는 의존성 그래프의 **최상위(루트 바이너리)**에 위치한다. 피의존(usedBy)이 없고, 폰트·mux·터미널·윈도우·설정 계층을 모두 끌어와 통합한다. 즉 파이프라인의 최종 소비자다: 하위 계층(`config` → `mux`/`wezterm-term` → `wezterm-font` → `window`)이 만든 상태와 능력을 받아 화면에 출력하고 입력을 되돌려보낸다.

### 워크스페이스 내부 의존(deps, 29개)

| 크레이트 | 역할 |
| --- | --- |
| `codec` | mux 프로토콜 직렬화(`SpawnV2` 등 GUI↔서버 명령) |
| `config` | 설정 로딩·`ConfigHandle`·`keyassignment`·Lua 컨텍스트 |
| `env-bootstrap` | 헤드리스 환경/로깅/Lua 부트스트랩(`bootstrap()`) |
| `frecency` | 명령 팔레트·문자 선택기의 최근 사용 빈도 점수 |
| `lfucache` | 셰이프·라인·쿼드 캐시(LFU) |
| `luahelper` | Rust↔Lua 동적값 변환 헬퍼 |
| `mux` | 터미널 멀티플렉서 코어(도메인·윈도우·탭·팬) |
| `mux-lua` | mux 객체의 Lua 바인딩(`MuxDomain`, `MuxPane` 등) |
| `portable-pty` | `CommandBuilder` 등 PTY 명령 빌더 |
| `promise` | 비동기 future·메인 스레드 스폰 |
| `rangeset` | 검색·선택 행 범위 집합 |
| `tabout` | 통계 출력 표 정렬(`stats.rs`) |
| `termwiz` | 셀/라인/색/이스케이프/입력 등 터미널 원형 타입 |
| `termwiz-funcs` | 라인→이스케이프 변환 등 termwiz 보조 |
| `umask` | umask 저장/복원(`UmaskSaver`) |
| `url-funcs` | Lua용 `Url` 래퍼 |
| `wezterm-bidi` | 양방향 텍스트 방향 처리 |
| `wezterm-blob-leases` | 다운로드/이미지 blob 임시 저장 |
| `wezterm-client` | GUI 소켓 디스커버리·기존 인스턴스 연결 |
| `wezterm-dynamic` | 동적값(`Value`)·`FromDynamic`/`ToDynamic` |
| `wezterm-font` | 폰트 로딩·셰이핑(`FontConfiguration`, harfbuzz/freetype) |
| `wezterm-gui-subcommands` | CLI 서브커맨드 구조체(`StartCommand` 등) |
| `wezterm-mux-server-impl` | 로컬 mux 서버 리스너·도메인 갱신 |
| `wezterm-open-url` | URL 열기 |
| `wezterm-ssh` | SSH 도메인 |
| `wezterm-term` | 터미널 에뮬레이터 코어(셀·커서·알림) |
| `wezterm-toast-notification` | Windows 토스트 알림 |
| `window` | OS 윈도우·GL/wgpu 컨텍스트·입력 추상(`::window::*`) |
| `window-funcs` | `window` 계층의 Lua 등록 함수 |

### 피의존(usedBy)

없음. 최상위 바이너리이므로 다른 크레이트가 이를 참조하지 않는다.

---

## 3. 공개 API 표면

이 크레이트는 바이너리(`main`)이므로 외부에 라이브러리 API를 게시하지 않는다(`publish = false`). 그러나 크레이트 내부 모듈 간 계약과 Lua로 노출되는 표면은 명확하다.

### 3.1 크레이트 루트 재노출(`main.rs`)

- `pub use selection::SelectionMode;`
- `pub use termwindow::{set_window_class, set_window_position, TermWindow, ICON_DATA};`
- `pub fn run_ls_fonts(config, cmd) -> anyhow::Result<()>` — `ls-fonts` 서브커맨드 구현.

### 3.2 프런트엔드(`frontend.rs`)

- `pub struct GuiFrontEnd` — 스레드 로컬 싱글톤. mux 알림 구독, 워크스페이스↔윈도우 조정, 클립보드 할당, 토스트 알림 게이팅을 담당.
  - `pub fn try_new() -> anyhow::Result<Rc<GuiFrontEnd>>`
  - `pub fn run_forever(&self) -> anyhow::Result<()>` — OS 메시지 루프 실행.
  - `pub fn reconcile_workspace(&self) -> Future<()>` — 활성 워크스페이스의 mux 윈도우와 GUI 윈도우를 1:1 매핑.
  - `pub fn gui_windows(&self) -> Vec<GuiWin>`, `gui_window_for_mux_window(...)`, `record_known_window(...)`, `forget_known_window(...)`, `switch_workspace(...)`.
- `pub fn front_end() / try_front_end() -> (Option<)Rc<GuiFrontEnd>(>)` — 스레드 로컬 접근자.
- `pub struct WorkspaceSwitcher` — RAII 가드. Drop 시 워크스페이스 전환 완료.
- `pub fn try_new() / shutdown()`.

### 3.3 `TermWindow`(`termwindow/mod.rs`)

윈도우별 GUI 상태 전체를 보유하는 핵심 타입.

- `pub async fn new_window(mux_window_id: MuxWindowId) -> anyhow::Result<()>` — 폰트·렌더 메트릭·윈도우·GL/WebGPU 컨텍스트를 모두 구성하고 윈도우를 표시.
- `TermWindowNotif` (pub enum) — `Window::notify`로 윈도우 이벤트 루프에서 작업을 수행하기 위한 메시지(예: `PerformAssignment`, `SetLeftStatus`, `Apply(Box<dyn FnOnce>)`, `SwitchToMuxWindow`, `MuxNotification`). 이 enum이 GUI 비동기 제어의 중심 채널이다.
- `UIItem`/`UIItemType` (pub) — 히트 테스트 가능한 화면 영역(탭바·스크롤 썸·스플릿). `hit_test(x,y)`.
- `pub struct TabInformation` / `PaneInformation` — Lua `UserData`로 노출되어 `format-tab-title` 등 콜백에 전달.
- `MouseCapture`, `OverlayState`, `PaneState`, `TabState`, `SemanticZoneCache` 등 상태 보조 타입.
- `impl UserData for GuiWin`(`scripting/guiwin.rs`) — Lua에서 `window:active_tab()`, `window:perform_action(...)` 등 메서드 제공.

### 3.4 Lua 표면(`scripting/mod.rs`)

`wezterm.gui` 서브모듈에 등록: `gui_window_for_mux_window`, `gui_windows`, `default_keys`, `default_key_tables`, `enumerate_gpus`. `stats::register`, `window_funcs::register`도 Lua 컨텍스트 설정 함수로 등록(`main.rs:1205`).

### 3.5 명령/입력(`commands.rs`, `inputmap.rs`)

- `pub struct CommandDef` / `ExpandedCommand` — 명령 메타데이터(brief/doc/keys/menubar/icon). `recreate_menubar`, `default_key_assignments`, `derive_command_from_key_assignment` 제공.
- `pub struct InputMap { keys: KeyTables, mouse: HashMap<...> }` — 키/마우스 바인딩 해석. `new(config)`, `default_input_map()`, `show_keys()`, `dump_config()`.

### 3.6 오버레이(`overlay/mod.rs`)

- `pub fn start_overlay<T,F>(...) -> (Arc<dyn Pane>, Pin<Box<Future>>)`, `start_overlay_pane(...)` — 일반 오버레이 부착 진입점.
- 재노출: `confirm_close_pane/tab/window`, `confirm_quit_program`, `CopyOverlay`/`CopyModeParams`, `show_debug_overlay`, `launcher`/`LauncherArgs`/`LauncherFlags`, `QuickSelectOverlay`.

---

## 4. 내부 구조

### 4.1 부트스트랩과 서브커맨드 흐름(`main.rs`)

`main()` → `run()`이 진입점이다. 순서: AppUserModelID 설정 → CLI 파싱(`Opt`/`SubCommand`) → `env_bootstrap::bootstrap()` → Lua 컨텍스트 설정 함수 등록 → 통계 초기화 → `config::common_init` → 서브커맨드 분기. `Start`는 `run_terminal_gui`로 진입하며, 여기서 `Publish::resolve`/`try_spawn`을 통해 **이미 실행 중인 GUI 인스턴스에 명령을 위임**할지 결정한다(소켓 디스커버리 기반). 위임 실패 시 `frontend::try_new()`로 프런트엔드를 만들고 `async_run_terminal_gui`를 스폰한 뒤 `run_forever()`로 메시지 루프에 진입한다.

### 4.2 프런트엔드 조정(`frontend.rs`)

`GuiFrontEnd`는 스레드 로컬 싱글톤으로, `Mux::subscribe`로 모든 `MuxNotification`을 수신한다. 워크스페이스 변경·윈도우 생성/제거 시 `reconcile_workspace()`가 mux 윈도우 목록과 `known_windows`(BTreeMap)를 1:1로 맞추며 부족분은 `TermWindow::new_window`로 생성, 잉여분은 닫는다. 토스트 알림은 `notification_handling` 설정에 따라 포커스된 페인/탭/윈도우 기준으로 게이팅한다(`frontend.rs:114`).

### 4.3 termwindow/ 모듈 — 렌더 루프·입력·탭/팬·오버레이·명령 팔레트

`termwindow/mod.rs`(3,616행, **이 크레이트에서 두 번째로 큰 비대 모듈**)가 `TermWindow` 구조체와 윈도우 이벤트 디스패치(`dispatch_window_event`), Lua 이벤트 발행, 오버레이 부착(`assign_overlay`), 상태 관리를 담당한다. 하위 모듈은 다음과 같이 책임이 분할되어 있다.

- **렌더 루프(`termwindow/render/`)**
  - `paint.rs` — `paint_impl`. FPS 계산, `AllowImage` 정책(텍스처 부족 시 이미지 스케일 다운/비활성), 텍스처 아틀라스 고갈(`OutOfTextureSpace`) 시 다중 패스 재시도.
  - `draw.rs` — `call_draw`. Glium(OpenGL)과 WebGPU 백엔드로 분기하여 실제 드로콜 수행.
  - `mod.rs` — 라인→요소 셰이핑 캐시 키/값(`LineQuadCacheKey`, `LineToEleShapeCacheKey`, `LineToElementShape`), 커서 속성, 화면 라인 렌더 파라미터 등 렌더 파이프라인 자료형 정의. 렌더 서브모듈 11개를 묶는다.
  - `screen_line.rs` — 한 줄을 정점 버퍼로 렌더(다중 셀 글리프·하이퍼링크·커서 처리). 901행으로 비대.
  - `pane.rs` — 박스 모델 기반 페인 렌더(`build_pane`).
  - `tab_bar.rs` / `fancy_tab_bar.rs` — 기본/팬시(이미지·둥근 모서리) 탭바.
  - `window_buttons.rs` — 통합 타이틀바 버튼(닫기/최소화/최대화). Windows 스타일 폴리곤 포함.
  - `borders.rs`, `corners.rs`, `split.rs` — 윈도우 보더·둥근 모서리·스플릿 분할선.
- **입력**
  - `keyevent.rs` — `KeyTableState`(키 테이블 스택, one-shot/until-unknown/타임아웃), 리더 키, 데드 키, 키 어사인먼트 수행.
  - `mouseevent.rs` — UI 아이템 히트 테스트, 마우스 캡처, 드래그, 마우스 바인딩 해석.
- **레이아웃/리사이즈**
  - `resize.rs` — 윈도우↔셀 치수 변환, DPI/스케일 변경 큐(`ScaleChange`).
  - `spawn.rs`(termwindow) — 탭/팬 스폰 진입.
  - `selection.rs`(termwindow) — 페인 내 텍스트 선택 상태.
- **오버레이/모달**
  - `modal.rs` — `Modal` 트레이트(perform_assignment/mouse/key/computed_element/reconfigure). 명령 팔레트·문자 선택기·페인 선택기가 구현.
  - `palette.rs` — 명령 팔레트(`CommandPalette`), 퍼지 매칭·최근 사용 빈도(frecency) 정렬.
  - `charselect.rs` — 문자/이모지 선택기(`CharSelector`), 유니코드 이름·이모지 그룹 매칭.
  - `paneselect.rs` — 페인 선택/스왑 모달.
  - `clipboard.rs` — 복사/붙여넣기 클립보드 연동.
  - `box_model.rs` — CSS 유사 박스 모델(`Element`, `ComputedElement`, Display/Float/VerticalAlign). 팬시 탭바·팔레트·선택기 UI의 레이아웃 엔진. 1,233행으로 비대.
  - `background.rs` — 윈도우 배경 이미지/그라디언트 로딩·캐시.
  - `webgpu.rs` — WebGPU 상태(`WebGpuState`, `ShaderUniform`, `WebGpuTexture`).
  - `prevcursor.rs` — 커서 블링크 위상 추적.

### 4.4 렌더 지원 모듈(크레이트 루트)

`glyphcache.rs`(글리프·이미지·블록 텍스처 아틀라스 캐시), `customglyph.rs`(6,040행, 박스드로잉·파워라인·진행바·git 브랜치 등을 폴리곤으로 직접 래스터화), `shapecache.rs`(셰이핑 결과 캐시 키/값), `renderstate.rs`(`RenderContext` Glium/WebGpu 추상, 정점·인덱스 버퍼·아틀라스 할당), `quad.rs`(정점 포맷·쿼드 할당자), `utilsprites.rs`(밑줄/취소선 등 유틸 스프라이트와 `RenderMetrics`), `uniforms.rs`(셰이더 유니폼 빌더), `colorease.rs`(블링크/비주얼벨 색 이징).

### 4.5 보조 기능

`update.rs`(GitHub 릴리스 확인·배너), `download.rs`(원격 다운로드 파일명 무해화·저장), `stats.rs`(metrics Recorder 구현·히스토그램·처리량), `tabbar.rs`(`TabBarState`/`TabBarItem` 모델), `scrollbar.rs`(`ScrollHit` 썸 계산), `selection.rs`(선택 좌표/범위 모델).

---

## 5. 핵심 데이터 구조·타입

- **`TermWindow`(`termwindow/mod.rs:365`)** — 윈도우당 1개. 불변식: `render_state`는 `created()` 이후 항상 `Some`(없으면 `panic!("No OpenGL")`). `gl`과 `webgpu`는 상호 배타적(`front_end` 설정에 따라 하나만 채움). `mux_window_id`로 mux 윈도우와 연결되며, 캐시들(`shape_cache`/`line_state_cache`/`line_quad_cache`/`line_to_ele_shape_cache`)은 `config_generation`/`shape_generation`/`quad_generation` 카운터로 무효화된다. 세 generation 카운터가 증가하면 대응 캐시 키가 불일치하여 재계산을 강제한다.
- **`TermWindowNotif`(`termwindow/mod.rs:118`)** — 윈도우 이벤트 루프에 작업을 주입하는 메시지. `Apply(Box<dyn FnOnce(&mut TermWindow) + Send + Sync>)` 변형으로 임의 클로저를 메인 스레드에서 실행. 비동기 코드가 `&mut TermWindow`에 접근하는 유일한 경로.
- **`RenderContext`(`renderstate.rs:23`)** / **`RenderFrame`** — `Glium`/`WebGpu` 2-변형 enum. 모든 GPU 자원 할당이 이 분기를 통과한다.
- **`LineQuadCacheKey`(`termwindow/render/mod.rs:57`)** — 라인 렌더 캐시 키. `shape_hash`(16바이트), 세 generation, 선택 범위, 커서 속성, 반전 비디오, 패스워드 입력 등을 포함. 불변식: 키 구성요소 중 하나라도 바뀌면 캐시 미스.
- **`KeyTableState`(`termwindow/keyevent.rs:37`)** — 키 테이블 스택. `one_shot`은 키 인식 후 자동 pop, `until_unknown`은 미인식 키에서 pop, `expiration`은 타임아웃 만료. 스택 LIFO 불변식.
- **`Selection`(`selection.rs:12`)** — `origin`/`range`/`seqno`/`rectangular`. `range`는 정규화 전 값(드래그 방향 보존). `seqno`로 페인 내용 변경 시 무효화.
- **`Vertex`(`quad.rs:33`)** — `#[repr(C)] bytemuck::Pod`. `has_color` 필드가 `IS_GLYPH`(0)/`IS_COLOR_EMOJI`(1)/`IS_BG_IMAGE`(2)/`IS_SOLID_COLOR`(3)/`IS_GRAY_SCALE`(4) 모드를 선택. Glium·wgpu 양쪽 정점 레이아웃을 동시에 정의.
- **`GlyphKey`/`SizedBlockKey`/`CellMetricKey`(`glyphcache.rs`)** — 글리프 캐시 키. 폰트 인덱스·글리프 위치·셀 수·스타일·뒤 공백 여부 등으로 캐시 엔트리를 식별.
- **`TabBarState`/`TabBarItem`/`TabEntry`(`tabbar.rs`)** — 탭바를 termwiz `Line`으로 렌더하고 클릭 영역을 `TabEntry`로 추적.
- **`Element`/`ComputedElement`(`box_model.rs`)** — 박스 모델 트리. `ComputedElement`는 픽셀 좌표가 확정된 결과로, `ui_items()`로 히트 테스트 항목을 산출.

---

## 6. 외부 의존성

- **`wgpu`** — WebGPU 백엔드(`front_end = "WebGpu"`). `WebGpuState`가 surface·device·queue·render_pipeline을 보유(`termwindow/webgpu.rs`). `enumerate_gpus`로 GPU 목록도 Lua에 노출.
- **`window`의 `glium`** — OpenGL 백엔드(기본). Windows에서는 빌드 시 ANGLE(`libEGL.dll`/`libGLESv2.dll`)와 Mesa(`opengl32.dll`)를 출력 폴더로 복사(`build.rs`).
- **`tiny-skia`** — `customglyph.rs`에서 박스드로잉/파워라인/진행바 글리프를 폴리곤으로 직접 래스터화(폰트에 없는 글자를 wezterm이 그림).
- **`image`** — 배경 이미지·이모지·셀 이미지(애니메이션 포함) 디코딩.
- **`colorgrad`** — 배경 그라디언트 생성(`background.rs`).
- **`nucleo-matcher`** — 명령 팔레트·선택기 퍼지 매칭(`overlay/selector.rs`의 `matcher_score`).
- **`emojis`** — 문자 선택기 이모지 그룹/검색(`charselect.rs`).
- **`frecency`** — 명령·문자 최근 사용 빈도 점수.
- **`rayon`** — 매칭/필터링 병렬화(팔레트·선택기·런처).
- **`hdrhistogram` + `metrics`** — 처리량/지연 통계(`stats.rs`, metrics `Recorder` 구현).
- **`http_req`** — 업데이트 확인(GitHub 릴리스 조회, `update.rs`).
- **`mlua`** — `send`+`serialize` 피처로 Lua 바인딩.
- **`bytemuck`** — 정점/유니폼 구조체의 GPU 버퍼 안전 캐스팅.
- **`finl_unicode`/`unicode-normalization`/`unicode-segmentation`** — 그래핌 분할·NFC 정규화·열 폭 계산.

---

## 7. 설정·기능 플래그

### Cargo 피처(`Cargo.toml`)

| 피처 | 의미 |
| --- | --- |
| `default = ["vendored-fonts"]` | 기본 활성 |
| `vendored-fonts` | Nerd Font Symbols·JetBrains·Roboto·Noto Emoji 폰트 번들(개별 `vendor-*-font` 피처의 합) |
| `distro-defaults` | `config/distro-defaults` 전달 |
| `dhat-heap` / `dhat-ad-hoc` | `dhat` 기반 힙/애드혹 프로파일링(`main.rs:61`의 전역 할당자 교체) |

빌드 의존성으로 Windows에서 `cc`(MSVC 레지스트리 탐색)와 `embed-resource`(아이콘/매니페스트/버전 리소스 임베드)를 사용한다(`build.rs`).

### 주요 config 항목(코드에서 직접 참조)

`front_end`(Glium/WebGpu 선택), `enable_tab_bar`/`hide_tab_bar_if_only_one_tab`, `enable_scroll_bar`, `use_resize_increments`, `window_padding`, `notification_handling`, `quit_when_all_windows_are_closed`, `allow_download_protocols`, `custom_block_glyphs`, `window_close_confirmation`, `cursor_blink_rate`/`text_blink_rate`(이징), `shape_cache_size`/`line_state_cache_size`/`line_quad_cache_size`/`line_to_ele_shape_cache_size`, `default_gui_startup_args`, `default_domain`/`default_workspace`, `quick_select_alphabet`. 명령 메타데이터는 `commands_i18n.ko.json`을 임베드하여 한국어로 현지화(`commands.rs:18`).

---

## 8. Windows 전용 고려사항

- **windows_subsystem**: `main.rs:2`에서 `windows_subsystem = "windows"`로 빌드되어 콘솔 창을 띄우지 않는다. 그 대가로 콘솔에서 실행 시 `--attach-parent-console` 플래그로 `winapi`의 `AttachConsole(ATTACH_PARENT_PROCESS)`를 호출(`main.rs:1197`).
- **AppUserModelID**: `run()`에서 `windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID`를 호출해 토스트 알림이 올바르게 귀속되도록 설정(`main.rs:1175`).
- **빌드 시 DLL 복사**: `build.rs`가 `assets/windows/`의 conhost(`conpty.dll`, `OpenConsole.exe`), ANGLE(`libEGL.dll`, `libGLESv2.dll`), Mesa(`opengl32.dll`)를 타깃 출력 폴더로 복사하고, `winres` 기반 `resource.rc`(아이콘·매니페스트·버전)를 MSVC `rc`로 컴파일. MSVC vcvars 환경이 필요한 지점.
- **윈도우 의존성**: `[target.'cfg(windows)'.dependencies]`로 `winapi`(winuser/consoleapi/handleapi/fileapi/namedpipeapi/synchapi/winsock2)와 `windows`(Win32_UI_Shell) 사용.
- **HWND/클라이언트 영역**: `webgpu.rs:536`에서 `RawWindowHandle::Win32`를 받아 `GetClientRect`로 실제 픽셀 크기 조회.
- **통합 타이틀바 버튼**: `render/window_buttons.rs`에 `Style::Windows` 분기로 Windows 스타일 닫기/최소화/최대화 버튼 폴리곤 정의.
- **`unix_socket_path`**: `async_run_terminal_gui`(`main.rs:412`)는 GUI mux 서버 소켓을 `libc::getpid()` 기반으로 생성한다. 명칭은 unix 소켓이지만 Windows에서도 named pipe 형태로 동작하는 경로다.
- **죽은 코드**: 소스 트리에 `cfg(unix)`/`cfg(target_os=...)` 분기는 검색되지 않았다(이미 제거됨). 다만 `libc::getpid` 등 POSIX 명칭 호출이 잔존하며, `selection.rs`의 일부 unix 셀렉션(PrimarySelection) 개념은 Windows에서 사실상 비활성 경로다. `Opt::attach_parent_console`에는 `#[allow(dead_code)]`가 붙어 있다.

---

## 9. 리팩토링 주의점

1. **`termwindow::TermWindow` 거대 구조체**: 60개 이상 필드를 가진 단일 구조체가 윈도우 상태 전체를 보유한다. `mod.rs` 3,616행은 렌더·입력·이벤트·캐시 관리를 한 `impl`로 모은다. 필드 추가/삭제 시 `new_window`의 초기화 블록(`mod.rs:680`)을 반드시 동기화해야 한다. 책임 분리(상태/렌더/입력 분할)가 최우선 부채.
2. **세 generation 카운터 불변식**: `quad_generation`/`shape_generation`/`config_generation`이 캐시 키에 박혀 있다. 새 캐시를 추가하거나 무효화 조건을 바꿀 때 이 카운터 증가 지점(`focus_changed`, `config_was_reloaded`, 셰이프 캐시 무효화 등)을 누락하면 **스테일 렌더**가 발생한다.
3. **GPU 백엔드 이중화**: 모든 GPU 자원 경로가 `RenderContext`/`RenderFrame`의 Glium/WebGpu 분기를 통과한다. 한쪽만 수정하면 다른 백엔드에서 정점 레이아웃·셰이더 유니폼 불일치가 생긴다. `quad.rs`의 `Vertex`는 두 백엔드의 단일 진실 원천이므로 변경 시 셰이더(`window` 크레이트 측)와 동시 검토 필요.
4. **`box_model.rs`의 `#![allow(dead_code)]`**: 박스 모델은 CSS 유사 기능(Float 등)을 일부만 사용한다. 미사용 변형이 존재하므로 정리 여지가 있으나, 팬시 탭바·팔레트·선택기·문자 선택기가 모두 의존하므로 변경 파급이 크다.
5. **오버레이↔TermWindow 결합**: 오버레이는 별도 스레드(`spawn_into_new_thread`)에서 `TermWizTerminal`을 구동하고, 완료 시 `schedule_cancel_overlay`로 윈도우에 통지한다. 동기화는 `TermWindowNotif::Apply`와 페인 ID 매칭에 의존한다. 페인 생명주기(닫힌 페인에 대한 Apply)의 안전성에 주의.
6. **`main.rs`의 인스턴스 위임 로직**: `Publish`/`try_spawn`은 실행 파일 경로·설정 파일 경로가 일치할 때만 기존 GUI에 위임한다. 디스커버리는 `wezterm-client`에 의존하므로 소켓 명칭 규칙(`get_window_class()`) 변경 시 양쪽 동기화 필요.
7. **`unicode_names.rs`(생성물)**: 절대 수동 편집 금지. `ucd-generate names`로 생성된 `NAMES: &[(&str, u32)]` 테이블이며, 유니코드 16.0.0 버전이다. 유일한 소비처는 `charselect.rs:206`(문자 선택기에서 코드포인트↔이름 매칭). 유니코드 버전 갱신은 도구 재실행으로 처리.
8. **순환 의존 위험**: 모듈 간 `crate::termwindow::{...}` 재노출과 `box_model`/`render`/`palette`/`charselect` 상호 참조가 조밀하다. 모듈 분할 시 `mod.rs`가 사실상 facade 역할을 하므로 재노출 경로를 유지하지 않으면 광범위한 컴파일 오류가 발생한다.
9. **스레드 모델**: `front_end()`/`TermWindow`는 GUI 메인 스레드 전용(`Rc`/`RefCell` 사용). mux 알림은 다른 스레드에서 오므로 반드시 `spawn_into_main_thread`로 진입해야 한다. 이 경계를 넘는 코드 추가 시 패닉(`expect("to be called on gui thread")`) 위험.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `main.rs` | 1,278 | 진입점·CLI 서브커맨드·mux 초기화·인스턴스 위임(`Publish`)·`ls-fonts`/`show-keys` |
| `frontend.rs` | 549 | `GuiFrontEnd` 싱글톤·mux 알림 구독·워크스페이스↔윈도우 조정·토스트 게이팅 |
| `commands.rs` | 1,948 | `CommandDef`/`ExpandedCommand`·기본 키 어사인먼트·메뉴바·한국어 현지화 |
| `inputmap.rs` | 811 | 키/마우스 바인딩 해석(`InputMap`)·리더·기본 바인딩 병합 |
| `customglyph.rs` | 6,040 | 박스드로잉·파워라인·진행바·git 브랜치 글리프 폴리곤 래스터화(tiny-skia) |
| `glyphcache.rs` | 1,376 | 글리프·이미지·블록 텍스처 아틀라스 캐시·애니메이션 프레임 |
| `renderstate.rs` | 786 | `RenderContext`/`RenderFrame`(Glium/WebGpu)·버퍼·아틀라스 할당 |
| `shapecache.rs` | 698 | 셰이핑 결과 캐시 키/값(`ShapeCacheKey`, `ShapedInfo`) |
| `quad.rs` | 406 | 정점 포맷(`Vertex`)·쿼드 할당자·GPU 렌더 모드 상수 |
| `utilsprites.rs` | 169 | 밑줄/취소선 유틸 스프라이트·`RenderMetrics` |
| `uniforms.rs` | 57 | 셰이더 유니폼 빌더(glium) |
| `colorease.rs` | 142 | 블링크/비주얼벨 색 이징 |
| `selection.rs` | 357 | 텍스트 선택 좌표·범위 모델(`Selection`) |
| `scrollbar.rs` | 70 | 스크롤바 썸 위치/크기 계산(`ScrollHit`) |
| `tabbar.rs` | 728 | `TabBarState`/`TabBarItem`·`format-tab-title` Lua 콜백 |
| `stats.rs` | 438 | metrics `Recorder` 구현·히스토그램·처리량 통계 |
| `update.rs` | 235 | GitHub 릴리스 확인·업데이트 배너 |
| `download.rs` | 74 | 원격 다운로드 파일명 무해화·다운로드 폴더 저장 |
| `spawn.rs` | 155 | 탭/팬/윈도우 스폰(`SpawnWhere`, `spawn_command_impl`) |
| `resize_increment_calculator.rs` | 29 | 셀 단위 리사이즈 증분 계산 |
| `unicode_names.rs` | 140,378 | [생성] `ucd-generate names`로 생성된 유니코드 이름 테이블(16.0.0). 소비처: `charselect.rs` |
| `scripting/mod.rs` | 102 | `wezterm.gui.*` Lua 함수 등록 |
| `scripting/guiwin.rs` | 339 | `GuiWin` Lua `UserData`(윈도우 조작 메서드) |
| `overlay/mod.rs` | 91 | 오버레이 부착 진입점(`start_overlay`/`start_overlay_pane`)·재노출 |
| `overlay/copy.rs` | 2,021 | 복사 모드·검색 오버레이(점프·정규식 검색·선택) |
| `overlay/quickselect.rs` | 983 | 퀵 셀렉트(정규식 패턴 14종으로 URL/경로/해시 등 추출) |
| `overlay/launcher.rs` | 678 | 런처 메뉴(도메인 스폰·워크스페이스·셸 선택) |
| `overlay/selector.rs` | 446 | 입력 선택기(`InputSelector`)·퍼지 매칭 헬퍼 |
| `overlay/confirm.rs` | 201 | 일반 확인 프롬프트 오버레이 |
| `overlay/confirm_close_pane.rs` | 89 | 페인/탭/윈도우 닫기·프로그램 종료 확인 |
| `overlay/prompt.rs` | 107 | 라인 입력 프롬프트(`PromptInputLine`) |
| `overlay/debug.rs` | 282 | 디버그 오버레이(로그·상태 표시) |
| `termwindow/mod.rs` | 3,616 | `TermWindow` 구조체·이벤트 디스패치·Lua 이벤트·오버레이 부착·캐시 관리 |
| `termwindow/box_model.rs` | 1,233 | CSS 유사 박스 모델 레이아웃 엔진(팬시 UI 공통) |
| `termwindow/keyevent.rs` | 871 | 키 입력·키 테이블 스택·리더·데드 키 |
| `termwindow/mouseevent.rs` | 1,043 | 마우스 입력·UI 히트 테스트·드래그·캡처 |
| `termwindow/resize.rs` | 554 | 윈도우↔셀 치수 변환·DPI/스케일 변경 |
| `termwindow/charselect.rs` | 754 | 문자/이모지 선택기(`CharSelector`) |
| `termwindow/palette.rs` | 703 | 명령 팔레트(`CommandPalette`) |
| `termwindow/paneselect.rs` | 295 | 페인 선택/스왑 모달 |
| `termwindow/background.rs` | 588 | 윈도우 배경 이미지/그라디언트 로딩·캐시 |
| `termwindow/webgpu.rs` | 557 | WebGPU 상태·텍스처·유니폼 |
| `termwindow/clipboard.rs` | 60 | 복사/붙여넣기 클립보드 연동 |
| `termwindow/selection.rs` | 279 | 페인 내 텍스트 선택 동작 |
| `termwindow/spawn.rs` | 36 | 탭/팬 스폰 진입(termwindow 측) |
| `termwindow/modal.rs` | 29 | `Modal` 트레이트 정의 |
| `termwindow/prevcursor.rs` | 35 | 커서 블링크 위상 추적 |
| `termwindow/render/mod.rs` | 967 | 렌더 파이프라인 자료형·라인→요소 캐시·서브모듈 통합 |
| `termwindow/render/screen_line.rs` | 901 | 한 줄을 정점 버퍼로 렌더(다중 셀 글리프·커서·하이퍼링크) |
| `termwindow/render/pane.rs` | 689 | 박스 모델 기반 페인 렌더 |
| `termwindow/render/fancy_tab_bar.rs` | 532 | 팬시 탭바(이미지·둥근 모서리) |
| `termwindow/render/window_buttons.rs` | 353 | 통합 타이틀바 버튼(Windows 스타일 포함) |
| `termwindow/render/paint.rs` | 279 | `paint_impl`·FPS·이미지 정책·아틀라스 고갈 재시도 |
| `termwindow/render/draw.rs` | 274 | `call_draw`(Glium/WebGPU 드로콜) |
| `termwindow/render/borders.rs` | 148 | 윈도우 보더 렌더 |
| `termwindow/render/tab_bar.rs` | 119 | 기본(레거시) 탭바 렌더 |
| `termwindow/render/split.rs` | 81 | 스플릿 분할선 렌더 |
| `termwindow/render/corners.rs` | 37 | 둥근 모서리 폴리곤 상수 |
| `build.rs` | 153 | [Windows] DLL 복사·리소스 컴파일·버전 임베드 |
