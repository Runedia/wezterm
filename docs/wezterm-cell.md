# wezterm-cell 폴더 기능 명세

본 문서는 `E:\Project\wezterm\wezterm-cell` 크레이트에 대한 상세 기능 명세다. 추후 리팩토링을 위한 참조 자료로서 "무엇을·왜·어떻게"를 코드 근거와 함께 기술한다. 코드 인용은 `파일경로:라인` 형식을 따른다.

---

## 1. 개요 및 책임

`wezterm-cell`은 **터미널 디스플레이의 단일 셀(cell)을 모델링**하는 저수준 데이터 크레이트다. 단일 책임은 다음과 같다.

- 화면 격자(grid)의 한 칸을 표현하는 `Cell` 타입과, 그 칸이 가지는 스타일 속성(`CellAttributes`)을 정의한다.
- 셀이 차지하는 시각적 폭(column width)을 유니코드 버전 의존적으로 계산한다(`grapheme_column_width`, `unicode_column_width`).
- 셀에 부착될 수 있는 부가 데이터, 즉 하이퍼링크와 이미지 데이터(`ImageCell`, `ImageData`)를 모델링한다.

이 크레이트는 **렌더링·셰이핑·터미널 상태 기계 로직을 포함하지 않는다**. 오직 "한 칸이 무엇을 담고 어떤 스타일을 가지는가"라는 데이터 구조와, 그 칸의 폭 계산이라는 순수 함수만 제공한다. 상위 크레이트(`wezterm-term`, `wezterm-surface`, `termwiz`)가 이 타입들을 격자에 배치하고 변경을 적용한다.

Windows fork에서의 실제 책임은 upstream과 동일하다. 이 크레이트는 본질적으로 플랫폼 독립적인 데이터 모델이며, 플랫폼 분기 코드가 거의 없다. 단, 메모리 절감을 위한 `TeenyString`의 `unsafe` 포인터 패킹은 64비트 포인터 폭(`target_pointer_width = "64"`)을 전제로 하므로, x86_64 Windows 빌드를 암묵적으로 가정한다(`wezterm-cell\src\lib.rs:1019-1029`).

핵심 설계 동기는 **per-cell 메모리 풋프린트 최소화**다. 터미널 화면과 스크롤백은 수십만 개의 셀을 보유할 수 있으므로, 셀 하나의 크기가 전체 메모리 사용량을 지배한다. 이 목표 때문에 두 가지 비대한 최적화가 도입되었다.

1. `CellAttributes`의 불린/열거 속성을 단일 `u32` 비트필드에 패킹하고, 드물게 쓰이는 속성은 힙 할당 `Box<FatAttributes>`로 분리(`wezterm-cell\src\lib.rs:46-63`).
2. 셀 텍스트를 단일 `u64`에 인라인 패킹하는 `TeenyString`(`wezterm-cell\src\lib.rs:541-554`).

---

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 사용 목적 |
|---|---|
| `wezterm-char-props` | 유니코드 문자 속성. `emoji::Presentation`(이모지/텍스트 표현), `emoji_variation::WCWIDTH_TABLE`(폭 분류 테이블), `widechar_width::WcWidth`(폭 분류 enum), `white_space::WHITE_SPACE`(공백 판정) |
| `wezterm-color-types` | 색상 원시 타입. `LinearRgba`, `SrgbaTuple` 재노출 |
| `wezterm-dynamic` | Lua 상호운용을 위한 `FromDynamic`/`ToDynamic` derive |
| `wezterm-escape-parser` | 이스케이프 시퀀스 파서가 정의한 색상·SGR 타입 재사용. `osc::Hyperlink`, `csi::{Blink, Intensity, Underline, VerticalAlign}`, `color::{AnsiColor, ColorSpec, PaletteIndex, RgbColor}` |
| `wezterm-blob-leases` | (`use_image` 전용) 인코딩 이미지 블롭을 디스크로 스왑아웃하는 `BlobLease`/`BlobManager` |
| `finl_unicode` | 그래핌 클러스터 분할(`Graphemes`) |
| `image` | (`use_image` 전용) 이미지 디코딩(PNG/GIF/WebP/APNG) |
| `ordered-float` | (`use_image` 전용) NaN 불허 `NotNan<f32>` 텍스처 좌표 |
| `sha2` | (`use_image` 전용) 이미지 데이터 SHA-256 해시 |
| `serde` | (`use_serde` 전용) 직렬화 |
| `thiserror` | (`use_image` 전용) `ImageCellError` 정의 |
| `log` | (`use_image` 전용) 디코딩 경고 로깅 |

### 피의존(usedBy)

| 크레이트 | 활성화 feature | 사용 형태 |
|---|---|---|
| `termwiz` | `std`(+조건부 `use_serde`/`use_image`) | 터미널 화면 모델·`Surface`·`Change` 스트림에서 `Cell`/`CellAttributes` 사용 |
| `wezterm-surface` | 기본(+조건부 전파) | 화면 표면(Surface) 셀 격자 |
| `wezterm-term` | `std`, `use_image` | 터미널 상태 기계가 셀을 격자에 기록 |

근거: `termwiz/Cargo.toml:42`, `wezterm-surface/Cargo.toml:23`, `term/Cargo.toml:37`, 워크스페이스 등록 `Cargo.toml:7,200`.

### 계층상 위치

`wezterm-cell`은 의존성 그래프의 **잎(leaf)에 가까운 기반 데이터 계층**이다. `wezterm-escape-parser`/`wezterm-char-props`/`wezterm-color-types`가 제공하는 원시 타입을 조립하여 "셀"이라는 도메인 타입을 만들고, 이를 상위의 표면·터미널·termwiz 계층이 격자로 구성한다. 파이프라인상 이스케이프 파싱과 화면 렌더링 사이의 데이터 모델 위치에 해당한다.

---

## 3. 공개 API 표면

### 3.1 루트 모듈 (`lib.rs`)

핵심 타입:

- `pub struct Cell` — 셀 한 칸. 텍스트(`TeenyString`)와 속성(`CellAttributes`)을 보유(`lib.rs:709-722`).
  - `pub fn new(text: char, attrs: CellAttributes) -> Self` (`lib.rs:744`)
  - `pub const fn blank() -> Self` / `pub const fn blank_with_attrs(attrs) -> Self` (`lib.rs:752,759`)
  - `pub fn new_grapheme(text: &str, attrs, unicode_version: Option<&UnicodeVersion>) -> Self` (`lib.rs:783`)
  - `pub fn new_grapheme_with_width(text: &str, width: usize, attrs) -> Self` (`lib.rs:796`)
  - `pub fn str(&self) -> &str` / `pub fn width(&self) -> usize` (`lib.rs:805,810`)
  - `pub fn attrs(&self) -> &CellAttributes` / `pub fn attrs_mut(&mut self) -> &mut CellAttributes` (`lib.rs:815,819`)
  - `pub fn presentation(&self) -> Presentation` — 셀의 이모지/텍스트 표현(`lib.rs:769`)

- `pub struct CellAttributes` — 셀 스타일 속성(`lib.rs:51-63`). 빌더식 setter 체이닝(`&mut Self` 반환).
  - 비트필드 getter/setter(매크로 `bitfield!` 생성, `lib.rs:196-206`): `intensity/set_intensity`, `underline/set_underline`, `blink/set_blink`, `italic/set_italic`, `reverse/set_reverse`, `strikethrough/set_strikethrough`, `invisible/set_invisible`, `wrapped/set_wrapped`, `overline/set_overline`, `semantic_type/set_semantic_type`, `vertical_align/set_vertical_align`
  - 색상: `set_foreground`/`foreground`, `set_background`/`background`, `set_underline_color`/`underline_color` (`lib.rs:234,261,269,296,405,477`)
  - `pub const fn blank() -> Self` (`lib.rs:208`)
  - `pub fn attribute_bits_equal(&self, other) -> bool` — 비트필드만 비교(렌더러 최적화용, `lib.rs:220`)
  - `pub fn compute_shape_hash<H: Hasher>(&self, hasher)` — 셰이핑 캐시 키 해시(`lib.rs:224`)
  - `pub fn clear(&mut self)` (`lib.rs:305`)
  - `pub fn clone_sgr_only(&self) -> Self` — 하이퍼링크·이미지 등 부가 데이터를 제외하고 SGR 스타일만 복제(라인 클리어 시 배경색 보존용, `lib.rs:422`)
  - `pub fn set_hyperlink(&mut self, Option<Arc<Hyperlink>>)` / `pub fn hyperlink(&self) -> Option<&Arc<Hyperlink>>` (`lib.rs:344,461`)
  - `pub fn apply_change(&mut self, change: &AttributeChange)` — 변경 스트림 적용(`lib.rs:484`)
  - (`use_image`) `pub fn set_image(Box<ImageCell>)`, `clear_images`, `detach_image_with_placement`, `attach_image(Box<ImageCell>)`, `images() -> Option<Vec<ImageCell>>` (`lib.rs:356-402,468-475`)

- `pub enum SemanticType { Output, Input, Prompt }` — 셀의 의미 구분(셸 chrome/입력/출력). `#[repr(u8)]`, 기본값 `Output`(`lib.rs:175-184`).
- `pub enum AttributeChange` — 변경 스트림의 속성 단위 변경(`lib.rs:981-994`).
- `pub struct UnicodeVersion` — 폭 계산 기준 유니코드 버전(`lib.rs:824-830`). `pub const fn new(version: u8)`, `pub fn idx(&self) -> usize`.
- `pub const LATEST_UNICODE_VERSION: UnicodeVersion` — 버전 14(`lib.rs:874-879`).
- 폭/공백 순수 함수:
  - `pub fn unicode_column_width(s: &str, version: Option<&UnicodeVersion>) -> usize` (`lib.rs:901`)
  - `pub fn grapheme_column_width(s: &str, version: Option<&UnicodeVersion>) -> usize` (`lib.rs:937`)
  - `pub fn is_white_space_char(c: char) -> bool` / `pub fn is_white_space_grapheme(g: &str) -> bool` (`lib.rs:882,888`)
- 재노출: `pub use wezterm_char_props::emoji::Presentation`, `pub use wezterm_escape_parser::osc::Hyperlink`, `pub use wezterm_escape_parser::csi::{Blink, Intensity, Underline, VerticalAlign}` (`lib.rs:12,16,187`).

### 3.2 `color` 모듈 (`color.rs`)

- `pub enum ColorAttribute` — 셀 색상 지정(`color.rs:28-38`). 4변형: `TrueColorWithPaletteFallback`, `TrueColorWithDefaultFallback`, `PaletteIndex`, `Default`(기본).
  - `From<AnsiColor>`, `From<ColorSpec>` 변환 구현(`color.rs:41,47`).
- 재노출: `pub use wezterm_color_types::{LinearRgba, SrgbaTuple}`, `pub use wezterm_escape_parser::color::{AnsiColor, ColorSpec, PaletteIndex, RgbColor}` (`color.rs:7,19`).

### 3.3 `image` 모듈 (`image.rs`, `use_image` feature)

- `pub struct TextureCoordinate { x, y: NotNan<f32> }` — 이미지 내 텍스처 좌표(`image.rs:43-60`).
- `pub struct ImageCell` — 한 셀에 매핑되는 이미지 슬라이스. 텍스처 좌표·z-index·패딩·`image_id`/`placement_id` 보유(`image.rs:81-100`). 접근자 다수 + `with_z_index`(인자 10개), `matches_placement`, `compute_shape_hash`.
- `pub enum ImageDataType` — 이미지 페이로드 표현(`image.rs:201-231`): `EncodedFile`, `EncodedLease`(`std`), `Rgba8`, `AnimRgba8`. 디코딩/해시/스왑아웃 메서드 보유.
- `pub struct ImageData` — 디코딩된 이미지 + 해시. `Mutex<ImageDataType>`로 데이터 보호(`image.rs:516-520`).
- `pub enum ImageCellError` — IO/BlobLease/Image 에러 래핑(`image.rs:503-514`).

---

## 4. 내부 구조

3개 모듈로 구성된다.

- `lib.rs`(약 1224행, 테스트 약 230행 포함) — 크레이트의 중핵. `Cell`, `CellAttributes`, `TeenyString`, `UnicodeVersion`, 폭 계산 함수, `bitfield!` 매크로를 모두 포함하는 **비대 모듈**이다. 리팩토링 시 분할 후보.
- `color.rs`(약 56행) — 색상 속성 enum과 변환. 얇은 어댑터 계층.
- `image.rs`(약 593행) — 이미지 모델링·디코딩. `use_image` feature로 게이트됨.

### 제어/데이터 흐름

1. **셀 생성**: 상위 크레이트가 그래핌 문자열과 `CellAttributes`로 `Cell::new_grapheme`을 호출 → `TeenyString::from_str`이 제어문자를 공백으로 정화(de-fang)하고 폭을 계산, u64에 인라인 또는 힙 할당(`lib.rs:595-635`).
2. **폭 계산**: `grapheme_column_width`가 단일 ASCII 바이트면 핫패스(~3-4ns)로 `WCWIDTH_TABLE.classify` 후 `UnicodeVersion::width`로 분류(`lib.rs:937-976`). 멀티바이트면 유니코드 14 이상에서 `Presentation::for_grapheme`로 이모지/텍스트 표현을 먼저 판정하여 폭을 강제. 최종 폭은 `min(2)`로 클램프.
3. **속성 변경**: `apply_change`가 `AttributeChange` 변형을 대응 setter로 디스패치(`lib.rs:484-518`).
4. **Fat 속성 라이프사이클**: 색상이 TrueColor이거나 하이퍼링크/이미지/언더라인 색상이 설정될 때만 `allocate_fat_attributes`로 힙 할당. 모든 fat 필드가 기본값이 되면 `deallocate_fat_attributes_if_none`이 즉시 해제하여 풋프린트를 회수(`lib.rs:309-342`).

---

## 5. 핵심 데이터 구조·타입

### `TeenyString` (`lib.rs:541-707`)

단일 `u64`에 셀 텍스트를 패킹하는 union-like 표현. 불변식:

- 마커 비트(little-endian이면 MSB, big-endian이면 LSB)가 셋이면 텍스트가 u64 내에 인라인 저장(최대 워드폭-1=7바이트). 미셋이면 u64가 `Box<TeenyStringHeap>` 원시 포인터를 담는다(`lib.rs:561-593`).
- 인라인 시 다음 MSB(double-wide 비트)가 폭 2 여부를 나타내 `grapheme_column_width` 재호출을 단축(`lib.rs:582-593`).
- **저장 바이트는 항상 유효 UTF-8**임이 생성자에 의해 보장됨. 이 불변식 위에서 `str()`/직렬화가 `from_utf8_unchecked`로 검증을 생략한다(`lib.rs:659-663,535-538`).
- 길이 0 문자열은 허용되지 않음(빈 문자열·`\r\n`·제어문자는 공백으로 정화, `lib.rs:603-610`).
- `unsafe` 포인터 조작과 수동 `Drop`(`lib.rs:683-690`)을 수반하므로 64비트 포인터 폭을 전제로 한다.

### `CellAttributes` (`lib.rs:51-63`)

- `attributes: u32` 비트필드(intensity 2bit, underline 3bit, blink 2bit, italic/reverse/strikethrough/invisible/wrapped/overline 각 1bit, semantic_type 2bit, vertical_align 2bit). 비트 배치는 `lib.rs:196-206`에 명시.
- `foreground`/`background`: `SmallColor`(Default 또는 PaletteIndex만). TrueColor는 fat으로 스필.
- `fat: Option<Box<FatAttributes>>`: 하이퍼링크·이미지·언더라인색·TrueColor 전경/배경을 담는 힙 분리 구조(`lib.rs:86-99`).
- **불변식**: fat은 비기본값이 하나라도 있을 때만 존재. 모두 기본값이면 `None`이어야 함(`deallocate_fat_attributes_if_none`이 강제).
- `enum` 비트필드 getter는 `mem::transmute`로 u8 → enum 변환(`lib.rs:153-167`). enum의 `#[repr(u8)]`와 비트마스크 범위가 정확히 일치해야 하는 **안전성 의존**이 존재.
- 메모리 크기 회귀 테스트: `size_of::<CellAttributes>() == 16`, `size_of::<Cell>() == 24`(`lib.rs:1024-1025`).

### `Cell` (`lib.rs:709-722`)

`TeenyString`(8바이트) + `CellAttributes`(16바이트) = 24바이트. 직렬화 시 텍스트만 문자열로 직렬화하는 커스텀 (de)serializer 사용(`lib.rs:713-719`).

### `UnicodeVersion` (`lib.rs:824-872`)

- `version: u8`, `ambiguous_are_wide: bool`, (`std`) `cell_widths: Option<Arc<HashMap<u32,u8>>>`(사용자 폭 오버라이드).
- `width()`가 폭 결정 로직의 핵심: `Unassigned`→1(심볼 폰트 대응, issue #1864), `Ambiguous && ambiguous_are_wide`→2, 버전≥9면 unicode 9+ 폭, 아니면 8 이하 폭(`lib.rs:842-856`).

### `ImageData` / `ImageDataType` (`image.rs:199-592`)

- `ImageData`의 동등성은 `hash`(SHA-256) 비교로만 판정(`image.rs:542-546`). 데이터 자체 비교를 피하는 최적화이자 불변식.
- `Rgba8`는 `width*height*4 == data.len()` 불변식을 생성 시 단언(`image.rs:274-281`).

---

## 6. 외부 의존성

- `finl_unicode` = 그래핌 클러스터 분할(`Graphemes`). `unicode_column_width`가 문자열을 그래핌 단위로 쪼개 폭을 합산하는 데 필수(`lib.rs:9,902`).
- `image` = 이미지 디코딩. PNG/GIF/WebP/APNG를 프레임으로 디코딩하고 RGBA8/AnimRGBA8로 정규화(`image.rs:377-501`). `use_image` 전용.
- `sha2` = 이미지 콘텐츠 해시. 동등성·캐시 키·blob content-id 산출(`image.rs:300-328`). `use_image` 전용.
- `wezterm-blob-leases` = 인메모리 인코딩 이미지를 디스크 블롭으로 스왑아웃하여 메모리 절감(`image.rs:363-372`). `use_image` 전용.
- `ordered-float` = `NotNan<f32>`. 텍스처 좌표가 NaN이 되지 않음을 타입으로 보장하여 `Eq`/`Hash` 파생 가능(`image.rs:14,43-60`).
- `thiserror` = `ImageCellError` 정의(`image.rs:503`). `use_image` 전용.
- `wezterm-dynamic` = Lua 설정 상호운용(`FromDynamic`/`ToDynamic`). `ColorAttribute`·`SemanticType`·`AttributeChange`에 적용.

---

## 7. 설정·기능 플래그

`Cargo.toml:26-29` 정의 feature:

| feature | 효과 |
|---|---|
| `std` | 표준 라이브러리 활성. 미설정 시 `#![no_std]` + `alloc`(`lib.rs:1`). 의존 크레이트들에 `std` 전파. `UnicodeVersion::cell_widths`(사용자 폭 오버라이드)와 `EncodedLease`는 `std` 전용 |
| `use_serde` | serde 직렬화 derive. `Cell`/`CellAttributes`/`ColorAttribute`/이미지 타입에 `Serialize`/`Deserialize` 부여 |
| `use_image` | 이미지 지원 전체. `image`/`wezterm-blob-leases`/`log`/`ordered-float`/`sha2`/`thiserror` 의존성 활성화 및 `CellAttributes`의 이미지 메서드·`image` 모듈 노출 |

소비처별 활성화: `termwiz`는 `std`(조건부 `use_serde`/`use_image`), `wezterm-term`은 `std`+`use_image`, `wezterm-surface`는 기본 + 상위 feature 전파(2절 표 참조). 즉 GUI 빌드 경로에서는 `use_image`가 사실상 항상 켜진다.

관련 config 항목: 폭 계산은 `UnicodeVersion`(version, ambiguous_are_wide, cell_widths)으로 파라미터화된다. 이 값들은 상위 `config` 크레이트가 사용자 설정(unicode 버전, 모호 문자 폭, 셀 폭 오버라이드)으로부터 채워 넘긴다. `wezterm-cell` 자체에는 config 의존이 없다(소스 기본값 수정 철학과 정합).

---

## 8. Windows 전용 고려사항

- 이 크레이트는 **플랫폼 분기 코드를 포함하지 않는다.** 모든 `cfg`는 feature 게이트(`std`, `use_serde`, `use_image`) 또는 엔디안 게이트(`target_endian`)이며, OS 분기(`cfg(unix)`/`cfg(windows)`)는 존재하지 않는다. 따라서 Windows fork에서 제거할 죽은 플랫폼 경로가 없다.
- Windows API 직접 호출 지점 없음. 이미지 디코딩은 순수 `image` 크레이트, blob 스왑아웃은 `wezterm-blob-leases`에 위임한다.
- **암묵적 64비트 가정**: `TeenyString`의 u64 포인터 패킹과 메모리 크기 회귀 테스트(`#[cfg(target_pointer_width = "64")]`, `lib.rs:1020`)는 x86_64를 전제로 한다. Windows on ARM(aarch64) 역시 64비트이므로 유효하나, 32비트 타겟에서는 `TeenyString::from_str`의 `len < size_of::<u64>()` 조건이 의도와 어긋날 여지가 있다(설계 주석은 "machine word size" 기준, `lib.rs:541-548`). Windows 전용 분기로서 빌드 타겟이 x86_64로 고정되어 있으므로 현재는 안전하다.
- `target_endian` 분기(`lib.rs:561-575,637-643`)는 little-endian/big-endian 모두를 다루나, Windows(x86_64/aarch64)는 항상 little-endian이므로 big-endian 경로는 죽은 경로다. 다만 제거 시 이식성을 잃으므로 보존이 합리적이다.

---

## 9. 리팩토링 주의점

1. **`unsafe` 밀집 영역 — `TeenyString`**: `from_utf8_unchecked`, 원시 포인터 캐스팅, 수동 `Drop`, `copy_nonoverlapping`을 사용한다. "저장 바이트는 항상 유효 UTF-8", "마커 비트가 저장 형태를 결정"이라는 불변식이 깨지면 UB로 직결된다. 생성자(`from_str`)의 정화 로직과 마커/double-wide 비트 설정을 변경할 때는 `as_bytes`/`str`/`Drop`/`Clone`/`PartialEq` 전부의 정합을 함께 검증해야 한다.

2. **`mem::transmute` 비트필드 enum**: `bitfield!`의 enum 변형(`lib.rs:153-167`)은 비트마스크 범위의 정수를 `transmute`로 enum화한다. `Intensity`/`Underline`/`Blink`/`SemanticType`/`VerticalAlign`의 `#[repr(u8)]` 판별자 값이 비트마스크가 표현 가능한 범위(예: underline 3bit=0..7)를 벗어나면 잘못된 판별자가 생성되어 UB가 된다. 이 enum들은 `wezterm-escape-parser`에 정의되어 있어 **크레이트 경계를 넘는 결합**이다. 해당 enum 확장 시 비트폭을 동반 점검해야 한다.

3. **메모리 크기 회귀 테스트**: `size_of` 단언(`lib.rs:1019-1029`)이 의도적 가드다. `CellAttributes`/`Cell`에 필드 추가 시 24바이트가 깨지며 테스트가 실패한다. 이는 풋프린트 회귀를 막는 의도된 마찰이므로, 단순히 테스트 값을 올리지 말고 fat 스필 여부를 먼저 검토할 것.

4. **fat 할당/해제 불변식**: 모든 색상·하이퍼링크·언더라인색 setter가 `allocate_fat_attributes`/`deallocate_fat_attributes_if_none` 쌍을 정확히 호출해야 "fat은 비기본값이 있을 때만 존재"가 유지된다. 새 fat 필드를 추가하면 `deallocate` 판정 조건(`lib.rs:322-342`)에 반드시 반영해야 한다. 누락 시 메모리 누수성 잔류 또는 잘못된 동등성 비교가 발생한다.

5. **외부 호출부와 결합된 시그니처**: `set_image`/`attach_image`/`with_z_index`가 `Box<ImageCell>`·인자 10개 형태를 유지하는 이유는 `wezterm-surface`/`wezterm-term`/`wezterm-client` 등 다수 호출부 때문이다(주석 `lib.rs:359-362,388`, `image.rs:124-125`). 시그니처 변경은 광역 변경이 되므로 신중해야 한다.

6. **`lib.rs` 비대화**: 텍스트 패킹·속성 비트필드·폭 계산·유니코드 버전이 한 파일에 혼재한다. `teeny_string.rs`/`attributes.rs`/`width.rs`로 분할하면 가독성과 `unsafe` 격리가 개선된다. 단, `bitfield!` 매크로와 메모리 테스트의 위치 의존성에 유의.

7. **순환 의존 없음**: deps/usedBy 그래프에 순환은 없다. 다만 `Hyperlink`/`Blink`/`Intensity` 등 다수 타입을 `wezterm-escape-parser`에서 재노출하므로(`lib.rs:12,16,187`, `color.rs:19`), 그 크레이트의 API 변경이 본 크레이트의 공개 표면에 직접 전파된다.

8. **폭 계산의 미묘함**: `grapheme_column_width`는 유니코드 버전·이모지 표현·ambiguous 폭·private use·unassigned에 따라 분기가 많고(`lib.rs:937-976`), 다수의 회귀 테스트(issue 997/1161/1573/6637 등)가 동작을 고정한다. 핫패스(단일 ASCII) 최적화를 건드릴 때는 이 테스트군을 반드시 통과시켜야 한다.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `Cargo.toml` | 33 | 패키지·의존성·feature(`std`/`use_serde`/`use_image`) 정의 |
| `src/lib.rs` | 1224 (테스트 ~230 포함) | 크레이트 중핵. `Cell`·`CellAttributes`·`TeenyString`·`UnicodeVersion`·`SemanticType`·`AttributeChange`, `bitfield!` 매크로, 폭/공백 계산 순수 함수. 비대 모듈 |
| `src/color.rs` | 56 | `ColorAttribute` enum과 `AnsiColor`/`ColorSpec` 변환, 색상 타입 재노출 |
| `src/image.rs` | 593 | `use_image` 게이트. `ImageCell`·`ImageData`·`ImageDataType`·`TextureCoordinate`·`ImageCellError`, 이미지 디코딩(PNG/GIF/WebP/APNG)·SHA-256 해시·blob 스왑아웃 |

이 크레이트에는 생성된(generated) 데이터 테이블 파일이 없다. 폭 분류 테이블 등 생성물은 의존 크레이트 `wezterm-char-props`에 위치한다.
