# window 폴더 기능 명세

본 문서는 `E:\Project\wezterm\window` 의 단일 크레이트 `window`에 대한 상세 기능 명세다. 추후 리팩토링을 위해 "무엇을·왜·어떻게"가 드러나도록 작성하였다. 모든 코드 인용은 `파일경로:라인` 형식을 따른다.

---

## 1. 개요 및 책임

`window` 크레이트는 **OS 윈도우 시스템과 GUI 렌더링 컨텍스트를 추상화하는 플랫폼 계층**이다. 단일 책임은 다음으로 요약된다.

- 네이티브 윈도우의 생성·표시·이동·크기 조정·파괴 (`window/src/os/windows/window.rs`)
- OS 이벤트(키보드·마우스·포커스·리사이즈·파일 드롭·IME)를 플랫폼 중립적인 `WindowEvent`로 정규화하여 애플리케이션 콜백으로 전달 (`window/src/lib.rs:151`)
- OpenGL 렌더링 컨텍스트 부트스트랩: EGL(`ANGLE`/`Mesa`) 우선, 실패 시 WGL(네이티브 `opengl32.dll`)로 폴백 (`window/src/egl.rs`, `window/src/os/windows/wgl.rs`)
- 비동기 작업을 메인(GUI) 스레드의 메시지 루프에 통합하기 위한 스폰 큐 제공 (`window/src/spawn.rs`)
- GPU 텍스처 아틀라스·비트맵 이미지 등 렌더러가 사용할 저수준 그래픽 프리미티브 제공 (`window/src/bitmaps/`)
- 멀티 모니터 정보·DPI·외관(라이트/다크) 질의 (`window/src/screen.rs`, `window/src/os/windows/connection.rs`)

이 크레이트는 WezTerm GUI(`wezterm-gui`)와 OS 사이의 경계다. 터미널 로직·셀 렌더링·폰트 셰이핑은 일절 포함하지 않으며, 오직 "창과 입력, GL 컨텍스트"만 담당한다.

**Windows fork에서의 실제 책임**: upstream의 X11/Wayland/macOS 백엔드는 제거되었고, `window/src/os/mod.rs:1`은 `windows` 모듈만 노출한다. 따라서 본 크레이트는 사실상 **Win32 윈도우 래퍼**이며, 플랫폼 중립을 표방하는 trait(`WindowOps`, `ConnectionOps`)은 그 단일 구현체로서 Windows 백엔드만 가진다. 코드 곳곳의 `cfg!(windows)`·`cfg(unix)`·Wayland/X11 주석은 대부분 죽은 경로다(9절 참조).

---

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 사용 목적 |
|---|---|
| `config` | `ConfigHandle`, `WindowDecorations`, `SystemBackdrop`, `ImePreeditRendering`, DPI·geometry·키 할당 등 사용자 설정 소비 |
| `promise` | `Future`/`Promise` 및 메인 스레드 스폰 스케줄러(`promise::spawn`) 연동 |
| `wezterm-bidi` | (재노출 의존) BiDi 텍스트 지원. 본 크레이트 내 직접 호출은 없음 |
| `wezterm-color-types` | `LinearRgba`, `SrgbaPixel` 색상 타입. `crate::color`로 재노출(`window/src/lib.rs:12`) |
| `wezterm-font` | `ParsedFont`, `FontConfiguration`, GDI 로그폰트 파서(타이틀바 폰트 추출) |
| `wezterm-input-types` | `KeyCode`, `Modifiers`, `MouseEvent`, `PhysKeyCode`, `WindowDecorations` 등 입력 타입. `lib.rs:36`에서 전량 재노출 |

### 피의존(usedBy)

| 크레이트 | 사용 형태 |
|---|---|
| `wezterm-gui` | 주 소비자. `Window::new_window`로 창을 만들고 `WindowEvent`를 받아 터미널을 렌더링. `Atlas`/`Texture2d`로 글리프 캐시 구성 |
| `window-funcs` | Lua 노출용. 윈도우 관련 타입(`Appearance`, `Dimensions` 등)을 스크립트 API로 변환 |

### 파이프라인 위치

`window`는 워크스페이스의 **플랫폼 추상화 최하단 계층**에 위치한다. 위로는 `wezterm-gui`가 이 크레이트의 이벤트 루프와 GL 컨텍스트 위에서 동작하고, 아래로는 Win32 API(`winapi`/`windows` 크레이트)와 GL 드라이버(EGL/WGL)를 직접 호출한다.

---

## 3. 공개 API 표면

### 핵심 트레이트

- **`WindowOps`** (`window/src/lib.rs:246`, `#[async_trait(?Send)]`): 창 조작의 표준 인터페이스. `show/hide/close/focus/maximize/restore/toggle_fullscreen`, `set_title`, `set_inner_size`, `set_cursor`, `invalidate`, `set_window_position`, `set_text_cursor_position`, `get_clipboard`/`set_clipboard`, `enable_opengl`, `finish_frame`, `get_os_parameters`, `config_did_change`, `set_maximize_button_position` 등. 다수 메서드는 빈 기본 구현을 가져 비Windows 백엔드용으로 설계되었던 흔적이며, Windows 구현은 `window/src/os/windows/window.rs:756`의 `impl WindowOps for Window`에 있다.
- **`ConnectionOps`** (`window/src/connection.rs:29`): 애플리케이션 연결(이벤트 루프) 인터페이스. `init`/`get`/`run_message_loop`/`terminate_message_loop`/`screens`/`get_appearance`/`resolve_geometry`/`beep`/`dispatch_app_event`. `Connection`은 스레드 로컬 싱글턴(`CONN`, `window/src/connection.rs:10`).

### 핵심 타입

- **`Window`** (`window/src/os/windows/window.rs:133`): `HWindow`(=HWND 래퍼)를 감싼 핸들. `Clone`+`Eq`+`Hash`. `Window::new_window(...)`(`window.rs:511`)로 비동기 생성.
- **`Connection`** (`window/src/os/windows/connection.rs:34`): 이벤트 핸들, 열린 창 맵(`HashMap<HWindow, Rc<RefCell<WindowInner>>>`), 공유 GL 연결 보유.
- **`WindowEvent`** (`window/src/lib.rs:151`): 정규화된 이벤트 enum. `CloseRequested`, `Destroyed`, `Resized`, `NeedRepaint`, `FocusChanged`, `RawKeyEvent`/`KeyEvent`, `MouseEvent`/`MouseLeave`, `AppearanceChanged`, `AdviseDeadKeyStatus`, `DroppedFile/Url/String`, `Notification`, `PerformKeyAssignment` 등.
- **`WindowEventSender`** (`window/src/lib.rs:217`): 사용자 핸들러(`Box<dyn FnMut(WindowEvent, &Window)>`)와 대상 `Window`를 묶어 `dispatch`.
- **`Dimensions`/`WindowState`/`Appearance`/`MouseCursor`/`Clipboard`** (`lib.rs:47~96`): 기하·상태·외관·커서·클립보드 식별자.
- **`RequestedWindowGeometry`/`ResolvedGeometry`/`ResizeIncrement`** (`lib.rs:350~387`): 창 기하 요청·해석 결과.
- **`Screens`/`ScreenInfo`** (`window/src/screen.rs`): 멀티 모니터 정보.

### 그래픽 API (`pub mod bitmaps`)

- **`BitmapImage` 트레이트 / `Image` 구조체** (`window/src/bitmaps/mod.rs:84`, `:313`): big-endian BGRA32 비트맵. 픽셀 접근·클리어·라인/사각형/이미지 그리기(`draw_line`은 Xiaolin Wu 안티앨리어싱), `resize`(lanczos3/mitchell).
- **`Texture2d` 트레이트** (`bitmaps/mod.rs:16`): GPU/RAM 텍스처 추상화. `glium::SrgbTexture2d`와 `ImageTexture`(CPU) 두 구현.
- **`Atlas`/`Sprite`** (`window/src/bitmaps/atlas.rs:21`, `:143`): `guillotiere` 기반 정사각 스프라이트 아틀라스 할당기. `OutOfTextureSpace` 에러로 확장 신호.

### 자유 함수·재노출

- `default_dpi()` (`lib.rs:23`), `os::windows::wide_string`/`is_running_in_rdp_session` (`os/windows/mod.rs`).
- `pub use raw_window_handle`, `pub use glium`, `pub use os::*`, `pub use connection::*`, `pub use wezterm_input_types::*` (`lib.rs:19~36`).

---

## 4. 내부 구조

### 모듈 분해

- `lib.rs` — 크레이트 루트. 플랫폼 중립 타입·trait 정의 및 하위 모듈 재노출.
- `connection.rs` — `ConnectionOps` trait, 스레드 로컬 `Connection` 싱글턴, `ApplicationEvent`, `resolve_geometry`(설정 기반 기하 계산).
- `configuration.rs` — `prefer_swrast()`(RDP 세션 또는 `front_end=Software`면 소프트웨어 렌더링) 단일 함수.
- `spawn.rs` — `SpawnQueue`: 2단계 우선순위(`spawned_funcs`/`spawned_funcs_low_pri`) 큐. Win32 이벤트 핸들로 메시지 루프를 깨움. `promise` 스케줄러 등록.
- `screen.rs` — `Screens`/`ScreenInfo` 데이터 구조 정의(POD).
- `egl.rs` — EGL 로더·컨텍스트(`GlConnection`/`GlState`). `glium::backend::Backend` 구현.
- `bitmaps/{mod,atlas}.rs` — 비트맵·텍스처·아틀라스.
- `os/mod.rs` — Windows 백엔드만 재노출.
- `os/parameters.rs` — 커스텀 타이틀바·테두리 파라미터 POD.
- `os/windows/` — Win32 백엔드 본체(아래 상술).

### `os/windows/` 분해

- `mod.rs` — 재노출 + `wide_string`(UTF-16 변환), `is_running_in_rdp_session`(SM_REMOTESESSION + 레지스트리 GlassSessionId 비교).
- `connection.rs` — `Connection`, 메시지 루프(`run_message_loop`, `PeekMessageW`+`MsgWaitForMultipleObjects`), 모니터 열거(`ScreenInfoHelper`), DPI·친화적 모니터 이름 조회(DisplayConfig API), `get_appearance`(레지스트리 `AppsUseLightTheme`).
- `event.rs` — `EventHandle`: `CreateEventW` 수동 리셋 이벤트 래퍼(Send+Sync).
- `window.rs` — **비대 모듈(2,995행)**. 윈도우 생성, `WindowInner` 상태, `wnd_proc` 디스패처, 모든 메시지 핸들러, 키보드 레이아웃/데드키 프로빙, IME, DWM 테마/백드롭, 풀스크린.
- `wgl.rs` — WGL OpenGL 컨텍스트(확장 픽셀 포맷 우선, 기본 폴백).
- `keycodes.rs` — VK 코드 → `PhysKeyCode` 매핑 테이블.
- `extra_constants.rs` — uxtheme 상수 3개.

### 제어·데이터 흐름

1. **부트스트랩**: `Connection::init()`(`connection.rs:56`)가 스레드 로컬 `CONN`에 `Connection`을 저장하고 `SPAWN_QUEUE`의 promise 스케줄러를 등록.
2. **창 생성**: `Window::new_window`가 `WindowInner`를 `Rc<RefCell<…>>`로 만들고, 원시 포인터를 `CreateWindowExW`의 `lpCreateParams`로 전달. `WM_NCCREATE`(`window.rs:1093`)에서 `GWLP_USERDATA`에 포인터를 박아 양방향 연결을 수립.
3. **메시지 루프**: `run_message_loop`(`connection.rs:70`)가 매 반복마다 `SPAWN_QUEUE.run()`으로 대기 작업을 비우고 `PeekMessageW`로 메시지를 처리. 대기 시 `MsgWaitForMultipleObjects`로 이벤트 핸들 또는 입력을 기다림.
4. **이벤트 디스패치**: `wnd_proc`(`window.rs:2979`)→`do_wnd_proc`(`window.rs:2916`)가 메시지를 핸들러로 분기. 핸들러는 `rc_from_hwnd`로 `WindowInner`를 복원해 `WindowEvent`를 만들고 `inner.events.dispatch(...)`로 사용자 콜백 호출.
5. **스레드 안전 조작**: `WindowOps`의 변형(mutating) 메서드는 대부분 `Connection::with_window_inner`(`connection.rs:147`)를 통해 메인 스레드 future로 디스패치되어 `WindowInner`를 안전하게 빌림.
6. **렌더링 throttle**: `wm_paint`(`window.rs:1613`)는 `NeedRepaint`를 보내고 `paint_throttled` 플래그로 `max_fps`에 맞춰 재페인트를 제한.

---

## 5. 핵심 데이터 구조·타입

- **`WindowInner`** (`window/src/os/windows/window.rs:107`): 창의 실제 상태. 불변식:
  - `hwnd`는 생성 후 `WM_NCCREATE`에서 채워지고 `WM_NCDESTROY`에서 null로 복귀(`window.rs:1097`, `:1116`).
  - `GWLP_USERDATA`의 원시 포인터와 `WindowInner`는 정확히 한 쌍의 `Rc` 참조로 균형을 이뤄야 함. `rc_to_pointer`/`rc_from_pointer`/`take_rc_from_pointer`(`window.rs:164~196`)가 참조 카운트를 수동 관리하며, 잘못 다루면 누수 또는 use-after-free.
  - `saved_placement.is_some()` ⇔ 현재 풀스크린(데코레이션 변경 무시 트리거, `window.rs:359`).
  - `paint_throttled`가 참인 동안 `invalidated`만 기록하고 실제 페인트는 보류.
  - `hscroll_remainder`/`vscroll_remainder`: 휠 델타의 미세 누적분. 방향 전환 시 0으로 리셋(`window.rs:1930`).
- **`Window`/`Connection`** — 3절 참조. `Window`는 값 핸들, `Connection`은 싱글턴.
- **`WindowState`** (`lib.rs:98`, bitflags): `FULL_SCREEN`/`MAXIMIZED`/`HIDDEN`/`ALWAYS_ON_TOP`/`ALWAYS_ON_BOTTOM`. `can_resize()`는 풀스크린·최대화 시 false, `can_paint()`는 hidden 시 false.
- **`KeyboardLayoutInfo`** (`window.rs:2174`): 현 키보드 레이아웃 캐시. 불변식: `layout` 핸들이 바뀔 때만 `probe_alt_gr`/`probe_dead_keys` 재실행(`update`, `window.rs:2391`). `dead_keys`는 (mods, vk)→`DeadKey`(데드키 + 결합 맵) 매핑.
- **`Atlas`** (`atlas.rs:21`): 불변식 — 텍스처는 반드시 정사각(`new`에서 `ensure!`), 각 스프라이트는 `PADDING`(=1px) 여백을 가져 보간 아티팩트 방지. 공간 부족 시 `OutOfTextureSpace{size: 다음 2의 거듭제곱}` 반환.
- **`Image`** (`bitmaps/mod.rs:313`): `data.len() == width*height*4` 불변식. BGRA32, 행 우선.
- **`GlState`** (egl/wgl 각각): `Drop`에서 컨텍스트/서피스를 해제. EGL `GlConnection`은 `Rc`로 공유되며 마지막 참조 해제 시 `Terminate`(`egl.rs:74`).

---

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
|---|---|
| `glium` | OpenGL 추상화. `Backend` trait을 EGL/WGL `GlState`가 구현하고, `Context`·`SrgbTexture2d`·`Frame`을 렌더러에 노출 |
| `winapi` | Win32 API 대부분(winuser/wingdi/dwmapi/imm/shellscalingapi/synchapi 등). 메시지 루프·윈도우·DPI·IME·DWM |
| `windows` | 일부 신형 API: `UI::ViewManagement`(액센트 색), `Win32::Devices::Display`(DisplayConfig로 친화적 모니터 이름) |
| `libloading` | `libEGL.dll`/`opengl32.dll`/`mesa` 동적 로드 |
| `shared_library` | `user32.dll`의 비공개 `SetWindowCompositionAttribute` 동적 바인딩(다크모드·acrylic) |
| `clipboard-win` | Windows 클립보드 읽기/쓰기 |
| `winreg` | 레지스트리 조회(테마, RDP GlassSessionId, 휠 스크롤 속도, 액센트 사용 여부) |
| `guillotiere` | 텍스처 아틀라스 사각형 할당기(`SimpleAtlasAllocator`) |
| `euclid` | 타입 안전 기하(`Rect`/`Point`/`Size`, `PixelUnit`/`ScreenPixelUnit` 단위) |
| `resize` | 이미지 리샘플링(Lanczos3/Mitchell) |
| `tiny-skia` | (의존 선언) 소프트웨어 래스터화 지원 |
| `line_drawing` | Xiaolin Wu 안티앨리어스 라인 |
| `raw-window-handle` | `HasWindowHandle`/`HasDisplayHandle` 구현으로 GL 백엔드에 윈도우 핸들 전달 |
| `async-io`/`async-task`/`async-channel`/`async-trait` | 페인트 throttle 타이머, 비동기 `WindowOps` |
| `metrics` | 스폰 지연·아틀라스 할당 성공률/지연 계측 |
| `gl_generator` (build-dep) | EGL/WGL FFI 바인딩 생성(7절·10절) |

---

## 7. 설정·기능 플래그

본 크레이트의 `Cargo.toml`에는 `[features]` 섹션이 없다. 빌드 분기는 전적으로 `[target."cfg(windows)".dependencies]`(`window/Cargo.toml:47`)에 의존하며, Windows 타깃에서만 `winapi`/`windows`/`winreg`/`clipboard-win`/`shared_library`를 끌어온다.

소비하는 주요 `config` 항목:

- `front_end`(`FrontEndSelection::Software`) — 소프트웨어 렌더링 선택(`configuration.rs:11`).
- `prefer_egl` — EGL 우선 여부(`window.rs:227`). false면 즉시 WGL.
- `window_decorations`(`WindowDecorations`: `RESIZE`/`TITLE`/`NONE`/`INTEGRATED_BUTTONS`) — Win32 스타일·커스텀 타이틀바 결정(`window.rs:390`, `:1123`).
- `win32_system_backdrop`(`SystemBackdrop`: `Auto`/`Disable`/`Acrylic`/`Mica`/`Tabbed`) 및 `win32_acrylic_accent_color` — DWM 백드롭(`window.rs:1440`).
- `ime_preedit_rendering`(`Builtin`/`System`) — IME 후보/조합 창을 자체 렌더링할지 시스템에 맡길지(`window.rs:671`, `:2076`).
- `use_dead_keys`, `treat_left_ctrlalt_as_altgr` — 데드키·AltGr 처리(`window.rs:2755`, `:2578`).
- `dpi`, `dpi_by_screen`, `max_fps` — DPI 오버라이드 및 페인트 throttle.

설정 철학(소스 기본값 수정 우선)에 따라, 동작 고정은 위 항목의 `config` 크레이트 기본값을 바꾸는 방식으로 이뤄진다.

---

## 8. Windows 전용 고려사항

### Windows API 사용 지점

- **메시지 루프**: `PeekMessageW`/`DispatchMessageW`/`MsgWaitForMultipleObjects`(`connection.rs:70~141`). `TranslateMessage`는 무조건 호출하지 않고 IME 활성·ALT-space/ALT-F4 등 특정 경우에만 수동 호출(`window.rs:2528`, `:2877`).
- **DPI 인식**: `GetDpiForWindow`/`GetDpiForMonitor`/`GetSystemMetricsForDpi`/`AdjustWindowRectExForDpi`(per-monitor DPI v2 가정).
- **커스텀 타이틀바**: `WM_NCCALCSIZE`(`window.rs:1128`)로 비클라이언트 영역을 제거하고, `WM_NCHITTEST`(`window.rs:1181`)에서 리사이즈·캡션·스냅 레이아웃 최대화 버튼 히트 테스트를 직접 수행.
- **DWM 통합**: `DwmEnableBlurBehindWindow`(알파 합성), `DwmExtendFrameIntoClientArea`, `DwmSetWindowAttribute`(immersive dark mode, Mica/Acrylic backdrop)(`window.rs:1319`, `:1340`).
- **IME**: `ImmGetContext`/`ImmGetCompositionStringW`/`ImmSetCandidateWindow`/`ImmSetCompositionWindow`(`window.rs:1981~2168`).
- **키보드**: `ToUnicode`/`MapVirtualKeyW`/`GetKeyboardState`로 데드키·AltGr를 직접 프로빙(`window.rs:2204~2409`). `ToUnicode`의 전역 상태성 때문에 `clear_key_state`로 부작용을 청소하는 방어 코드가 다수.
- **버전 분기**: `IS_WIN10`(빌드 < 22000), `IS_WIN11_22H2`(빌드 >= 22621)로 비클라이언트 영역 보정·백드롭 API·스냅 레이아웃을 분기(`window.rs:74~98`).
- **RDP 감지**: OpenGL 단절 문제 회피를 위해 RDP 세션이면 소프트웨어 렌더링 강제(`mod.rs:23`, `configuration.rs:4`).

### 죽은 코드·잔존 비Windows 흔적

- `os/mod.rs`는 `windows`만 노출하므로 비Windows 백엔드는 부재. `WindowOps`의 기본 구현 다수(`request_drag_move`, `set_window_level` 등)는 Wayland/X11용으로 설계된 흔적이며 Windows에서 미사용.
- `egl.rs:404~452`의 2-패스 swrast 폴백 로직 중 `LIBGL_ALWAYS_SOFTWARE`·Mesa 경로는 비Windows용이고, `cfg!(windows)`에서 첫 패스 후 즉시 `break`(`egl.rs:445`)하므로 Windows에선 사실상 1회만 시도.
- `egl.rs:316~332`의 XWayland/Wayland 관련 alpha 정렬 주석, `egl.rs:540` PBUFFER/PIXMAP 주석은 X11/Wayland 시절 잔존.
- `build.rs:34~43`의 android/ios/apple 분기, EGL 레지스트리의 x11/wayland/gbm 확장 나열은 Windows 빌드에서 무의미하나 코드 생성에는 무해.
- `Clipboard::PrimarySelection`(`lib.rs:42`)은 X11 개념. Windows 구현은 `_clipboard` 인자를 무시(`window.rs:958`).

---

## 9. 리팩토링 주의점

- **`window.rs` 비대화(2,995행)**: 윈도우 생성·메시지 디스패치·키보드/IME·DWM 테마·풀스크린이 한 파일에 응집. 모듈 분리(예: `keyboard`, `theme`, `ime`, `message_loop`) 시 `WindowInner`의 `pub(crate)` 가시성과 다수 자유 함수의 결합을 정리해야 한다.
- **수동 `Rc` 참조 관리**: `rc_to_pointer`/`rc_from_pointer`/`take_rc_from_pointer`(`window.rs:164~196`)는 `GWLP_USERDATA`를 통해 `Rc` 카운트를 손으로 맞춘다. `WM_NCCREATE`/`WM_NCDESTROY` 경로를 건드리면 누수 또는 이중 해제 위험이 크다. 불변식: 창 수명 동안 정확히 +1 ref가 `USERDATA`에 박혀 있어야 한다.
- **`ToUnicode` 전역 상태**: 키보드 처리(`key`, `probe_dead_keys`)는 OS 전역 키보드 상태를 변형한다. 호출 순서·`clear_key_state` 누락 시 데드키 오작동. 순수 함수로 리팩토링 불가, 신중히 다룰 것.
- **재진입(reentrancy)**: `wm_nccalcsize`/`wm_nchittest`는 `try_borrow` 실패 시 기본 처리로 폴백(`window.rs:1130`, `:1183`). `ShowWindow`·`SetWindowPos`가 wnd_proc를 재귀 호출하므로 변형 작업을 `promise::spawn`으로 지연 디스패치하는 패턴이 일관되게 사용된다. 이를 깨면 borrow 패닉.
- **`unwrap()`/`expect()` 다발**: `Connection::get().unwrap()`, `SPAWN_QUEUE` 초기화 `expect` 등 초기화 순서 의존. `Connection::init` 선행 보장이 암묵적 계약.
- **버전 분기 산재**: `IS_WIN10`/`IS_WIN11_22H2` 조건이 비클라이언트 계산·백드롭·스냅 레이아웃에 흩어져 있어 OS 지원 변경 시 다지점 수정 필요.
- **에러 처리 비대칭**: `swap_buffers`의 컨텍스트 손실 처리(`egl.rs:620`)는 있으나 WGL `swap_buffers`는 항상 `Ok`(`wgl.rs:412`). 컨텍스트 손실 복원 경로가 백엔드별로 다름.
- **순환 의존 없음**: deps 목록상 `window`는 `config`/`promise`/`wezterm-*` 하위 크레이트에만 의존하고 `wezterm-gui`가 단방향으로 의존하므로 순환은 없다. 다만 `config`의 타입(특히 `WindowDecorations`·`SystemBackdrop`) 변경은 본 크레이트로 직접 파급된다.
- **생성 코드 면제**: `egl.rs`·`wgl.rs`의 `ffi` 모듈은 `gl_generator` 산출물을 `include!` 하며 clippy lint를 면제한다(`egl.rs:6`, `wgl.rs:13`). 수동 편집 금지, `build.rs`를 통해서만 갱신.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `window/src/lib.rs` | 387 | 크레이트 루트. `WindowOps`·`WindowEvent`·기하/상태/외관 타입 정의 및 재노출 |
| `window/src/connection.rs` | 137 | `ConnectionOps` trait, 스레드 로컬 `Connection` 싱글턴, geometry 해석 |
| `window/src/configuration.rs` | 12 | `prefer_swrast()`(소프트웨어 렌더링 선택 로직) |
| `window/src/spawn.rs` | 111 | `SpawnQueue`(2단계 우선순위 메인 스레드 작업 큐) |
| `window/src/screen.rs` | 19 | `Screens`/`ScreenInfo` POD |
| `window/src/egl.rs` | 679 | EGL 로더·컨텍스트, `glium::Backend` 구현 (`ffi`는 [생성] 바인딩 include) |
| `window/src/bitmaps/mod.rs` | 468 | `BitmapImage`/`Texture2d`/`Image`/`ImageTexture` |
| `window/src/bitmaps/atlas.rs` | 172 | `Atlas`/`Sprite`(guillotiere 스프라이트 할당기) |
| `window/src/os/mod.rs` | 4 | Windows 백엔드·`parameters` 재노출 |
| `window/src/os/parameters.rs` | 30 | `Parameters`/`TitleBar`/`Border`(커스텀 장식 파라미터) |
| `window/src/os/windows/mod.rs` | 55 | `wide_string`, `is_running_in_rdp_session`, 재노출 |
| `window/src/os/windows/connection.rs` | 430 | Win32 `Connection`, 메시지 루프, 모니터/DPI/외관 열거 |
| `window/src/os/windows/window.rs` | 2,995 | **비대 모듈**. 윈도우 생성, `wnd_proc`, 모든 메시지 핸들러, 키보드/데드키/IME, DWM 테마·백드롭, 풀스크린 |
| `window/src/os/windows/event.rs` | 39 | `EventHandle`(`CreateEventW` 수동 리셋 이벤트) |
| `window/src/os/windows/wgl.rs` | 456 | WGL OpenGL 컨텍스트(확장/기본), `glium::Backend` 구현 (`ffi`는 [생성] 바인딩 include) |
| `window/src/os/windows/keycodes.rs` | 210 | VK 코드 → `PhysKeyCode` 매핑 테이블(상단 주석 블록은 죽은 코드) |
| `window/src/os/windows/extra_constants.rs` | 3 | uxtheme 상수 3개 |
| `window/build.rs` | 77 | [생성] gl_generator로 EGL/WGL FFI 바인딩 생성(빌드 시 `OUT_DIR`에 출력) |
| `window/examples/async.rs` | 151 | 사용 예제(창 생성·이벤트 디스패치·GL 프레임). 라이브러리 본체 아님 |

> 비고: `egl.rs`·`wgl.rs`의 FFI 모듈과 `build.rs` 산출물(`egl_bindings.rs`/`wgl_bindings.rs`/`wgl_extra_bindings.rs`)은 `gl_generator`가 EGL/WGL 레지스트리로부터 생성하는 코드다. 내용은 OpenGL 함수 포인터 로더이며 수작업 편집 대상이 아니다.
