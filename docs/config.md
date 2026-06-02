# config 폴더 기능 명세

본 문서는 워크스페이스 폴더 `E:\Project\wezterm\config` 에 포함된 두 크레이트 `config`(`config/`)와 `wezterm-config-derive`(`config/derive/`)의 기능 명세이다. 추후 리팩토링을 위한 상세 참조용이며, 모든 인용은 실제 소스 코드를 근거로 한다.

이 저장소는 WezTerm의 Windows 전용 영구 분기이다. upstream 동기화는 포기되었고, X11/Wayland/macOS 등 비Windows 플랫폼 코드는 제거되었다. 설정은 `wezterm.lua`보다 `config` 크레이트의 소스 기본값 수정으로 동작을 고정하는 방식을 우선한다.

---

## 1. 개요 및 책임

`config` 크레이트는 WezTerm 전체의 **설정 모델·로딩·검증·재로딩의 단일 출처**다. 책임은 다음과 같이 요약된다.

- **설정 스키마 정의**: 사용자가 조정 가능한 모든 옵션을 단일 거대 구조체 `Config`(`config/src/config.rs:51`)로 정의한다. 약 250개 필드와 수십 개의 보조 struct/enum이 여기에 묶인다.
- **설정 로딩 파이프라인**: `wezterm.lua` 파일을 탐색·실행하고(`config/src/config.rs:973` `Config::load_with_overrides`), 그 Lua 반환값을 동적 값(`wezterm_dynamic::Value`)을 거쳐 `Config`로 역직렬화한다.
- **Lua 실행 환경 구성**: `wezterm` 모듈을 등록하고 사용자 스크립트에 노출할 함수·이벤트 시스템을 제공한다(`config/src/lua.rs:211` `make_lua_context`).
- **전역 설정 핸들 관리**: 프로세스 전역 싱글턴 `CONFIG`(`config/src/lib.rs:72`)을 통해 현재 설정을 제공하고, 파일 변경 감시(`notify`)에 따른 핫 리로드와 구독자 알림을 수행한다.
- **파생 데이터 계산**: 폰트 규칙 자동 확장, 색 구성표 해석, 배경 레이어 합성 등(`config/src/config.rs:1283` `compute_extra_defaults`).
- **색 구성표 데이터 제공**: 1001개의 번들 색 구성표(`config/src/scheme_data.rs`)를 파싱해 이름→`Palette` 맵을 구축한다.

`wezterm-config-derive` 크레이트는 `config`만이 의존하는 절차적 매크로(`proc-macro`)다. 단 하나의 derive 매크로 `ConfigMeta`를 제공하여(`config/derive/src/lib.rs:8`), 구조체 필드로부터 설정 메타데이터(이름·문서·기본값·컨테이너 종류)를 컴파일 타임에 추출한다. 명령 팔레트나 설정 자기소개(introspection) 기능을 위한 토대다.

Windows fork에서의 실제 책임 측면에서 중요한 점은, 이 크레이트의 **소스 기본값(`default_*` 함수)이 곧 RUNE의 설정 정책**이라는 것이다. 예컨대 기본 런처 메뉴는 cmd / PowerShell 7 / Windows PowerShell을 노출하도록 하드코딩되어 있고(`config/src/config.rs:904` `default_launch_menu`), 기본 폰트는 `JetBrains Mono`(`config/src/font.rs:428`), 기본 폰트 로케이터는 `Gdi`(`config/src/font.rs:662`)다. `wezterm.lua`가 없어도 이 기본값들이 동작을 결정한다.

## 2. 워크스페이스 내 위치

### `config` 크레이트

| 구분 | 크레이트 |
| --- | --- |
| 의존(deps) | luahelper, portable-pty, promise, termwiz, wezterm-bidi, wezterm-config-derive, wezterm-dynamic, wezterm-input-types, wezterm-ssh, wezterm-term |
| 피의존(usedBy) | battery, codec, color-funcs, env-bootstrap, filesystem, lfucache, logging, mux, mux-lua, plugin, procinfo-funcs, ratelim, serde-funcs, share-data, spawn-funcs, ssh-funcs, termwiz-funcs, time-funcs, url-funcs, wezterm, wezterm-client, wezterm-font, wezterm-gui, wezterm-gui-subcommands, wezterm-mux-server, wezterm-mux-server-impl, window, window-funcs |

### `wezterm-config-derive` 크레이트

| 구분 | 크레이트 |
| --- | --- |
| 의존(deps) | (없음 — proc-macro2, quote, syn 외부 크레이트만 사용) |
| 피의존(usedBy) | config |

`config`는 워크스페이스의 **최대 허브**다. 28개 크레이트가 직접 의존하며, GUI(`wezterm-gui`), 멀티플렉서(`mux`, `wezterm-mux-server`), 폰트(`wezterm-font`), Lua 노출 함수군(`*-funcs`)이 모두 이 크레이트의 `Config`·`ConfigHandle`·Lua 컨텍스트에 결합되어 있다. 계층적으로는 터미널 모델(`wezterm-term`)·입력 타입(`wezterm-input-types`)·동적 값(`wezterm-dynamic`) 위에 놓이며, 그 위의 모든 애플리케이션 계층이 이 크레이트를 통해 설정을 읽는다. `wezterm-config-derive`는 잎(leaf) 위치의 빌드 전용 보조 크레이트다.

## 3. 공개 API 표면

### 3.1 `config` 크레이트

크레이트 루트(`config/src/lib.rs`)는 거의 모든 하위 모듈을 `pub use ... ::*`로 평탄하게 재노출한다(`config/src/lib.rs:45-62`). 따라서 소비자는 `config::Config`, `config::Palette`, `config::KeyAssignment` 등 단일 네임스페이스로 접근한다. 명시적으로 `pub mod`로 노출되는 모듈은 `keyassignment`, `lua`, `meta`, `window` 네 개다.

핵심 공개 항목:

- **`Config`** (`config/src/config.rs:51`): 전체 설정 구조체. `FromDynamic + ToDynamic + ConfigMeta`. 주요 메서드:
  - `Config::load() -> LoadedConfig` / `load_with_overrides(&Value)` — 파일 탐색·로딩 진입점.
  - `Config::default_config() -> Self` — 파생 기본값까지 계산된 기본 설정.
  - `key_bindings(&self) -> KeyTables` / `mouse_bindings(&self)` — 키/마우스 바인딩 해석.
  - `compute_extra_defaults(&self, Option<&Path>) -> Self` — 파생 값 확장.
  - `build_prog(...)` / `apply_cmd_defaults(...)` — 자식 프로세스 커맨드 빌드(환경변수·`TERM` 주입).
  - `ssh_domains()` / `wsl_domains()` — 지연 계산되는 기본 도메인 목록.
- **`Configuration`** (`config/src/lib.rs:667`) 및 자유 함수군:
  - `configuration() -> ConfigHandle` (`config/src/lib.rs:426`) — 현재 설정 핸들 취득. 가장 빈번히 호출되는 진입점.
  - `reload()`, `use_default_configuration()`, `use_test_configuration()`, `use_this_configuration(Config)`.
  - `subscribe_to_config_reload(F) -> ConfigSubscription` (`config/src/lib.rs:241`) — 리로드 이벤트 구독.
  - `common_init(...)` (`config/src/lib.rs:342`) — CLI 인자(config 파일·override·skip)로 초기화.
  - `configuration_result()`, `configuration_warnings_and_errors()`, `show_error`, `assign_error_callback`.
- **`ConfigHandle`** (`config/src/lib.rs:764`): `Arc<Config>` + 세대 번호 래퍼. `Deref<Target=Config>`로 투명 접근. `generation()`로 변경 감지, `unicode_version()` 제공.
- **Lua 통합** (`config/src/lua.rs`): `make_lua_context(&Path) -> Lua`, `get_or_create_module`/`get_or_create_sub_module`, `register_event`/`emit_event`/`emit_sync_callback`/`emit_async_callback`, `wrap_callback`, `add_context_setup_func`(외부 크레이트가 `wezterm` 모듈을 확장하기 위한 훅).
- **메타데이터**(`config/src/meta.rs`): `ConfigMeta` 트레이트와 `ConfigOption`/`ConfigContainer` 타입.
- **버전 정보**(`config/src/version.rs`): `assign_version_info`, `wezterm_version`, `wezterm_target_triple`, `running_under_wsl`.
- **경로 전역**(`config/src/lib.rs:66-79`): `HOME_DIR`, `CONFIG_DIRS`, `RUNTIME_DIR`, `DATA_DIR`, `CACHE_DIR`, `COLOR_SCHEMES`(lazy_static).

### 3.2 `wezterm-config-derive` 크레이트

- `#[derive(ConfigMeta)]` (`config/derive/src/lib.rs:8`): `attributes(config)`를 받는 derive 매크로. 명명 필드를 가진 구조체에만 적용되며(그 외는 컴파일 에러, `config/derive/src/configmeta.rs:12`), `ConfigMeta::get_config_options()`를 생성한다.

## 4. 내부 구조

### 4.1 `config` 크레이트 모듈 분해

루트 `lib.rs`는 모듈 선언·재노출 외에 **전역 상태와 로딩 오케스트레이션**을 담는다. 핵심 흐름:

1. **전역 싱글턴**: `CONFIG: Configuration`(`lib.rs:72`)이 `Mutex<ConfigInner>`를 감싼다. `ConfigInner`(`lib.rs:455`)는 현재 `Arc<Config>`, 에러/경고, 세대 번호, `notify` 워처, 구독자 맵을 보유한다.
2. **로딩**: `Config::load()` → `load_with_overrides()`가 후보 경로 목록을 우선순위대로 구성한다(실행 파일 디렉터리 > `WEZTERM_CONFIG_FILE` 환경변수 > config 파일 override > `~/.wezterm.lua` > `CONFIG_DIRS`). 실행 파일 옆 `wezterm.lua`를 최우선에 두는 것은 **Windows 포터블(USB) 설치** 시나리오를 위한 의도적 설계다(`config/src/config.rs:984-996`).
3. **Lua 평가**: `try_load`(`config.rs:1057`)가 파일을 읽고 BOM을 제거한 뒤 Lua로 평가하고, `apply_overrides_to`/`apply_overrides_obj_to`로 `--config key=value` override를 주입한 다음 `Config::from_lua`로 변환, `check_consistency`로 도메인 이름 중복을 검사한다.
4. **파생 계산**: `compute_extra_defaults`(`config.rs:1283`)가 상대 경로 폰트 디렉터리 절대화, 폰트 규칙(half-bright/bold/italic 조합) 자동 추가, 색 구성표 디렉터리 로드, `color_scheme`/`colors` 해석을 통해 `resolved_palette`를 채우고, 레거시 배경 옵션을 `background` 레이어로 변환한다.
5. **핫 리로드**: `ConfigInner::reload`(`lib.rs:561`)가 watch 경로를 갱신하고, 성공 시 `Arc<Config>` 교체 + 세대 증가 + Lua 컨텍스트를 `LUA_PIPE`로 전달, 구독자에게 알림한다. 워처 스레드는 200ms grace period로 이벤트를 합치고 `reload()`를 호출한다(`lib.rs:494-546`).

**Lua 스레드 모델**은 이 크레이트의 가장 미묘한 부분이다. `mlua::Lua`는 `Send`이지만 `!Sync`이므로, 백그라운드 파일 감시 스레드에서 로드된 Lua 컨텍스트를 메인 스레드로 전달하기 위해 `LUA_PIPE`(채널)와 thread-local `LUA_CONFIG`(`lib.rs:82`)를 사용한다. `with_lua_config_on_main_thread`/`with_lua_config`/`run_immediate_with_lua_config`가 이 메커니즘을 캡슐화한다(주석 `lib.rs:183-202` 참조).

데이터 모델은 도메인별로 분할된다:

- `config.rs` (2149행, **비대 모듈**): `Config` 본체 + 약 80개의 `default_*` 함수 + 다수의 보조 enum(`ExitBehavior`, `DefaultCursorStyle`, `WindowPadding`, `DroppedFileQuoting` 등). 리팩토링 시 1차 분할 후보.
- `color.rs` (806행): `Palette`, `RgbaColor`, `ColorSpec`, `TabBarColors`/`TabBarColor`, `WindowFrameConfig`, `ColorSchemeFile`/`ColorSchemeMetaData`, TOML/JSON ↔ dynamic 변환.
- `lua.rs` (954행): Lua 컨텍스트 구성과 `wezterm` 모듈 함수, config builder 메타테이블(`config_builder_new_index`로 잘못된 옵션 설정 시 스택 추적 경고), 이벤트 등록/방출.
- `font.rs` (705행): `FontAttributes`, `TextStyle`, `StyleRule`, `FontWeight`(OpenType weight 래퍼), `FreeTypeLoadFlags`, 각종 폰트 선택 enum.
- `keyassignment.rs` (712행): `KeyAssignment`(거대 enum, 모든 키/마우스 동작), `SpawnCommand`, 런처/퀵셀렉트/입력 셀렉터 인자 구조체, `CopyModeAssignment`.
- `background.rs` (467행): 배경 레이어·그래디언트·CSS 유사 정렬/반복 옵션, `colorgrad` 프리셋.
- `scheme_data.rs` (1007행, **[생성]**): 색 구성표 데이터 테이블.
- 나머지 도메인 모듈(`ssh.rs`, `unix.rs`, `tls.rs`, `wsl.rs`, `serial.rs`, `exec_domain.rs`, `daemon.rs`)과 보조 모듈(`units.rs`, `keys.rs`, `bell.rs`, `terminal.rs`, `frontend.rs`, `cell.rs`, `version.rs`, `meta.rs`, `window.rs`).

### 4.2 `wezterm-config-derive` 크레이트

세 모듈로 구성된다.

- `lib.rs`: `proc_macro_derive` 진입점.
- `configmeta.rs`: 구조체 → `impl ConfigMeta` 토큰 생성. `skip` 필드는 제외, 각 필드를 `ConfigOption`으로 변환(`config/derive/src/configmeta.rs:42`).
- `attr.rs` (280행): `#[dynamic(...)]`·`#[doc]` 속성 파싱. `rename`, `default`(경로 지정 또는 `Default`), `deprecated`, `into`, `try_from`, `validate`, `skip`, `flatten`을 인식하고, 필드 타입을 분석해 `ContainerType`(None/Option/Vec/Map)을 판정한다. 미지원 타입은 `panic!`(`attr.rs:156` 등).
- `bound.rs`: 제네릭 타입 파라미터에 `crate::ConfigMeta` 바운드를 추가하는 where 절 생성기.

생성된 `get_config_options`는 `default_value`를 `|| #default.to_dynamic()` 클로저로 박아 런타임에 기본값을 동적 값으로 얻을 수 있게 한다(`config/derive/src/attr.rs:100`).

## 5. 핵심 데이터 구조·타입

- **`Config`** (`config/src/config.rs:51`): 약 250개 필드의 평면 구조체. 불변식은 derive 매크로와 `FromDynamic`가 강제한다. 각 필드는 `#[dynamic(default = "...")]`로 기본값을, `validate = "..."`로 제약을 선언한다(예: `validate_scrollback_lines`가 `<= 999_999_999`를 보장 `config.rs:1652`, `validate_row_or_col`가 행/열 ≥ 1을 보장 `config.rs:2110`). `Default`는 빈 동적 객체로부터 `from_dynamic`을 호출해 derive 기본값을 단일 출처로 사용한다(`config.rs:932-943`).
- **`ConfigHandle`** (`config/src/lib.rs:764`): `Arc<Config>` + `generation: usize`. 불변식: `generation`은 리로드마다 단조 증가하며, 소비자는 이를 비교해 캐시 무효화를 결정한다. `Deref`로 `Config` 필드에 직접 접근.
- **`Palette`** (`config/src/color.rs:129`): 모든 색 슬롯이 `Option`. `overlay_with`(`color.rs:183`)는 "other에 값이 있으면 덮어쓰고, 없으면 self 유지"라는 합성 규칙을 가진다. `indexed`는 16~255 인덱스 맵이며, `ColorPalette` 변환 시 16 미만 인덱스는 경고 후 무시한다(불변식: ansi/brights가 0~15를 담당 `color.rs:305`).
- **`RgbaColor`** (`config/src/color.rs:33`): `SrgbaTuple`의 newtype. 문자열 ↔ 색 상호 변환(`try_from="String", into="String"`). `Deref<Target=SrgbaTuple>`.
- **`KeyAssignment`** (`config/src/keyassignment.rs:511`): 모든 사용자 동작을 표현하는 거대 enum. `KeyTables`(`keyassignment.rs:704`)는 기본 테이블 + 이름별 테이블로 구성되고, 키는 `(KeyCode, Modifiers)`로 정규화된다(`config.rs:1234-1264`, `normalize_shift` 적용).
- **`DeferredKeyCode`** (`config/src/keys.rs:17`): `phys:`/`mapped:`/`raw:` 접두사를 파싱해 물리 키와 매핑 키를 모두 보존하는 `Either` 변형을 가진다. `KeyMapPreference`(`keys.rs:8`, 기본 `Mapped`)에 따라 `resolve`로 한쪽을 선택한다. 불변식: `Either`는 양쪽 파싱이 모두 성공할 때만 생성된다(`keys.rs:98-107`).
- **`Dimension`** (`config/src/units.rs:108`): Points/Pixels/Percent/Cells 4종 단위. `PixelUnit`/`OptPixelUnit`는 `try_from` 어댑터로, `"123px"`·`"50%"`·`"2cell"` 등 문자열을 파싱한다(`units.rs:63`). `evaluate_as_pixels`가 `DimensionContext`(dpi·pixel_max·pixel_cell)로 픽셀 환산한다.
- **`FontWeight`** (`config/src/font.rs:73`): OpenType weight(u16) 래퍼. 100~1000의 명명 라벨(Thin~ExtraBlack)과 숫자를 양방향 변환하며, `bolder`/`lighter`가 ±200을 가한다(synthetic bold/half-bright 생성에 사용).
- **도메인 구조체군**: `UnixDomain`(`unix.rs:9`), `SshDomain`(`ssh.rs:42`), `TlsDomainClient`/`TlsDomainServer`(`tls.rs`), `WslDomain`(`wsl.rs:8`), `SerialDomain`(`serial.rs:5`), `ExecDomain`(`exec_domain.rs:13`). 공통 불변식: `name`이 `validate_domain_name`으로 검증되어 `"local"`(내장 도메인) 및 빈 문자열을 거부하고(`config.rs:2128`), `check_domain_consistency`(`config.rs:1191`)가 도메인 종류를 가로질러 이름 중복을 금지한다.

## 6. 외부 의존성

`config` 크레이트의 주요 외부 의존성과 사용 이유:

| 크레이트 | 용도 |
| --- | --- |
| `mlua` (vendored, lua54, async, send, serialize) | Lua 설정 스크립트 실행·이벤트 시스템의 핵심 |
| `wezterm-dynamic` | 설정 직렬화 추상화 계층(`Value`, `FromDynamic`/`ToDynamic`). TOML/JSON/Lua 모두 이 중간 표현을 경유 |
| `notify` | 설정 파일 변경 감시(핫 리로드) |
| `toml` / `serde_json` | 색 구성표 파일 및 JSON 입력 파싱 |
| `colorgrad` | 배경 그래디언트 프리셋(Viridis, Turbo 등 38종) 생성(`background.rs:363`) |
| `termwiz` | 색(`SrgbaTuple`, `AnsiColor`), 셀 속성, 하이퍼링크 규칙 등 터미널 원시 타입 |
| `wezterm-term` | 터미널 모델 연동(`TerminalConfiguration` 구현, `ColorPalette` 변환) |
| `portable-pty` | 자식 프로세스 커맨드 빌드(`CommandBuilder`) |
| `wezterm-ssh` | SSH 설정 파일 열거(기본 SSH 도메인 생성, `ssh.rs:103`) |
| `wezterm-bidi` | 양방향 텍스트 방향 힌트(`ParagraphDirectionHint`) |
| `wezterm-input-types` | 키코드·수정자·창 장식 등 입력 타입 |
| `luahelper` | Lua ↔ dynamic 변환 헬퍼, `impl_lua_conversion_dynamic!` 매크로 |
| `dirs-next` | 홈·캐시·데이터·런타임 디렉터리 해석 |
| `shlex` | 셸 인자 분할/인용(드롭 파일 인용, `wezterm.shell_*` 함수) |
| `bitflags` | `FreeTypeLoadFlags`, `LauncherFlags` 등 플래그 집합 |
| `smol` / `promise` | 비동기 실행(Lua 평가·이벤트 방출) |
| `winapi` (Windows 타깃) | `CREATE_NO_WINDOW` 플래그(WSL 배포판 열거 시 콘솔 창 억제) |

`wezterm-config-derive`는 `proc-macro2`, `quote`, `syn`만 사용한다(표준 절차 매크로 3종).

## 7. 설정·기능 플래그

- **Cargo feature `distro-defaults`** (`config/Cargo.toml:14`): 활성화 시 `default_check_for_updates`가 `false`를 반환한다(`config.rs:1569`). 배포판 패키징 시 자동 업데이트 확인을 끄기 위한 플래그. 그 외 동작 분기는 없다.
- **`mlua` features**: `vendored`(Lua 정적 빌드), `lua54`, `async`, `send`, `serialize`.
- **런타임 설정 항목**은 `Config` 구조체 자체가 전부다. 대표적 그룹: 폰트/렌더링(`font`, `font_rules`, `font_locator`, `freetype_*`, `harfbuzz_features`), 창/탭바(`window_decorations`, `tab_bar_style`, `window_frame`, `integrated_title_button*`), 색(`color_scheme`, `colors`, `color_schemes`), 도메인(`unix_domains`, `ssh_domains`, `wsl_domains`, `tls_*`, `exec_domains`, `serial_ports`), 입력(`keys`, `key_tables`, `mouse_bindings`, `leader`, `disable_default_*`), 멀티플렉서(`mux_*`, `ratelimit_*`), 캐시 크기(`shape_cache_size` 등).
- **환경변수**: `WEZTERM_CONFIG_FILE`, `WEZTERM_CONFIG_DIR`(로딩 시 설정/해제), `XDG_CONFIG_HOME`(설정 디렉터리 탐색에 여전히 사용 `lib.rs:378`).

## 8. Windows 전용 고려사항

- **플랫폼 분기 부재**: `config/src/` 전체에 `cfg(unix)`·`cfg(windows)`·`cfg(target_os=...)` 분기가 **존재하지 않는다**(grep 확인). 플랫폼 코드는 제거되었고, Windows 동작이 기본값으로 직접 하드코딩되어 있다.
- **Windows 지향 기본값**:
  - 폰트 로케이터 기본 `Gdi`(`font.rs:662`).
  - 기본 런처 메뉴 = cmd / PowerShell 7(`pwsh.exe`) / Windows PowerShell(`powershell.exe`)(`config.rs:904`).
  - 멀티플렉서 서버 기본 실행 파일 `wezterm-mux-server.exe`(`unix.rs:122`).
  - 드롭 파일 인용 기본 `Windows` 스타일(`config.rs:2008`).
  - 무상태 프로세스 목록·정리 대상에 `cmd.exe`/`pwsh.exe`/`powershell.exe` 포함(`config.rs:1785`).
  - `default_swap_backspace_and_delete`는 `false`(과거 macOS 전용 분기가 `false`로 단순화됨, `config.rs:1642`).
- **Windows API 사용**:
  - `WslDistro::load_distro_list`(`wsl.rs:48`)가 `wsl.exe -l -v`를 `std::process::Command`로 실행하되 `winapi::um::winbase::CREATE_NO_WINDOW` 플래그로 콘솔 창 팝업을 억제한다. 출력은 UTF-16(WSL 버그 https://github.com/microsoft/WSL/issues/4456)으로 디코딩한다.
  - `lua.rs`의 `utf16_to_utf8`(`lua.rs:838`) 역시 동일 WSL UTF-16 이슈 대응.
  - `win32_system_backdrop`(`SystemBackdrop`: Auto/Disable/Acrylic/Mica/Tabbed, `background.rs:274`), `win32_acrylic_accent_color`(`config.rs:581`), `integrated_title_button*` 등 Windows 11 창 효과 옵션.
- **죽은(dead) 경로**:
  - `running_under_wsl()`는 무조건 `false`를 반환한다(`version.rs:23`). Lua의 `wezterm.running_under_wsl`도 항상 false.
  - 비Windows 전용 옵션이 구조체에 남아 있으나 사실상 무동작: `enable_wayland`, `enable_zwlr_output_manager`, `xcursor_theme`/`xcursor_size`, `xim_im_name`, `macos_*`(`macos_window_background_blur`, `native_macos_fullscreen_mode`, `macos_fullscreen_extend_behind_notch`, `macos_forward_to_ime_modifier_mask`), `kde_window_background_blur`, `tiling_desktop_environments`(전부 X11 항목 `config.rs:1770`). 주석은 여전히 "Only works on MacOS/KDE" 등으로 남아 있다.
  - `FontLocatorSelection::FontConfig`/`CoreText`(`font.rs:659,665`), `daemon.rs:set_sticky_bit`(no-op `daemon.rs:17`), `DaemonOptions::pid_file`(`#[cfg_attr(windows, allow(dead_code))]` `daemon.rs:34`)도 Windows에서 비활성 경로다.
  - `unix.rs`의 이름과 달리 `UnixDomain`은 mux 소켓 도메인 모델이며 POSIX 전용 코드가 아니다. 단 `xdg_config_home`·`compute_runtime_dir` 등 XDG 경로 로직은 유지된다.

## 9. 리팩토링 주의점

- **`config.rs` 비대화 (2149행)**: `Config` 구조체와 약 80개의 `default_*` 함수, 다수 보조 enum이 한 파일에 응집되어 있다. 분할 시 derive 매크로(`ConfigMeta`)가 단일 구조체에 적용된다는 제약 때문에 `Config` 자체는 쪼갤 수 없고, `default_*` 함수군·보조 enum만 도메인별 파일로 이동 가능하다.
- **거대 단일 구조체의 파급력**: `Config`는 28개 크레이트의 진입점이다. 필드 추가/삭제/개명은 (1) derive 메타데이터, (2) `FromDynamic`의 `UnknownFieldAction::Deny`(엄격 모드 `lua.rs:122`), (3) 기존 `wezterm.lua` 호환성에 동시 영향을 준다. 필드 제거 시 `deprecated` 속성으로 단계적 폐기를 권장한다(`show_update_window` 예시 `config.rs:737`).
- **불변식 강제 위치의 분산**: 유효성 검사는 세 곳에 흩어져 있다 — 필드 `validate` 속성, `check_consistency`(`config.rs:1186`), 그리고 `compute_extra_defaults`의 묵시적 보정. 변경 시 세 경로를 모두 점검해야 한다.
- **Lua 스레드 모델의 취약성**: `LUA_PIPE`/`LUA_CONFIG`/`designate_this_as_the_main_thread` 메커니즘은 `mlua::Lua`의 `!Sync` 제약을 우회하는 정교한 구조다(`lib.rs:183-315`). `with_lua_config_on_main_thread`를 메인 스레드 외에서 호출하면 패닉한다. 비동기 lifetime 추적 회피를 위해 `schedule_with_lua` 간접 호출이 필요하다는 주석이 명시되어 있다. 함부로 단순화하면 데드락/패닉/데이터 경합을 유발한다.
- **전역 가변 상태**: `CONFIG`, `CONFIG_OVERRIDES`, `CONFIG_FILE_OVERRIDE`, `SHOW_ERROR`, `SETUP_FUNCS` 등 다수의 `lazy_static`/`static` Mutex가 프로세스 전역 상태다. 테스트 격리가 어렵고(`use_test_configuration`으로 우회), `notify` 워처 스레드가 `reload()`를 비동기 호출하므로 리로드 타이밍이 비결정적이다.
- **`scheme_data.rs`(생성물)**: 1001개 항목, 1007행. 손으로 수정하지 말 것. 항목 추가/갱신은 upstream의 `sync-color-schemes` 도구가 담당했으나 fork에서는 upstream 동기화를 포기했으므로, 이 파일은 사실상 동결 상태로 취급한다.
- **derive 매크로의 panic 동작**: `wezterm-config-derive`의 `attr.rs`는 미지원 필드 타입을 만나면 `panic!`한다(`attr.rs:156,169,176,179`). 지원 컨테이너는 `Option`/`Vec`/`HashMap`뿐이다. `Config`에 다른 제네릭 컨테이너(예: `BTreeMap`, 중첩 `Vec<Vec<_>>`) 필드를 추가하면 컴파일 타임 패닉이 발생한다.
- **순환 의존 없음**: `config → wezterm-config-derive`는 단방향이며, derive는 외부 크레이트만 의존한다. 다만 `config`는 `wezterm-term`·`termwiz`·`wezterm-ssh` 등과 양방향 결합으로 보일 수 있으나(타입 재사용), Cargo 수준 순환은 없다.
- **FreeType 플래그 상수 동기화**: `FreeTypeLoadFlags`의 비트 값은 `deps/freetype/src/lib.rs`와 강하게 결합되어 있으나 직접 참조하지 않고 숫자를 복제한다(주석 `font.rs:244`). 한쪽 변경 시 다른 쪽 수동 동기화 필요.

## 10. 파일별 요약

`config/src/`:

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| config.rs | 2149 | `Config` 본체, 로딩 파이프라인, 파생 기본값 계산, default_* 함수군. **비대 모듈** |
| scheme_data.rs | 1007 | **[생성]** 1001개 번들 색 구성표 TOML 문자열 테이블. `build_default_schemes`가 소비 |
| lua.rs | 954 | Lua 컨텍스트 구성, `wezterm` 모듈 함수, config builder 메타테이블, 이벤트 시스템 |
| lib.rs | 819 | 모듈 재노출, 전역 `CONFIG` 싱글턴, 리로드/구독/워처, Lua 스레드 브릿지, 경로 전역 |
| color.rs | 806 | `Palette`, `RgbaColor`, `TabBarColors`, `WindowFrameConfig`, `ColorSchemeFile`, TOML/JSON 변환 |
| keyassignment.rs | 712 | `KeyAssignment` 거대 enum, `SpawnCommand`, 런처/퀵셀렉트/입력셀렉터, `CopyModeAssignment` |
| font.rs | 705 | `FontAttributes`, `TextStyle`, `StyleRule`, `FontWeight`, `FreeTypeLoadFlags`, 폰트 선택 enum |
| background.rs | 467 | 배경 레이어·그래디언트(colorgrad 프리셋), CSS 유사 정렬/반복, `SystemBackdrop` |
| units.rs | 310 | `Dimension`(4단위), `PixelUnit`/`OptPixelUnit` 파서, `GuiPosition`, `DimensionContext` |
| wsl.rs | 208 | `WslDomain`, `wsl.exe -l -v` 열거(`CREATE_NO_WINDOW`), UTF-16 파싱 |
| keys.rs | 188 | `DeferredKeyCode`, `Key`/`LeaderKey`/`Mouse`, `KeyMapPreference`, `MouseEventTriggerMods` |
| ssh.rs | 173 | `SshDomain`, `SshBackend`, `SshParameters`, 기본 SSH 도메인 열거 |
| terminal.rs | 137 | `TermConfig` — `wezterm_term::TerminalConfiguration` 어댑터(config → 터미널 모델) |
| unix.rs | 127 | `UnixDomain`(mux 소켓 도메인), `UnixTarget`, 기본 도메인/serve_command |
| tls.rs | 105 | `TlsDomainClient`/`TlsDomainServer`(PEM 인증서 경로, bootstrap_via_ssh) |
| bell.rs | 74 | `EasingFunction`(cubic bezier), `VisualBell`, `AudibleBell` |
| daemon.rs | 63 | `DaemonOptions`(pid/stdout/stderr 로그 파일), `set_sticky_bit`(no-op) |
| frontend.rs | 54 | `FrontEndSelection`(OpenGL/WebGpu/Software), `GpuInfo`, `WebGpuPowerPreference` |
| meta.rs | 33 | `ConfigMeta` 트레이트, `ConfigOption`/`ConfigContainer`(derive 출력 대상) |
| version.rs | 25 | 버전/타깃 트리플 전역, `running_under_wsl`(항상 false) |
| cell.rs | 22 | `CellWidth`(유니코드 셀 폭 오버라이드), `compile_to_map` |
| serial.rs | 19 | `SerialDomain`(COM 포트·baud) |
| exec_domain.rs | 19 | `ExecDomain`, `ValueOrFunc`(Lua 콜백 라벨) |
| window.rs | 9 | `WindowLevel`(AlwaysOnBottom/Normal/AlwaysOnTop) |

`config/derive/src/`:

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| attr.rs | 280 | `#[dynamic]`/`#[doc]` 속성 파싱, `FieldInfo`/`ContainerInfo`, 컨테이너 타입 판정, 필드→`ConfigOption` 토큰화 |
| configmeta.rs | 57 | 구조체 → `impl ConfigMeta::get_config_options` 토큰 생성(명명 필드 구조체만 지원) |
| bound.rs | 16 | 제네릭 파라미터에 `ConfigMeta` 바운드 추가하는 where 절 생성 |
| lib.rs | 13 | `#[derive(ConfigMeta)]` proc-macro 진입점 |
