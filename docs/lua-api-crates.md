# lua-api-crates 폴더 기능 명세

> 본 문서는 WezTerm Windows 전용 영구 분기의 `lua-api-crates` 폴더에 대한 리팩토링용 상세 기능 명세다. 코드 인용은 `파일경로:라인` 형식을 따른다. upstream 동기화는 포기되었고 비Windows 플랫폼 코드는 대부분 죽은 경로다.

## 1. 개요 및 책임

`lua-api-crates`는 **WezTerm의 Lua 설정·자동화 API 표면을 구현하는 크레이트 모음**이다. 사용자가 `wezterm.lua`(또는 RUNE의 선호 방식인 `config` 소스 기본값) 내에서 호출하는 `wezterm.*` 전역 모듈과 그 하위 모듈(`wezterm.mux`, `wezterm.color`, `wezterm.serde`, `wezterm.time`, `wezterm.url`, `wezterm.gui`, `wezterm.plugin`, `wezterm.procinfo`)을 Rust 측에서 정의한다.

이 폴더의 단일 책임은 **Rust 런타임 기능을 Lua 값/함수/UserData로 노출하는 바인딩 계층**이다. 폴더 자체는 크레이트가 아니라 15개 서브크레이트를 담는 디렉터리이며, 각 서브크레이트는 도메인별로 분리된 API 묶음(배터리, 색상, 파일시스템, 로깅, mux, 플러그인, 프로세스 정보, 직렬화, 전역 공유 데이터, 프로세스 spawn, SSH, 터미널 텍스트, 시간, URL, 윈도우)을 담당한다.

각 크레이트는 공통적으로 `pub fn register(lua: &Lua) -> anyhow::Result<()>` 단일 진입점을 노출한다. 이 함수는 `config::lua::get_or_create_module`/`get_or_create_sub_module`로 Lua 테이블을 확보한 뒤, 그 안에 Rust 클로저(`lua.create_function` / `lua.create_async_function`)나 UserData를 등록한다. 비즈니스 로직은 하위 크레이트(`mux`, `procinfo`, `window`, `termwiz`, `config` 등)에 위임하고, 이 계층은 Lua ↔ Rust 타입 변환과 오류 매핑만 수행한다.

Windows fork에서의 실제 책임은 동일하다. 다만 일부 크레이트(`battery`, `ssh-funcs`, `procinfo-funcs`)는 `libc` 또는 크로스플랫폼 추상화를 호출하는 코드가 그대로 남아 있고, 그 중 `unsafe { libc::getpid() }` 호출(`mux/src/*.rs`)이나 SSH config 파일 탐색 경로는 Windows에서도 실제로 사용된다.

## 2. 워크스페이스 내 위치

| 크레이트 (디렉터리) | 의존(deps) | 피의존(usedBy) |
|---|---|---|
| `battery` | config, luahelper, wezterm-dynamic, (starship-battery, anyhow) | env-bootstrap |
| `color-funcs` | config, luahelper, wezterm-dynamic, wezterm-term, (csscolorparser, deltae, image, lru, plist, serde) | env-bootstrap |
| `filesystem` | config, (filenamegen, smol) | env-bootstrap |
| `logging` | config, luahelper, (log) | env-bootstrap |
| `mux-lua` (`mux/`) | config, luahelper, mux, portable-pty, termwiz, termwiz-funcs, url-funcs, wezterm-dynamic, wezterm-term, (parking_lot, libc) | env-bootstrap, wezterm-gui |
| `plugin` | config, luahelper, wezterm-dynamic, (git2, tempfile) | env-bootstrap |
| `procinfo-funcs` | config, procinfo, (libc) | env-bootstrap |
| `serde-funcs` | config, luahelper, wezterm-dynamic, (serde_json, serde_yaml, toml) | env-bootstrap |
| `share-data` | config, (lazy_static, ordered-float) | env-bootstrap |
| `spawn-funcs` | config, wezterm-open-url, (bstr, smol, winapi[windows]) | env-bootstrap |
| `ssh-funcs` | config, wezterm-ssh | env-bootstrap |
| `termwiz-funcs` | config, luahelper, termwiz, wezterm-dynamic, wezterm-input-types, (finl_unicode, terminfo) | env-bootstrap, mux, mux-lua, wezterm, wezterm-gui |
| `time-funcs` | config, promise, (chrono, smol, spa) | env-bootstrap |
| `url-funcs` | config, (percent-encoding, url) | env-bootstrap, mux-lua, wezterm-gui |
| `window-funcs` | config, luahelper, wezterm-dynamic, window | wezterm-gui |

파이프라인 상 위치: 이 크레이트들은 **Lua 인터프리터 위, 기능 크레이트 아래**에 놓인 어댑터 계층이다. 거의 모든 크레이트가 `env-bootstrap`의 `register_lua_modules`(`env-bootstrap/src/lib.rs:136`)에 의해 등록 함수가 수집되어 Lua 컨텍스트 생성 시점에 호출된다. 예외적으로 `window-funcs`는 `env-bootstrap`이 아니라 `wezterm-gui`에서 직접 등록되며(GUI 연결이 전제), `termwiz-funcs`/`url-funcs`는 다른 lua-api 크레이트와 GUI/CLI에서 라이브러리로 재사용된다.

## 3. 공개 API 표면

### 3.1 공통 등록 진입점

모든 크레이트는 `pub fn register(lua: &Lua) -> anyhow::Result<()>`를 노출한다. 이것이 외부(주로 `env-bootstrap`, `wezterm-gui`)에서 호출하는 유일한 필수 심볼이다.

### 3.2 라이브러리로 재사용되는 공개 타입·함수

`register` 외에 다른 크레이트가 직접 import하는 공개 항목:

- `termwiz-funcs`
  - `pub fn format_as_escapes(items: Vec<FormatItem>) -> anyhow::Result<String>` (`termwiz-funcs/src/lib.rs:137`)
  - `pub fn lines_to_escapes(lines: Vec<Line>) -> anyhow::Result<String>` (`termwiz-funcs/src/lib.rs:268`) — `mux-lua`(`pane.rs`)와 `wezterm`(`cli/get_text.rs`)에서 사용.
  - `pub fn new_wezterm_terminfo_renderer() -> TerminfoRenderer` (`termwiz-funcs/src/lib.rs:264`)
  - `pub fn pad_right/pad_left/truncate_left/truncate_right` (`termwiz-funcs/src/lib.rs:150~199`)
  - `pub enum FormatItem`, `pub enum FormatColor` (`termwiz-funcs/src/lib.rs:68,96`)
- `url-funcs`
  - `pub struct Url { pub url: url::Url }` (`url-funcs/src/lib.rs:22`) — `mux-lua`(`pane.rs`)가 `get_current_working_dir` 반환 타입으로 사용.
- `mux-lua`
  - `pub use domain::MuxDomain; pub use pane::MuxPane; pub use tab::MuxTab; pub use window::MuxWindow;` (`mux/src/lib.rs:22~25`) — `wezterm-gui`가 GUI 측 객체를 mux 객체로 변환할 때 사용.
- `window-funcs`
  - `pub struct ScreenInfo`, `pub struct Screens` (`window-funcs/src/lib.rs:16,29`)
- `color-funcs`
  - `pub struct ColorWrap` (`color-funcs/src/lib.rs:11`), `pub mod schemes` (`color-funcs/src/lib.rs:8`)

### 3.3 Lua에 노출되는 모듈·함수 (소비 관점)

각 크레이트가 `register`에서 설치하는 Lua API의 요약. 별도 표기가 없으면 `wezterm.<함수>` 또는 `wezterm.<모듈>.<함수>`다.

#### battery
- `wezterm.battery_info() -> {BatteryInfo,...}`: 시스템 배터리 목록 조회. `BatteryInfo`는 충전율/벤더/모델/상태/잔여시간 필드를 가진다(`battery/src/lib.rs:12`).

#### color-funcs
- `wezterm.color.parse(spec)`, `from_hsla(h,s,l,a)`: 문자열/HSLA → `ColorWrap` UserData.
- `wezterm.color.extract_colors_from_image(file, params?)`: 이미지에서 대표 색 추출.
- `wezterm.color.get_default_colors()`, `get_builtin_schemes()`(=`wezterm.get_builtin_color_schemes()`).
- `wezterm.color.load_scheme/save_scheme/load_terminal_sexy_scheme/load_base16_scheme`: 색 스킴 파일 입출력.
- `wezterm.color.gradient(...)`(=`wezterm.gradient_colors(...)`): 그라디언트 색 생성.
- `ColorWrap` 메서드: `complement`, `triad`, `square`, `saturate`/`desaturate`(±`_fixed`), `lighten`/`darken`(±`_fixed`), `adjust_hue_fixed(_ryb)`, `srgba_u8`, `linear_rgba`, `hsla`, `laba`, `contrast_ratio`, `delta_e` (`color-funcs/src/lib.rs:48~106`).

#### filesystem
- `wezterm.read_dir(path)` (async): 디렉터리 항목 UTF-8 경로 목록.
- `wezterm.glob(pattern, path?)` (async): glob 패턴 매칭(blocking을 `smol::unblock`로 오프로드).

#### logging
- `wezterm.log_error/log_info/log_warn(...)`: 가변 인자를 `ValuePrinter`로 포매팅해 `log` 매크로로 전달.
- `wezterm.to_string(value)`: 임의 Lua 값을 디버그 문자열로.
- 전역 `print(...)`를 `log::info!`로 재정의(`logging/src/lib.rs:41`).

#### mux-lua (`wezterm.mux`)
- 워크스페이스: `get_active_workspace`, `get_workspace_names`, `set_active_workspace`, `rename_workspace`.
- 객체 조회: `get_window(id)`, `get_pane(id)`, `get_tab(id)`, `get_domain(nil|name|id)`, `all_windows`, `all_domains`.
- 생성·기본값: `spawn_window(SpawnWindow)` (async), `set_default_domain(domain)`.
- 반환 UserData 4종(`MuxWindow`/`MuxTab`/`MuxPane`/`MuxDomain`)의 메서드는 4절에서 상술.

#### plugin (`wezterm.plugin`)
- `require(url)`: 플러그인 git repo를 `config::DATA_DIR/plugins`에 클론(없으면)한 뒤 Lua `require`로 로드.
- `list()`: 설치된 플러그인 `RepoSpec` 목록.
- `update_all()`: 모든 플러그인을 git fetch/fast-forward/merge.

#### procinfo-funcs (`wezterm.procinfo`)
- `pid()`, `get_info_for_pid(pid)`, `current_working_dir_for_pid(pid)`, `executable_path_for_pid(pid)`.

#### serde-funcs (`wezterm.serde`)
- 디코더: `json_decode`, `yaml_decode`, `toml_decode`.
- 인코더: `json_encode`, `yaml_encode`, `toml_encode`, `json_encode_pretty`, `toml_encode_pretty`.
- 하위 호환: `wezterm.json_parse`(=json_decode), `wezterm.json_encode`.

#### share-data
- `wezterm.GLOBAL`: 프로세스 전역 공유 객체(UserData). 인덱싱·할당·`pairs`·`#` 연산 지원(4절).

#### spawn-funcs
- `wezterm.open_with(url, app?)`: 외부 앱/기본 핸들러로 열기.
- `wezterm.run_child_process(args)` (async): 자식 프로세스 실행, `(success, stdout, stderr)` 반환.
- `wezterm.background_child_process(args)` (async): stdin 없이 백그라운드 실행.

#### ssh-funcs
- `wezterm.enumerate_ssh_hosts(files...)`: ssh config 파싱 → `{host -> ConfigMap}`. 파싱한 파일을 config 리로드 감시 목록에 추가.
- `wezterm.default_ssh_domains()`: `config::SshDomain::default_domains()`.

#### termwiz-funcs
- `wezterm.format(items)`: `FormatItem` 배열을 ANSI 이스케이프 문자열로 렌더.
- `wezterm.nerdfonts`: NerdFont 글리프를 `nerdfonts["name"]`로 조회하는 UserData.
- `wezterm.column_width(s)`, `pad_right/pad_left`, `truncate_right/truncate_left`.
- `wezterm.permute_any_mods(item)`, `permute_any_or_no_mods(item)`: 수정자(Ctrl/Shift/Alt/Super) 조합 순열 생성.

#### time-funcs (`wezterm.time`)
- `now()`, `parse_rfc3339(s)`, `parse(s, fmt)`: `Time` UserData 생성.
- `call_after(seconds, fn)`: 지연 콜백 스케줄(config 세대 추적으로 중복 실행 방지).
- 하위 호환: `wezterm.sleep_ms(ms)` (async), `wezterm.strftime(fmt)`, `wezterm.strftime_utc(fmt)`.
- `Time` 메서드: `format`, `format_utc`, `sun_times(lat, lon)`.

#### url-funcs (`wezterm.url`)
- `parse(s) -> Url`: URL 파싱. `Url` 필드: `scheme/username/password/host/port/query/fragment/path/file_path`.

#### window-funcs (`wezterm.gui`)
- `screens()`: 디스플레이 배치(`Screens`) 조회. GUI 스레드에서만 동작.
- `get_appearance()`: 라이트/다크 외형. GUI 미기동 시 `"Light"` 가정.

## 4. 내부 구조

### 4.1 등록 메커니즘 (폴더 전체 공통)

1. `env-bootstrap::register_lua_modules`(`env-bootstrap/src/lib.rs:136`)가 14개 크레이트의 `register` 함수를 배열로 모아 각각 `config::lua::add_context_setup_func`로 전역 `SETUP_FUNCS` 목록에 push한다. `window-funcs`는 이 목록에 없고 `wezterm-gui`가 별도 등록한다.
2. Lua 컨텍스트가 생성될 때 등록된 setup 함수들이 순차 호출되며, 각 `register`는 `get_or_create_module(lua, "wezterm")`(`config/src/lua.rs:33`)로 `package.loaded.wezterm` 테이블을, 또는 `get_or_create_sub_module(lua, name)`(`config/src/lua.rs:55`)로 `wezterm.<name>` 하위 테이블을 멱등하게 얻는다.
3. 각 함수/UserData는 `Lua` 클로저로 등록된다. 동기 함수는 `create_function`, I/O 비차단 함수는 `create_async_function`을 쓴다.

이 구조 덕분에 어느 크레이트든 `wezterm` 또는 그 하위 테이블에 안전하게 키를 추가할 수 있다(이미 존재하면 재사용, 타입 충돌 시 명시적 오류).

### 4.2 크레이트별 제어·데이터 흐름

- **단순 어댑터형** (`battery`, `filesystem`, `logging`, `procinfo-funcs`, `spawn-funcs`, `ssh-funcs`, `url-funcs`, `window-funcs`): `register` 한 함수에 모든 로직이 모여 있다. Lua 인자 → Rust 호출 → 결과/오류 매핑의 단방향 흐름.
- **양방향 변환형** (`serde-funcs`, `share-data`): Lua 값 ↔ 중간 표현(serde_json `Value` 또는 자체 `Value` enum) 재귀 변환이 핵심. 순환 테이블 방지를 위해 방문 포인터 집합(`HashSet<usize>`)을 사용한다(`serde-funcs/src/lib.rs:148`, `share-data/src/lib.rs:97`).
- **UserData 중심형** (`color-funcs`, `time-funcs`, `mux-lua`): Rust struct를 Lua UserData로 노출하고 메서드를 `add_method`/`add_async_method`로 단다.
- **상태 보유형** (`time-funcs`, `color-funcs::image_colors`, `share-data`): `lazy_static`로 전역 상태를 둔다. `time-funcs`는 config 리로드 구독(`CONFIG_SUBSCRIPTION`)과 Lua registry(`SCHEDULED_EVENTS`)를, `image_colors`는 LRU 캐시(`IMG_COLOR_CACHE`)를, `share-data`는 프로세스 전역 `GLOBALS`를 유지한다.

### 4.3 비대한 모듈

- **`mux/src/pane.rs` (497행)** — 폴더 내 최대 단일 파일. `MuxPane`에 약 30개 메서드(텍스트 추출, 시맨틱 존, split, activate, 프로세스 정보 등)가 집중. `get_text_from_semantic_zone`의 줄/열 범위 계산 로직(`mux/src/pane.rs:30~84`)이 가장 복잡하다.
- **`share-data/src/lib.rs` (476행)** — 자체 `Value` enum과 `Object`/`Array`(둘 다 `Arc<Mutex<...>>` 래퍼)에 대한 `Ord/PartialEq/Hash` 수동 구현, 그리고 `Index/NewIndex/Pairs/Len` 메타메서드 구현이 길다.
- **`serde-funcs/src/lib.rs` (366행)** — 다수의 인/디코더 + Lua↔json 양방향 변환 + 3개 라운드트립 테스트.

## 5. 핵심 데이터 구조·타입

| 타입 (위치) | 역할 / 불변식 |
|---|---|
| `MuxWindow(WindowId)`, `MuxTab(TabId)`, `MuxPane(PaneId)`, `MuxDomain(DomainId)` (`mux/src/*.rs`) | mux 객체의 **ID 핸들**. 값 자체는 ID만 보유(Copy)하고, 매 메서드 호출마다 `resolve(&mux)`로 실제 객체를 lookup한다. 불변식: 핸들은 stale 가능하며, resolve 실패 시 "not found" 오류. `MuxWindow`는 읽기/쓰기 가드(`resolve`/`resolve_mut`)를 구분한다(`mux/src/window.rs:8,16`). |
| `share-data::Value` enum (`share-data/src/lib.rs:81`) | `wezterm.GLOBAL`의 내부 표현. `Object`/`Array`는 `Arc<Mutex<>>` 공유 가변 컨테이너. 불변식: 동일 객체를 여러 Lua 참조가 공유하며, `Hash`/`Ord`는 lock 후 내용 기반(`Object`/`Array`는 포인터 기반 `Ord`). NewIndex는 희소 배열 생성을 금지(`share-data/src/lib.rs:426`). |
| `ColorWrap(RgbaColor)` (`color-funcs/src/lib.rs:11`) | 색 연산 UserData. 모든 변환 메서드는 새 `ColorWrap`을 반환(불변). |
| `ExtractColorParams` (`color-funcs/src/image_colors.rs:29`) | 이미지 색 추출 파라미터. `f32` 필드를 `to_ne_bytes`로 해시해 LRU 캐시 키로 사용(`Hash`/`Eq` 수동 구현). 불변식: 캐시 적중은 파일 수정시각(`modified`)까지 일치해야 유효(`image_colors.rs:197`). |
| `Time { utc: DateTime<Utc> }` (`time-funcs/src/lib.rs:199`) | 항상 UTC로 정규화 보관. 로컬 표현은 출력 시 변환. |
| `ScheduledEvent { user_event_id, interval_seconds }` (`time-funcs/src/lib.rs:60`) | `call_after` 타이머 상태. 불변식: schedule 시점의 config `generation`을 캡처해, 콜백 실행 시 세대가 다르면 실행을 건너뛴다(리로드 시 콜백 지수증식 방지, `time-funcs/src/lib.rs:82~106`). |
| `RepoSpec { url, component, plugin_dir }` (`plugin/src/lib.rs:11`) | 플러그인 repo 식별. `component`는 URL을 파일시스템 안전 문자열로 인코딩(`compute_repo_dir`, `plugin/src/lib.rs:20`)한 단일 경로 컴포넌트. 불변식: `.`으로 시작하면 거부. |
| `FormatItem` / `FormatColor` (`termwiz-funcs/src/lib.rs:96,68`) | `wezterm.format`의 입력 DSL. `FormatItem` → termwiz `Change`로 변환되어 terminfo 렌더러에 투입. |
| `Url { pub url: url::Url }` (`url-funcs/src/lib.rs:22`) | `url::Url`의 얇은 newtype(`Deref`). `file_path` 필드는 Windows 드라이브 문자(`C:`) 뒤에 슬래시를 보정한다(`url-funcs/src/lib.rs:69`). |
| `Screens` / `ScreenInfo` (`window-funcs/src/lib.rs:16,29`) | 디스플레이 배치를 `window::screen::*`에서 Lua 친화 평면 구조로 변환. |
| `SpawnWindow` / `SpawnTab` / `SplitPane` / `CommandBuilderFrag` (`mux/src/lib.rs`, `pane.rs`) | spawn/split 요청 DSL. `CommandBuilderFrag`는 `#[dynamic(flatten)]`로 상위 구조에 인라인되어 `args`/`cwd`/`set_environment_variables`를 공통 수용. |

## 6. 외부 의존성

| 크레이트 | 사용 크레이트 | 사용 이유 |
|---|---|---|
| `starship-battery` | battery | OS 배터리 정보 조회(상태/충전율/잔여시간). |
| `csscolorparser`(lab feature), `deltae`, `image`, `lru` | color-funcs | CSS 색 파싱, LAB 색공간 DeltaE 지각 거리, 이미지 디코드/리사이즈, 추출 결과 LRU 캐시. |
| `plist` | color-funcs | iTerm2 색 스킴(plist 형식) 파싱. |
| `serde_json`/`serde_yaml`/`plist` | color-funcs, serde-funcs | terminal.sexy(JSON), base16(YAML), iTerm2(plist) 및 serde 인/디코더. |
| `filenamegen` | filesystem | glob 패턴 워킹. blocking이므로 `smol::unblock`로 오프로드. |
| `smol` | filesystem, spawn-funcs, time-funcs | async 파일/프로세스 I/O 및 타이머(`smol::Timer`). |
| `git2` | plugin | 플러그인 repo clone/fetch/merge(libgit2 바인딩). |
| `tempfile` | plugin | clone을 임시 디렉터리에 받은 뒤 원자적 rename. |
| `procinfo` | procinfo-funcs | PID 기반 프로세스 정보(cwd, 실행 경로 등). |
| `ordered-float` | share-data | `f64`를 `Eq`/`Hash` 가능하게 래핑(`Value::F64`). |
| `wezterm-open-url` | spawn-funcs | URL/파일을 기본 또는 지정 앱으로 열기. |
| `winapi`(winuser) | spawn-funcs | Windows에서 자식 프로세스에 `CREATE_NO_WINDOW` 부여(콘솔 창 깜빡임 방지, `spawn-funcs/src/lib.rs:41`). |
| `wezterm-ssh` | ssh-funcs | ssh config 파싱·호스트 열거. |
| `termwiz`, `terminfo`, `finl_unicode` | termwiz-funcs | 셀/속성/렌더링(`format`), terminfo DB(`xterm-256color` 내장), 그래핌 분할(폭 계산·truncate). |
| `chrono`, `spa` | time-funcs | 날짜/시간 파싱·포매팅, 일출·일몰(태양 위치) 계산. |
| `promise` | time-funcs | 메인 스레드 스케줄러(`spawn_into_main_thread`)로 `!Send` 콜백 실행. |
| `url`, `percent-encoding` | url-funcs | URL 파싱 및 path segment percent-decode. |
| `portable-pty` | mux-lua | spawn/split용 `CommandBuilder` 구성. |
| `parking_lot` | mux-lua | mux Window의 매핑 RwLock 가드 타입(`MappedRwLockReadGuard` 등). |
| `libc` | mux-lua, procinfo-funcs | `getpid()`(객체 `tostring` 표시·`procinfo.pid`). |
| `wezterm-dynamic`, `luahelper` | 다수 | `FromDynamic`/`ToDynamic` 파생 및 Lua↔dynamic 변환 헬퍼(`impl_lua_conversion_dynamic!`, `to_lua`/`from_lua`/`dynamic_to_lua_value`). |

## 7. 설정·기능 플래그

- 명시적 cargo feature를 정의하는 크레이트는 없다. 대신 의존성에 feature를 켜서 쓴다:
  - `color-funcs`: `csscolorparser`의 `lab` feature(`color-funcs/Cargo.toml:12`), `wezterm-term`의 `use_serde`(`:24`).
  - `termwiz-funcs`: `termwiz`의 `use_serde`(`termwiz-funcs/Cargo.toml:16`).
  - `spawn-funcs`: `[target.'cfg(windows)']`에서만 `winapi`(`winuser`) 의존(`spawn-funcs/Cargo.toml:16`).
- 관련 config 항목:
  - `time-funcs`는 `config::subscribe_to_config_reload`로 리로드를 구독하고 `config::configuration().generation()`으로 콜백 유효성을 판정한다.
  - `ssh-funcs`는 파싱한 파일을 `config::lua::add_to_config_reload_watch_list`로 감시 목록에 추가(파일 변경 시 자동 리로드).
  - `mux-lua::SpawnWindow`는 `width/height` 미지정 시 `config::configuration().initial_size(...)`를 사용(`mux/src/lib.rs:239`).
  - `plugin`은 `config::DATA_DIR/plugins`를 클론 루트로 사용(`plugin/src/lib.rs:94`).
  - `color-funcs`는 `config::COLOR_SCHEMES`를 builtin 스킴 소스로 노출(`color-funcs/src/lib.rs:175`).

## 8. Windows 전용 고려사항

- **실제 Windows 코드**:
  - `spawn-funcs`: `#[cfg(windows)]` 블록에서 `CREATE_NO_WINDOW` 플래그로 콘솔 창 표시를 억제한다(`spawn-funcs/src/lib.rs:38,60`). 이 fork에서 유효한 유일한 명시적 플랫폼 분기다.
  - `url-funcs`: `file_path` getter가 Windows 드라이브 문자 경로(`C:` → `C:/`)를 보정한다(`url-funcs/src/lib.rs:69`).
- **Windows에서도 호출되지만 Unix 어휘를 쓰는 코드(잠재적 정리 대상)**:
  - `mux/src/{domain,window,tab,pane}.rs`의 `tostring` 메타메서드가 `unsafe { libc::getpid() }`를 호출한다. Windows에서도 `libc` 크레이트가 `getpid`를 제공하므로 동작하지만, 표시 문자열에 "pid"를 넣는 디버그 목적일 뿐이다. `MuxDomain`의 표시는 "MuxDomain(pane_id:...)"로 라벨이 잘못 붙어 있다(`mux/src/domain.rs:19`).
  - `procinfo-funcs::pid`도 `libc::getpid()` 사용(`procinfo-funcs/src/lib.rs:9`).
- **죽은/미사용 경로**:
  - `color-funcs/src/schemes/gogh.rs`의 `GoghTheme::load_all`은 워크스페이스 내 호출자가 없다(grep 결과 정의·선언만 존재). gogh 스킴 데이터는 별도 생성 단계에서 소비되었던 흔적으로, 현재는 사실상 dead code다. `mod gogh`는 `pub`로 노출되어 있으나 `register`에서 연결되지 않는다.
  - `iterm2.rs`는 `pub`이지만 `lib.rs::register`에서 직접 사용하지 않는다(`ITerm2::load_file`/`parse_str` 호출처 없음). base16·sexy만 Lua API(`load_base16_scheme`/`load_terminal_sexy_scheme`)에 연결된다.
- **GUI 전제**: `window-funcs`의 `screens()`/`get_appearance()`는 `window::Connection::get()`이 성공해야 동작하며, GUI 스레드 밖이면 오류 또는 라이트 외형 가정으로 폴백한다(`window-funcs/src/lib.rs:9,95`).

## 9. 리팩토링 주의점

- **stale 핸들 패턴 (mux-lua)**: 모든 mux UserData는 ID만 들고 매 호출마다 resolve한다. 호출 사이에 객체가 사라질 수 있으므로(닫힌 pane 등) 모든 메서드가 "not found" 오류를 던질 수 있다. 메서드를 추가할 때 resolve 실패 처리를 반드시 포함해야 한다.
- **약결합 호출 (mux → gui)**: `MuxWindow::gui_window`(`mux/src/window.rs:35`)는 mux가 `wezterm-gui`를 하드 의존할 수 없어, 런타임에 `wezterm.gui.gui_window_for_mux_window` Lua 함수를 동적으로 찾아 호출한다. 함수명/시그니처를 GUI 측과 동기화하지 않으면 런타임에만 깨진다.
- **버그/불일치**:
  - `MuxTab::rotate_clockwise`가 `tab.rotate_counter_clockwise()`를 호출한다(`mux/src/tab.rs:121`) — 시계방향이 반시계방향으로 잘못 매핑됨. 정정 시 동작이 바뀌므로 사용자 영향 검토 필요.
  - `MuxDomain` `tostring`이 "pane_id" 라벨을 쓴다(`mux/src/domain.rs:19`).
- **콜백 수명·세대 추적 (time-funcs)**: `ScheduledEvent::schedule`은 Lua 컨텍스트 수명을 타이머 만료까지 연장한다. config 세대 비교로 중복 실행은 막지만 큰 interval에서는 메모리를 더 점유한다(`time-funcs/src/lib.rs:82` 주석 참고). 스케줄링 로직 변경 시 지수증식 회귀에 주의.
- **전역 가변 상태 (share-data)**: `wezterm.GLOBAL`은 `lazy_static` + `Arc<Mutex<>>` 프로세스 전역이며 Lua 참조 간 가변 공유된다. lock 순서·재진입(예: NewIndex 안에서 다시 변환)에 주의. `Object`/`Array`의 `Ord`는 포인터 기반이라 컬렉션 정렬에 사용하면 비결정적이다.
- **순환 데이터 방어**: `serde-funcs`/`share-data`의 Lua→내부 변환은 방문 포인터 집합으로 순환을 Null 처리한다. 변환 로직 분리/리팩토링 시 이 방어를 유지해야 무한 재귀를 막을 수 있다.
- **캐시 무효화 (color-funcs)**: `extract_colors_from_image` 캐시 키는 `(file_name, params)`이며 적중 검증에 파일 `modified` 시각을 쓴다. 파라미터 구조 변경 시 `Hash`/`Eq` 수동 구현(`f32`의 `to_ne_bytes`)을 함께 갱신해야 한다.
- **순환 의존 없음**: 폴더 내부 의존은 `mux-lua → {termwiz-funcs, url-funcs}` 단방향뿐이다. `termwiz-funcs`/`url-funcs`는 다른 lua-api 크레이트나 GUI/CLI에 널리 재사용되므로, 공개 함수 시그니처 변경 시 `mux-lua`·`wezterm`·`wezterm-gui` 파급을 확인할 것.
- **register 멱등성 가정**: 모든 크레이트가 `get_or_create_(sub_)module`의 멱등성에 의존한다. 같은 키를 두 크레이트가 다른 타입으로 설정하면 등록이 실패한다(`config/src/lua.rs:46,68`).

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `battery/src/lib.rs` | 53 | `wezterm.battery_info` 및 `BatteryInfo` 정의. |
| `color-funcs/src/lib.rs` | 203 | `wezterm.color.*` 등록, `ColorWrap` UserData 색 연산. |
| `color-funcs/src/image_colors.rs` | 330 | 이미지 → LAB/DeltaE 기반 대표색 추출 + LRU 캐시. |
| `color-funcs/src/schemes/mod.rs` | 4 | 스킴 서브모듈 선언. |
| `color-funcs/src/schemes/base16.rs` | 91 | base16(YAML) 스킴 → `ColorSchemeFile`. |
| `color-funcs/src/schemes/sexy.rs` | 58 | terminal.sexy(JSON) 스킴 파서. |
| `color-funcs/src/schemes/iterm2.rs` | 153 | iTerm2(plist) 스킴 파서. `register` 미연결(미사용). |
| `color-funcs/src/schemes/gogh.rs` | 95 | gogh(JSON) 스킴 파서. 호출처 없음(dead code). |
| `filesystem/src/lib.rs` | 54 | `wezterm.read_dir`/`glob` (async). |
| `logging/src/lib.rs` | 75 | `wezterm.log_*`/`to_string` 및 전역 `print` 재정의. |
| `mux/src/lib.rs` | 344 | `wezterm.mux.*` 등록, spawn/split DSL 구조체, 4객체 재노출. |
| `mux/src/domain.rs` | 87 | `MuxDomain` UserData(attach/detach/state 등). |
| `mux/src/window.rs` | 108 | `MuxWindow` UserData(workspace/title/tabs/spawn_tab 등). |
| `mux/src/tab.rs` | 157 | `MuxTab` UserData(panes/split 방향/activate/rotate 등). |
| `mux/src/pane.rs` | 497 | `MuxPane` UserData(텍스트 추출, 시맨틱 존, split, 프로세스 정보 등). 최대 파일. |
| `plugin/src/lib.rs` | 273 | `wezterm.plugin.*`(require/list/update_all), git2 기반 repo 관리. |
| `procinfo-funcs/src/lib.rs` | 30 | `wezterm.procinfo.*`(pid/info/cwd/exe path). |
| `serde-funcs/src/lib.rs` | 366 | `wezterm.serde.*` 인/디코더, Lua↔json 변환, 라운드트립 테스트. |
| `share-data/src/lib.rs` | 476 | `wezterm.GLOBAL` 전역 공유 `Value` + 메타메서드. |
| `spawn-funcs/src/lib.rs` | 71 | `open_with`/`run_child_process`/`background_child_process` (Windows `CREATE_NO_WINDOW`). |
| `ssh-funcs/src/lib.rs` | 43 | `enumerate_ssh_hosts`/`default_ssh_domains`. |
| `termwiz-funcs/src/lib.rs` | 303 | `format`/`nerdfonts`/폭·패딩·truncate/수정자 순열, 재사용 렌더 헬퍼(내장 `xterm-256color` terminfo). |
| `time-funcs/src/lib.rs` | 276 | `wezterm.time.*` + 하위호환 시간 함수, `call_after` 스케줄러, `Time`/일출·일몰. |
| `url-funcs/src/lib.rs` | 83 | `wezterm.url.parse` 및 `Url` UserData(Windows 경로 보정). |
| `window-funcs/src/lib.rs` | 106 | `wezterm.gui.screens`/`get_appearance`, `Screens`/`ScreenInfo`. |

> 생성 파일: 이 폴더에는 거대 생성 데이터 테이블 파일이 없다(NerdFont/Unicode 등 생성물은 `termwiz`/`wezterm-char-props` 등 외부 크레이트에 위치하며, `termwiz-funcs`는 그 데이터를 `termwiz::nerdfonts::NERD_FONTS`로 참조만 한다).
