# color-types 폴더 기능 명세

본 문서는 `E:\Project\wezterm\color-types` 폴더에 포함된 단일 크레이트 `wezterm-color-types`(버전 0.3.0)에 대한 상세 기능 명세다. 모든 기술 내용은 `color-types/Cargo.toml`과 `color-types/src/lib.rs`의 실제 코드에 근거한다.

## 1. 개요 및 책임

`wezterm-color-types`는 색상 값을 표현·변환하는 **저수준 타입 라이브러리**다. 단일 책임은 다음과 같다.

- 세 가지 색상 표현 사이의 변환을 제공한다.
  - **sRGBA 정수 패킹**: `SrgbaPixel` — 빅엔디안 `u32` 1워드에 RGBA 8비트 채널을 담는 GPU/메모리 친화적 표현.
  - **sRGBA 부동소수**: `SrgbaTuple` — `f32` 4채널(0.0–1.0). 사람이 다루는 색 이름·문자열 파싱·색공간 조작의 진입점.
  - **선형 RGBA 부동소수**: `LinearRgba` — 감마 보정이 해제된 선형 색공간 `f32` 4채널. GPU 셰이딩·블렌딩·대비 계산용.
- sRGB ↔ 선형 변환에 필요한 감마 보정을 정확하고 빠르게(룩업 테이블) 수행한다(`color-types/src/lib.rs:144-222`).
- X11/SVG/CSS3 색 이름과 다양한 색 문자열 구문(`#RGB`, `rgb:`, `rgba:`, `hsl:`, CSS)을 파싱한다(`color-types/src/lib.rs:765-898`).
- 색 조작(채도/명도/색상환 회전), 대비비(WCAG) 계산, Lab/Oklab 기반 가독성 보정 같은 상위 색상 연산을 제공한다.

이 크레이트는 렌더링 파이프라인의 최하단에 위치하는 **순수 데이터 타입 계층**으로, I/O나 플랫폼 API에 직접 의존하지 않는다. `#![no_std]` 지원(기본)과 `std` 기능을 통한 풍부한 기능 세트의 두 모드를 가진다.

### Windows fork에서의 실제 책임

이 fork는 Windows 전용 영구 분기이나, 이 크레이트 자체에는 **플랫폼 분기가 전혀 없다**. 코드 분기는 전적으로 `feature = "std"` / `not(feature = "std")` 축에서만 이루어진다(자세한 내용은 8절). 즉 Windows fork 맥락에서도 이 크레이트의 책임은 변하지 않으며, GUI 렌더링(`window`, `wezterm-font`)과 셀/이스케이프 파싱(`wezterm-cell`, `wezterm-escape-parser`, `termwiz`, `wezterm-surface`)이 사용하는 공용 색상 타입 공급자 역할을 그대로 수행한다.

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 종류 | 용도 |
|---|---|---|
| `wezterm-dynamic` | 내부(필수) | `FromDynamic`/`ToDynamic` 구현을 통해 동적 설정 값(`Value`)과 `SrgbaTuple` 간 변환 |
| `num-traits` (libm) | 외부(필수) | `no_std` 모드에서 `powf` 등 부동소수 수학 함수 제공 |
| `serde` | 외부(선택, `use_serde`) | `SrgbaTuple` 직렬화/역직렬화 |
| `csscolorparser` | 외부(선택, `std`) | CSS 색 구문·`Color` 타입(Lab/HSL 변환) |
| `deltae` | 외부(선택, `std`) | CIEDE2000 색차(ΔE) 계산 |

### 피의존(usedBy)

| 크레이트 | 활성화 기능 |
|---|---|
| `termwiz` | `std` |
| `wezterm-cell` | `std`(전이), `use_serde`(전이) |
| `wezterm-escape-parser` | `use_serde`(전이) |
| `wezterm-font` | 기본(워크스페이스 기본 `default-features=false`) |
| `wezterm-surface` | 기본 |
| `window` | 기본 |

워크스페이스 루트 `Cargo.toml:203`은 이 크레이트를 `default-features=false`로 선언한다. 따라서 기본 형태는 `no_std`이며, `std` 기능은 소비처(예: `termwiz`)가 명시적으로 켤 때만 활성화된다.

### 계층 위치

이 크레이트는 색상 표현의 **기반 타입 계층**이다. 터미널 셀 속성(`wezterm-cell`), 이스케이프 시퀀스 색상 파싱(`wezterm-escape-parser`/`termwiz`), 폰트 렌더링 색상(`wezterm-font`), 화면 표면(`wezterm-surface`), 윈도우/GPU 렌더링(`window`)이 모두 이 크레이트의 `SrgbaPixel`/`SrgbaTuple`/`LinearRgba`를 공유 통화로 사용한다.

## 3. 공개 API 표면

크레이트는 모듈 분할 없이 `lib.rs` 단일 파일에 모든 공개 항목을 평탄하게(flat) 노출한다.

### 자유 함수

- `pub fn linear_u8_to_srgb8(f: u8) -> u8` (`lib.rs:153-156`, `std` 한정) — 선형 8비트 값을 sRGB 8비트로 룩업 테이블 변환.

### `struct SrgbaPixel(u32)` (`lib.rs:225-269`)

빅엔디안 패킹 sRGBA 픽셀.

- `rgba(r, g, b, a: u8) -> Self`
- `as_rgba(self) -> (u8, u8, u8, u8)`
- `to_linear(self) -> LinearRgba`
- `with_srgba_u32(word: u32) -> Self`
- `as_srgba32(self) -> u32`
- `as_srgba_tuple(self) -> (f32, f32, f32, f32)`

### `struct SrgbaTuple(pub f32, pub f32, pub f32, pub f32)` (`lib.rs:271-657`)

sRGBA 부동소수 색. 가장 광범위한 API 표면을 가진다.

- 변환: `premultiply`, `demultiply`, `to_tuple_rgba`, `as_rgba_u8`, `to_srgb_u8`, `to_linear() -> LinearRgba`
- 보간: `interpolate(self, other, k: f64) -> Self` (프리멀티플라이 후 선형 보간)
- 문자열화: `to_rgb_string`(`#RRGGBB`), `to_rgba_string`(`rgba(% % % %)`), `to_x11_16bit_rgb_string`(`rgb:RRRR/GGGG/BBBB`)
- 이름/파싱: `from_named(name: &str) -> Option<Self>`, `FromStr` 구현(`from_str`)
- 알파: `mul_alpha(self, alpha: f32) -> Self`
- 색공간(이하 `std` 한정): `to_laba`, `to_hsla`, `from_hsla`
- 색 조작(`std` 한정): `saturate`, `saturate_fixed`, `lighten`, `lighten_fixed`, `adjust_hue_fixed`, `adjust_hue_fixed_ryb`, `complement`, `complement_ryb`, `triad`, `square`
- 대비/색차(`std` 한정): `delta_e`, `contrast_ratio`, `ensure_contrast_ratio`

트레이트 구현: `Display`(`lib.rs:322-331`), `ToDynamic`/`FromDynamic`(`lib.rs:333-347`), `Hash`/`Eq`(`lib.rs:728-737`), 다수의 `From`(`SrgbaPixel`, `(f32×4)`, `(u8×4)`, `(u8×3)`, `Color`), 그리고 `use_serde` 시 `Serialize`/`Deserialize`.

### `struct LinearRgba(pub f32, pub f32, pub f32, pub f32)` (`lib.rs:900-1115`)

선형 색공간 색. GPU·블렌딩·대비 연산의 핵심.

- 생성: `with_srgba(r,g,b,a: u8)`(RGB만 감마 해제, 알파는 선형), `with_rgba(r,g,b,a: u8)`, `const with_components(...)`, 상수 `TRANSPARENT`
- 질의/조작: `is_fully_transparent`, `when_fully_transparent`, `mul_alpha`, `tuple`
- 변환: `srgba_pixel() -> SrgbaPixel`, `to_srgb() -> SrgbaTuple`
- 대비(이하 `std` 한정): `relative_luminance`, `contrast_ratio`, `ensure_contrast_ratio`

트레이트 구현: `Hash`/`Eq`(`lib.rs:904-916`), `From<(f32×4)>`, `From<[f32;4]>`, `Into<[f32;4]>`.

## 4. 내부 구조

단일 파일 모듈이며, 논리적으로 다음 블록으로 구성된다.

1. **감마 룩업 테이블 생성·접근** (`lib.rs:24-222`) — `std` 모드에서 `LazyLock` 정적 테이블 4종(`SRGB_TO_F32_TABLE`, `F32_TO_U8_TABLE`, `RGB_TO_SRGB_TABLE`, `RGB_TO_F32_TABLE`)을 지연 초기화하고, 변환 함수(`linear_f32_to_srgb8`, `srgb8_to_linear_f32`, `rgb_to_linear_f32`)가 이를 `get_unchecked`로 참조한다. `no_std` 모드에서는 동일 변환을 매 호출마다 직접 계산한다.
2. **타입 정의·연산** — `SrgbaPixel`(`225-269`), `SrgbaTuple`(`271-657`), `LinearRgba`(`900-1115`).
3. **색 이름 테이블** (`lib.rs:398-465`) — `rgb.txt`를 `include_str!`로 임베드하고 `iter_rgb_txt`로 순회. `std`에서는 `NAMED_COLORS` `HashMap`을 한 번만 구축, `no_std`에서는 선형 스캔.
4. **문자열 파싱** (`lib.rs:739-898`) — `x_parse_color_component`와 `FromStr::from_str`이 X11/CSS 구문을 분기 처리.
5. **색공간 보조 함수**(`std` 한정, `lib.rs:659-726`) — RYB↔RGB 색상환 매핑, 각도 정규화, 스케일/고정 적용.
6. **테스트**(`lib.rs:1117-1208`).

**제어/데이터 흐름**: 외부에서 들어온 색 문자열·이름은 `SrgbaTuple`로 파싱된다 → 렌더링 시 `to_linear()`로 `LinearRgba` 변환 → 블렌딩/대비 계산 → `srgba_pixel()`로 `SrgbaPixel` 패킹되어 GPU/버퍼로 전달된다. sRGB↔선형 경계에서만 감마 보정이 개입한다.

**비대 지점**: `SrgbaTuple`의 `impl` 블록(`lib.rs:442-657`)이 가장 크다. 파싱·문자열화·색공간·조작·대비를 모두 한 타입에 모았으나, 대부분이 `std` 한정이라 `no_std` 모드에서는 표면이 크게 축소된다.

## 5. 핵심 데이터 구조·타입

### `SrgbaPixel(u32)`

- **불변식**: 내부 `u32`는 항상 **빅엔디안** 바이트 순서로 sRGBA를 담는다. `rgba()`는 `(b<<24 | g<<16 | r<<8 | a)`를 `to_be()`로 저장하고, `as_rgba()`는 `from_be()`로 복원한다(`lib.rs:230-246`). 알파는 최하위 바이트.
- `Copy`/`Eq` 가능. 워드 단위 비교가 색 동등성과 일치한다.

### `SrgbaTuple(f32×4)`

- **의미**: 채널은 sRGB 색공간의 0.0–1.0 정규화 값. **알파는 항상 선형**으로 취급되어 감마 보정 대상이 아니다(`lib.rs:469`, `483-489`).
- **불변식 주의**: 일부 연산(`from_str`의 퍼센트/255 경로 등)은 범위를 검증하나, 산술 결과(`saturate`/`lighten`/`interpolate`)는 0–1 범위로 강제 클램프되지 않는다. 값이 범위를 벗어날 수 있음을 전제로 사용해야 한다.
- `Eq`/`Hash`는 `f32::to_ne_bytes()` 기반 비트 동등성으로 구현된다(`lib.rs:728-737`). 따라서 `NaN`·`-0.0`이 일반적 부동소수 비교와 다르게 취급된다.

### `LinearRgba(f32×4)`

- **의미**: 선형 색공간 0.0–1.0. `with_srgba`는 RGB만 감마 해제하고 알파는 선형 유지(`lib.rs:937-947`).
- `TRANSPARENT` 상수(`0,0,0,0`), `const fn with_components`로 컴파일타임 생성 가능.
- `SrgbaTuple`과 동일한 `to_ne_bytes` 기반 `Hash`/`Eq` 불변식을 공유한다(`lib.rs:904-916`).

## 6. 외부 의존성

- **`num-traits`(libm 기능, 필수)**: `no_std` 모드에서 `Float::powf` 등 부동소수 초월함수를 제공한다(`lib.rs:7-9`). `std` 모드에서는 표준 `f32`/`f64` 메서드가 쓰이므로 사실상 `no_std` 경로 전용 백엔드다.
- **`csscolorparser`(선택, `std` 기능 시 + `lab` 기능)**: `csscolorparser::parse`로 일반 CSS 색 문자열을 처리하고(`lib.rs:889-894`), `Color` 타입의 `to_lab`/`to_hsla`/`from_hsla`를 통해 Lab·HSL 변환을 수행한다(`lib.rs:531-545`).
- **`deltae`(선택, `std` 기능 시)**: `DeltaE::new(..., DE2000)`으로 CIEDE2000 색차를 계산한다(`lib.rs:622-637`). 두 색의 지각적 차이를 정량화하는 데 사용한다.
- **`serde`(선택, `use_serde`)**: `SrgbaTuple`에 `Serialize`/`Deserialize` 파생을 부여한다(`lib.rs:272-274`).
- **`wezterm-dynamic`(필수)**: 설정 시스템의 동적 `Value`와 `SrgbaTuple` 간 양방향 변환(`ToDynamic`/`FromDynamic`)을 제공한다. 설정에서 색은 문자열로 표현되며, `FromDynamic`이 `String`→`from_str` 경로로 파싱한다(`lib.rs:339-347`).

## 7. 설정·기능 플래그

`color-types/Cargo.toml:10-12`이 정의하는 기능 플래그.

| 플래그 | 의존 활성화 | 효과 |
|---|---|---|
| `use_serde` | `serde` | `SrgbaTuple`의 serde 직렬화 |
| `std` | `serde/std`, `dep:deltae`, `csscolorparser/lab` | 룩업 테이블·CSS 파싱·HSL/Lab·색차·대비·색 조작 API 전체 활성화 |

기본 기능 세트는 비어 있다. 워크스페이스 기본(`default-features=false`, 루트 `Cargo.toml:203`)에서는 `no_std`로 컴파일된다. 이 크레이트 자체에는 별도 런타임 config 항목이 없으나, `wezterm-dynamic` 연동으로 상위 `config` 크레이트의 색상 설정 값이 `SrgbaTuple`로 역직렬화된다.

**기능 상호작용 주의**: `from_str`(`lib.rs:768-897`)은 X11/`#`/`rgb:`/`rgba:`/`hsl:` 구문을 직접 처리하지만, 일반 CSS 함수 구문(`rgb(...)`, `rgba(...)`) 분기는 `std`에서만 `csscolorparser`로 동작한다. 따라서 `no_std`에서는 파싱 가능한 색 구문 범위가 좁아진다.

## 8. Windows 전용 고려사항

- **플랫폼 분기 없음**: 이 크레이트에는 `cfg(windows)`/`cfg(unix)`/`target_os` 등 OS 조건부 컴파일이 **하나도 없다**. 모든 `cfg`는 `feature = "std"` 축에만 존재한다. 따라서 Windows fork에서 제거 대상이 된 "죽은 비Windows 분기"가 이 크레이트에는 없다.
- **Windows API 직접 호출 없음**: 순수 산술·룩업·문자열 처리만 수행하며 Win32/Direct 계열 API를 사용하지 않는다.
- **`unsafe` 사용 지점**: `std` 모드 룩업 테이블 접근 4곳(`lib.rs:155`, `176`, `200`, `216`)에서 `get_unchecked`를 사용한다. 인덱스가 `u8`(0–255) 또는 비트 연산으로 테이블 크기 내로 제한되므로 메모리 안전성은 보장되나, 테이블 크기를 바꿀 경우 경계 검토가 필요하다.

요약하면 이 크레이트는 플랫폼 중립적이며, Windows fork 고유의 정리 대상이 사실상 없는 모듈이다.

## 9. 리팩토링 주의점

- **빅엔디안 패킹 불변식**: `SrgbaPixel`의 바이트 순서는 GPU 정점/텍스처 포맷과 직접 맞물린다. `rgba`/`as_rgba`의 시프트·`to_be`/`from_be` 쌍을 변경하면 `window`·`wezterm-font`의 렌더 색이 어긋난다. 변경 시 두 메서드를 짝으로만 수정해야 한다.
- **알파의 선형성 가정**: 여러 변환(`to_linear`, `with_srgba`, `to_srgb`)이 "알파는 감마 보정하지 않는다"는 가정을 코드 곳곳에 명시(`lib.rs:469`, `483`, `939-947`, `1001`)한다. 이 가정을 깨면 블렌딩 결과가 미묘하게 틀어진다.
- **`Hash`/`Eq`의 비트 동등성**: `to_ne_bytes` 기반이므로 `NaN`/`-0.0` 취급이 IEEE 비교와 다르다. 색을 `HashMap` 키로 쓰는 소비처가 있으므로(예: `NAMED_COLORS`) 동등성 의미 변경은 위험하다.
- **`std`/`no_std` 이중 경로**: 동일 변환이 두 번 구현되어 있다(테이블 vs 직접 계산, 예 `srgb8_to_linear_f32` `lib.rs:197-211`). 한쪽만 수정하면 기능 조합에 따라 결과가 갈린다. 항상 양쪽을 동기화해야 한다.
- **알려진 부정확성 두 곳**:
  - `F32_TO_U8_TABLE` 생성 코드는 주석 처리되어 있고 하드코딩 상수 배열을 사용한다. 원본 동적 생성 코드가 다른 수치를 내는 이유가 미해명 상태다(`lib.rs:68-91`). 테이블 재생성 시도 시 회귀 검증 필요.
  - `ensure_contrast_ratio`(`lib.rs:1080-1114`)에서 `increased_ratio`가 `increased_col`이 아닌 `reduced_col`로 계산되는 버그성 코드가 있다(`lib.rs:1088`). 대비 보정 결과에 영향을 줄 수 있으므로 수정 시 동작 변경에 유의한다.
- **거대 `impl` 분해 여지**: `SrgbaTuple`의 단일 `impl`이 파싱·문자열화·색공간·조작·대비를 모두 안고 있다. 모듈 분리 리팩토링 시 다수 `cfg(feature="std")` 게이트를 보존해야 한다.
- **순환 의존 없음**: 내부 의존은 `wezterm-dynamic` 단방향뿐이며 순환은 없다. 피의존이 6개 크레이트로 넓으므로 공개 시그니처 변경의 파급 범위가 크다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `color-types/Cargo.toml` | 19 | 패키지·기능 플래그(`use_serde`, `std`)·의존성 선언 |
| `color-types/src/lib.rs` | 1208 | 전체 구현: 감마 테이블, `SrgbaPixel`/`SrgbaTuple`/`LinearRgba`, 파싱, 색공간·대비 연산, 테스트 |
| `color-types/src/rgb.txt` | 782 | X11/SVG/CSS3 색 이름→RGB 매핑 데이터. `include_str!`로 임베드되어 `from_named`/`NAMED_COLORS`가 소비. 명명색 사전(데이터 파일) — 코드가 런타임에 파싱하는 정적 자원 |

`rgb.txt`는 코드 생성물은 아니나 임베드되는 정적 데이터 자원이며, 각 행은 `R G B<탭>색이름` 형식이다(`lib.rs:410-428`이 파싱). 동일 색의 공백/카멜케이스 표기 변형이 별도 행으로 존재하며, 모두 소문자 정규화 후 조회된다.
