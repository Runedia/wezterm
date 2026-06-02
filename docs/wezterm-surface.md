# wezterm-surface 폴더 기능 명세

본 문서는 `E:\Project\wezterm\wezterm-surface` 단일 크레이트(`wezterm-surface`)에 대한 상세 기능 명세다. 모든 기술은 해당 폴더의 `Cargo.toml`과 `src/**/*.rs` 실제 코드를 근거로 한다. 코드 인용은 `파일경로:라인` 형식을 따른다.

---

## 1. 개요 및 책임

`wezterm-surface`는 **터미널 화면의 메모리 모델**을 제공하는 크레이트다. 실제 터미널 디바이스나 GPU 렌더러와 직접 결합하지 않고, 두 개의 핵심 추상을 책임진다.

- **`Line`** — 한 행(row)의 셀 데이터 모델. 셀 텍스트·속성·더블폭/더블하이트·BiDi·암묵적 하이퍼링크 상태를 보유하며, 두 가지 저장 표현(편집용 `VecStorage`, 압축용 `ClusteredLine`)을 투명하게 전환한다 (`src/line/inner.rs:46`).
- **`Surface`** — 화면 전체(행의 집합) 모델. `Change` 열거형으로 표현되는 변경 명령을 누적·적용하고, 마지막 렌더 이후의 최소 변경 스트림을 산출(`get_changes`)하거나 두 서피스 간 차분(`diff_region`)을 계산한다 (`src/lib.rs:111`).

크레이트 자신의 문서 설명대로, `Surface`는 "터미널 디바이스에 직접 연결되지 않은, 버퍼와 변경 로그로 구성된 화면 내용"이다 (`src/lib.rs:86`). 즉 이 크레이트의 단일 책임은 **"무엇이 화면에 있는가"를 표현하고, 그 상태를 효율적으로 갱신·차분·복원하는 자료구조와 알고리즘을 제공하는 것**이다. 렌더링(폰트 셰이핑, GPU 출력)이나 PTY 입출력은 책임 범위 밖이다.

Windows fork에서의 실제 책임도 동일하다. 이 크레이트는 본질적으로 플랫폼 독립적인 자료구조 계층이며, OS API를 직접 호출하지 않는다. `#![cfg_attr(not(feature = "std"), no_std)]`로 선언되어 `alloc`만 의존하는 코어 라이브러리 형태를 유지한다 (`src/lib.rs:1`). 따라서 Windows 전용 분기에서도 코드 변경 없이 그대로 채택되며, 상위 크레이트(`wezterm-term`, `termwiz`)가 이 모델을 소비한다.

---

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 사용 목적 |
| --- | --- |
| wezterm-cell | `Cell`, `CellAttributes`, `ColorAttribute`, `AttributeChange`, `SemanticType`, `UnicodeVersion`, 이미지 셀 등 셀 단위 원자 타입 제공. 본 크레이트의 가장 기초적 의존 |
| wezterm-char-props | `emoji::Presentation` (이모지 표현 형식 판정). 셀 클러스터링 시 presentation 경계 판정에 사용 (`src/cellcluster.rs:5`) |
| wezterm-bidi | `BidiContext`, `Direction`, `ParagraphDirectionHint`. 양방향(BiDi) 텍스트 재정렬에 사용 (`src/cellcluster.rs:3`) |
| wezterm-escape-parser | `hyperlink::Hyperlink` 타입 재노출(re-export). 하이퍼링크 모델의 정의처 (`src/hyperlink.rs:22`) |
| wezterm-dynamic | `FromDynamic`/`ToDynamic` 파생. `Rule`, `CursorShape`, `CursorVisibility` 등을 Lua 설정 값으로 직렬화/역직렬화 |
| wezterm-color-types | 색상 타입 그래프(간접). `ColorAttribute` 경유 |
| wezterm-input-types | 입력 좌표/타입(간접 의존) |
| wezterm-blob-leases | (선택, `appdata` 미사용 경로) blob 임대. 본 크레이트 코드에서 직접 참조 없음 |

외부 크레이트: `bitflags`, `fancy-regex`, `finl_unicode`, `fixedbitset`, `ordered-float`, `serde`(선택), `siphasher`, `unicode-segmentation` (`Cargo.toml:12`).

### 피의존(usedBy)

| 크레이트 | 소비 형태 |
| --- | --- |
| wezterm-term (`term/`) | 터미널 에뮬레이터의 화면 버퍼로 `Line`을 직접 보유. 스크롤백·재래핑·하이퍼링크 스캔에 `Line` API 전면 활용 |
| termwiz | 터미널 UI 툴킷. `Surface`/`Change`/`ChangeSequence`를 위젯 합성과 렌더 차분에 사용 |

### 계층 위치

`wezterm-cell`(셀 원자) → **`wezterm-surface`(행·화면 모델)** → `wezterm-term`/`termwiz`(에뮬레이터·UI)로 이어지는 파이프라인의 **중간 계층**이다. 셀 단위 타입을 받아 행·화면 단위 자료구조와 차분 알고리즘으로 끌어올리고, 그 위의 에뮬레이터/UI 계층에 모델을 제공한다.

---

## 3. 공개 API 표면

### 3.1 lib 루트 (`src/lib.rs`)

- `pub enum Position { Relative(isize), Absolute(usize), EndRelative(usize) }` — 커서 위치 지정 모드. 0-기반 (`src/lib.rs:38`).
- `pub enum CursorVisibility { Hidden, Visible }` (기본 `Visible`) (`src/lib.rs:50`).
- `pub enum CursorShape { Default, BlinkingBlock, SteadyBlock, … }` + `pub fn is_blinking(self) -> bool` (`src/lib.rs:60`, `:73`).
- `pub type SequenceNo = usize` / `pub const SEQ_ZERO: SequenceNo = 0` — 변경 스트림 내 논리적 위치. `Surface` 인스턴스 안에서만 의미를 가짐 (`src/lib.rs:83`).
- `pub struct Surface { … }` — 화면 모델. 주요 공개 메서드:
  - 생성/크기: `new(width, height)`, `dimensions()`, `resize(width, height)` (`src/lib.rs:202`, `:213`, `:242`).
  - 상태 조회: `cursor_position()`, `cursor_shape()`, `cursor_visibility()`, `title()` (`src/lib.rs:217`–`:231`).
  - 변경 적용: `add_change<C: Into<Change>>(change) -> SequenceNo`, `add_changes(Vec<Change>) -> SequenceNo` (`src/lib.rs:285`, `:271`).
  - 변경 산출: `get_changes(seq) -> (SequenceNo, Cow<[Change]>)`, `has_changes(seq)`, `current_seqno()`, `flush_changes_older_than(seq)` (`src/lib.rs:536`, `:556`, `:560`, `:568`).
  - 차분/합성: `diff_region(...)`, `diff_lines(...)`, `diff_against_numbered_line(...)`, `diff_screens(other)`, `draw_from_screen(other, x, y)`, `copy_region(...)` (`src/lib.rs:744`–`:836`).
  - 테스트/조회용: `screen_chars_to_string()`, `screen_cells() -> Vec<&mut [Cell]>`, `screen_lines() -> Vec<Cow<Line>>` (`src/lib.rs:498`, `:513`, `:521`).
- 재노출: `pub use change::{Change, LineAttribute}`, `pub use line::Line`. `use_image` 시 `Image`, `TextureCoordinate` 추가 (`src/lib.rs:26`).

### 3.2 change 모듈 (`src/change.rs`)

- `pub enum Change { Attribute, AllAttributes, Text, ClearScreen, ClearToEndOfLine, ClearToEndOfScreen, CursorPosition{x,y}, CursorColor, CursorShape, CursorVisibility, Image(선택), ScrollRegionUp{…}, ScrollRegionDown{…}, Title, LineAttribute }` (`src/change.rs:31`). 화면 갱신 명령의 핵심 어휘.
- `impl Change { pub fn is_text(&self) -> bool; pub fn text(&self) -> &str }` (`src/change.rs:116`).
- `From` 변환: `String`, `&str`, `AttributeChange`, `LineAttribute` → `Change` (`src/change.rs:128`–`:150`).
- `pub enum LineAttribute { DoubleHeightTopHalfLine, DoubleHeightBottomHalfLine, DoubleWidthLine, SingleWidthLine }` (`src/change.rs:19`).
- `pub struct ChangeSequence { … }` — 커서 위치/렌더 높이를 추적하면서 `Change`를 누적하는 빌더. `new(rows, cols)`, `consume() -> Vec<Change>`, `current_cursor_position()`, `move_to((x,y))`, `render_height()`, `add_changes(...)`, `add<C: Into<Change>>(...)` (`src/change.rs:158`–`:213`). 라인 에디터처럼 화면 일부만 점유하는 동적 출력에 사용.
- `pub struct Image { width, height, top_left, bottom_right, image: Arc<ImageData> }` (`use_image` 전용) (`src/change.rs:300`).

### 3.3 hyperlink 모듈 (`src/hyperlink.rs`)

- `pub use wezterm_escape_parser::hyperlink::Hyperlink` (`src/hyperlink.rs:22`).
- `pub struct Rule { regex: Regex, format: String, highlight: usize }` — 암묵적 하이퍼링크 규칙 (정규식 + URL 포맷 + 강조 캡처) (`src/hyperlink.rs:35`).
  - `pub fn new(regex, format) -> Result<Self, fancy_regex::Error>`, `with_highlight(regex, format, highlight)`, `match_hyperlinks(line, rules) -> Vec<RuleMatch>` (`src/hyperlink.rs:172`, `:178`, `:192`).
- `pub struct RuleMatch { range: Range<usize>, link: Arc<Hyperlink> }` (`src/hyperlink.rs:115`).
- `pub const CLOSING_PARENTHESIS_HYPERLINK_PATTERN`, `pub const GENERIC_HYPERLINK_PATTERN` — 기본 URL 매칭 정규식 (`src/hyperlink.rs:164`).

### 3.4 cellcluster 모듈 (`src/cellcluster.rs`)

- `pub struct CellCluster { attrs, text, width, presentation, direction, first_cell_idx, … }` — 동일 속성 셀 런(run)을 묶은 셰이핑 단위 (`src/cellcluster.rs:18`).
  - `pub fn byte_to_cell_idx(byte_idx) -> usize`, `byte_to_cell_width(byte_idx) -> u8`, `make_cluster(hint, iter, bidi_hint) -> Vec<CellCluster>` (`src/cellcluster.rs:32`, `:40`, `:50`).

### 3.5 line 모듈 (`src/line/`)

- `pub use line::{Line, DoubleClickRange, CellRef}` (`src/line/mod.rs:9`).
- `pub struct Line` — 단일 행 모델. 공개 메서드는 §4에서 분류.
- `pub enum DoubleClickRange { Range(Range<usize>), RangeWithWrap(Range<usize>) }` — 더블클릭 선택 영역(래핑 여부 포함) (`src/line/inner.rs:38`).
- `pub struct ZoneRange { semantic_type: SemanticType, range: Range<u16> }` — 의미 영역(semantic zone) (`src/line/inner.rs:33`).
- `pub enum CellRef<'a> { CellRef{…}, ClusterRef{…} }` — 저장 표현에 무관하게 셀을 가리키는 통합 참조. `cell_index()`, `str()`, `width()`, `attrs()`, `presentation()`, `as_cell()`, `same_contents()`, `compute_shape_hash()` (`src/line/cellref.rs:6`).

---

## 4. 내부 구조

### 4.1 모듈 분해

```
src/lib.rs        Surface, Position, Cursor*, SequenceNo, DiffState(비공개), 차분 알고리즘
src/change.rs     Change, LineAttribute, ChangeSequence, Image
src/hyperlink.rs  Rule, RuleMatch, 정규식 기반 암묵적 하이퍼링크 매칭
src/cellcluster.rs CellCluster — 셰이핑용 클러스터링 + BiDi 재정렬
src/line/
  mod.rs          공개 재노출만
  inner.rs        Line 본체 (비대 모듈, 1222행)
  cellref.rs      CellRef 통합 참조 enum
  clusterline.rs  ClusteredLine 압축 저장 + 반복자
  vecstorage.rs   VecStorage 편집 저장 + 반복자 + 하이퍼링크 셀 적용
  storage.rs      CellStorage(V|C) 디스패치 enum + VisibleCellIter
  linebits.rs     LineBits 비트플래그
  test.rs         단위 테스트 (#![cfg(test)])
```

### 4.2 Surface의 제어 흐름

`Surface`는 **모델 상태(`lines`, `xpos`/`ypos`, `attributes`, 커서/타이틀)** 와 **변경 로그(`changes: Vec<Change>`, `seqno`)** 를 동시에 유지한다 (`src/lib.rs:111`).

- `add_change`/`add_changes`는 변경을 `apply_change`로 모델에 반영하면서 동시에 `changes` 로그에 적재하고 `seqno`를 증가시킨다 (`src/lib.rs:285`, `:294`).
- `apply_change`는 `Change` 변종별로 `print_text`, `clear_*`, `scroll_region_*`, `set_cursor_pos`, `line_attribute`, (선택) `add_image`로 디스패치한다 (`src/lib.rs:294`).
- `get_changes`는 휴리스틱으로 비용을 비교한다. 시퀀스 연속성이 깨졌거나 델타 비용이 전체 재페인트 추정 비용(`estimate_full_paint_cost`, 셀당 1 + 20% 오버헤드)을 초과하면 `repaint_all()`로 전체 재페인트를 생성하고, 아니면 `changes`의 부분 슬라이스를 `Cow::Borrowed`로 반환한다 (`src/lib.rs:536`, `:579`).
- `repaint_all`은 후미의 빈 줄을 `ClearToEndOfScreen`로 합치고, 줄 끝 상대 커서 이동(`\r\n` 유도)을 사용하며, 불필요한 후미 커서 이동을 제거하는 등 출력 트래픽 최소화 로직을 포함한다 (`src/lib.rs:584`).

데이터 흐름의 차분 경로는 비공개 `DiffState`가 담당한다. `diff_cells`/`set_cell`이 커서·속성 중복 발행을 억제하고 연속 텍스트를 하나의 `Change::Text`로 합친다 (`src/lib.rs:140`). `diff_line` 자유 함수가 더블폭 셀로 인해 셀 인덱스가 어긋나는 경우까지 정확히 처리한다 (`src/lib.rs:841`).

### 4.3 Line의 이중 저장 구조

`Line`은 `CellStorage` enum으로 두 표현을 투명 전환한다 (`src/line/storage.rs:9`).

- **`VecStorage`** — `Vec<Cell>`. 임의 위치 편집에 적합. 모든 변이 메서드는 `coerce_vec_storage()`로 먼저 이 표현으로 강제 전환한다 (`src/line/inner.rs:1059`).
- **`ClusteredLine`** — 연속 문자열 + 속성 클러스터 + 더블폭 비트셋. 메모리 효율적이며 **추가(append) 전용**. 스크롤백 압축(`compress_for_scrollback`)과 신규 라인(`Line::new`) 생성 시 사용 (`src/line/inner.rs:1072`, `:106`).

`set_cell_grapheme`/`set_cell_impl`은 추가가 가능한 경우(인덱스가 클러스터 끝과 일치) `ClusteredLine`을 그대로 유지하고, 임의 편집이 필요하면 자동으로 `VecStorage`로 승격한다 (`src/line/inner.rs:759`, `:808`).

### 4.4 비대 모듈

`src/line/inner.rs`(1222행)와 `src/lib.rs`(1779행, 단 약 870행은 테스트)가 비대하다. `inner.rs`는 `Line`의 모든 책임(저장 전환, BiDi/방향, 더블폭/하이트, 하이퍼링크 스캔, 재래핑, zone 계산, 더블클릭 범위, 변경 산출)을 한 `impl` 블록에 집약한다.

---

## 5. 핵심 데이터 구조·타입과 불변식

### `Surface` (`src/lib.rs:111`)
- 불변식: `lines.len() == height`, 각 `Line`의 길이는 `resize` 시 `width`로 정렬됨 (`src/lib.rs:256`). `xpos`/`ypos`는 항상 `0..width` / `0..height` 범위로 클램프됨 (`compute_position_change`, `src/lib.rs:893`). `seqno`는 단조 증가. `changes`는 `seqno`에 대해 시간순.
- `resize`는 변경 로그를 무효화한다(버퍼된 변경이 있으면 `seqno`만 증가시키고 `changes`를 비워, 다음 `get_changes`가 전체 재페인트가 되도록 함) (`src/lib.rs:251`).

### `Line` (`src/line/inner.rs:46`)
- 필드: `cells: CellStorage`, `zones: Vec<ZoneRange>`, `seqno`, `bits: LineBits`, (`appdata` 선택) `appdata: Mutex<Option<Weak<…>>>`.
- 불변식: `PartialEq`/`Clone`을 수동 구현하며 `appdata`(Weak)는 동등성에서 제외하고 clone 시 약한참조를 복사한다 (`src/line/inner.rs:56`, `:69`). `seqno`는 `update_last_change_seqno`로 단조 증가(`max`) (`src/line/inner.rs:294`). 셀 변이 시 항상 `invalidate_zones`와 `invalidate_implicit_hyperlinks`를 호출해 캐시 일관성 유지 (`src/line/inner.rs:808`). 더블폭 셀 뒤의 가려진 셀은 항상 blank로 채워야 한다(부분 렌더 방지) — `set_cell_impl`/`invalidate_grapheme_at_or_before`가 보장 (`src/line/inner.rs:856`, `:887`).

### `ClusteredLine` (`src/line/clusterline.rs:28`)
- 불변식: `len`(셀 단위) == 클러스터들의 `cell_width` 합. `is_double_wide` 비트셋은 더블폭 셀의 셀 인덱스만 1로 설정. `text`는 가시 그래핌의 연속(가려진 blank 미포함). 64비트에서 `size_of::<ClusteredLine>() == 64`를 테스트로 고정 (`src/line/clusterline.rs:378`).

### `CellRef<'a>` (`src/line/cellref.rs:6`)
- 두 저장 표현을 통합. `same_contents`는 str·width·attrs 동등 비교로 차분 시 변경 여부를 판정한다 (`src/line/cellref.rs:66`).

### `LineBits` (`src/line/linebits.rs:8`)
- `u16` 비트플래그. 하이퍼링크 상태 3종, 더블폭/하이트 3종 + 마스크, BiDi 3종(`BIDI_ENABLED`, `RTL`, `AUTO_DETECT_DIRECTION`). `is_single_width`/`set_double_*` 등은 이 마스크를 일관되게 조작한다 (`src/line/inner.rs:300`).

### `CellCluster` (`src/cellcluster.rs:18`)
- 불변식: `byte_to_cell_idx`/`byte_to_cell_width`는 비어 있으면 "단일폭·연속 인덱스"를 의미하는 최적화(메모리 절약). 다중바이트/더블폭 셀이 끼어들 때만 실제 벡터를 채운다 (`src/cellcluster.rs:32`, `:243`). BiDi 재정렬 시에도 harfbuzz 입력은 논리적 순서(`run.range`)를 유지한다 (`src/cellcluster.rs:190`).

---

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
| --- | --- |
| `bitflags` | `LineBits` 비트플래그 정의 (`src/line/linebits.rs:1`) |
| `fancy-regex` | 하이퍼링크 규칙의 정규식 엔진. 백레퍼런스/룩어라운드 지원이 필요(괄호 균형 URL 패턴 등) (`src/hyperlink.rs:10`) |
| `finl_unicode` | 그래핌 클러스터 분할(`Graphemes`). 텍스트 출력/래핑의 단위 결정 (`src/lib.rs:5`) |
| `fixedbitset` | `ClusteredLine`의 더블폭 셀 비트셋(`is_double_wide`) (`src/line/clusterline.rs:5`) |
| `unicode-segmentation` | `grapheme_indices`로 바이트↔셀 인덱스 매핑(하이퍼링크 셀 적용) (`src/line/vecstorage.rs:5`) |
| `siphasher` | `compute_shape_hash`의 128비트 SipHash. 셰이핑 캐시 키 산출 (`src/line/inner.rs:19`) |
| `ordered-float` | 이미지 텍스처 좌표의 `NotNan` 누산(`add_image`) (`src/lib.rs:325`) |
| `serde` (선택) | `use_serde` 시 스크롤백 직렬화. 정규식·비트셋 커스텀 직렬화 포함 (`src/hyperlink.rs:95`, `src/line/clusterline.rs:44`) |

GPU/셰이핑 라이브러리(wgpu, harfbuzz)에 대한 직접 의존은 없다. 본 크레이트는 셰이핑 입력(`CellCluster`)을 산출할 뿐, harfbuzz 호출 자체는 상위 렌더 계층이 수행한다.

---

## 7. 설정·기능 플래그

`Cargo.toml:30` 기준 feature:

| feature | 효과 |
| --- | --- |
| `default = []` | 기능 없음(코어) |
| `std` | `fancy-regex`/`fixedbitset` 등의 std 경로 활성화. `fancy-regex/perf`, `unicode` 포함. `no_std` 해제 (`src/lib.rs:1`) |
| `appdata` | `std`를 포함. `Line`에 `appdata: Mutex<Option<Weak<dyn Any>>>` 필드와 `set_appdata`/`get_appdata`/`clear_appdata` API 활성화 (`src/line/inner.rs:51`, `:254`) |
| `use_serde` | `Change`, `Line`, `Rule`, `ClusteredLine` 등에 `Serialize`/`Deserialize` 파생. 스크롤백 영속화 용도 |
| `use_image` | `Change::Image`, `Image`, `TextureCoordinate`, 이미지 셀 처리(`add_image`, `VecStorage::set_cell`의 placement 보존) 활성화 (`src/lib.rs:323`, `src/line/vecstorage.rs:24`) |

`config` 크레이트의 항목과 직접 연동되는 타입은 하이퍼링크 `Rule`이다(`FromDynamic`/`ToDynamic`로 Lua 설정에서 정규식·포맷을 주입받음, `src/hyperlink.rs:34`). Windows fork의 "소스 기본값 수정" 철학을 적용한다면, 기본 하이퍼링크 규칙(`GENERIC_HYPERLINK_PATTERN` 등)이나 커서 기본 형상(`CursorShape::Default`)이 후보다.

---

## 8. Windows 전용 고려사항

- **플랫폼 분기 없음**: 이 크레이트의 어떤 소스에도 `cfg(unix)`/`cfg(windows)`/`cfg(target_os)` 분기가 없다. Win32 API 호출도 없다. 순수 자료구조 계층이므로 Windows fork에서의 죽은 코드는 존재하지 않는다.
- 유일한 조건부 컴파일은 feature 플래그(`appdata`, `use_serde`, `use_image`, `std`)와 `cfg(target_pointer_width = "64")`(메모리 크기 회귀 테스트, `src/line/clusterline.rs:377`, `src/line/storage.rs:35`)뿐이다.
- `appdata` feature는 `std::sync::Mutex`를 사용하므로(`src/line/inner.rs:21`), Windows에서도 표준 라이브러리 동기화 프리미티브로 동작한다. 별도 플랫폼 처리 불필요.
- 결론: 본 크레이트는 Windows 전용 분기의 영향을 받지 않는 **플랫폼 중립 코어**다. 리팩토링 시 OS 관련 우려 사항은 없다.

---

## 9. 리팩토링 주의점

1. **이중 저장 전환의 암묵적 부작용**: 거의 모든 변이 메서드가 `coerce_vec_storage()`를 호출해 `ClusteredLine`→`VecStorage` 승격을 유발한다 (`src/line/inner.rs:1059`). 스크롤백 메모리 최적화의 핵심은 "압축 유지"인데, 무심코 변이 메서드를 추가하면 압축이 풀린다. 새 메서드는 `set_cell_grapheme`처럼 append-fast-path를 우선 시도해야 한다.

2. **캐시 무효화 불변식**: 셀을 바꾸는 모든 경로는 반드시 `invalidate_zones` + `invalidate_implicit_hyperlinks` + `update_last_change_seqno`를 호출해야 한다 (`src/line/inner.rs:808`). 하나라도 누락하면 zone/하이퍼링크/렌더 캐시가 stale 상태가 된다. 특히 `cells_mut_for_attr_changes_only`는 텍스트 내용 변경을 금지하는 계약(주석 명시)이 있으나 컴파일러가 강제하지 못한다 (`src/line/inner.rs:1142`).

3. **더블폭 셀의 가려진 blank 보존**: `append_line`은 `VecStorage`에서 더블폭 셀 뒤 blank를 명시 push해야 메트릭이 어긋나지 않는다(이슈 #2568 회귀 테스트 존재) (`src/line/inner.rs:1122`, `src/line/test.rs:11`). `diff_line`도 셀 인덱스 어긋남을 명시적으로 처리하므로(`src/lib.rs:867`), 차분 로직 변경 시 더블폭 케이스 테스트(`draw_double_width`, `diff_cursor_double_width`)를 반드시 통과시켜야 한다.

4. **`get_changes` 휴리스틱 결합**: `Surface`의 델타/전체 재페인트 선택은 `estimate_full_paint_cost`의 마법 상수(셀당 1 + 20%)에 의존한다 (`src/lib.rs:579`). 이 값과 `repaint_all`의 최적화 패스는 다수의 테스트(`clear_eos`, `clear_eol_opt`, `delta_change` 등)가 정확한 `Change` 시퀀스를 단언하므로, 출력 형태를 바꾸면 광범위한 테스트 수정이 동반된다.

5. **하이퍼링크 에러 박싱 보류**: `Rule::new`/`with_highlight`는 `clippy::result_large_err`를 의도적으로 허용한다. 외부 `config` 크레이트가 `Result<Self, fancy_regex::Error>`를 직접 unwrap하기 때문(주석 명시, `src/hyperlink.rs:170`). 시그니처를 바꾸면 `config` 크레이트 호출부가 깨진다 — 순환은 아니나 피의존 파급 위험.

6. **`ChangeSequence`와 `Surface`의 커서 모델 중복**: 두 타입이 각자 커서 위치/래핑 로직을 별도 구현한다(`src/change.rs:213` vs `src/lib.rs:434`). 한쪽만 수정하면 라인 에디터와 풀스크린 렌더의 동작이 어긋날 수 있다.

7. **모놀리식 `impl Line`**: `inner.rs`의 1200행 단일 impl은 책임이 과밀하다. trait 분리 또는 서브모듈 분해가 가능하나, `pub(crate)` 필드(`cells`)와 비공개 헬퍼들의 결합이 강해 신중한 경계 설정이 필요하다.

순환 의존: 워크스페이스 의존 그래프상 `wezterm-cell`/`wezterm-bidi` 등 하위로만 의존하고, 상위(`term`/`termwiz`)에서 소비되므로 순환은 없다.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `Cargo.toml` | 39 | 패키지·의존·feature 정의 (`std`/`appdata`/`use_serde`/`use_image`) |
| `src/lib.rs` | 1779 | `Surface` 모델, `Position`/`Cursor*`/`SequenceNo`, 비공개 `DiffState`, 차분·재페인트 알고리즘. 약 870행이 테스트 |
| `src/change.rs` | 313 | `Change`/`LineAttribute` 어휘, `ChangeSequence` 빌더, `Image`(선택) |
| `src/hyperlink.rs` | 302 | 암묵적 하이퍼링크 `Rule`/`RuleMatch`, 정규식 매칭, 기본 URL 패턴 |
| `src/cellcluster.rs` | 310 | `CellCluster` — 셰이핑용 속성 런 클러스터링 + BiDi 재정렬 |
| `src/line/mod.rs` | 10 | line 서브모듈 선언 및 `Line`/`CellRef` 재노출 |
| `src/line/inner.rs` | 1222 | `Line` 본체(비대). 저장 전환·BiDi·더블폭/하이트·하이퍼링크·재래핑·zone·더블클릭·변경 산출 |
| `src/line/cellref.rs` | 74 | `CellRef<'a>` 통합 셀 참조 enum |
| `src/line/clusterline.rs` | 385 | `ClusteredLine` 압축 저장 + 클러스터 반복자 + (선택) 비트셋 직렬화 |
| `src/line/vecstorage.rs` | 108 | `VecStorage` 편집 저장 + 가시 셀 반복자 + 하이퍼링크 셀 적용 |
| `src/line/storage.rs` | 40 | `CellStorage`(V/C) 디스패치 enum, `VisibleCellIter` |
| `src/line/linebits.rs` | 53 | `LineBits` u16 비트플래그(하이퍼링크/더블폭·하이트/BiDi) |
| `src/line/test.rs` | 614 | line 모듈 단위 테스트(`#![cfg(test)]`) |

생성 파일([생성])은 본 크레이트에 존재하지 않는다. `clusterline.rs`/`storage.rs`의 `memory_usage` 테스트는 생성물이 아니라 구조체 크기 회귀를 고정하는 수기 테스트다.
