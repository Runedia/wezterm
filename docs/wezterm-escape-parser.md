# wezterm-escape-parser 폴더 기능 명세

본 문서는 `E:\Project\wezterm\wezterm-escape-parser` 폴더(단일 크레이트 `wezterm-escape-parser`)의 기능 명세다. 추후 리팩토링을 위해 "무엇을·왜·어떻게"를 코드 근거와 함께 기술한다. 코드 인용은 `파일경로:라인` 형식을 따른다.

본 저장소는 WezTerm의 Windows 전용 영구 분기이며 upstream 동기화를 포기한 상태다. 이 크레이트의 소스에는 `cfg(unix)`/`cfg(target_os = "android")` 분기가 일부 잔존하나, Windows 빌드에서는 컴파일되지 않는 죽은 경로다(8장 참조).

---

## 1. 개요 및 책임

`wezterm-escape-parser`는 터미널 이스케이프 시퀀스의 **인코딩·디코딩 전용** 크레이트다. `wezterm-escape-parser/src/lib.rs:6-9` 모듈 문서가 책임 범위를 명시한다. "바이트 스트림에 의미(semantic)를 부여해 파싱하고, 의미 값을 다시 이스케이프 시퀀스로 인코딩한다. 단, 터미널 에뮬레이션 자체는 제공하지 않는다."

단일 책임은 다음 두 방향의 변환이다.

- **디코딩**: 바이트열 → `Action`(`lib.rs:42-62`). `Parser`(`parser/mod.rs:64-67`)가 상태 기계를 구동해 C0/C1 제어 코드, CSI, OSC, ESC, DCS(Sixel·DECRQSS·XTGETTCAP·tmux), APC(Kitty 이미지)를 의미 타입으로 분해한다.
- **인코딩**: 의미 타입 → 바이트열. 모든 핵심 타입이 `Display`를 구현해 원본 시퀀스를 재생성한다(예: `lib.rs:102-129`의 `impl Display for Action`).

이 크레이트는 화면 버퍼·커서 상태·렌더링을 다루지 않는다. 그러한 에뮬레이션은 상위 크레이트(`wezterm-term` 등)의 책임이다. 즉, 본 크레이트는 "프로토콜 직렬화/역직렬화 계층"이며, 그 위에 셀 모델(`wezterm-cell`)과 터미널 모델(`wezterm-term`)이 쌓인다.

Windows fork에서의 실제 책임도 동일하다. 다만 Kitty 이미지 프로토콜의 공유 메모리 적재 경로(`apc.rs`)에서 Windows API를 직접 사용하는 부분이 유일한 플랫폼 종속 코드다(8장).

`no_std` 호환을 목표로 설계되었다. `lib.rs:5`에서 `#![cfg_attr(not(feature = "std"), no_std)]`로 선언하고 `lib.rs:16-17`에서 `alloc`을 가져온다. 단, 실제 워크스페이스 소비자(termwiz·term)는 모두 `std` feature를 켠다.

---

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 용도 |
| --- | --- |
| `vtparse` | DEC ANSI 상태 기계의 저수준 엔진. `Parser`가 `VTParser`를 보유하고 `VTActor`를 구현해 콜백을 받는다(`parser/mod.rs:13,65,210`). |
| `wezterm-blob-leases` | (optional, `image` feature) Kitty 이미지 블롭의 임대 기반 저장. 에러 변환에 사용(`error.rs:66-67`). |
| `wezterm-color-types` | `SrgbaTuple`·`LinearRgba` 색 타입. 색 인코딩·선형 변환에 사용(`color.rs:4`, `lib.rs:14`). |
| `wezterm-dynamic` | `FromDynamic`/`ToDynamic` 직렬화 트레이트. 설정 직렬화 대상 타입(`Hyperlink`, `RgbColor` 등)에 구현(`hyperlink.rs:5`, `color.rs:5`). |
| `wezterm-input-types` | `KittyKeyboardFlags` 재노출 등 입력 비트플래그(`csi.rs:151`). |

기타 비워크스페이스 의존: `base64`·`hex`(데이터 인/디코드), `bitflags`(`Selection` 등), `num-derive`/`num-traits`(정수↔enum 변환), `ordered-float`(부동소수 키), `thiserror`(에러), 그리고 optional `image`·`nix`·`pest`/`pest_derive`·`serde`·`sha2`·`winapi`(`Cargo.toml:12-48`).

### 피의존(usedBy)

| 크레이트 | 사용 방식 |
| --- | --- |
| `termwiz` | `std` feature로 의존. use_serde/use_image/image/tmux_cc feature를 본 크레이트로 전파(`termwiz/Cargo.toml:46,53-56`). |
| `wezterm-cell` | 셀 속성(SGR 등) 의미 타입을 소비(`wezterm-cell/Cargo.toml:23`). |
| `wezterm-surface` | 표면(서피스) 모델에서 사용(`wezterm-surface/Cargo.toml:27`). |
| `wezterm-term` | `std`+`use_image` feature로 의존. 터미널 에뮬레이션의 입력 디코더(`term/Cargo.toml:38`). |

### 계층 위치

본 크레이트는 터미널 처리 파이프라인의 **최하단 프로토콜 계층**이다. 바이트 스트림을 의미 `Action`으로 바꾸어 상위 셀/서피스/터미널 모델에 공급하고, 역방향으로 의미 타입을 시퀀스 바이트로 직렬화한다. `vtparse`(상태 기계)와 도메인 의미 타입(SGR·CSI·OSC·Kitty 등) 사이의 어댑터 역할을 한다.

---

## 3. 공개 API 표면

### 크레이트 루트(`lib.rs`)

- `enum Action`(`lib.rs:42-62`): 파서가 방출하는 최상위 의미 단위. 변형: `Print(char)`, `PrintString(String)`, `Control(ControlCode)`, `DeviceControl(DeviceControlMode)`, `OperatingSystemCommand(Box<…>)`, `CSI(CSI)`, `Esc(Esc)`, `Sixel(Box<Sixel>)`, `XtGetTcap(Vec<String>)`, `KittyImage(Box<KittyImage>)`.
  - `Action::append_to(self, dest: &mut Vec<Self>)`(`lib.rs:69-87`): 인접한 `Print`를 `PrintString`으로 병합해 힙 사용을 줄이는 누적기.
  - `impl Display for Action`(`lib.rs:102-129`): `Action` → 시퀀스 바이트 재생성.
- `enum ControlCode`(`lib.rs:476-542`): C0(0x00–0x1f)·C1(0x82–0x9f) 제어 코드. `#[repr(u8)]`+`FromPrimitive`.
- `enum DeviceControlMode`(`lib.rs:221-238`): DCS 모드. `Enter`/`Exit`/`Data(u8)`/`ShortDeviceControl`/`TmuxEvents`(tmux_cc feature 한정).
- `struct ShortDeviceControl`(`lib.rs:136-146`): 자체 완결형 짧은 DCS(예: DECRQSS).
- `struct EnterDeviceControlMode`(`lib.rs:192-201`): 장기 DCS 모드 진입 파라미터.
- `struct Sixel`(`lib.rs:281-305`) + `enum SixelData`(`lib.rs:402-438`) + `type SixelValue = u8`(`lib.rs:400`): Sixel 이미지 의미 모델. `Sixel::dimensions()`(`lib.rs:309-348`)는 래스터 속성 부재 시 데이터로 치수 계산.
- `struct OneBased`(`lib.rs:546-617`): 1-기반 파라미터 안전 래퍼. `new`/`from_zero_based`/`from_esc_param`/`from_esc_param_with_big_default`/`from_optional_esc_param`/`as_zero_based`/`as_one_based`. 0을 1 또는 max로 매핑하는 관례를 캡슐화.
- 재노출(`lib.rs:33-37`): `KittyImage`, `CSI`, `Error`, `Result`, `Esc`, `EscCode`, `OperatingSystemCommand`.

### `parser` 모듈(`parser/mod.rs`)

- `struct Parser`(`parser/mod.rs:64-67`): 상태 기계 보유. 스트리밍 입력 가능.
  - `Parser::new()`/`Default`(`parser/mod.rs:69-81`).
  - `parse<F: FnMut(Action)>(&mut self, bytes, callback)`(`parser/mod.rs:92-124`): 콜백 방식. tmux_cc 모드 바이패스 분기 포함.
  - `parse_first(&mut self, bytes) -> Option<(Action, usize)>`(`parser/mod.rs:130-164`): 첫 액션 1개와 소비 길이.
  - `parse_as_vec(&mut self, bytes) -> Vec<Action>`(`parser/mod.rs:166-170`).
  - `parse_first_as_vec(&mut self, bytes) -> Option<(Vec<Action>, usize)>`(`parser/mod.rs:175-193`): 첫 시퀀스의 모든 액션을 수집하고 ground 상태 보장.

### `csi` 모듈(`csi.rs`)

- `enum CSI`(`csi.rs:109-136`): `Sgr`/`Cursor`/`Edit`/`Mode`/`Device(Box)`/`Mouse`/`Window(Box)`/`Keyboard`/`SelectCharacterPath`/`Unspecified(Box)`.
  - `CSI::parse(params, parameters_truncated, control) -> impl Iterator<Item = CSI>`(`csi.rs:1708-1719`): 하나의 CSI 시퀀스가 다중 의미를 담을 수 있어(예: `CSI 1;3 m`) 이터레이터를 반환. 의미 미상 잔여는 `CSI::Unspecified`로 포장.
- 하위 의미 enum 다수: `Sgr`(`csi.rs:1461`), `SgrCode`(`csi.rs:2868`), `Cursor`(`csi.rs:984`), `Edit`(`csi.rs:1122`), `Mode`(`csi.rs:789`), `Device`(`csi.rs:445`), `MouseReport`(`csi.rs:660`), `Window`(`csi.rs:528`), `Keyboard`(`csi.rs:162`), `Intensity`·`Underline`·`Blink`·`VerticalAlign`·`Font`, `DecPrivateMode`/`DecPrivateModeCode`(`csi.rs:864,870`), `TerminalMode`/`TerminalModeCode`(`csi.rs:961,967`), `XtSmGraphics` 관련(`csi.rs:333-390`), `DeviceAttribute(s)`(`csi.rs:279-323`) 등.
- 재노출: `KittyKeyboardFlags`(`csi.rs:151`), `LinearRgba`/`SrgbaTuple`(`color.rs:4`).

### `osc` 모듈(`osc.rs`)

- `enum OperatingSystemCommand`(`osc.rs:33-55`): 창/아이콘 제목, 하이퍼링크, 셀렉션, 시스템 알림, iTerm 독자 확장, FinalTerm 시맨틱 프롬프트, 색 변경(`ChangeColorNumber`/`ChangeDynamicColors`/`ResetDynamicColor`/`ResetColors`), CWD, RxvtExtension, ConEmuProgress, `Unspecified`.
  - `OperatingSystemCommand::parse(osc: &[&[u8]]) -> Self`(`osc.rs:155-`): 실패 시 `Unspecified`로 폴백(`osc.rs:156-160`).
- 보조 타입: `ColorOrQuery`(`osc.rs:18`), `DynamicColorNumber`(`osc.rs:59`), `ChangeColorPair`(`osc.rs:73`), `Selection`(bitflags, `osc.rs:80`), `OperatingSystemCommandCode`(`osc.rs:406`), `FinalTermClick`/`FinalTermPromptKind`/`FinalTermSemanticPrompt`(`osc.rs:622-701`), `Progress`(`osc.rs:879`), `ITermProprietary`/`ITermUnicodeVersionOp`/`ITermFileData`/`ITermDimension`(`osc.rs:888-1105`).

### `esc` 모듈(`esc.rs`)

- `enum Esc`(`esc.rs:5-14`): `Unspecified{intermediate,control}` / `Code(EscCode)`.
- `enum EscCode`(`esc.rs:25-110`): RIS·IND·NEL·HTS·RI·DECSC/DECRC·문자셋 지정·DECDHL 등. intermediate+control을 u16로 패킹한 판별식(`esc!` 매크로 `esc.rs:16-23`).
- `Esc::parse(intermediate: Option<u8>, control: u8) -> Esc`(`esc.rs:113-118`): 미인식 시 `Unspecified` 폴백.

### `apc` 모듈(`apc.rs`)

- `enum KittyImage`(`apc.rs:1002-1039`): Kitty 그래픽 프로토콜 명령. `TransmitData`/`TransmitDataAndDisplay`/`Display`/`Delete`/`Query`/`TransmitFrame`/`ComposeFrame`.
  - `KittyImage::parse_apc(data: &[u8]) -> Option<Self>`(`apc.rs:1054-`): APC 페이로드 파싱. `G`로 시작하지 않으면 `None`.
  - `KittyImage::verbosity()`(`apc.rs:1042-1052`).
- 보조 타입: `KittyImageData`(`apc.rs:20`)와 `load_data`(`apc.rs:177-`, kitty-shm feature), `KittyImageTransmit`(`apc.rs:516`), `KittyImagePlacement`(`apc.rs:575`), `KittyImageFrame`/`KittyImageFrameCompose`(`apc.rs:926,831`), `KittyImageVerbosity`/`KittyImageFormat`/`KittyImageCompression`/`KittyImageDelete`/`KittyFrameCompositionMode`.

### `color` 모듈(`color.rs`)

- `enum AnsiColor`(`color.rs:14-47`), `struct RgbColor`(`color.rs:57-60`, u32 비트 팩), `type PaletteIndex = u8`(`color.rs:220`), `enum ColorSpec`(`color.rs:228-234`).
- `RgbColor` 생성/변환: `new_8bpc`/`new_f32`/`to_tuple_rgb8`/`to_tuple_rgba`/`to_linear_tuple_rgba`/`from_named`/`from_rgb_str`/`from_named_or_rgb_string`/`to_rgb_string`/`to_x11_16bit_rgb_string`.

### `hyperlink` 모듈(`hyperlink.rs`)

- `struct Hyperlink`(`hyperlink.rs:11-17`): `params`/`uri`/`implicit`. 생성자 다수(`new`/`new_implicit`/`new_with_id`/`new_with_params`)와 `parse(osc) -> Result<Option<Hyperlink>>`(`hyperlink.rs:76-97`), `compute_shape_hash`(`hyperlink.rs:24-31`).

### `error` 모듈(`error.rs`)

- `struct Error`(`error.rs:13`, `Box<InternalError>` 뉴타입), `type Result<T>`(`error.rs:16`), `enum InternalError`(`error.rs:38-82`, `#[doc(hidden)]`), `trait Context<T,E>`(`error.rs:143-155`).
- 매크로 재노출: `format_err!`·`bail!`·`ensure!`(`error.rs:95-138`, `#[macro_export]`).

### `tmux_cc` 모듈(`tmux_cc/mod.rs`, tmux_cc feature 한정)

- `enum Event`(`tmux_cc/mod.rs:28-130`): tmux control-mode 이벤트.
- `struct Parser`(`tmux_cc/mod.rs:839-`): `new`/`advance_byte`/`advance_string`/`advance_bytes`(`tmux_cc/mod.rs:851-871`).
- `fn unvis(s) -> Result<String>`(`tmux_cc/mod.rs:683`), `fn parse_layout(layout) -> Result<Vec<WindowLayout>>`(`tmux_cc/mod.rs:827`), `struct PaneLayout`(`:133`), `enum WindowLayout`(`:142`), `type Tmux{Window,Pane,Session}Id`(`:7-9`).

---

## 4. 내부 구조

### 모듈 분해

| 모듈 | 책임 |
| --- | --- |
| `lib.rs` | 크레이트 루트. `Action`·`ControlCode`·DCS/Sixel 타입·`OneBased`·`no_std` 부트스트랩 정의 및 재노출. |
| `allocate.rs` | `std`/`no_std` 전환을 위한 alloc 타입 재노출 셋. |
| `parser/mod.rs` | `Parser` + `vtparse::VTActor` 구현(`Performer`). 디코딩 제어 흐름의 중심. |
| `parser/sixel.rs` | DCS Sixel 페이로드를 누적·해석하는 `SixelBuilder`. |
| `csi.rs` | CSI 시퀀스 파싱/인코딩(비대 모듈, 3384행). |
| `osc.rs` | OSC 파싱/인코딩(비대 모듈, 1987행). |
| `apc.rs` | Kitty 이미지 APC 파싱/인코딩 + 데이터 적재(1262행). |
| `esc.rs` | 단순 ESC 코드. |
| `color.rs` | 색 타입(RgbColor·AnsiColor·ColorSpec). |
| `hyperlink.rs` | OSC 8 하이퍼링크. |
| `error.rs` | 에러 타입·매크로·Context 트레이트. |
| `tmux_cc/mod.rs` | tmux control-mode 이벤트 파서(pest 기반). |
| `tmux_cc/tmux.pest` | tmux 이벤트 PEG 문법(`tmux_cc/mod.rs`가 `#[derive(Parser)]`로 임베드). |

### 제어/데이터 흐름(디코딩)

1. `Parser::parse`(`parser/mod.rs:92`)가 입력 바이트를 `VTParser`로 전달한다(tmux 모드면 우회, `parser/mod.rs:93-117`).
2. `VTParser`가 상태 전이마다 `Performer`(`parser/mod.rs:196-199`)의 `VTActor` 콜백을 호출한다: `print`, `execute_c0_or_c1`, `csi_dispatch`, `esc_dispatch`, `osc_dispatch`, `apc_dispatch`, `dcs_hook`/`dcs_put`/`dcs_unhook`(`parser/mod.rs:210-346`).
3. 각 콜백이 도메인 파서(`CSI::parse`·`OperatingSystemCommand::parse`·`Esc::parse`·`KittyImage::parse_apc`·`SixelBuilder`)를 호출해 `Action`을 만들어 사용자 콜백으로 방출한다.
4. DCS는 상태 기계가 다단계(hook→put*→unhook)로 동작한다. `ParseState`(`parser/mod.rs:49-56`)가 현재 어떤 DCS를 모으는지(`sixel`/`dcs`/`get_tcap`/`tmux_state`)를 보유하며, `dcs_hook`(`parser/mod.rs:233-270`)에서 최종 바이트·intermediate로 분기를 결정한다. (`q`+no-intermediate → Sixel; `q`+`+` → XTGETTCAP; `$q` → ShortDeviceControl/DECRQSS; `p`+params=[1000] → tmux_cc 모드 진입).

### 인코딩 흐름

모든 의미 타입이 `Display`를 구현해 역방향(의미→바이트)을 담당한다. `Action`의 `Display`(`lib.rs:102-129`)가 하위 타입의 `Display`로 위임한다. 인코딩은 라운드트립 테스트로 검증된다(`parser/mod.rs:740-752`의 `round_trip_parse`/`parse_as`).

### 비대 모듈

- `csi.rs`(3384행): CSI는 SGR·커서·편집·모드·디바이스·마우스·창·키보드 등 의미 분류가 가장 많아 파일이 가장 크다. `CSIParser`/`Cracked`(`csi.rs:1755-1795`) 보조 구조와 `noparams!`/`parse!`(`csi.rs:1797-`) 매크로로 반복 파싱을 압축한다.
- `osc.rs`(1987행): OSC 명령군 + iTerm 독자 확장 + FinalTerm 시맨틱 프롬프트로 인해 크다.
- `apc.rs`(1262행): Kitty 그래픽 프로토콜의 키-값 파라미터 다양성으로 크다.

---

## 5. 핵심 데이터 구조·타입과 불변식

- **`Action`(`lib.rs:42-62`)**: 64-bit에서 `size_of::<Action>() == 32`를 단언(`lib.rs:90-98`). 큰 변형(`OperatingSystemCommand`·`Sixel`·`KittyImage`·`Window`·`Device`·`Unspecified`)을 `Box`로 박싱해 공통 경로의 크기를 32바이트로 유지하는 것이 불변식이다. 변형 추가/확장 시 이 단언이 회귀 가드 역할을 한다.
- **`OneBased`(`lib.rs:546-617`)**: 내부 `value`는 1 이상이어야 한다. `new`는 `debug_assert!(value != 0)`(`lib.rs:553-556`). 0 입력은 `from_esc_param`이 1로, `from_esc_param_with_big_default`가 `u32::MAX`로 매핑(`lib.rs:568-594`). 이스케이프 파라미터의 1-기반 관례 위반을 타입으로 차단.
- **`Sixel`/`SixelData`(`lib.rs:281-438`)**: `SixelData::Data`/`Repeat`는 6비트 값(0–63)을 담아야 한다. 인코딩 시 `value + 0x3f`로 ASCII 가시영역에 매핑(`lib.rs:443`). `SixelBuilder::push`(`parser/sixel.rs:42-91`)가 입력 시 `data - 0x3f`로 복호화한다. 래스터 크기는 `MAX_SIXEL_SIZE = 100_000_000`(`parser/sixel.rs:5`)를 초과하거나 곱셈 오버플로 시 데이터를 폐기(`parser/sixel.rs:150-170`)하는 DoS 방어 불변식이 있다.
- **`RgbColor`(`color.rs:57-60`)**: u32 비트 팩(R<<16|G<<8|B). 내부 표현은 8bpc sRGB. serde 직렬화는 7바이트 `#RRGGBB` 문자열로 라운드트립(`color.rs:178-200`).
- **`ParseState`(`parser/mod.rs:49-56`)**: DCS 수집 슬롯은 상호 배타적으로 사용된다. `dcs_hook`(`parser/mod.rs:240-242`)이 새 DCS 시작 시 `sixel`/`get_tcap`/`dcs`를 모두 `take()`로 비우는 것이 불변식이다. 위반 시 이전 DCS 잔여가 누설된다.
- **`Selection`(`osc.rs:80-96`)**: bitflags. 빈 입력은 `SELECT|CUT0`로 해석(`osc.rs:100-102`).
- **`Error`(`error.rs:13`)**: `Box<InternalError>` 뉴타입. `InternalError`가 `#[doc(hidden)]`+`#[non_exhaustive]`로 외부에는 불투명. 박싱으로 `Result`의 `Ok` 크기 비대화를 방지.

---

## 6. 외부 의존성

- **`vtparse`**: DEC ANSI 상태 기계 엔진. 본 크레이트는 이 위에 의미 계층을 올린다. `VTActor`(`parser/mod.rs:210`) 구현이 핵심 결합점. `alloc` feature 사용.
- **`base64`/`hex`**: 페이로드 인/디코드. base64는 Kitty 이미지·iTerm 파일 데이터, hex는 XTGETTCAP termcap 이름(`parser/mod.rs:28`)·동적 색 등.
- **`bitflags`**: `Selection`(`osc.rs:80`), `KittyKeyboardFlags`(재노출).
- **`num-derive`/`num-traits`**: 정수↔enum 변환. `ControlCode`·`EscCode`·`DynamicColorNumber` 등 `FromPrimitive`/`ToPrimitive`로 시퀀스 바이트와 매핑(`lib.rs:476`, `esc.rs:25`, `osc.rs:57`).
- **`ordered-float`**: `NotNan`(`osc.rs:11`). 부동소수를 키/비교 가능 값으로 사용.
- **`thiserror`**: 에러 derive. `default-features=false`로 `no_std` 호환(`Cargo.toml:26`).
- **`wezterm-color-types`**: `SrgbaTuple`·`LinearRgba`. 색 모델의 단일 출처.
- **`wezterm-dynamic`**: 설정 직렬화용 `FromDynamic`/`ToDynamic`.
- **`wezterm-input-types`**: Kitty 키보드 플래그 등 입력 비트플래그.
- **(optional) `image`/`sha2`/`wezterm-blob-leases`**: 이미지(`use_image`/`image`) feature에서 디코딩·해시·블롭 임대.
- **(optional) `pest`/`pest_derive`**: tmux_cc feature에서 PEG 파서(`tmux.pest`).
- **(optional) `nix`**: `kitty-shm`+unix용 `shm_open`(Windows fork에서는 죽은 경로).
- **(optional) `winapi`**: `kitty-shm`+windows용 공유 메모리 매핑(`apc.rs:317-` 참조; 유일한 라이브 Windows API 사용처).

---

## 7. 설정·기능 플래그

`Cargo.toml:50-57`에 feature가 정의되어 있다.

| feature | 효과 |
| --- | --- |
| `std` | `std` 의존. vtparse·thiserror·input-types·hex·dynamic의 std 전파. 워크스페이스 소비자(termwiz·term)는 항상 켬. |
| `kitty-shm` | `nix`+`winapi` 활성. Kitty 이미지 공유 메모리/파일 적재(`KittyImageData::load_data`, `apc.rs:176`). |
| `use_serde` | serde 직렬화. color-types/blob-leases/bitflags/input-types serde 전파. |
| `use_image` | `image` 크레이트 디코딩. 내부적으로 `image` feature 포함. |
| `image` | `sha2`+`wezterm-blob-leases` 활성(이미지 블롭 해시·임대). |
| `tmux_cc` | `pest`/`pest_derive` 활성. tmux control-mode 파서·`DeviceControlMode::TmuxEvents` 노출. |
| `docs` | 위 전체를 켜는 문서 빌드용 집합(`Cargo.toml:57,64-66`). |

소비자 측 전파(`term/Cargo.toml:38`): `wezterm-term`은 `std`+`use_image`로 의존한다. `use_image`는 termwiz 경유로 `kitty-shm`까지 켠다(`termwiz/Cargo.toml:54`). 따라서 실제 GUI 빌드에서는 `apc.rs`의 Windows 공유 메모리 경로가 라이브 코드다.

이 크레이트는 자체 `config` 항목을 갖지 않는다. 설정 철학상 동작 고정은 상위 `config` 크레이트나 본 크레이트 소스 기본값 수정으로 이뤄진다.

---

## 8. Windows 전용 고려사항

- **라이브 Windows API 사용처**: `apc.rs:317-`의 `read_shared_memory_data`(`cfg(all(feature = "kitty-shm", windows))`). `OpenFileMappingW`·`MapViewOfFile`·`UnmapViewOfFile`·`VirtualQuery`·`CloseHandle`로 Kitty 이미지의 공유 메모리 페이로드를 읽는다(`apc.rs:319-323,375`). `winapi`의 `memoryapi`·`handleapi`·`winnt` 등 feature가 `Cargo.toml:33-48`에 한정 선언됨. `use_image` 경유로 GUI 빌드에서 컴파일된다.
- **죽은 경로(비Windows)**:
  - `apc.rs:262-307`의 `cfg(all(feature = "kitty-shm", unix, not(target_os = "android")))` 및 `cfg(...unix, target_os = "android")` 공유 메모리 구현(`nix::sys::mman`). Windows 빌드에서 미컴파일.
  - `apc.rs:220-233`의 임시파일 경로 판정(`looks_like_temp_path`)은 `/tmp`·`/var/tmp`·`/dev/shm`·`$TMPDIR`만 인식한다. Windows에서는 실효성이 없어, 임시파일 자동 삭제가 사실상 동작하지 않고 경고 로그만 남길 수 있다(`apc.rs:235-249`). 이는 리팩토링 시 Windows 임시경로 보강 후보.
- **나머지 코드는 플랫폼 중립**: 파싱/인코딩 본체(`csi`·`osc`·`esc`·`parser`·`sixel`·`color`)는 OS 의존이 없다.

---

## 9. 리팩토링 주의점

- **`vtparse` 결합**: `Performer`의 `VTActor` 구현(`parser/mod.rs:210-346`)이 vtparse 콜백 시그니처에 강하게 묶여 있다. vtparse 버전 변경 시 `dcs_hook`/`dcs_put`/`dcs_unhook`의 다단계 상태 누적 로직(`ParseState`)이 깨지기 쉽다.
- **DCS 상태 슬롯의 암묵적 배타성**: `ParseState`(`parser/mod.rs:49-56`)의 `sixel`/`dcs`/`get_tcap`/`tmux_state`는 동시에 하나만 활성이어야 한다. `dcs_hook`의 선행 `take()`(`parser/mod.rs:240-242`)를 제거하거나 순서를 바꾸면 잔여 데이터 누설이 발생한다.
- **`()` 에러 관례**: CSI 내부 파서 전반이 `Result<_, ()>`를 사용한다(`lib.rs:566-568`, `csi.rs:1723,1747` 등). `?`로 전파해 미인식 시 `Unspecified` 폴백으로 귀결시키는 의도된 설계다. `Option`으로 전환하면 `ok_or(())`가 호출부 전반에 확산된다(주석 명시, `lib.rs:566`).
- **크기 단언 회귀 가드**: `lib.rs:90-98`·`csi.rs:138-149`의 `size_of` 단언. 변형 추가나 박싱 제거 시 깨진다. 의도된 메모리 예산 변경이 아니라면 단언을 함께 갱신하지 말 것.
- **`InternalError` 자동 From 의존**: 외부 크레이트(termwiz·term)가 `?`/`.into()`로 `#[from]` 변환을 생성한다(`error.rs:33` 주석). variant를 박싱하면 소스 타입이 바뀌어 자동 `From`이 깨진다. 현재 `Error` 뉴타입이 이미 `Box<InternalError>`라 영향은 무해하나, variant 구조 변경 시 주의.
- **Sixel DoS 방어 상수**: `MAX_SIXEL_SIZE`(`parser/sixel.rs:5`)와 오버플로 검사(`parser/sixel.rs:150-170`)는 신뢰 불가 입력 방어다. 제거·완화 금지.
- **인코딩 라운드트립 계약**: 다수 타입의 `Display`가 파싱과 역대칭이어야 한다(`round_trip_parse` 테스트, `parser/mod.rs:740-`). 일부는 정규화로 라운드트립이 깨진다(예: `parse_as`로 `\x1b[1;3m` → `\x1b[1m\x1b[3m`, `parser/mod.rs:573-586`). `Display` 수정 시 스냅샷 테스트가 회귀 가드.
- **`OneBased` 0-처리 분기**: 같은 0 입력이 컨텍스트에 따라 1 또는 max로 갈린다(`lib.rs:568-594`). 호출부가 올바른 생성자를 선택해야 한다.
- **순환 의존 없음**: deps(vtparse·color-types·dynamic·input-types·blob-leases)는 모두 하위 유틸 크레이트이며, 본 크레이트를 역참조하지 않는다. usedBy(termwiz·cell·surface·term)는 상위 계층이다. 단방향 계층이 유지된다.
- **비대 모듈 분할 후보**: `csi.rs`(3384행)는 SGR/커서/모드/마우스/창 등으로 하위 모듈 분리가 가능하나, `CSIParser` 상태 공유와 매크로 의존으로 분리 비용이 크다.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `Cargo.toml` | 67 | 크레이트 메타·의존·feature 정의. |
| `src/lib.rs` | 617 | 루트. `Action`·`ControlCode`·DCS/Sixel 타입·`OneBased`·`no_std` 부트스트랩. |
| `src/allocate.rs` | 13 | std/no_std alloc 타입 재노출. |
| `src/parser/mod.rs` | 1286 | `Parser`+`VTActor`(`Performer`) 디코딩 제어 흐름. 다수 테스트 포함. |
| `src/parser/sixel.rs` | 302 | DCS Sixel 누적기 `SixelBuilder`. DoS 방어 포함. |
| `src/csi.rs` | 3384 | CSI 파싱/인코딩(SGR·커서·편집·모드·디바이스·마우스·창·키보드). 비대 모듈. |
| `src/osc.rs` | 1987 | OSC 파싱/인코딩(제목·하이퍼링크·셀렉션·색·iTerm·FinalTerm·ConEmu). |
| `src/apc.rs` | 1262 | Kitty 이미지 APC 파싱/인코딩 + 데이터 적재(Windows/유닉스 공유 메모리). |
| `src/esc.rs` | 206 | 단순 ESC 코드(`Esc`/`EscCode`). |
| `src/color.rs` | 281 | 색 타입(`RgbColor`·`AnsiColor`·`ColorSpec`)·변환. |
| `src/hyperlink.rs` | 117 | OSC 8 하이퍼링크(`Hyperlink`). |
| `src/error.rs` | 185 | 에러 타입·`Context` 트레이트·`format_err!`/`bail!`/`ensure!` 매크로. |
| `src/tmux_cc/mod.rs` | 1182 | tmux control-mode 이벤트 파서(pest). tmux_cc feature. |
| `src/tmux_cc/tmux.pest` | (문법) | tmux 이벤트 PEG 문법. `#[derive(Parser)]`로 임베드. |

생성 파일([생성])은 본 크레이트에 없다. 거대 파일(`csi.rs`·`osc.rs`·`apc.rs`)은 수기 작성된 도메인 파서이며 생성물이 아니다.
