# termwiz 폴더 기능 명세

본 문서는 `E:\Project\wezterm\termwiz` 디렉터리에 위치한 단일 크레이트 `termwiz`(Terminal Wizardry, v0.24.0)의 기능 명세다. WezTerm의 Windows 전용 영구 분기를 전제로, 추후 리팩토링을 위한 상세 분석을 목적으로 한다. 모든 코드 인용은 `파일경로:라인` 형식을 따른다.

---

## 1. 개요 및 책임

`termwiz`는 터미널을 다루는 저수준 지원 라이브러리다. 터미널에 데이터를 표시하거나 터미널 에뮬레이터를 구축하려는 애플리케이션을 위한 토대를 제공한다(`termwiz/src/lib.rs:1`~`36`).

단일 책임을 한 문장으로 요약하면 다음과 같다. **터미널 디바이스(콘솔 API와 ANSI/terminfo 이스케이프)에 대한 추상화, 입력(키/마우스/붙여넣기) 디코딩, 화면 셀 모델(`Surface`)을 디바이스로 렌더링하는 파이프라인, 그리고 그 위에 얹는 라인 에디터·위젯 같은 상위 UI 빌딩 블록을 제공한다.**

이 크레이트의 실제 구현 코드는 대부분 다음 영역에 집중되어 있다.

- 터미널 능력(capability) 탐지 및 표현 (`caps`)
- 입력 바이트열·콘솔 입력 레코드의 이벤트 디코딩 (`input`, `keymap`, `readbuf`)
- `Terminal` 트레이트와 그 Windows 구현 (`terminal`)
- 렌더러: terminfo/ANSI 이스케이프 기반과 Win32 콘솔 API 기반 (`render`)
- 라인 에디터 (`lineedit`)
- 위젯 레이아웃 시스템 (`widgets`, 선택적 feature)

반면 셀·색상·이스케이프 파서·서피스·하이퍼링크·이미지·유니코드 속성 등 핵심 데이터 모델은 별도 워크스페이스 크레이트로 분리되어 있고, `termwiz`는 이를 **재노출(re-export)**하여 외부에 단일 진입점을 제공한다(`termwiz/src/lib.rs:43`~`68`). 즉 `termwiz`는 "어셈블리 지점이자 디바이스 계층"이며, 순수 데이터 타입의 정의 책임은 의존 크레이트에 위임한다.

Windows fork에서의 실제 책임: `Terminal`의 구체 타입은 `WindowsTerminal` 하나뿐이며 `SystemTerminal = WindowsTerminal`로 별칭된다(`termwiz/src/terminal/mod.rs:21`,`109`). `istty`는 Windows 콘솔 핸들 전용으로 구현되어 있다(`termwiz/src/istty.rs:5`~`6`). 따라서 이 크레이트가 fork 환경에서 담당하는 핵심은 **Win32 콘솔(CONIN$/CONOUT$, ConPTY 가상 터미널)과의 직접 상호작용**이다.

---

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 사용 목적 |
| --- | --- |
| filedescriptor | 콘솔 핸들 복제(`FileDescriptor::dup`)·소유권(`OwnedHandle`)·I/O 추상화 |
| vtparse | 이스케이프 시퀀스 저수준 상태 머신(파서 백엔드) |
| wezterm-bidi | 양방향(BiDi) 텍스트 처리 |
| wezterm-blob-leases | 이미지 데이터 blob 임대(이미지 feature 시) |
| wezterm-cell | `Cell`, `CellAttributes`, 색상·이미지·유니코드 폭 모델 (재노출: `cell`, `color`, `image`) |
| wezterm-char-props | 유니코드 문자 속성·NerdFonts 데이터 (재노출: `nerdfonts`) |
| wezterm-color-types | 색상 타입(`AnsiColor`, `ColorAttribute` 등) |
| wezterm-dynamic | 동적 값 직렬화/역직렬화 지원 |
| wezterm-escape-parser | 이스케이프 시퀀스의 의미론적 파싱·재인코딩 (재노출: `escape`, `tmux_cc`) |
| wezterm-input-types | `Modifiers`, ctrl 매핑 등 입력 기본 타입 (재노출: `input::Modifiers`) |
| wezterm-surface | `Surface`, `Change`, `cellcluster`, `hyperlink` (재노출: `surface` 등) |

기타 직접 의존: anyhow, bitflags, fancy-regex, finl_unicode, fixedbitset, libc, log, memmem, num-derive/num-traits, ordered-float, phf, siphasher, terminfo, thiserror, ucd-trie, unicode-segmentation, winapi(Windows 한정). 선택적: cassowary, fnv, image, pest/pest_derive, serde, sha2 (`termwiz/Cargo.toml:13`~`78`).

### 피의존(usedBy)

| 크레이트 | 비고 |
| --- | --- |
| codec | 프로토콜 인코딩에서 입력/서피스 타입 사용 |
| config | 설정 모델에서 키/색상 타입 사용 |
| env-bootstrap | 환경 초기화 |
| mux, mux-lua | 멀티플렉서: 터미널 이벤트·서피스 변경 전파 |
| tabout | 탭 출력 |
| termwiz-funcs | termwiz 노출 함수 래퍼 |
| wezterm (CLI) | 최상위 바이너리 |
| wezterm-char-props | (역방향) 데이터 생성·소비 협력 |
| wezterm-client | 클라이언트 측 입력/렌더 |
| wezterm-font | 폰트 셰이핑 시 셀/클러스터 타입 사용 |
| wezterm-gui | GUI 프론트엔드: 입력 이벤트·서피스·키 인코딩 |
| wezterm-mux-server-impl | 서버 측 멀티플렉서 |
| wezterm-ssh | SSH 채널의 입력 디코딩·렌더 |

**계층상 위치**: `termwiz`는 순수 데이터 모델 크레이트(`wezterm-cell`, `wezterm-surface`, `wezterm-escape-parser` 등) 위에 있고, 그 위에 멀티플렉서·GUI·클라이언트/서버가 얹힌다. 즉 데이터 모델과 애플리케이션 계층 사이의 **터미널 디바이스·입출력 어댑터 계층**이다.

---

## 3. 공개 API 표면

`lib.rs`는 자체 모듈과 의존 크레이트 재노출을 함께 노출한다(`termwiz/src/lib.rs:43`~`68`). 자체 모듈: `caps`, `error`, `input`, `istty`, `keymap`, `lineedit`, `render`, `terminal`, `widgets`(feature). 재노출: `cell`(=wezterm-cell), `color`, `image`(use_image 시), `cellcluster`, `hyperlink`(=wezterm-surface), `nerdfonts`(=wezterm-char-props), `surface`(=wezterm-surface), `tmux_cc`(tmux_cc 시), `escape`(=wezterm-escape-parser). 또한 `error::{Context, Error, Result}`를 최상위로 노출한다.

### 핵심 타입·트레이트·함수

- `trait Terminal` (`terminal/mod.rs:55`): 디바이스 추상화. 주요 메서드 — `set_raw_mode`/`set_cooked_mode`, `enter_alternate_screen`/`exit_alternate_screen`, `get_screen_size`/`set_screen_size`, `render(&[Change])`, `flush`, `poll_input(Option<Duration>) -> Result<Option<InputEvent>>`, `probe_capabilities`, `waker`.
- `fn new_terminal(caps: Capabilities) -> Result<impl Terminal>` (`terminal/mod.rs:120`): CONIN$/CONOUT$를 직접 열어 `WindowsTerminal`을 생성.
- `type SystemTerminal = WindowsTerminal` (`terminal/mod.rs:109`).
- `struct WindowsTerminal` (`terminal/windows.rs:483`): `new`, `new_with`, `new_from_stdio`, `enable_virtual_terminal_processing`.
- `struct ScreenSize { rows, cols, xpixel, ypixel }` (`terminal/mod.rs:31`), `enum Blocking` (`terminal/mod.rs:46`).
- `struct BufferedTerminal<T: Terminal>` (`terminal/buffered.rs:17`): `new`, `flush`, `repaint`, `check_for_resize`; `Surface`로 `Deref`.
- `struct Capabilities` (`caps/mod.rs:172`): `new_from_env`, `new_with_hints`, 접근자 `color_level`/`sixel`/`hyperlinks`/`iterm2_image`/`bce`/`terminfo_db`/`bracketed_paste`/`mouse_reporting`/`force_terminfo_render_to_use_ansi_sgr`.
- `struct ProbeHints` (`caps/mod.rs:72`, `builder!` 매크로로 빌더 메서드 자동 생성), `enum ColorLevel` (`caps/mod.rs:147`).
- `struct ProbeCapabilities<'a>` (`caps/probed.rs:46`): `xt_version`, `outer_xt_version`, `screen_size`. `struct XtVersion` (`caps/probed.rs:16`).
- `struct InputParser` (`input.rs:625`): `new`, `parse`, `parse_as_vec`, `decode_input_records`/`decode_input_records_as_vec`(Windows).
- `enum InputEvent`, `enum KeyCode`, `struct KeyEvent`, `struct MouseEvent`, `struct PixelMouseEvent` (`input.rs:51`~`93`), `enum KeyboardEncoding`/`struct KeyCodeEncodeModes` (`input.rs:21`,`34`). `KeyCode::encode(...)`, `normalize_shift_to_upper_case`, `is_modifier` (`input.rs:208`,`220`,`241`). `Modifiers`, `MouseButtons` 재노출(`input.rs:15`,`47`).
- `struct KeyMap<Value>` / `enum Found<Value>` (`keymap.rs:145`,`123`): `insert`, `lookup`.
- `trait RenderTty` (`render/mod.rs:5`): `get_size_in_cells`. `struct TerminfoRenderer` (`render/terminfo.rs:19`), `struct WindowsConsoleRenderer` (`render/windows.rs:18`); 둘 다 `render_to(&[Change], out)`.
- `trait IsTty` / `fn is_tty` (`istty.rs:9`).
- 라인 에디터: `struct LineEditor` (`lineedit/mod.rs:72`), `fn line_editor_terminal()` (`lineedit/mod.rs:857`), `trait LineEditorHost`/`struct NopLineEditorHost`/`struct CompletionCandidate`/`enum OutputElement` (`lineedit/host.rs`), `trait History`/`struct BasicHistory`/`enum SearchStyle`/`SearchDirection` (`lineedit/history.rs`), `struct LineEditBuffer` (`lineedit/buffer.rs:6`), `enum Action`/`Movement` (`lineedit/actions.rs`).
- 위젯(feature `widgets`): `trait Widget`, `struct Ui`, `struct WidgetId`, `enum WidgetEvent`, `struct Rect`, `RenderArgs`/`UpdateArgs`, 좌표 타입 (`widgets/mod.rs`), `layout::{Constraints, DimensionSpec, ...}` (`widgets/layout.rs`).
- 에러 매크로: `format_err!`, `bail!`, `ensure!` (`error.rs:98`~`141`), `trait Context` (`error.rs:146`).

---

## 4. 내부 구조

모듈 분해와 데이터 흐름은 다음과 같다.

**입력 경로**: 콘솔 또는 PTY에서 바이트/`INPUT_RECORD`가 들어온다 → `WindowsTerminal::poll_input`이 `WaitForMultipleObjects`로 입력 또는 waker 이벤트를 대기(`terminal/windows.rs:811`~`853`) → `ReadConsoleInputW`로 레코드 수집 → `InputParser::decode_input_records`가 키/마우스/리사이즈로 변환. ANSI 바이트 경로에서는 `InputParser::parse`가 `ReadBuffer`(`readbuf.rs`)에 누적하고 `process_bytes`가 상태 머신(`InputState::{Normal, EscapeMaybeAlt, Pasting}`)으로 디코딩(`input.rs:1311`~`1435`). 키 시퀀스 매칭은 트라이 구조 `KeyMap`(`keymap.rs`)이 담당하며, `Found::{Exact, Ambiguous, NeedData, None}`로 부분 입력(버퍼 경계에 걸친 시퀀스)을 구분 처리한다. 마우스 SGR 리포트는 `escape` 파서로 별도 처리(`input.rs:1340`~`1375`).

**출력/렌더 경로**: 애플리케이션은 `Surface`에 그린 뒤 `Change` 델타를 얻는다 → `Terminal::render`가 렌더러로 위임(`terminal/windows.rs:798`). 렌더러는 두 종류다.
- `TerminfoRenderer`(`render/terminfo.rs`): terminfo 능력 또는 강제 ANSI SGR로 이스케이프 시퀀스를 생성. `flush_pending_attr`가 속성 비교 후 최소 SGR을 방출(`render/terminfo.rs:55`~). **이 파일이 1297행으로 크레이트에서 가장 비대한 모듈**이며, 색상 단계(16/256/TrueColor)·BCE·이미지(iTerm2/sixel) 분기를 모두 포함한다.
- `WindowsConsoleRenderer`(`render/windows.rs`): `Change`를 Win32 콘솔의 `CHAR_INFO` 버퍼·속성 워드로 변환하여 `WriteConsoleOutputW`/`ScrollConsoleScreenBufferW`로 출력. `ScreenBuffer` 내부 구조가 커서·스크롤·dirty 추적을 관리(`render/windows.rs:121`~`296`).

렌더러 선택 로직(`terminal/windows.rs:591`~`597`): terminfo DB가 있으면 `Terminfo`, 없지만 ConPTY가 가상 터미널 처리(`ENABLE_VIRTUAL_TERMINAL_PROCESSING`)를 지원하면 내장 xterm-256color terminfo를 입혀 `Terminfo`, 그 외에는 레거시 `Windows` 콘솔 API. `TERMWIZ_BYPASS_VIRTUAL_TERMINAL=1`로 가상 터미널 경로를 우회 가능(`terminal/windows.rs:583`~`589`).

**능력 탐지**: `Capabilities::new_with_hints`가 환경변수(`TERM`/`COLORTERM`/`NO_COLOR` 등)와 terminfo DB로부터 휴리스틱하게 색상 단계·하이퍼링크·sixel·BCE 등을 결정(`caps/mod.rs:210`~`327`). 런타임 능동 탐지는 `ProbeCapabilities`가 XTVERSION·텍스트 영역/셀 픽셀 쿼리 이스케이프를 써서 수행(`caps/probed.rs`).

**라인 에디터**: `LineEditor`가 `Terminal`·`LineEditBuffer`·완성/검색 상태를 보유(`lineedit/mod.rs:72`~`100`). `read_line`이 `poll_input`→키 매핑→`Action`→`apply_action`→`Surface` 재렌더 루프를 돈다. 키 바인딩 표는 `lineedit/mod.rs:19`~`39`에 문서화. `EditorState`가 활성/검색 모드를 추적.

**위젯**: `Ui`가 위젯 그래프(`Graph`: root/children/parent 맵, fnv 해시)와 각 위젯의 `RenderData`(서피스·커서·좌표)를 보유(`widgets/mod.rs:140`~`190`). `process_event_queue`로 이벤트를 포커스·전파 규칙에 따라 분배, `render_to_screen`으로 합성. 자동 레이아웃은 `cassowary` 제약 솔버 기반(`widgets/layout.rs`, 884행).

---

## 5. 핵심 데이터 구조·타입과 불변식

- `KeyCode` (`input.rs:100`): 디코딩된 키. `InternalPasteStart/End`는 `#[doc(hidden)]`로 붙여넣기 모드 내부 신호이며 외부 이벤트로 누설되지 않는다(파서가 `InputState::Pasting`으로 흡수, `input.rs:1264`~`1289`). `encode`는 `is_down=false`면 빈 문자열을 반환한다(불변식: down 이벤트만 인코딩, `input.rs:247`).
- `InputState` (`input.rs:618`): 입력 파서 상태 머신. `EscapeMaybeAlt`는 단독 ESC를 받은 뒤 다음 키가 오면 ALT 조합으로, 아니면 ESC 단독으로 해소(`input.rs:1290`~`1306`). `Pasting(offset)`은 종료 마커 재탐색 시작 위치를 보존하여 8K 단위 분할 붙여넣기에서 O(n²) 탐색을 피한다(`input.rs:1322`~`1331`).
- `ReadBuffer` (`readbuf.rs:6`): 항상 연속 슬라이스를 보장하는 단순 버퍼. `advance`는 `rotate_left`+`truncate`로 소비분을 앞으로 당긴다(불변식: `as_slice()` 첫 바이트가 다음 디코딩 대상).
- `KeyMap`/`Node` (`keymap.rs:5`,`145`): 바이트 라벨 트라이. 자식은 라벨로 정렬 유지(`binary_search_by`로 삽입/탐색, `keymap.rs:26`~`37`). `lookup`은 `maybe_more` 플래그로 부분 일치를 `Ambiguous`/`NeedData`로 구분. 불변식: 자식이 없는 노드는 반드시 값을 가진다(`keymap.rs:49`의 panic가 위반 시 발동).
- `Capabilities` (`caps/mod.rs:172`): 모든 필드는 비공개이며 접근자로만 노출. `apply_builtin_terminfo`(Windows 전용)는 내장 `xterm-256color` 데이터를 주입하고 색상 단계를 TrueColor로 격상(`caps/mod.rs:201`~`207`).
- `WindowsTerminal` (`terminal/windows.rs:483`): 생성 시 저장한 입력/출력 모드·코드페이지를 `Drop`에서 복원해야 한다는 불변식을 가진다(`terminal/windows.rs:498`~`536`). raw/cooked 모드를 어떻게 조합하든 원래 상태로 되돌린다(`terminal/mod.rs:55`~`62` 트레이트 계약).
- `LineEditBuffer` (`lineedit/buffer.rs:6`): `cursor`는 라인 UTF-8 바이트 인덱스이며 그래프임 수가 아니다. `set_line_and_cursor`는 `is_char_boundary` 단언으로 불변식을 강제(`lineedit/buffer.rs:45`~`54`).
- `Found`/`NodeFind` (`keymap.rs:107`,`123`): 외부 노출은 `Found`, 내부 백트래킹용은 `NodeFind`로 분리.
- `Error`/`InternalError` (`error.rs:12`,`35`): 공개 `Error`는 불투명 newtype, 내부 변종은 `#[doc(hidden)]`·`#[non_exhaustive]`로 은닉. 사실상 private enum을 newtype으로 우회한 패턴.

---

## 6. 외부 의존성

주목할 외부 크레이트와 사용 이유:

- **winapi** (Windows 한정): 콘솔 입출력의 핵심. 사용 모듈 — `consoleapi`, `wincon`, `winuser`, `synchapi`, `winnls`, `handleapi`, `fileapi`, `memoryapi` 등(`Cargo.toml:64`~`78`). `Get/SetConsoleMode`, `ReadConsoleInputW`, `WriteConsoleOutputW`, `WaitForMultipleObjects`, `VK_*` 가상키 코드 매핑에 직접 사용.
- **terminfo**: terminfo DB 로드·능력 질의(`cap::TrueColor`, `MaxColors`, `SetAttributes`, `BackColorErase` 등). 능력 탐지와 terminfo 렌더러 양쪽에서 핵심.
- **filedescriptor**: 콘솔 핸들의 안전한 복제(`dup`)와 소유(`OwnedHandle`). stdio 잠금에서 분리된 독립 핸들 확보(`terminal/windows.rs:562`~`564`).
- **vtparse**: `wezterm-escape-parser`의 저수준 VT 상태 머신 백엔드.
- **cassowary** (feature `widgets`): 제약 기반 자동 레이아웃 솔버(`widgets/layout.rs`).
- **fnv** (feature `widgets`): 작은 `WidgetId` 키에 적합한 빠른 해시(`widgets/mod.rs:8`,`13`).
- **unicode-segmentation**: 그래프임 경계 커서로 라인 에디터의 커서 이동·삽입 처리(`lineedit/buffer.rs:1`,`31`).
- **memmem**: `ReadBuffer::find_subsequence`의 붙여넣기 종료 마커 탐색(`readbuf.rs:1`,`45`).
- **fancy-regex**: 정규식(에러 변종에 포함, `error.rs:42`).
- **image / sha2 / wezterm-blob-leases** (feature `image`/`use_image`): 터미널 인라인 이미지(iTerm2/sixel) 지원.
- **pest / pest_derive** (feature `tmux_cc`): tmux control mode 파싱(`wezterm-escape-parser`로 위임 재노출).
- **thiserror**: 에러 파생. **anyhow**: 외부 에러 흡수. **ordered-float**: NaN 안전 부동소수 비교.

---

## 7. 설정·기능 플래그

`Cargo.toml:50`~`57`의 feature:

| feature | 기본 | 효과 |
| --- | --- | --- |
| `default` | — | `image`, `tmux_cc` 활성 |
| `widgets` | off | `widgets`/`layout` 모듈, `cassowary`+`fnv` |
| `use_serde` | off | 다수 struct에 serde 직렬화(전이적으로 의존 크레이트 serde feature 활성) |
| `image` | on(default) | sha2·blob-leases·escape-parser 이미지 지원 |
| `use_image` | off | 실제 이미지 렌더(kitty SHM 포함). `image`보다 강한 활성화 |
| `tmux_cc` | on(default) | pest 기반 tmux control mode |
| `docs` | off | docs.rs 빌드용 종합 feature |

`#[cfg(feature = "use_serde")]`, `#[cfg(feature = "use_image")]`, `#[cfg(feature = "widgets")]`, `#[cfg(feature = "tmux_cc")]` 분기가 코드 전반에 산재한다(예: `input.rs:10`,`49`; `lib.rs:48`,`61`,`63`).

**런타임 환경변수**: `TERM`/`COLORTERM`/`COLORTERM_BCE`/`TERM_PROGRAM`/`TERM_PROGRAM_VERSION`/`NO_COLOR`(`caps/mod.rs:126`~`142`), `TERMWIZ_BYPASS_VIRTUAL_TERMINAL`(`terminal/windows.rs:584`). 이 크레이트 자체는 `config` 크레이트의 항목을 직접 참조하지 않는다(역방향: `config`가 `termwiz`에 의존). fork 설정 철학상, 색상·키 인코딩 기본 동작을 바꾸려면 `Capabilities`/`ProbeHints` 기본값과 `caps/mod.rs`의 휴리스틱을 수정하는 것이 진입점이다.

---

## 8. Windows 전용 고려사항

- **유일 구현체**: `Terminal`의 구체 타입은 `WindowsTerminal`뿐이고 `SystemTerminal`·`new_terminal`이 이를 가리킨다(`terminal/mod.rs:21`,`109`,`120`). `terminal/mod.rs:17`은 `pub mod windows`만 선언하며 unix 모듈 선언이 없다 — 비Windows 분기는 이미 제거된 상태.
- **콘솔 API 직접 사용 지점**: `terminal/windows.rs` 전체와 `render/windows.rs`, `istty.rs`. `Get/SetConsoleMode`, `Set/GetConsoleCP`/`OutputCP`(코드페이지를 `CP_UTF8`로 강제, `terminal/windows.rs:615`~`616`), `ReadConsoleInputW`, `ReadConsoleOutputW`/`WriteConsoleOutputW`, `ScrollConsoleScreenBufferW`, `CreateEventW`/`SetEvent`/`WaitForMultipleObjects`(waker 및 입력 대기).
- **ConPTY/가상 터미널**: `ENABLE_VIRTUAL_TERMINAL_PROCESSING`+`DISABLE_NEWLINE_AUTO_RETURN` 설정 가능 여부로 ANSI 경로를 택한다(`terminal/windows.rs:574`~`597`). `apply_builtin_terminfo`로 내장 `xterm-256color`를 입혀 win32 콘솔 API 대신 이스케이프를 쓰도록 opt-in(`caps/mod.rs:200`~`207`).
- **`screen_size` 프로빙의 Windows 타이밍**: ConPTY가 dev_attributes 응답을 픽셀 쿼리 응답보다 먼저 보내는 재배열 문제를 100ms 슬립으로 우회(`caps/probed.rs:132`~`144`). 리팩토링 시 이 타이밍 의존을 제거하기 어렵다는 점을 인지해야 한다.
- **Windows 입력 디코딩**: `input.rs:631`~`869`의 `#[cfg(windows)] mod windows`가 `INPUT_RECORD`(키/마우스/버퍼크기)를 `InputEvent`로 변환. `VK_*` 가상키를 `KeyCode`로 매핑.
- **죽은/미해소 경로**: `enter_alternate_screen`/`exit_alternate_screen`의 `Renderer::Windows` 분기는 `// TODO: CreateConsoleScreenBuffer ...` 주석만 있고 미구현(`terminal/windows.rs:735`~`738`,`742`~`758`). 즉 레거시 콘솔 모드에서는 대체 화면이 동작하지 않는다. `WindowsConsoleRenderer`는 이미지·커서 색/모양·타이틀·라인 속성을 무시한다(`render/windows.rs:423`~`468`).
- **`cfg(windows)` 분기 잔재**: `caps/probed.rs:132`의 `cfg!(windows)`는 fork에서 항상 참이므로 실질적으로 상수 분기다. `error.rs`의 `filedescriptor::Error` 등은 크로스플랫폼 타입이나 Windows에서도 유효.

---

## 9. 리팩토링 주의점

- **`render/terminfo.rs`(1297행)와 `input.rs`(1989행)의 비대화**: 색상 단계·SGR·이미지·키 인코딩 분기가 거대한 `match`에 집중되어 있다. 분할 리팩토링 시 미묘한 분기 의존(예: `force_terminfo_render_to_use_ansi_sgr`, `modify_other_keys`, `KeyboardEncoding::CsiU` 조합)을 깨기 쉽다. `KeyCode::encode`의 분기 순서 자체가 의미를 가지므로(ambiguous ctrl, ALT 선행 ESC 등) 순서 보존이 필수.
- **재노출 결합**: `lib.rs`가 의존 크레이트를 `pub use`로 재노출하므로(`cell`, `surface`, `escape`, `color`, `nerdfonts`), 이 경로명은 사실상 공개 API다. 의존 크레이트의 타입 경로 변경이 `termwiz` 사용처(특히 `wezterm-gui`, `mux`) 전반에 파급된다.
- **`Drop` 기반 상태 복원**: `WindowsTerminal::drop`이 콘솔 모드·코드페이지 복원과 DEC private mode 리셋·대체화면 종료를 수행한다(`terminal/windows.rs:498`~`536`). `expect`/`unwrap`을 사용하므로 drop 중 실패가 패닉을 유발할 수 있다. 생성-소멸 쌍의 불변식을 깨면 터미널이 raw 모드에 갇힐 위험.
- **타이밍 의존(100ms 슬립)**: `caps/probed.rs`의 슬립은 ConPTY/tmux 재배열 회피용이며 느린 SSH에서 불완전하다고 주석에 명시(`caps/probed.rs:135`~`143`). 이벤트 기반으로 대체하려면 응답 순서 보장 메커니즘 설계가 선행되어야 한다.
- **입력 파서 상태 머신의 미묘함**: `process_bytes`의 `maybe_more` 의미론(부분 시퀀스 처리)과 `EscapeMaybeAlt`/`Pasting` 전이는 버퍼 경계·붙여넣기 분할 시나리오를 정밀히 다룬다(`input.rs:1311`~`1435`). 테스트(`input.rs:1470`~ 및 `keymap.rs:203`~)가 회귀 방어선이므로 변경 시 반드시 동반 검증.
- **위젯 `WidgetId`의 전역 카운터**: `static WIDGET_ID: AtomicUsize`로 프로세스 전역 단조 증가(`widgets/mod.rs:117`~`132`). 테스트 격리·재현성에 영향. feature `widgets`는 기본 비활성이므로 fork에서 실제 사용 여부 확인 후 정리 가능.
- **레거시 콘솔 렌더러의 기능 결손**: 대체화면 미구현·이미지/커서 모양 무시는 의도된 한계다. 이를 ANSI 경로로 일원화하려면 항상 `apply_builtin_terminfo` 경로를 강제하는 방향이 fork 철학과 부합하나, 가상 터미널 미지원 콘솔에서의 폴백 상실을 감수해야 한다.
- **순환 의존 없음**: deps/usedBy 목록상 `wezterm-char-props`가 양방향에 등장하나, `termwiz`→`wezterm-char-props`(데이터 소비) 방향이며 역방향은 빌드 의존이 아닌 협력 관계로 보인다. Cargo 차원의 순환은 존재하지 않는다.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `src/lib.rs` | 68 | 크레이트 루트. 자체 모듈 선언 + 의존 크레이트 재노출 |
| `src/error.rs` | 188 | 불투명 `Error` newtype, `InternalError` enum, `Context` 트레이트, `bail!`/`ensure!`/`format_err!` 매크로 |
| `src/macros.rs` | 30 | `builder!` 매크로(빌더 메서드 자동 생성) |
| `src/readbuf.rs` | 50 | 연속 슬라이스 보장 입력 버퍼 `ReadBuffer` |
| `src/keymap.rs` | 260 | 키 시퀀스 트라이 `KeyMap`/`Node`, `Found` 결과 enum (테스트 포함) |
| `src/input.rs` | 1989 | 입력 파서·키/마우스 이벤트·`KeyCode::encode`·Windows `INPUT_RECORD` 디코딩 (가장 큰 입력 모듈) |
| `src/istty.rs` | 21 | `IsTty` 트레이트, Windows 콘솔 핸들 판별 |
| `src/caps/mod.rs` | 564 | `Capabilities`/`ProbeHints`/`ColorLevel` 능력 탐지 휴리스틱 (테스트 포함) |
| `src/caps/probed.rs` | 234 | `ProbeCapabilities`: XTVERSION·화면 크기 능동 프로빙, `XtVersion` |
| `src/terminal/mod.rs` | 127 | `Terminal` 트레이트, `ScreenSize`/`Blocking`, `new_terminal`, `SystemTerminal` 별칭 |
| `src/terminal/windows.rs` | 861 | `WindowsTerminal` 및 콘솔 입출력 핸들, Win32 콘솔 API 전반, 렌더러 선택, Drop 복원 |
| `src/terminal/buffered.rs` | 122 | `BufferedTerminal`: `Surface`+`Terminal` 래퍼, 델타 flush·리사이즈 감지 |
| `src/render/mod.rs` | 9 | `RenderTty` 트레이트, 렌더러 모듈 선언 |
| `src/render/terminfo.rs` | 1297 | terminfo/ANSI 이스케이프 렌더러 `TerminfoRenderer` (가장 큰 모듈) |
| `src/render/windows.rs` | 476 | Win32 콘솔 API 렌더러 `WindowsConsoleRenderer`, `ScreenBuffer` |
| `src/lineedit/mod.rs` | 861 | `LineEditor` 본체, 키 바인딩, `read_line` 루프, `line_editor_terminal` |
| `src/lineedit/actions.rs` | 31 | `Action`/`Movement`/`RepeatCount` enum |
| `src/lineedit/buffer.rs` | 182 | `LineEditBuffer`: 그래프임 인식 커서·삽입·삭제·이동 |
| `src/lineedit/history.rs` | 131 | `History` 트레이트·`BasicHistory`·검색(`SearchStyle`/`Direction`) |
| `src/lineedit/host.rs` | 122 | `LineEditorHost` 트레이트·`NopLineEditorHost`·`CompletionCandidate`·`OutputElement` |
| `src/widgets/mod.rs` | 540 | `Widget`/`Ui`/`WidgetId`, 위젯 그래프·이벤트 분배·합성 (feature `widgets`) |
| `src/widgets/layout.rs` | 884 | cassowary 제약 기반 자동 레이아웃, `Constraints`/`DimensionSpec` |
| `data/xterm-256color` 등 | — | 컴파일된 terminfo DB와 `.terminfo` 소스. `apply_builtin_terminfo`·테스트가 `include_bytes!`로 소비 (생성/외부 데이터) |
| `benches/cell.rs` | — | 셀 벤치마크(criterion) |
| `examples/*.rs` | — | 사용 예제(hello, line_editor, widgets_basic 등). 빌드 검증용 |

생성/외부 데이터 표기: `data/` 디렉터리의 terminfo 파일은 시스템과 무관한 결정적 테스트·Windows ANSI 폴백을 위해 포함된 외부 데이터다. 본 크레이트 `src` 내에는 거대 생성 테이블 파일이 없으며, 유니코드/NerdFonts 생성 테이블은 의존 크레이트 `wezterm-char-props`에 위치한다(재노출만 수행).
