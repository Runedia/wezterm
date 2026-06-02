# term 폴더 기능 명세

본 문서는 `E:\Project\wezterm\term`에 위치한 크레이트 `wezterm-term`의 기능 명세다. 추후 리팩토링을 위한 상세 참조 자료로 작성되었으며, 모든 기술은 `term/` 하위 소스 코드 확인에 근거한다. 코드 인용은 `파일경로:라인` 형식을 따른다.

본 저장소는 WezTerm의 Windows 전용 영구 분기다. upstream 동기화를 포기했고, 비Windows 플랫폼 코드는 이미 제거되었다. 다만 `wezterm-term` 자체는 플랫폼 비특정 코어 모델이므로 대부분이 Windows/비Windows 양쪽에서 동일하게 동작하며, 플랫폼 분기는 극소수다(8장 참조).

---

## 1. 개요 및 책임

`wezterm-term`은 WezTerm의 **가상 터미널 에뮬레이터 코어 모델**이다. GUI도, PTY도 직접 다루지 않는다. 책임은 단일하다.

- 입력: PTY로부터 받은 바이트 스트림(`advance_bytes`)을 받아 escape 시퀀스를 파싱·해석하고, 그 결과를 화면 셀 모델(`Screen`)에 반영한다.
- 출력: 키보드·마우스 입력 이벤트와 일부 escape 시퀀스의 답신(answerback)을 인코딩하여, 호출자가 제공한 `Box<dyn std::io::Write>`(PTY 입력단)에 기록한다.

크레이트 doc 주석이 책임을 명확히 한다: "This crate does not provide any kind of gui, nor does it directly manage a PTY; you provide a `std::io::Write` implementation ... and supply bytes to the model via the `advance_bytes` method."(`term/src/lib.rs:11`)

제공 기능 범위(`term/src/lib.rs:6`):

- 터미널 escape 시퀀스 파싱 및 적용(CSI/OSC/ESC/DCS/Sixel/Kitty)
- 키보드·마우스 입력 인코딩
- 스크롤백을 포함한 화면 셀 모델
- Sixel, iTerm2, Kitty 이미지 프로토콜
- OSC 8 하이퍼링크 및 다양한 셀 속성

진입점은 `Terminal` 구조체이며(`term/src/lib.rs:16`), 실제 상태는 `TerminalState`가 보유한다. `Terminal`은 `TerminalState`를 `Deref`/`DerefMut`로 노출한다(`term/src/terminal.rs:92`).

### Windows fork에서의 실제 책임

코어 모델 자체는 플랫폼 독립적이다. Windows 분기에서의 고유 책임은 ConPTY(Windows 콘솔 PTY) 계층의 동작 특성에 맞춰 리사이즈·줄바꿈·초기 타이틀 변경 동작을 보정하는 데 있다. 이는 `enable_conpty_quirks` 플래그를 중심으로 구현되어 있다(8장 참조). 또한 붙여넣기 시 기본 줄바꿈 정규화를 CRLF로 설정하는 분기가 `cfg!(windows)`로 존재한다(`term/src/config.rs:117`).

---

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 사용 목적 |
| --- | --- |
| `wezterm-bidi` | 양방향(BiDi) 텍스트 방향 힌트(`ParagraphDirectionHint`) 및 라인 BiDi 정보 적용 |
| `wezterm-cell` | 셀 모델(`Cell`, `CellAttributes`), 색상 타입, 이미지 데이터, grapheme 폭 계산, `UnicodeVersion` |
| `wezterm-dynamic` | `FromDynamic`/`ToDynamic` 파생(설정·직렬화 호환 타입) |
| `wezterm-escape-parser` | escape 시퀀스 파서(`Parser`)와 `Action`/`CSI`/`OSC`/`Esc`/`Sixel`/`KittyImage` 타입 |
| `wezterm-surface` | 라인 모델(`Line`), 시퀀스 번호(`SequenceNo`), 커서 형상/가시성, 텍스처 좌표, `ImageData` |

기타 외부 의존성은 6장 참조.

### 피의존(usedBy)

| 크레이트 | 비고 |
| --- | --- |
| `codec` | 원격 mux 프로토콜 직렬화 |
| `color-funcs` | 색상 유틸리티 |
| `config` | 설정 계층(`TerminalConfiguration` 구현체 제공) |
| `mux` | 탭/페인 멀티플렉서 — 주 소비처 |
| `mux-lua` | mux의 Lua 바인딩 |
| `wezterm` | CLI 진입점 |
| `wezterm-client` | mux 클라이언트 |
| `wezterm-font` | 폰트 셰이핑(셀/속성 참조) |
| `wezterm-gui` | GUI 렌더러 — 주 소비처 |
| `wezterm-mux-server` / `wezterm-mux-server-impl` | mux 서버 |

### 계층상의 위치

`wezterm-term`은 파서(`wezterm-escape-parser`)와 셀/라인 모델(`wezterm-cell`, `wezterm-surface`) 위에 얹혀, 바이트 스트림을 화면 상태로 변환하는 **터미널 코어 계층**이다. 상위 계층인 `mux`(터미널 인스턴스 소유·관리)와 `wezterm-gui`(렌더링)가 이 모델을 읽어 화면을 그리고, 입력을 다시 이 모델을 통해 PTY로 전달한다.

---

## 3. 공개 API 표면

### 3.1 진입점 — `Terminal` (`term/src/terminal.rs:85`)

| 항목 | 시그니처/용도 |
| --- | --- |
| `Terminal::new` | `(size: TerminalSize, config: Arc<dyn TerminalConfiguration + Send + Sync>, term_program: &str, term_version: &str, writer: Box<dyn Write + Send>) -> Terminal`. 터미널 인스턴스 생성. `term_program`/`term_version`은 `\033[>q` 식별 응답에 사용. |
| `Terminal::advance_bytes` | `<B: AsRef<[u8]>>(&mut self, bytes: B)`. PTY 출력 바이트를 파서에 공급. seqno 증가 → `Performer`로 파싱·적용 → 미관측 출력 알림 트리거(`term/src/terminal.rs:164`). |
| `Terminal::perform_actions` | `(&mut self, actions: Vec<Action>)`. 이미 파싱된 `Action` 벡터를 직접 적용(원격 mux 경로). |
| `Deref/DerefMut` | `Target = TerminalState`. `Terminal`은 `TerminalState`의 모든 공개 메서드를 그대로 노출. |

### 3.2 상태 모델 — `TerminalState` (`term/src/terminalstate/mod.rs:237`)

주요 공개 메서드(전수가 아닌 카테고리별 요약):

- 생성·설정: `new`, `enable_conpty_quirks`, `set_config`, `get_config`, `set_clipboard`, `set_device_control_handler`, `set_notification_handler`, `set_download_handler`.
- 시퀀스 번호: `current_seqno`, `increment_seqno`(변경 추적의 핵심, 5장 참조).
- 조회: `get_title`, `get_progress`, `get_current_dir`, `palette`, `palette_mut`, `screen`, `screen_mut`, `get_size`, `cursor_pos`, `pen`, `user_vars`, `is_mouse_grabbed`, `is_alt_screen_active`, `bracketed_paste_enabled`, `has_unseen_output`, `get_reverse_video`, `get_keyboard_encoding`.
- 동작: `resize`, `make_all_lines_dirty`, `erase_scrollback`, `erase_scrollback_and_viewport`, `send_paste`, `focus_changed`, `get_semantic_zones`, `implicit_palette_reset_if_same_as_configured`.
- 입력 인코딩: `key_down`/`key_up`(`term/src/terminalstate/keyboard.rs:60`), `mouse_event`(`term/src/terminalstate/mouse.rs:322`).

### 3.3 호스트 콜백 트레이트 (`term/src/terminal.rs`)

호스트(GUI/mux)가 구현하여 터미널이 외부 세계와 상호작용하게 하는 인터페이스다. 모두 `Send + Sync`.

| 트레이트 | 메서드 | 용도 |
| --- | --- | --- |
| `Clipboard` (`:13`) | `set_contents(selection, data)` | OSC 52 등 클립보드 쓰기. `ClipboardSelection::{Clipboard, PrimarySelection}`. |
| `DeviceControlHandler` (`:31`) | `handle_device_control(control)` | DCS(Device Control) 처리 위임. |
| `AlertHandler` (`:76`) | `alert(alert: Alert)` | 벨, 타이틀 변경, 토스트 알림, 팔레트 변경, 진행률 등 호스트 통지. |
| `DownloadHandler` (`:80`) | `save_to_downloads(name, data)` | iTerm2 파일 전송 중 비인라인 파일 저장. |

`Alert` enum(`term/src/terminal.rs:47`)은 호스트에 전달되는 이벤트 종류를 정의한다: `Bell`, `ToastNotification{title,body,focus}`, `CurrentWorkingDirectoryChanged`, `IconTitleChanged`, `WindowTitleChanged`, `TabTitleChanged`, `PaletteChanged`, `SetUserVar`, `OutputSinceFocusLost`, `Progress(Progress)`.

### 3.4 설정 트레이트 — `TerminalConfiguration` (`term/src/config.rs:135`)

호스트가 구현하여 런타임 설정을 코어에 주입하는 트레이트. `Downcast + Debug + Send + Sync`. 대부분의 메서드는 기본 구현을 가진다(소스 기본값 수정 철학과 직접 연결되는 지점). 5장 및 7장 참조.

### 3.5 재노출(re-export)

`lib.rs`는 하위 모듈과 의존 크레이트 타입을 광범위하게 재노출한다(`term/src/lib.rs:26`–`:42`): `config::*`, `input::*`, `wezterm_cell::*`, `wezterm_surface::line::*`, `screen::*`, `terminal::*`, `terminalstate::*`, `color`. 따라서 `wezterm-term`은 소비처에 셀/라인 타입까지 통합 노출하는 사실상의 파사드(facade)다.

### 3.6 좌표 인덱스 타입 (`term/src/lib.rs:44`–`:80`)

서로 다른 크기·부호의 타입을 의도적으로 분리하여, 컴파일러가 잘못된 좌표 혼용을 잡도록 설계되었다.

| 타입 | 정의 | 의미 |
| --- | --- | --- |
| `PhysRowIndex` | `usize` | `screen.lines`의 물리 인덱스. 0 = 스크롤백 최상단. |
| `VisibleRowIndex` | `i64` | 가시 영역 행 인덱스. 0 = 첫 가시 행. (부호 분리 목적의 signed) |
| `ScrollbackOrVisibleRowIndex` | `i32` | 스크롤백으로 음수 인덱싱 가능. 32비트로 의도적 분리. |
| `StableRowIndex` | `isize` | 스크롤백 최상단 기준 논리 행. 스크롤백 purge에도 안정적. |

---

## 4. 내부 구조

### 4.1 모듈 분해

| 모듈 | 역할 |
| --- | --- |
| `lib.rs` | 크레이트 루트, 좌표 타입, 재노출, `Position`/`CursorPosition`/`SemanticZone`, escape 접두어 상수. |
| `terminal.rs` | `Terminal` 파사드, 호스트 콜백 트레이트, `Alert`/`Progress`, `TerminalSize`, `ClipboardSelection`. |
| `config.rs` | `TerminalConfiguration` 트레이트, `NewlineCanon`(붙여넣기 줄바꿈 정규화), `BidiMode`. |
| `input.rs` | `MouseButton`/`MouseEventKind`/`MouseEvent`, `ClickPosition`, `LastMouseClick`(멀티클릭 streak). |
| `color.rs` | `ColorPalette`, `Palette256`(256색 + fg/bg/cursor/selection/scrollbar/split), 기본 XTerm 팔레트. |
| `screen.rs` | `Screen` — 라인 컨테이너, 스크롤·리사이즈·리랩, 좌표 변환, 논리 라인 순회. (1155행, 비대) |
| `terminalstate/mod.rs` | `TerminalState` 본체, escape 시퀀스 해석의 대부분(커서/편집/모드/장치/윈도/SGR), `ThreadedWriter`. (2741행, 최대 비대 모듈) |
| `terminalstate/performer.rs` | `Performer` — 파서 `Action` 디스패처. 출력 인쇄, control/ESC/CSI/OSC/DCS 분기. (1102행) |
| `terminalstate/keyboard.rs` | 키 이벤트 → 바이트 인코딩(`key_up_down`). |
| `terminalstate/mouse.rs` | 마우스 이벤트 → X10/UTF8/SGR/SGR-Pixels 인코딩. |
| `terminalstate/image.rs` | 이미지 셀 배치 공통 로직(`ImageAttachParams`, `assign_image_to_cells`), iTerm2 이미지. |
| `terminalstate/iterm.rs` | iTerm2 인라인 이미지/파일 전송 처리(`set_image`). |
| `terminalstate/sixel.rs` | Sixel 그래픽 디코딩 → RGBA → 이미지 셀 배치. |
| `terminalstate/kitty.rs` | Kitty 이미지 프로토콜(전송/배치/프레임/삭제/응답, 메모리 예산). (974행) |
| `test/` | 모델·escape 처리 단위 테스트(`c0`, `c1`, `csi`, `mod`; `selection`은 미연결). |

### 4.2 제어/데이터 흐름

출력(PTY → 화면) 경로:

```
PTY bytes
  → Terminal::advance_bytes (seqno++)
    → Parser::parse  (wezterm-escape-parser)
      → Performer::perform(action)            (performer.rs:252)
        ├ Print/PrintString → print() 버퍼 누적 → flush_print()  (grapheme 분해·폭계산·셀 기록)
        ├ Control           → control()        (CR/LF/BS/TAB/벨/SI·SO 등)
        ├ CSI               → csi_dispatch()   → TerminalState::perform_csi_*
        ├ OSC               → osc_dispatch()   (타이틀·색상·하이퍼링크·알림·CWD·진행률)
        ├ Esc               → esc_dispatch()   (charset·커서 저장/복원·DECALN·RIS)
        ├ DeviceControl     → device_control() (DECRQSS 등 + 핸들러 위임)
        ├ Sixel             → sixel()
        └ KittyImage        → kitty_img()
  → trigger_unseen_output_notif()  (포커스 잃은 상태에서 출력 시 Alert)
```

`Performer`는 `TerminalState`를 `Deref`로 감싸며(`performer.rs:38`), drop 시 잔여 `print` 버퍼를 flush한다(`performer.rs:52`). 인쇄 텍스트는 grapheme 단위로 모아 cell에 기록하므로(`flush_print`, `performer.rs:117`), 조합 문자·이모지 폭 계산이 정확해진다.

입력(이벤트 → PTY) 경로:

```
key_down/key_up   → KeyCode::encode(...) → writer.write_all  (keyboard.rs:33)
mouse_event       → 좌표 clamp → press/release/move/wheel 분기 → SGR/X10 인코딩 → writer  (mouse.rs:322)
send_paste(text)  → bracketed paste 래핑 + 줄바꿈 정규화 + de-fang → writer  (mod.rs:813)
```

모든 출력은 `BufWriter<ThreadedWriter>`를 통한다(`mod.rs:346`). `ThreadedWriter`는 별도 스레드로 데이터를 전송하여 쓰기 측이 절대 블록되지 않게 한다 — 대용량 붙여넣기 시의 교착(deadlock)을 방지하기 위한 설계다(`mod.rs:420`–`:484`).

### 4.3 비대 모듈

- `terminalstate/mod.rs`(2741행): escape 시퀀스 해석 로직의 대부분을 한 파일에 집중. `perform_csi_mode`(`:1419`), `perform_csi_edit`(`:2129`), `perform_csi_cursor`(`:2327`), `perform_csi_sgr`(`:2627`)가 각각 수백 행 규모. 리팩토링 시 1차 분할 후보.
- `screen.rs`(1155행): 리사이즈/리랩(`rewrap_lines`, `resize`)과 마진 인지 스크롤(`scroll_up_within_margins`, `scroll_down_within_margins`)이 복잡도의 핵심.
- `terminalstate/kitty.rs`(974행): Kitty 이미지 프로토콜의 전송 누적·프레임 합성·메모리 예산 관리.

---

## 5. 핵심 데이터 구조·타입

### 5.1 `TerminalState` (`term/src/terminalstate/mod.rs:237`)

터미널의 전체 가변 상태를 보유하는 중앙 구조체. 50개 이상의 필드를 가진다. 핵심 그룹:

- 화면: `screen: ScreenOrAlt`(주/대체 화면 전환). `pen: CellAttributes`(다음 인쇄 속성), `cursor: CursorPosition`, `wrap_next: bool`(지연 줄바꿈).
- 모드 플래그: `dec_auto_wrap`, `reverse_wraparound_mode`, `reverse_video_mode`, `dec_origin_mode`, `insert`, `application_cursor_keys`, `application_keypad`, `bracketed_paste`, `newline_mode`, `sixel_display_mode` 등.
- 마진: `top_and_bottom_margins: Range<VisibleRowIndex>`, `left_and_right_margins: Range<usize>`, `left_and_right_margin_mode`.
- 입력 모드: `mouse_encoding: MouseEncoding`, `mouse_tracking`/`button_event_mouse`/`any_event_mouse`, `keyboard_encoding`, `current_mouse_buttons`.
- charset: `g0_charset`/`g1_charset`(`CharSet`), `shift_out`.
- 메타: `title`, `icon_title`, `progress`, `current_dir: Option<Url>`, `user_vars`, `palette: Option<ColorPalette>`(미설정 시 config 값 사용).
- 핸들러: `clipboard`, `device_control_handler`, `alert_handler`, `download_handler`.
- 이미지: `image_cache: LruCache<[u8;32], Arc<ImageData>>`(용량 16, sha 키), `color_map`(그래픽 색 레지스터), `kitty_img: KittyImageState`.
- 변경 추적: `seqno: SequenceNo`. 포커스 추적: `focused`, `lost_focus_seqno`, `lost_focus_alerted_seqno`.
- Windows 보정: `enable_conpty_quirks`, `suppress_initial_title_change`.

**불변식**:

- `palette`가 `None`이면 색상은 항상 `config.color_palette()`에서 가져온다(`palette()`, `:653`). 동적 색상 escape가 처음 들어올 때만 fork된다(`palette_mut()`, `:663`). config가 변경되어 동일해지면 override를 제거하여 라이브 갱신을 허용한다(`implicit_palette_reset_if_same_as_configured`, `:674`).
- `seqno`는 단조 증가한다. 화면 변경마다 라인의 `last_change_seqno`를 갱신하여, 소비처가 `get_changed_stable_rows`로 변경분만 다시 그리게 한다.
- `cursor.x`는 항상 좌우 마진 내. `set_cursor_pos`가 origin mode/마진을 고려해 클램프한다(`:978`).

### 5.2 `ScreenOrAlt` (`term/src/terminalstate/mod.rs:134`)

주 화면(스크롤백 보유)과 대체 화면(스크롤백 없음)을 하나로 묶고, `alt_screen_is_active`로 전환. `Deref`/`DerefMut`로 활성 화면을 투명 노출(`:143`). 전환 시 StableRowIndex 0..num_rows의 의미가 바뀌므로 상단 물리 행을 dirty 처리하여 mux 캐시 무효화를 유도한다(`dirty_top_phys_rows`, `:209`).

### 5.3 `Screen` (`term/src/screen.rs:15`)

- `lines: VecDeque<Line>`: 스크롤백 전체 용량으로 할당. 인덱스 0 = 최상단, 후미 N개가 가시 행.
- `stable_row_index_offset: usize`: 스크롤백 purge 누적량. `PhysRowIndex ↔ StableRowIndex` 변환의 기준.
- `physical_rows`/`physical_cols`/`dpi`, `keyboard_stack: Vec<KeyboardEncoding>`(Kitty 키보드 모드 스택), `saved_cursor: Option<SavedCursor>`.

**불변식**: `phys_row(visible)`는 `lines.len() - physical_rows + clamp(row)`로 계산(`:464`). `StableRowIndex`는 절대 음수가 아니며, purge로 사라진 행은 `stable_row_to_phys`에서 `None`을 반환(`:527`). 리랩 시 wrap 표식이 있는 라인을 논리 라인으로 합친 뒤 새 폭으로 재분할하되, 커서가 열 0에 놓이는 특수 케이스를 보정한다(`rewrap_lines`, `:142`).

### 5.4 `CursorPosition` (`term/src/lib.rs:107`)

`{x: usize, y: VisibleRowIndex, shape, visibility, seqno}`. `SavedCursor`(`mod.rs:124`)는 위치 외에 `wrap_next`, `pen`, `dec_origin_mode`, g0/g1 charset까지 저장하여 DECSC/DECRC를 정확히 복원한다.

### 5.5 `ColorPalette` / `Palette256` (`term/src/color.rs:46`, `:10`)

256색 배열 + foreground/background/cursor_(fg,bg,border)/selection_(fg,bg)/scrollbar_thumb/split. `compute_default`(`:102`)가 XTerm 16색 + 216 색 큐브 + 24 그레이스케일을 구성한다. `Palette256`의 `Debug`는 `[suppressed]`로 출력하여 로그 폭주를 막는다(`:61`).

### 5.6 `ImageAttachParams` / `ImageAttachStyle` / `PlacementInfo` (`term/src/terminalstate/image.rs:20`, `:58`, `:13`)

Sixel/iTerm2/Kitty 세 경로가 공유하는 이미지 셀 배치 파라미터. `assign_image_to_cells`(`:65`)가 픽셀 치수를 셀 격자로 환산하여 셀에 `ImageCell`을 부착한다.

### 5.7 입력 타입 (`term/src/input.rs`)

`MouseButton`(휠 4방향 포함), `MouseEventKind`, `MouseEvent`(셀 좌표 + 픽셀 오프셋 + 수정자), `LastMouseClick`(500ms `CLICK_INTERVAL` 내 동일 셀·동일 버튼이면 streak 증가, `:80`).

### 5.8 `NewlineCanon` (`term/src/config.rs:7`)

붙여넣기 줄바꿈 정규화 모드(`None`/`LineFeed`/`CarriageReturn`/`CarriageReturnAndLineFeed`). `canonicalize`(`:25`)가 `\r`/`\n`/`\r\n`을 목표 표현으로 변환. 기본값은 Windows에서 CRLF(`:117`, 8장 참조).

---

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
| --- | --- |
| `anyhow` | 오류 전파(`Result`). |
| `bitflags` | 키 수정자·Kitty 키보드 플래그 등 비트 플래그. |
| `csscolorparser` | Sixel HSL 색 정의를 RGB로 변환(`sixel.rs:95`). |
| `downcast-rs` | `TerminalConfiguration`을 구체 타입으로 downcast(`config.rs:135`). |
| `finl_unicode` | grapheme cluster 분해(`Graphemes`, `performer.rs:6`). 셀 단위 정확한 그룹화. |
| `hex` | (이미지 키 등) 16진 처리. |
| `humansize` | 이미지 메모리 사용량 사람 친화 표기(`image.rs:3`). |
| `image` | 이미지 디코딩/리샘플링(Sixel·iTerm2·Kitty). |
| `lazy_static` | 기본 팔레트(`color.rs:90`), terminfo DB(`mod.rs:38`) 정적 초기화. |
| `lru` | 이미지 디코드 결과 캐시(`image_cache`, `mod.rs:348`). |
| `miniz_oxide` | Kitty 이미지 zlib 압축 해제. |
| `num-traits` | `FromPrimitive`/`ToPrimitive`(동적 색 번호 등). |
| `ordered-float` | iTerm2 셀 크기 응답의 `NotNan<f32>`(`performer.rs:9`). |
| `serde` | `use_serde` feature 시 직렬화(`rc` 포함). |
| `terminfo` | 내장 `wezterm` terminfo DB 로드, XTGETTCAP 응답(`mod.rs:39`, `xt_get_tcap`). |
| `unicode-normalization` | 출력 텍스트의 NFC 정규화(설정 시, `performer.rs:14`). |
| `url` | OSC 7 작업 디렉터리(`Url`, `mod.rs:341`). |
| `wezterm-bidi` | BiDi 방향. |
| `termwiz` | `KeyCode`/`Modifiers`/`KeyboardEncoding` 및 키 인코딩 로직(`input.rs:10`, `keyboard.rs:4`). |

dev-dependency: `env_logger`, `k9`(스냅샷 assert).

---

## 7. 설정·기능 플래그

### 7.1 Cargo feature

`Cargo.toml`에 feature는 `use_serde` 하나뿐이다(`term/Cargo.toml:13`). 활성화 시 termwiz 및 의존 셀/escape/surface 크레이트의 `use_serde`를 전파하여, 주요 모델 타입에 `Serialize`/`Deserialize`를 부여한다(원격 mux 직렬화 경로용). `wezterm-cell`/`wezterm-escape-parser`/`wezterm-surface`는 항상 `use_image` 등의 feature와 함께 켜진다.

### 7.2 `TerminalConfiguration` 항목 (`term/src/config.rs:135`)

호스트(보통 `config` 크레이트)가 구현하며, 대부분 기본값을 가진다. 소스 기본값 수정 철학상 이 기본값들이 직접 수정 대상이 될 수 있다.

| 메서드 | 기본값 | 의미 |
| --- | --- | --- |
| `generation()` | 0 | 설정 세대 카운터. 변경 시 증가시켜 캐시 무효화. |
| `scrollback_size()` | 3500 | 스크롤백 행 수. |
| `enable_csi_u_key_encoding()` | false | CSI-u(fixterms) 키 인코딩. |
| `color_palette()` | (구현 필수) | 초기 색 팔레트. |
| `canonicalize_pasted_newlines()` | `NewlineCanon::default()` | 붙여넣기 줄바꿈 정규화(Windows=CRLF). |
| `alternate_buffer_wheel_scroll_speed()` | 3 | 대체 화면 휠 스크롤 시 방향키 반복 수. |
| `enq_answerback()` | "" | ENQ 응답 문자열. |
| `enable_kitty_graphics()` | false | Kitty 이미지 프로토콜 활성. |
| `enable_kitty_keyboard()` | false | Kitty 키보드 프로토콜 활성. |
| `unicode_version()` | `{version:9, ambiguous_are_wide:false}` | 폭 계산 기준 유니코드 버전. `config/src/lib.rs:default_unicode_version`와 결합(`config.rs:189` 주석 명시). |
| `normalize_output_to_unicode_nfc()` | false | 출력 NFC 정규화. |
| `debug_key_events()` | false | 키 이벤트 로그 레벨 상향. |
| `bidi_mode()` | `{enabled:false, LeftToRight}` | 기본 BiDi 모드. |
| `enable_title_reporting()` | false | 타이틀 보고(보안상 기본 비활성). |
| `enable_checksum_rectangular_area()` | false | DECRQCRA 사각 체크섬. |
| `log_unknown_escape_sequences()` | false | 미처리 escape 경고 로깅. |

> 리팩토링 주의: `unicode_version()`의 기본값(9)은 `config` 크레이트의 `default_unicode_version`과 명시적으로 결합되어 있다(`term/src/config.rs:189`). 한쪽만 바꾸면 폭 계산 불일치가 발생한다.

---

## 8. Windows 전용 고려사항

이 크레이트는 본질적으로 플랫폼 독립적 코어다. 플랫폼 분기는 ConPTY 보정과 붙여넣기 줄바꿈에 한정된다.

### 8.1 ConPTY 보정(`enable_conpty_quirks`)

- 활성화: `enable_conpty_quirks()`가 `enable_conpty_quirks`와 `suppress_initial_title_change`를 함께 켠다(`mod.rs:577`). 상위 계층이 ConPTY 백엔드일 때 호출.
- 리사이즈 동작: `Screen::resize`의 `resize_preserves_scrollback = is_conpty`(`screen.rs:287`). Windows PTY 계층은 가변 스크롤백(bottom gravity)과 잘 맞지 않아 커서를 위로 과도하게 이동·화면을 손상시키므로, ConPTY에서는 스크롤백을 불변으로 취급하여 하단에 빈 행을 추가하는 보수적 경로를 택한다. 주석이 이를 명시(`screen.rs:280`–`:287`). 이 동작은 ssh로 원격 unix에 접속한 경우에는 나타나지 않는다.
- 줄바꿈 wrap 표식: ConPTY일 때 줄 끝이 알파뉴메릭/구두점일 때만 wrapped로 표시한다(`makes_sense_to_wrap`, `performer.rs:175`–`:192`). ConPTY의 잘못된 wrap 보고를 완화.
- 초기 타이틀 억제: ConPTY가 기동 직후 보내는 `SetIconNameAndWindowTitle` OSC를 1회 무시한다(`performer.rs:254`, `mod.rs:361` 주석).
- tmux 타이틀 상태 복구: ConPTY가 `ESC k TITLE ST`를 재작성하며 ST를 제거하는 버그에 대응해, escape 파싱 상태 전이 시 누적 중인 tmux 타이틀을 폐기한다(`pop_tmux_title_state`, `performer.rs:238`–`:250`).

### 8.2 붙여넣기 줄바꿈

`NewlineCanon::default()`가 `cfg!(windows)`에서 CRLF를 반환한다(`config.rs:117`). 단, 임베디드 앱이 bracketed paste를 켰으면 그것을 "앱이 unix 줄바꿈을 선호한다"는 신호로 보고 정규화를 하지 않는다(`send_paste`, `mod.rs:819`). 결과적으로 cmd.exe에는 CRLF, WSL/vim에는 unix 줄바꿈이 전달된다.

### 8.3 죽은 코드

- `NewlineCanon::default()`의 `else` 분기(비Windows, `\r` 기본)는 Windows 빌드에서는 죽은 경로다(`config.rs:119`–`:126`).
- `cfg(unix)`/`cfg(target_os=...)` 형태의 직접 플랫폼 분기는 이 크레이트 소스에 존재하지 않는다(전수 grep 확인). 즉 비Windows 분기는 위 한 곳의 런타임 `cfg!` 표현뿐이다.

### 8.4 Windows API 사용

이 크레이트는 winapi/windows 등 Windows 네이티브 API를 직접 호출하지 않는다. Windows 고유성은 전적으로 ConPTY의 *동작 특성에 대한 보정* 수준이다.

---

## 9. 리팩토링 주의점

1. **`seqno` 불변식의 광범위 결합**: 변경 추적은 모든 화면 변형이 라인의 `last_change_seqno`를 정확히 갱신한다는 전제에 의존한다. 스크롤·리사이즈·셀 기록 경로 중 하나라도 갱신을 누락하면, mux/GUI가 변경을 그리지 못한다(`get_changed_stable_rows`, `screen.rs:909`). 새 변형 메서드 추가 시 반드시 seqno 갱신을 동반해야 한다.

2. **좌표 인덱스 타입 혼용 위험**: `PhysRowIndex`/`VisibleRowIndex`/`StableRowIndex`/`ScrollbackOrVisibleRowIndex`는 의도적으로 분리된 타입이다(`lib.rs:44`). 변환은 반드시 `Screen`의 `phys_row`/`stable_range`/`phys_to_stable_row_index` 계열을 거쳐야 하며, 직접 캐스팅은 스크롤백 purge 시 오프바이원·패닉을 유발한다.

3. **`screen.rs`의 리사이즈/리랩 복잡도**: `rewrap_lines`(`screen.rs:100`)와 `resize`(`:193`)는 ConPTY 분기, 커서 열-0 특수 케이스, 후미 공백 라인 prune, 용량 재예약이 얽혀 있다. 가장 깨지기 쉬운 영역이며, 회귀 테스트(`test/`)로 보호해야 한다.

4. **`terminalstate/mod.rs` 비대(2741행)**: escape 해석 대부분이 한 파일·한 `impl`에 모여 있다. `perform_csi_*` 군을 하위 모듈로 분할할 수 있으나, 모두 `TerminalState`의 사적 필드에 직접 접근하므로(예: 마진·커서·pen) 단순 이동만으로는 가시성 조정이 필요하다.

5. **`Performer`의 `Deref` 결합**: `Performer`가 `TerminalState`로 `DerefMut`되어(`performer.rs:46`), 메서드 분배가 암묵적이다. 일부 로직은 `Performer`(인쇄 버퍼 보유)에, 일부는 `TerminalState`에 있어 경계가 모호하다. `csi_dispatch`에서 `self.state.perform_csi_*`로 명시 호출하는 부분(`performer.rs:489`)과 `self.c1_*`처럼 deref 경유 호출이 혼재한다.

6. **`ThreadedWriter`의 비동기 쓰기**: 출력이 별도 스레드로 비동기 전송된다(`mod.rs:436`). `write`는 즉시 성공을 반환하므로(`:471`), 실제 쓰기 실패는 호출 지점에서 관측되지 않는다(BrokenPipe는 다음 send에서만 표면화). answerback 응답 순서를 동기 가정하는 변경은 위험하다.

7. **팔레트 override 3-상태 로직**: `palette: Option<ColorPalette>`는 "config 추종(None)" vs "fork됨(Some)"의 2상태이며, 동적 색 escape마다 `implicit_palette_reset_if_same_as_configured`로 자동 회수된다(`performer.rs:956` 등 다수 호출). 이 회수를 빠뜨리면 config 라이브 갱신이 멈춘다.

8. **순환 의존 부재, 단방향 계층**: `wezterm-term`은 deps 5개에만 의존하고 상위 11개 크레이트가 단방향으로 의존한다. 공개 API(특히 `TerminalState`/`Screen`/`Alert`/`TerminalConfiguration`) 변경은 mux·gui·codec까지 광범위하게 파급된다. 시그니처 변경 전 피의존 목록(2장) 전체 영향 평가 필요.

9. **이미지 메모리 예산 하드코딩**: Kitty 이미지 예산은 `320MB` 하드코딩(`kitty.rs:47`, `FIXME` 주석), 이미지 캐시는 16개 하드코딩(`mod.rs:559`). 설정화 후보.

10. **`test/selection.rs` 미연결**: `test/mod.rs:9`에서 `mod selection`이 주석 처리("FIXME: port to render layer")되어 있다. 선택 영역 로직이 렌더 계층으로 이동했음을 시사하므로, 관련 리팩토링 시 이 분리를 전제해야 한다.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `term/Cargo.toml` | 48 | 패키지·의존성·`use_serde` feature 정의. |
| `term/src/lib.rs` | 134 | 크레이트 루트, 좌표 타입, 재노출, `Position`/`CursorPosition`/`SemanticZone`, escape 접두 상수. |
| `term/src/terminal.rs` | 186 | `Terminal` 파사드, 호스트 콜백 트레이트, `Alert`/`Progress`, `TerminalSize`. |
| `term/src/config.rs` | 243 | `TerminalConfiguration` 트레이트, `NewlineCanon`, `BidiMode`. |
| `term/src/input.rs` | 98 | 마우스 입력 타입, 멀티클릭 streak. |
| `term/src/color.rs` | 193 | `ColorPalette`/`Palette256`, 기본 XTerm 팔레트 생성. |
| `term/src/screen.rs` | 1155 | `Screen` 모델: 스크롤·리사이즈·리랩·좌표 변환·논리 라인 순회. (비대) |
| `term/src/terminalstate/mod.rs` | 2741 | `TerminalState` 본체, escape 해석 대부분, `ThreadedWriter`. (최대 비대) |
| `term/src/terminalstate/performer.rs` | 1102 | `Performer` 파서 Action 디스패처, 인쇄·control·ESC·CSI·OSC·DCS 분기. |
| `term/src/terminalstate/keyboard.rs` | 67 | 키 이벤트 → 바이트 인코딩. |
| `term/src/terminalstate/mouse.rs` | 364 | 마우스 이벤트 → X10/UTF8/SGR/SGR-Pixels 인코딩. |
| `term/src/terminalstate/image.rs` | 319 | 이미지 셀 배치 공통 로직, iTerm2 이미지 데이터 변환. |
| `term/src/terminalstate/iterm.rs` | 153 | iTerm2 인라인 이미지/파일 전송(`set_image`). |
| `term/src/terminalstate/sixel.rs` | 156 | Sixel 디코딩 → RGBA → 이미지 셀 배치. |
| `term/src/terminalstate/kitty.rs` | 974 | Kitty 이미지 프로토콜(전송/배치/프레임/삭제/응답/메모리 예산). |
| `term/src/test/mod.rs` | 1375 | 테스트 하네스(`TestTerm`, `TestTermConfig`)와 다수 단위 테스트. |
| `term/src/test/c0.rs` | 50 | C0 제어 코드 테스트. |
| `term/src/test/c1.rs` | 78 | C1 제어 코드 테스트. |
| `term/src/test/csi.rs` | 403 | CSI 시퀀스 테스트. |
| `term/src/test/selection.rs` | 100 | 선택 영역 테스트 — **미연결**(`mod.rs:9` 주석, 렌더 계층 이관 예정). |

> 생성 파일: 이 크레이트(`term/`) 내에는 생성 데이터 테이블 파일이 없다. (거대 유니코드 데이터 테이블은 `wezterm-char-props`·`wezterm-gui` 등 타 크레이트 소관이다.)
