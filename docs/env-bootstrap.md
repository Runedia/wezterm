# env-bootstrap 폴더 기능 명세

## 1. 개요 및 책임

`env-bootstrap`는 WezTerm의 모든 실행 가능 바이너리(`wezterm`, `wezterm-gui`, `wezterm-mux-server`)가 `main()` 초입에서 호출하는 **프로세스 부트스트랩 크레이트**다. 단일 책임은 "프로그램이 본격 로직을 수행하기 전에 갖춰야 하는 전역 런타임 환경을 한 번에 구성하는 것"이며, 구체적으로 다음을 담당한다.

- 버전 정보 등록 (`config`에 빌드 시점 버전·타깃 트리플 주입)
- 로깅 초기화 (stderr 프리티 로거 + 인메모리 링버퍼 + 파일 로그)
- 패닉 후크 등록 (백트레이스 포함 로그 출력)
- 실행 파일 경로 환경변수 설정 (`WEZTERM_EXECUTABLE`, `WEZTERM_EXECUTABLE_DIR`)
- Lua 컨텍스트 셋업 함수 등록 (각 `*-funcs` 크레이트의 `register`를 `config::lua`에 일괄 등록)
- 자식 프로세스로 누수되면 안 되는 오염 환경변수 제거 (`WINDOWID`, `VTE_VERSION`, `SHELL` 등)

이 크레이트는 정책을 결정하지 않는다. 어떤 Lua 함수가 존재하는지, 버전 문자열이 무엇인지는 의존 크레이트들이 정의하고, `env-bootstrap`는 이를 **호출 순서대로 묶어 실행하는 오케스트레이터**일 뿐이다.

### Windows fork에서의 실제 책임

`bootstrap()`의 호출 항목 중 `fixup_snap()`과 `fixup_appimage()`는 Linux 패키징(snapd, AppImage) 전용 보정 함수다. 이 fork는 Windows 전용 영구 분기이므로 두 함수는 **사실상 죽은 경로**다. `SNAP`/`APPIMAGE` 환경변수가 Windows에서 설정되는 일이 없으므로 본문은 항상 조기 반환한다. 따라서 이 fork에서 실제로 유효한 책임은 버전 등록·로거 셋업·패닉 후크·실행 파일 경로 설정·Lua 모듈 등록·환경변수 정리로 좁혀진다. `SHELL`/`WINDOWID`/`VTE_VERSION` 제거도 Unix 셸 통합 회피용이라 Windows에서는 대체로 무해한 no-op이지만 코드 경로 자체는 실행된다.

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 용도 |
|---|---|
| backtrace | 패닉 후크에서 스택 백트레이스 수집 |
| battery | 배터리 상태 Lua 함수 등록 |
| color-funcs | 색상 조작 Lua 함수 등록 |
| config | 버전 주입, Lua 셋업 함수 등록, `HOME_DIR`·`RUNTIME_DIR` 경로 제공 |
| filesystem | 파일시스템 Lua 함수 등록 |
| logging | 로깅 관련 Lua 함수 등록 |
| mux-lua | 멀티플렉서 Lua 바인딩 등록 |
| plugin | 플러그인 시스템 Lua 함수 등록 |
| procinfo-funcs | 프로세스 정보 Lua 함수 등록 |
| serde-funcs | 직렬화/역직렬화 Lua 함수 등록 |
| share-data | 공유 데이터 Lua 함수 등록 |
| spawn-funcs | 프로세스 스폰 Lua 함수 등록 |
| ssh-funcs | SSH Lua 함수 등록 |
| termwiz | `IsTty` 트레이트(stderr가 TTY인지 판별) |
| termwiz-funcs | termwiz 관련 Lua 함수 등록 |
| time-funcs | 시간 Lua 함수 등록 |
| url-funcs | URL Lua 함수 등록 |
| wezterm-version | 빌드 시점 버전 문자열·타깃 트리플 제공 |
| chrono | 로그 엔트리 타임스탬프(`DateTime<Local>`) |
| dirs-next | AppImage 보정 시 HOME 디렉터리 재해석(죽은 경로) |
| env_logger | `filter::Builder`로 로그 필터 파싱(0.10 고정, 0.11에서 제거됨) |
| lazy_static | 전역 `RINGS` 싱글턴 |
| libc | `getpid()`로 로그 파일명 생성 |
| log | 로깅 파사드(`Log` 트레이트 구현 대상) |

### 피의존(usedBy)

| 크레이트 | 사용 형태 |
|---|---|
| wezterm | `env_bootstrap::bootstrap()` (`wezterm/src/main.rs:729`) |
| wezterm-gui | `env_bootstrap::bootstrap()` (`wezterm-gui/src/main.rs:1201`), `ringlog::get_entries()` (`wezterm-gui/src/overlay/debug.rs:159`) |
| wezterm-mux-server | `env_bootstrap::bootstrap()` (`wezterm-mux-server/src/main.rs:70`) |

### 계층상 위치

`env-bootstrap`는 다수의 말단 기능 크레이트(`*-funcs`, `config`, `logging` 등)를 의존하면서 최상위 바이너리 3종이 호출하는 **부트스트랩 계층**에 위치한다. 즉 기능 크레이트 묶음과 진입점 바이너리 사이의 단일 집합점(aggregation point)으로, 초기화 순서·환경 정리를 일괄 강제하는 좁은 허리(narrow waist) 역할을 한다.

## 3. 공개 API 표면

크레이트는 모듈 2개(`lib`, `ringlog`)로 구성되며 공개 표면이 작다.

### `lib.rs`

- `pub fn bootstrap()` — 부트스트랩 전체를 수행하는 단일 진입점. 바이너리들이 `main()`에서 호출한다.
- `pub fn set_wezterm_executable()` — `current_exe()`로부터 `WEZTERM_EXECUTABLE`·`WEZTERM_EXECUTABLE_DIR` 환경변수 설정. `bootstrap()` 내부에서 호출되지만 `pub`로 노출됨.
- `pub fn fixup_snap()` — snapd 환경변수 정리(Linux 전용, Windows에서 죽은 경로). `pub`.
- `pub fn fixup_appimage()` — AppImage HOME/PATH/XDG 보정(Linux 전용, 죽은 경로). `pub`.
- `pub use ringlog::setup_logger` — 로거 초기화 함수 재노출.
- `pub mod ringlog` — 로깅 모듈 공개.

`register_panic_hook()`과 `register_lua_modules()`는 비공개(`fn`)이며 `bootstrap()` 내부에서만 호출된다.

### `ringlog.rs`

- `pub struct Entry { pub then: DateTime<Local>, pub level: Level, pub target: String, pub msg: String }` — 단일 로그 엔트리. `Ord`/`PartialOrd` 파생으로 시간순 정렬 가능.
- `pub fn setup_logger()` — `setup_pretty()`로 로거를 구성하고 `log::set_boxed_logger`로 전역 등록, `set_max_level` 설정.
- `pub fn get_entries() -> Vec<Entry>` — 인메모리 링버퍼의 모든 엔트리를 수집해 정렬 후 반환. `wezterm-gui`의 디버그 오버레이가 소비.

## 4. 내부 구조

크레이트는 두 모듈로 명확히 분리된다.

### `lib.rs` — 부트스트랩 오케스트레이션

`bootstrap()`이 정의하는 **고정 실행 순서**가 핵심 제어 흐름이다(`env-bootstrap/src/lib.rs:157`).

1. `config::assign_version_info(...)` — 버전·트리플 주입
2. `setup_logger()` — 로거 등록(이후 모든 로그가 캡처되도록 가장 먼저 수행)
3. `register_panic_hook()` — 패닉 후크
4. `set_wezterm_executable()` — 실행 파일 경로 환경변수
5. `fixup_appimage()` → `fixup_snap()` — Linux 패키징 보정(죽은 경로)
6. `register_lua_modules()` — 14개 `register` 함수를 `config::lua::add_context_setup_func`에 등록
7. `WINDOWID`·`VTE_VERSION`·`SHELL` 환경변수 제거

순서 의존성이 존재한다. 주석(`env-bootstrap/src/lib.rs:54`)에 따르면 `fixup_appimage`는 `WEZTERM_EXECUTABLE_DIR`을 참조하므로 반드시 `set_wezterm_executable` 이후에 호출되어야 한다.

### `ringlog.rs` — 이중 출력 로거

`log::Log`를 구현하는 `Logger` 한 개가 세 곳으로 동시 출력한다: (a) 전역 인메모리 링버퍼 `RINGS`, (b) stderr(ANSI 색상, TTY일 때), (c) 디스크 로그 파일. 데이터 흐름은 `Logger::log(record)` → 필터 통과 시 `RINGS.lock().log()` + stderr write + 파일 append이다. 비대한 모듈은 없으나 `Logger::log`(`ringlog.rs:160`)가 단일 함수로 색상 결정·포맷·stderr·파일 기록을 모두 담당해 책임이 다소 집중되어 있다.

## 5. 핵심 데이터 구조·타입

### `Entry` (`ringlog.rs:21`)
로그 한 줄. `then`(로컬 시각), `level`, `target`(로그 모듈명), `msg`(렌더된 메시지). `Ord` 파생으로 `get_entries()`에서 시간순 정렬에 사용된다. 불변식: 모든 필드는 `record`로부터 즉시 복사·문자열화되어 저장되므로 라이프타임 의존이 없다.

### `LevelRing` (`ringlog.rs:29`)
레벨별 고정 용량 16개 원형 버퍼. `entries`(길이 16 벡터), `first`, `last` 인덱스로 구현. 불변식:
- 생성 시 16개 더미 `Entry`로 미리 채워진다(재할당 회피).
- `rolling_inc`가 인덱스를 0~15로 순환시킨다.
- `len()`은 `last >= first`면 `last-first`, 아니면 랩어라운드 계산. 가득 차면 `push`가 `first`를 전진시켜 가장 오래된 항목을 덮어쓴다.

### `Rings` (`ringlog.rs:95`)
`HashMap<Level, LevelRing>`로 5개 레벨(Error/Warn/Info/Debug/Trace) 각각에 독립 링을 보유. `get_entries()`는 모든 링을 합쳐 반환한다. 불변식: 생성 시 5개 레벨 키가 모두 채워지므로 `log()`의 `get_mut`은 알려진 레벨에 대해 항상 성공한다.

### `Logger` (`ringlog.rs:134`)
- `file_name`: 로그 파일 경로
- `file: Mutex<Option<BufWriter<File>>>`: 지연 생성되는 파일 핸들(첫 로그 시 open)
- `filter: env_logger::filter::Filter`: 모듈별 레벨 필터
- `padding: AtomicUsize`: target 컬럼 정렬용 최대 폭(단조 증가, `fetch_max`)
- `is_tty: bool`: stderr ANSI 색상 출력 여부
- `Drop` 시 `flush()` 보장.

## 6. 외부 의존성

- **env_logger** = 로그 필터링. `filter::Builder`/`Filter`를 직접 사용한다. `Cargo.toml`(`env-bootstrap/Cargo.toml:17`)에 `0.10`으로 고정되어 있고 주석으로 "0.11에서 `filter::Builder`가 제거되어 의존"임을 명시. 즉 업그레이드 차단 의존성이다.
- **backtrace** = 패닉 후크에서 `Backtrace::new()`로 스택 캡처.
- **chrono** = 로그 타임스탬프(`DateTime<Local>`, `%H:%M:%S%.3f` 포맷).
- **libc** = `getpid()`로 PID 기반 로그 파일명 생성(`ringlog.rs:276`). `unsafe` 블록 사용.
- **lazy_static** = 전역 `RINGS` 싱글턴(`Mutex<Rings>`).
- **termwiz** = `IsTty` 트레이트로 stderr가 터미널인지 판별(색상 on/off 결정).
- **dirs-next** = AppImage 보정 경로에서 `home_dir()` 호출(죽은 경로).
- **각종 `*-funcs`·`config`·`logging`·`mux-lua`·`plugin`** = 각자의 `register` 함수를 제공하여 Lua 컨텍스트에 기능을 주입한다.

## 7. 설정·기능 플래그

- crate 자체 feature flag는 없다(`Cargo.toml`에 `[features]` 섹션 부재).
- 관련 환경변수/설정:
  - `WEZTERM_LOG` — 설정 시 `env_logger` 필터 문법으로 파싱, 미설정 시 기본 레벨 `Info`(`ringlog.rs:290`).
  - 하드코딩된 모듈별 억제 필터: `wgpu_core`, `wgpu_hal`, `gfx_backend_metal`, `tracing`, `zbus`를 모두 `Error` 이상만 출력(`ringlog.rs:280`). 이 중 `gfx_backend_metal`(macOS)·`zbus`(Linux D-Bus)는 Windows fork에서 의미 없는 잔재 필터다.
  - 로그 파일 위치: `config::RUNTIME_DIR/{base_name}-log-{pid}.txt`.
  - 로그 파일 정리: `base_name`에 `"gui"`가 포함될 때만 `prune_old_logs()`가 실행되어 `-log-` 패턴의 7일 경과 파일을 삭제(`ringlog.rs:241`, `ringlog.rs:268`). CLI 명령은 시작 오버헤드를 낮추려 정리를 생략한다.

## 8. Windows 전용 고려사항

- **죽은 경로**: `fixup_snap()`(`lib.rs:14`)와 `fixup_appimage()`(`lib.rs:40`)는 각각 `SNAP`/`APPIMAGE` 환경변수가 있을 때만 동작하며 Windows에서는 설정되지 않으므로 항상 조기 반환한다. `dirs-next`·`LD_LIBRARY_PATH`·`XDG_CONFIG_HOME`·`ARGV0` 처리 전부 비Windows 전용이다.
- **잔재 로그 필터**: `gfx_backend_metal`(Metal/macOS), `zbus`(D-Bus/Linux) 억제 필터(`ringlog.rs:283`)는 Windows에서 발생하지 않는 로그 모듈이라 무해하지만 의미 없는 코드다.
- **`libc::getpid()`**: `ringlog.rs:276`에서 `unsafe`로 호출. Windows MSVC 타깃에서도 `libc` 크레이트가 `getpid` 래퍼를 제공하므로 동작한다. `std::process::id()`로 대체 가능한 지점이다.
- **셸 통합 회피 환경변수 제거**: `VTE_VERSION`(GNOME Terminal/VTE), `SHELL`(Unix passwd 기반 셸 해석) 제거는 Unix 맥락의 보정이다. Windows에서 `SHELL`은 보통 미설정이라 제거는 무해한 no-op이다.
- 직접적인 Windows API(winapi/windows-rs) 호출 지점은 없다.

## 9. 리팩토링 주의점

- **죽은 코드 제거 후보**: `fixup_snap`/`fixup_appimage`와 `dirs-next` 의존성, `gfx_backend_metal`·`zbus` 필터는 Windows fork에서 제거 가능하다. 단 두 함수가 `pub`이므로 외부 호출 여부를 먼저 확인해야 한다(현재 워크스페이스 내 호출처는 `bootstrap()` 내부뿐).
- **`env_logger` 0.10 고정**: `filter::Builder`/`Filter` 직접 의존이 0.11 업그레이드를 막는다(`Cargo.toml:17`). 로그 필터링을 자체 구현하거나 다른 필터 크레이트로 교체하지 않는 한 버전 상향 불가.
- **초기화 순서 결합**: `bootstrap()`의 7단계는 순서 의존성이 있다. 특히 `set_wezterm_executable` → `fixup_appimage`(`WEZTERM_EXECUTABLE_DIR` 참조), `setup_logger`를 패닉 후크보다 먼저 두어야 패닉이 로깅되는 구조. 순서 변경 시 회귀 위험.
- **전역 가변 상태**: `RINGS`(`lazy_static` + `Mutex`)와 `log::set_boxed_logger`는 전역 싱글턴이다. `bootstrap()`을 두 번 호출하면 `set_boxed_logger`는 두 번째에 실패하지만(`is_ok()` 검사로 무시) 패닉 후크와 환경변수 제거는 재실행된다. 멱등성이 완전하지 않다.
- **환경변수 부작용**: `std::env::set_var`/`remove_var`는 프로세스 전역이며 멀티스레드 환경에서 비안전(Rust 2024 edition에서 `unsafe`로 분류 예정). 이 crate는 `edition = "2018"`(`Cargo.toml:5`)이라 현재는 경고 없으나 edition 상향 시 영향.
- **`Logger::log` 응집도**: 색상·포맷·stderr·파일 기록이 한 함수에 집중(`ringlog.rs:160`). 출력 대상을 늘리려면 이 함수를 분해하는 것이 안전하다.
- **순환 의존 없음**: `config` 등 하위 크레이트에 단방향 의존하며 역방향 의존은 없다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `env-bootstrap/Cargo.toml` | 35 | 패키지·의존성 선언. `env_logger=0.10` 고정 주석 포함. |
| `env-bootstrap/src/lib.rs` | 187 | 부트스트랩 오케스트레이션. `bootstrap()` 진입점, 실행 파일 경로 설정, Linux 패키징 보정(죽은 경로), Lua 모듈 일괄 등록, 패닉 후크, 환경변수 정리. |
| `env-bootstrap/src/ringlog.rs` | 316 | 이중 출력 로거. `log::Log` 구현(stderr + 파일 + 인메모리 링버퍼), 레벨별 원형 버퍼(`LevelRing`/`Rings`), `WEZTERM_LOG` 필터, 7일 경과 로그 정리, `get_entries()` 제공. |

생성 파일 없음.
