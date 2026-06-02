# wezterm-input-types 폴더 기능 명세

## 1. 개요 및 책임

`wezterm-input-types`는 WezTerm의 **입력 관련 기초 타입 어휘(vocabulary types)** 를 정의하는 단일 크레이트다. 키보드/마우스 입력과 창 장식(window decoration)을 표현하는 자료형과, 그 자료형을 터미널 프로토콜용 이스케이프 시퀀스로 직렬화하는 인코딩 로직을 한곳에 모은다.

단일 책임은 다음과 같이 요약된다.

- 키 식별자(`KeyCode`, `PhysKeyCode`), 수정자(`Modifiers`), LED 상태(`KeyboardLedStatus`), 키/마우스 이벤트(`KeyEvent`, `RawKeyEvent`, `MouseEvent`) 등 입력 도메인의 **공용 타입을 정의**한다.
- 이 타입들을 xterm 수정자 인코딩, Kitty 키보드 프로토콜, win32 입력 모드(`win32-input-mode`)로 **인코딩**하는 변환 함수를 제공한다.
- 문자열 ↔ 타입 상호 변환(`TryFrom<&str>`/`Display`)과 `wezterm-dynamic` 직렬화 트레이트(`FromDynamic`/`ToDynamic`) 구현을 통해 **설정 파일과의 직렬화 경계**를 형성한다.

이 크레이트는 의도적으로 의존성이 얕고(`wezterm-dynamic` 한 개만 워크스페이스 의존), 로직보다 타입 정의가 중심이다. `#![cfg_attr(not(feature = "std"), no_std)]` 선언으로 `no_std` 빌드까지 고려한 저수준 기반 크레이트다.

Windows fork 관점에서 실제 책임은 변하지 않으나, 인코딩 분기 중 `win32-input-mode` 경로(`encode_win32_input_mode`)와 `RawKeyEvent::scan_code`가 본 플랫폼의 핵심 경로이고, macOS·X11 전용 상수(`WindowDecorations`의 `MACOS_*` 플래그, `UIKeyCapRendering`의 `AppleSymbols`/`UnixLong` 등)는 대부분 죽은 경로로 남는다.

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 |
|------|----------|
| 의존(deps) | wezterm-dynamic |
| 피의존(usedBy) | config, termwiz, termwiz-funcs(lua-api-crates), wezterm-escape-parser, wezterm-font, wezterm-surface, window |

이 크레이트는 입력 처리 파이프라인의 **최하단 어휘 계층**에 위치한다. 상위의 창 시스템(`window`)이 OS 입력을 받아 여기 정의된 타입으로 정규화하고, 터미널 모델 계층(`termwiz`, `wezterm-escape-parser`, `wezterm-surface`)과 설정 계층(`config`)이 그 타입을 소비·직렬화하는 구조다. 즉 입력 데이터가 흐르는 모든 계층이 공유하는 공통 타입 정의 지점이다.

## 3. 공개 API 표면

크레이트는 단일 모듈(`lib.rs`)로 모든 항목을 크레이트 루트에 직접 노출한다. 별도 하위 모듈은 없다.

**좌표 타입 별칭** (`wezterm-input-types/src/lib.rs:23-27`)
- `pub struct PixelUnit;`, `pub struct ScreenPixelUnit;` — euclid 단위 태그.
- `pub type Point = euclid::Point2D<isize, PixelUnit>;` — 창 좌상단 기준 정수 좌표.
- `pub type PointF = euclid::Point2D<f32, PixelUnit>;` — 부동소수 좌표.
- `pub type ScreenPoint = euclid::Point2D<isize, ScreenPixelUnit>;` — 화면 좌표.

**키 식별자**
- `pub enum KeyCode` (`:34`) — 논리 키. 주요 메서드: `is_modifier(&self) -> bool`(`:126`), `normalize_shift(&self, Modifiers) -> (KeyCode, Modifiers)`(`:147`), `composed(s: &str) -> Self`(`:151`, 단일 문자는 `Char`로 축약), `to_phys(&self) -> Option<PhysKeyCode>`(`:169`, US ANSI 기준 물리키 환원). `TryFrom<&str>`(`:330`), `Display`(`:453`) 구현.
- `pub enum PhysKeyCode` (`:751`) — ANSI US 배열 물리 위치 기반 키. 메서드: `is_modifier(&self)`(`:875`), `to_key_code(self) -> KeyCode`(`:889`). `TryFrom<&str>`(`:1224`), `Display`(`:1235`) 구현.

**수정자·LED**
- `pub struct Modifiers: u16` (bitflags, `:496`) — 메서드: `encode_xterm(self) -> u8`(`:560`), `to_string_with_separator(&self, ModifierToStringArgs) -> String`(`:575`), `remove_positional_mods(self) -> Self`(`:736`). `TryFrom<String>`(`:514`)·`From<&Modifiers> for String`(`:544`)·`Display`(`:720`).
- `pub struct ModifierToStringArgs<'a>` (`:550`) — `separator`, `want_none`, `ui_key_cap_rendering` 필드.
- `pub struct KeyboardLedStatus: u8` (bitflags, `:470`) — `CAPS_LOCK`, `NUM_LOCK`. `Display`(`:476`).

**이벤트 타입**
- `pub struct MouseButtons: u8`(bitflags, `:1247`), `pub enum MousePress`(`:1259`), `pub enum MouseEventKind`(`:1266`), `pub struct MouseEvent`(`:1275`).
- `pub struct Handled(Arc<AtomicBool>)` (`:1286`) — 이벤트 처리 표시 플래그. `new()`, `set_handled()`, `is_handled()`.
- `pub struct RawKeyEvent` (`:1318`) — 조합/IME 이전 원시 키 이벤트. 메서드: `set_handled(&self)`(`:1343`). (`kitty_function_code`는 비공개.)
- `pub struct KeyEvent` (`:1489`) — 처리/조합 후 키 이벤트. 메서드: `normalize_shift(self)`(`:1552`), `resurface_positional_modifier_key(self)`(`:1564`), `normalize_ctrl(self)`(`:1641`), `encode_win32_input_mode(&self) -> Option<String>`(`:1650`), `encode_kitty(&self, KittyKeyboardFlags) -> String`(`:1713`).

**프로토콜·UI 보조 타입**
- `pub struct KittyKeyboardFlags: u16` (bitflags, `:2031`) — Kitty 키보드 프로토콜 진행 플래그.
- `pub struct WindowDecorations: u8` (bitflags, `:2045`) — 창 장식. `TryFrom<String>`/`From<&_> for String`/`Default`(TITLE|RESIZE).
- `pub enum IntegratedTitleButton`(`:2126`), `pub enum IntegratedTitleButtonAlignment`(`:2133`), `pub enum IntegratedTitleButtonStyle`(`:2141`) — 통합 타이틀바 버튼 설정.
- `pub enum UIKeyCapRendering` (`:2270`) — 키캡 렌더링 스타일(기본값 `WindowsSymbols`).

**자유 함수**
- `pub fn is_ascii_control(c: char) -> Option<char>` (`:1527`).
- `pub fn ctrl_mapping(c: char) -> Option<char>` (`:2229`) — 문자 → Ctrl 제어코드.

## 4. 내부 구조

단일 파일 `lib.rs`(약 3,196행)에 모든 것이 들어 있는 평면 구조다. 모듈 분해는 없으며, 논리적 영역은 다음과 같이 나뉜다.

1. 좌표/단위 별칭(`:23-27`).
2. `KeyCode` 정의 및 변환 로직(`:34-466`) — `to_phys`(`:169`)와 `TryFrom<&str>`(`:330`)의 대형 `match`가 비대하다.
3. `KeyboardLedStatus`·`Modifiers`와 그 문자열화/인코딩(`:468-745`) — `to_string_with_separator`(`:575`)는 7열짜리 표(label/unix/emacs/apple/windows/win_sym)를 순회하며 UI 스타일별 키캡을 생성한다.
4. `PhysKeyCode` 정의 및 매핑(`:751-1243`) — `to_key_code`(`:889`)와 `for_each_code`(`:1011`) 매크로가 전 키 목록을 두 번 열거한다. `std` 빌드에서는 `LazyLock` 정적 맵(`PHYSKEYCODE_MAP`, `INV_PHYSKEYCODE_MAP`, `:1217-1222`)을 캐싱하고, `no_std`에서는 선형 탐색으로 대체한다.
5. 마우스 타입과 `Handled`(`:1245-1314`).
6. 이벤트 타입과 인코딩(`:1316-2016`) — **가장 비대한 영역**. `kitty_function_code`(`:1349`)와 `encode_kitty`(`:1713`)가 Kitty 프로토콜 코드 포인트와 numpad/NumLock 분기, 레거시/대체 키 인코딩을 거대한 `match`로 구현한다.
7. 인코딩 보조 함수(`csi_u_encode`·`us_layout_unshift`·`ctrl_mapping`, `:2018-2266`).
8. 창 장식·UI 타입(`:2045-2282`).
9. 테스트 모듈(`:2285-3196`) — Kitty 인코딩 회귀 테스트(이슈 번호별). 파일 행수의 약 28%를 차지한다.

제어/데이터 흐름: OS 이벤트 → `RawKeyEvent` 구성 → `KeyEvent`로 정규화(`normalize_shift`/`normalize_ctrl`/`resurface_positional_modifier_key`) → 활성 프로토콜에 따라 `encode_kitty` 또는 `encode_win32_input_mode`로 직렬화.

## 5. 핵심 데이터 구조·타입

- **`KeyCode`** (`:34`): 논리 키. 불변식 — `Backspace`/`Tab`/`Enter`/`Escape`/`Delete`는 별도 variant가 없고 각각 `Char('\u{8}')`/`Char('\t')`/`Char('\r')`/`Char('\u{1b}')`/`Char('\u{7f}')`로 표현한다(`:47-78` 주석). `composed`은 단일 문자를 `Composed`이 아닌 `Char`로 정규화한다.
- **`PhysKeyCode`** (`:751`): ANSI US 물리 위치. `Copy` 가능. `KeyCode`와 양방향 매핑(`to_phys`/`to_key_code`)을 가지나, 비라틴 배열에서는 의미가 어긋날 수 있다는 점을 `to_phys` 주석(`:164-168`)이 명시한다. 문자열 이름 변환은 `for_each_code` 열거가 단일 출처(single source of truth)다.
- **`Modifiers`** (`:496`): `u16` 비트플래그. 논리 수정자(SHIFT/ALT/CTRL/SUPER/LEADER)와 위치 수정자(LEFT_*/RIGHT_*)·`ENHANCED_KEY`를 함께 담는다. 불변식 — 키 할당 매칭 시에는 위치/보조 비트를 `remove_positional_mods`(`:736`)로 제거해야 한다. `LEADER`는 OS에 없는 WezTerm 가상 수정자다(`:504-505`).
- **`KeyboardLedStatus`** (`:470`): `CAPS_LOCK`/`NUM_LOCK` 비트. Kitty 인코딩에서 numpad 코드 분기와 수정자 비트(64/128)에 영향.
- **`Handled`** (`:1286`): `Arc<AtomicBool>` 래퍼. 불변식 — `PartialEq`가 **항상 true**를 반환한다(`:1308-1311`). 이벤트 상등 비교에서 처리 상태를 무시하기 위함이며, 이벤트 구조체의 파생 `PartialEq`가 깨지지 않게 하는 의도적 설계다.
- **`RawKeyEvent`** (`:1318`): 조합 이전 원시 이벤트. `phys_code: Option<PhysKeyCode>`, `raw_code: u32` 보유. Windows에서만 `scan_code: u32` 필드 존재(`:1329-1330`).
- **`KeyEvent`** (`:1489`): 처리 후 이벤트. `raw: Option<RawKeyEvent>`로 원시 이벤트를 역참조한다. Windows에서만 `win32_uni_char: Option<char>` 필드 존재(`:1508-1509`).
- **`WindowDecorations`** (`:2045`): 창 장식 비트. 그림자 제어는 `Option<bool>` 의미를 흉내내기 위해 2비트(`MACOS_FORCE_DISABLE_SHADOW`=4, `MACOS_FORCE_ENABLE_SHADOW`=4|8)를 예약한다(`:2049-2052`). 기본값은 `TITLE|RESIZE`.

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
|----------|-----------|
| `bitflags` | `Modifiers`, `KeyboardLedStatus`, `MouseButtons`, `KittyKeyboardFlags`, `WindowDecorations` 비트플래그 정의. |
| `euclid` | 타입 단위(`PixelUnit`/`ScreenPixelUnit`)가 부여된 `Point2D` 좌표 타입 제공. `std` 비활성 시 `euclid/std`도 함께 꺼진다. |
| `serde` | 옵션 의존. `KeyCode`/`Modifiers`/`PhysKeyCode`/`WindowDecorations`에 `Serialize`/`Deserialize` 파생. `rc` 기능으로 `Arc` 직렬화 지원. |
| `wezterm-dynamic` | `FromDynamic`/`ToDynamic` 파생을 통한 WezTerm 동적 설정값(Lua/JSON형) 직렬화. 본 워크스페이스 내부 유일 의존. |

## 7. 설정·기능 플래그

Cargo feature(`wezterm-input-types/Cargo.toml:18-20`):
- `default = ["serde", "std"]`.
- `serde` — `Serialize`/`Deserialize` 파생 활성화. 비활성 시 `#[cfg_attr(feature = "serde", ...)]`로 보호된 모든 직렬화 코드가 제거된다.
- `std` — 표준 라이브러리 사용. `serde/std`와 `euclid/std`를 전파한다. 비활성 시 `no_std`(`:1`)로 빌드되며, `PhysKeyCode`의 이름 매핑이 `LazyLock` 정적 `HashMap` 대신 선형 탐색(`name_to_code`/`to_name`의 `cfg(not(feature = "std"))` 분기, `:1181-1213`)으로 동작한다.

관련 config 항목: 본 크레이트의 타입들이 `config` 크레이트에서 설정 값으로 직접 노출된다. 대표적으로 `WindowDecorations`(`window_decorations`), `IntegratedTitleButton*`(통합 타이틀바 설정), `UIKeyCapRendering`(키캡 표기 스타일), 그리고 `Modifiers`의 문자열 표기(`SHIFT|CTRL` 등)가 키 할당 설정의 직렬화 형식이다. 본 크레이트 자체에는 config 정의가 없고, 직렬화 가능한 타입만 제공한다.

## 8. Windows 전용 고려사항

- **활성 Windows 경로**:
  - `RawKeyEvent::scan_code` 필드는 `#[cfg(windows)]`로만 존재(`:1329-1330`).
  - `KeyEvent::win32_uni_char` 필드도 `#[cfg(windows)]` 전용(`:1508-1509`).
  - `encode_win32_input_mode`(`:1650`)는 `scan_code`를 직접 참조하므로 **Windows에서만 컴파일·동작**한다. ConPTY의 win32-input-mode(`\x1b[...._` 시퀀스)를 생성하며, `dwControlKeyState` 비트(SHIFT/ENHANCED/좌우 ALT·CTRL)를 Modifiers로부터 구성한다(`:1657-1690`).
- **죽은 경로(비Windows 잔존 코드)**:
  - `WindowDecorations`의 `MACOS_FORCE_DISABLE_SHADOW`/`MACOS_FORCE_ENABLE_SHADOW`/`MACOS_FORCE_SQUARE_CORNERS`/`MACOS_USE_BACKGROUND_COLOR_AS_TITLEBAR_COLOR`(`:2051-2055`) 및 해당 문자열 변환 분기.
  - `IntegratedTitleButtonStyle::MacOsNative`/`Gnome`(`:2143-2146`). 다만 `FromDynamic` 구현(`:2160-2169`)은 `MacOsNative`를 문자열로 받지 않고 `Windows`/`Gnome`만 허용한다(`possible` 목록에는 셋 다 표기되나 매치 분기에는 `MacOsNative` 없음 — 잠재적 불일치).
  - `UIKeyCapRendering`의 `AppleSymbols`/`UnixLong`/`Emacs` 및 `to_string_with_separator`의 apple/unix/emacs 열(`:584-589`, `:706-712`). Windows fork에서는 기본값 `WindowsSymbols`가 사용된다.
  - `KeyCode`의 `to_phys` 주석은 X11/non-latin 배열을 전제로 한 한계를 언급하나, 함수 자체는 플랫폼 무관하게 동작한다.
- 직접적인 Windows API(`winapi`/`windows-sys`) 호출은 없다. win32 상수는 코드 내 리터럴로 정의(`:1658-1663`)되어 OS 의존성을 두지 않는다.

## 9. 리팩토링 주의점

- **불변식 위반 위험**:
  - `Handled::eq`가 항상 true(`:1308`)라는 점은 `KeyEvent`/`RawKeyEvent`의 `PartialEq`/`Eq` 파생을 성립시키는 전제다. 이를 일반 `AtomicBool` 비교로 바꾸면 이벤트 상등 비교 시맨틱이 깨진다.
  - `KeyCode`에서 Backspace/Tab/Enter/Escape/Delete가 `Char(...)`로 표현된다는 규약(`:47-78`)은 `to_phys`, `encode_kitty`, `TryFrom<&str>` 전반이 의존한다. variant를 추가하면 다수 `match`를 동시 수정해야 한다.
- **거대 `match`의 중복**: `KeyCode`↔`PhysKeyCode`↔Kitty 코드 매핑이 `to_phys`(`:169`), `to_key_code`(`:889`), `kitty_function_code`(`:1349`), `encode_kitty`(`:1713`)에 분산·중복되어 있다. 키를 추가/변경할 때 네 곳을 동기화해야 하며 누락 시 조용한 오인코딩으로 이어진다.
- **`PhysKeyCode` 이름 목록 중복**: enum 정의·`to_key_code`·`for_each_code` 매크로가 전 키를 각각 열거한다. `for_each_code`의 목록(`:1028-1135`)에는 enum에 있는 `F21`–`F24`가 빠져 있어 이름 매핑에서 누락된다. 추가 시 정합성 점검 필요.
- **US ANSI 레이아웃 하드코딩**: `us_layout_unshift`(`:2187`)와 `ctrl_mapping`(`:2229`)이 US 배열을 가정한다. 코드 주석이 비US 배열에서의 부정확성을 명시한다. OS 레이아웃 질의로 대체하려면 상위 계층 plumbing이 필요하다(`:1919-1921` FIXME).
- **결합도**: 7개 상위 크레이트가 이 타입들을 직접 사용한다. 공개 enum의 variant 추가/삭제나 구조체 필드 변경은 워크스페이스 전반에 파급한다. 특히 `serde`/`wezterm-dynamic` 직렬화 형식 변경은 사용자 설정 파일 호환성에 직접 영향.
- **순환 의존 없음**: 내부 의존은 `wezterm-dynamic` 단방향뿐이며 순환은 없다.
- **`IntegratedTitleButtonStyle` 수동 `FromDynamic`**: 파생이 아닌 수작업 구현(`:2149`)이므로 variant 추가 시 자동 갱신되지 않는다. `MacOsNative` 누락 분기(8절)는 정리 후보다.
- **`no_std` 분기 유지비**: `std` feature 기준 두 갈래의 `name_to_code`/`to_name` 구현을 유지한다. 본 워크스페이스가 사실상 `std`만 쓴다면 `no_std` 경로는 검증되지 않는 부채일 수 있다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|------|-----------|------|
| `wezterm-input-types/Cargo.toml` | 21 | 패키지 메타·의존성(bitflags/euclid/serde/wezterm-dynamic)·feature(`serde`,`std`) 정의. |
| `wezterm-input-types/src/lib.rs` | 3,196 | 입력 도메인 전 타입·인코딩·문자열 변환·테스트를 담은 단일 모듈. 구현 약 2,280행, 테스트 약 910행. |
| `wezterm-input-types/LICENSE.md` | — | MIT 라이선스 텍스트. |

(생성 파일 없음. `lib.rs`는 손으로 작성된 단일 소스이며 본 크레이트에 빌드 생성물·데이터 테이블 파일은 존재하지 않는다.)
