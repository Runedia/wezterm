# bidi 폴더 기능 명세

본 문서는 워크스페이스 폴더 `bidi/`(크레이트 `wezterm-bidi`)의 상세 기능 명세다. 코드 인용은 `파일경로:라인` 형식을 따른다.

---

## 1. 개요 및 책임

`wezterm-bidi`는 **유니코드 양방향 알고리즘(Unicode Bidirectional Algorithm, UBA / UAX #9)**의 단일 구현체다. 단일 책임은 명확하다. **한 문단(paragraph)을 이루는 문자열을 입력받아, 각 코드포인트의 임베딩 레벨(embedding level)과 방향을 해석하고, 시각적 표시를 위한 재정렬(reorder) 결과를 산출한다.**

크레이트는 `#![no_std]`로 선언되어 있으며(`bidi/src/lib.rs:1`), `alloc`만 사용한다. 이는 이 크레이트가 OS·플랫폼·I/O와 완전히 무관한 순수 계산 라이브러리임을 의미한다.

Windows fork에서의 실제 책임은 변하지 않았다. 터미널 셀 클러스터를 셰이핑 직전에 논리 순서에서 시각 순서로 재배열하고, 각 런(run)의 방향을 결정하는 일을 담당한다. 이 크레이트 자체에는 플랫폼 분기가 전혀 없으므로 fork의 Windows 전용화에 영향을 받지 않는다(8절 참조).

알고리즘은 UBA 규칙 단위로 구현되어 있으며, 메서드 주석에 규칙 식별자(X1~X10, W1~W7, N0~N2, I1~I2, L1~L3, P2~P3, BD13, BD16 등)가 명시되어 있다. 구현은 Unicode가 배포하는 참조 구현 `bidiref`의 구조를 따른다(`bidi/src/level_stack.rs:11`의 "from bidiref" 주석 등).

---

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 용도 |
|---|---|
| `wezterm-dynamic` | `ParagraphDirectionHint`에 `FromDynamic`/`ToDynamic` 파생 부여(`bidi/src/lib.rs:7,28`). config 직렬화 체계와의 연동을 위해서만 필요. |
| `log` | `trace!`를 통한 알고리즘 단계별 상태 추적 로그(`bidi/src/lib.rs:6`). |

### 피의존(usedBy)

| 크레이트 | 사용 형태(확인됨) |
|---|---|
| `config` | `ParagraphDirectionHint`를 config 항목 `bidi_direction`의 타입으로 사용(`config/src/config.rs:42,793`). |
| `wezterm-surface` | `BidiContext`/`Direction`/`ParagraphDirectionHint`로 셀 클러스터를 재정렬(`wezterm-surface/src/cellcluster.rs:3,158,183`). |
| `termwiz` | 의존성으로 선언(터미널 셀/표면 처리 계층). |
| `wezterm-font` | 셰이핑 입력 방향 결정에 사용(`wezterm-font/src/shaper/`). |
| `wezterm-gui` | 렌더 경로에서 사용. |
| `wezterm-term` | 터미널 상태 처리에 사용. |
| `window` | 의존성으로 선언. |

### 계층상의 위치

`wezterm-bidi`는 워크스페이스 최하단의 **무의존(leaf-level) 순수 계산 크레이트**다. `wezterm-dynamic`(직렬화)과 `log`(로깅) 외에 다른 내부 크레이트에 의존하지 않으며, 셀 클러스터 → bidi 재정렬 → 폰트 셰이핑 → GPU 렌더로 이어지는 텍스트 표시 파이프라인의 **입구 직후**에 위치한다.

---

## 3. 공개 API 표면

크레이트 루트(`bidi/src/lib.rs`)가 재노출(re-export)하는 타입은 다음과 같다(`bidi/src/lib.rs:20-23`).

- `pub use bidi_class::BidiClass;`
- `pub use direction::Direction;`
- `pub use level::Level;`
- `BracketType`는 `use`만 하고 재노출하지 않음(crate-private).

### 핵심 진입 타입: `BidiContext`

상태를 누적하는 컨텍스트 구조체다(`bidi/src/lib.rs:56`). 주요 공개 메서드:

| 시그니처 | 용도 |
|---|---|
| `pub fn new() -> Self` (`:194`) | 기본 컨텍스트 생성. |
| `pub fn base_level(&self) -> Level` (`:198`) | 해석된 문단 기준 레벨 반환. |
| `pub fn set_reorder_non_spacing_marks(&mut self, reorder: bool)` (`:206`) | 규칙 L3(결합 마크 재정렬) 활성화. 터미널 용도에 권장. |
| `pub fn resolve_paragraph(&mut self, paragraph: &[char], hint: ParagraphDirectionHint)` (`:432`) | 문자열로부터 BidiClass를 산출한 뒤 전체 UBA 파이프라인 실행. 주 진입점. |
| `pub fn set_char_types(&mut self, char_types: &[BidiClass], hint: ParagraphDirectionHint)` (`:446`) | 문자 대신 사전 산출된 BidiClass 배열로 해석. UCD `BidiTest` 호환용. |
| `pub fn runs<'a>(&'a self) -> impl Iterator<Item = BidiRun> + 'a` (`:212`) | 문단 전체에 대한 동일 레벨 런 시퀀스 반환. |
| `pub fn line_runs(&self, line_range: Range<usize>) -> impl Iterator<Item = BidiRun>` (`:223`) | 줄 단위(래핑된 한 줄)로 L1 공백 레벨 리셋 후 런 반환. |
| `pub fn reordered_runs(&self, line_range: Range<usize>) -> Vec<ReorderedRun>` (`:233`) | 시각 순서로 재정렬된 인덱스를 담은 런 목록 반환. 표시용 핵심 API. |
| `pub fn reorder_line(&self, line_range: Range<usize>) -> (Vec<Level>, Vec<usize>)` (`:274`) | 줄에 대한 (레벨 벡터, 시각 순서 인덱스) 쌍 반환. |

### 자유 함수

- `pub fn bidi_class_for_char(c: char) -> BidiClass` (`:1987`) — 코드포인트의 Bidi_Class를 생성 테이블 이분 탐색으로 조회. 미등재 코드포인트는 `LeftToRight` 폴백(`:2008`).

### 보조 공개 타입

- `pub enum ParagraphDirectionHint` (`:30`) — `LeftToRight`(기본), `RightToLeft`, `AutoLeftToRight`, `AutoRightToLeft`. `direction(self) -> Direction` 메서드로 방향 부분만 추출(`:44`).
- `pub struct BidiRun` (`:82`) — `direction`, `level`, `range`, `removed_by_x9` 필드. `indices()` 이터레이터로 X9 제거 제어문자를 건너뛴 인덱스 순회(`:100`).
- `pub struct ReorderedRun` (`:169`) — `direction`, `level`, `range`, `indices`(조정된 시각 순서) 필드.
- `pub const NO_LEVEL: i8 = -1` (`:67`) — X9 규칙으로 제거된 포매팅 문자의 레벨 표식.
- `Level`(`bidi/src/level.rs:10`), `Direction`(`bidi/src/direction.rs:4`), `BidiClass`(`bidi/src/bidi_class.rs:5`)의 공개 메서드는 5절 참조.

---

## 4. 내부 구조

모듈 구성(`bidi/src/lib.rs:14-18`):

| 모듈 | 성격 |
|---|---|
| `lib.rs` | 알고리즘 전체. 2089행으로 **압도적으로 비대한 모듈**. |
| `bidi_class.rs` | [생성] `BidiClass` enum + 코드포인트→클래스 테이블. |
| `bidi_brackets.rs` | [생성] `BracketType` enum + 괄호쌍 테이블. |
| `direction.rs` | `Direction` enum과 방향 인지 이터레이터. |
| `level.rs` | `Level` 뉴타입과 레벨 산술. |
| `level_stack.rs` | X1~X8용 명시적 방향 스택. |

### 제어 흐름 (`resolve`, `bidi/src/lib.rs:452-501`)

`resolve_paragraph` → `populate_char_types`(BD1, 각 문자에 Bidi_Class 부여 `:439`) → `resolve`가 UBA 파이프라인을 순서대로 실행:

1. **기준 레벨 결정** — 힌트에 따라 `Level(0)`/`Level(1)` 또는 `paragraph_level`로 자동 검출(P2/P3, `:457-466`, `:1841`).
2. **X1~X8 `explicit_embedding_levels`** (`:1317`) — `LevelStack`을 사용해 명시적 임베딩/오버라이드/아이솔레이트 제어문자를 처리하고 레벨 부여. overflow_isolate/overflow_embedding/valid_isolate 카운터로 오버플로 처리.
3. **X9 `delete_format_characters`** (`:1487`) — RLE/LRE/RLO/LRO/PDF/BN을 `NO_LEVEL`로 표시("삭제").
4. **X10 `identify_runs`** (`:1504`) — 동일 레벨 런 분할, `calculate_sor_eor`로 런 경계 방향(sor/eor) 산출.
5. **`identify_isolating_run_sequences`** (`:1591`, BD13) — 레벨 런을 아이솔레이팅 런 시퀀스로 묶고, `calculate_sos_eos`·`build_text_chains`로 sos/eos와 통합 인덱스 체인 구성.
6. **W1~W7** (`:523`~`:780`) — 결합 마크, 유럽/아라비아 숫자, 분리자/종결자 약한 타입 해석.
7. **N0** `resolve_paired_brackets` (`:801`, BD16) — `BracketStack`으로 괄호쌍을 찾아 방향 해석.
8. **N1/N2** (`:1036`, `:1193`) — 중립 문자 문맥/레벨 기반 해석.
9. **I1/I2** `resolve_implicit_levels` (`:1204`) — 묵시적 레벨 증가.

### 데이터 흐름

`resolve`는 원본을 `orig_char_types`에 보존하고 `char_types`를 작업 사본으로 변형한다(`:454-455`). W/N 규칙은 `char_types`를 in-place 수정하고, X 규칙은 `levels`를 채운다. 표시 단계(`reorder_line` → `reverse_levels`, L2/L3)는 `resolve` 결과를 비파괴적으로 읽어 시각 순서를 생성한다.

L1(공백 레벨 리셋, `reset_whitespace_levels` `:1261`)은 줄 단위로 `levels`의 복제본에만 적용되므로, `line_runs`/`reordered_runs`를 줄마다 독립적으로 호출할 수 있다.

---

## 5. 핵심 데이터 구조·타입

### `Level(pub i8)` — `bidi/src/level.rs:10`

임베딩 레벨 뉴타입. **불변식**: 유효 레벨은 `0..=MAX_DEPTH`(125, `level.rs:7`)이며, `NO_LEVEL(-1)`은 X9 제거 표식이다. `removed_by_x9()`(`:25`)는 `self.0 == NO_LEVEL` 검사. 짝수=LTR, 홀수=RTL(`direction()` `:13`, `as_bidi_class()` `:17`). `least_greater_even`/`least_greater_odd`(`:33,46`)는 `MAX_DEPTH` 초과 시 `None` 반환—이는 X2~X5의 오버플로 판정 근거다.

### `BidiClass` — `bidi/src/bidi_class.rs:5` [생성 enum]

23개 변형의 양방향 문자 분류. `is_iso_init`(`lib.rs:1712`), `is_iso_control`(`:1721`), `is_neutral`(`:1731`) 분류 메서드는 `lib.rs`에서 구현. `#[repr(u8)]`.

### `Direction` — `bidi/src/direction.rs:4`

`LeftToRight`/`RightToLeft` 이진 enum. `with_level`(레벨 패리티 → 방향), `opposite`, `iter`(방향에 따라 정/역순 순회하는 `DirectionIter`) 제공.

### `BidiContext` — `bidi/src/lib.rs:56`

병렬 벡터 묶음(`orig_char_types`, `char_types`, `levels`, `runs`)과 `base_level`, `reorder_nsm` 플래그. **불변식**: `char_types`·`levels`·`orig_char_types`는 입력 문단과 동일 길이의 인덱스 정렬을 유지해야 한다. `levels`는 X1~X8 이후 항상 채워져 있어야 한다.

### `Run` — `bidi/src/lib.rs:1743` (private)

`start`/`end`/`len`/`seq_id`/`level`/`sor`/`eor`. **불변식**: `len > 0`(`identify_runs` `:1523` assert), `end == start + len`. `seq_id == 0`은 미할당 표식.

### `IsolatingRunSequence` — `bidi/src/lib.rs:1775` (private)

`runs`(런 인덱스 목록), `level`, `sos`/`eos`, `indices`(런들을 가로지르는 통합 인덱스 체인). **불변식**: 시퀀스당 최소 1개 런(`:1683` expect).

### `LevelStack` — `bidi/src/level_stack.rs:13` (private)

`MAX_DEPTH` 크기 고정 배열 3개(`embedding_level`/`override_status`/`isolate_status`)와 `depth`. **불변식**: `embedding_level()`/`override_status()`/`isolate_status()`는 `depth >= 1`일 때만 유효(`depth - 1` 인덱싱 `:60`). `push`는 `depth >= MAX_DEPTH`에서 무시(`:37`).

### `BracketStack` / `Pair` — `bidi/src/lib.rs:1881,1869` (private)

`MAX_PAIRING_DEPTH(63, :1880)` 고정 배열 기반 괄호 매칭 스택. **불변식**: 스택 깊이는 63을 넘지 못하며 초과 시 `push`가 `false`를 반환하여 N0 처리를 중단(UBA80 명세, `:818`). U+2329/U+232A와 U+3009의 정규 동치를 하드코딩 처리(`:1960-1962`).

---

## 6. 외부 의존성

외부(비워크스페이스) 의존성은 사실상 없다. `Cargo.toml`에 선언된 것은 워크스페이스 내부 `wezterm-dynamic`와 표준 `log`뿐이다(`bidi/Cargo.toml:14-16`). dev 의존성으로 스냅샷 테스트용 `k9`와 `env_logger`가 있다(`:18-20`).

- `log` = 알고리즘 단계별 `trace!` 추적. 릴리스에서는 비활성.
- `wezterm-dynamic` = config 직렬화/역직렬화를 위한 `FromDynamic`/`ToDynamic` 파생만 사용.

이 크레이트는 harfbuzz·wgpu 같은 네이티브 라이브러리에 의존하지 않는다. 순수 Rust 계산이다.

---

## 7. 설정·기능 플래그

`Cargo.toml`의 `[features]` 섹션은 비어 있다(`bidi/Cargo.toml:12`). feature flag 없음.

관련 config 항목은 `config` 크레이트에 있다.

- `bidi_enabled: bool`(`config/src/config.rs:790`) — 기본 false. true일 때만 `wezterm-surface`가 bidi 재정렬을 수행한다(`wezterm-surface/src/cellcluster.rs:155`).
- `bidi_direction: ParagraphDirectionHint`(`config/src/config.rs:793`) — 문단 방향 힌트. 기본 `LeftToRight`(`bidi/src/lib.rs:31`의 `#[default]`).

런타임 옵션 `set_reorder_non_spacing_marks`(L3)는 config 항목이 아니라 호출 측에서 직접 설정한다.

---

## 8. Windows 전용 고려사항

이 크레이트에는 **플랫폼 분기가 전혀 없다.** `cfg(unix)`/`cfg(windows)`/`cfg(target_os=...)` 등의 조건부 컴파일이 소스 전체에 존재하지 않으며, OS API 호출도 없다. `#![no_std]` 순수 계산 라이브러리이므로 Windows fork의 비Windows 코드 제거 작업의 영향권 밖이다.

따라서 이 폴더에는 fork로 인한 죽은 경로가 없다. `cfg(unix)` 잔재를 점검할 필요가 없는, fork 내에서 가장 플랫폼 중립적인 크레이트 중 하나다.

주의할 죽은/제한 경로는 플랫폼이 아니라 알고리즘 의미에서 존재한다. `set_char_types`(`:446`)와 `resolve_paired_brackets`의 빈 문단 분기(`:802`)는 UCD `BidiTest`/`BidiCharacterTest` 호환을 위한 경로다.

---

## 9. 리팩토링 주의점

1. **`lib.rs` 비대화** — 2089행 단일 파일에 컨텍스트·UBA 전 규칙·런 구조·괄호 스택·문단 레벨 계산이 모두 들어 있다. 규칙군(X/W/N/I/L)별 모듈 분리가 가장 큰 리팩토링 후보다. 단, 규칙들은 `BidiContext`의 동일 병렬 벡터를 공유하므로 분리 시 가시성·차용(borrow) 설계가 필요하다.

2. **병렬 벡터 결합** — `orig_char_types`/`char_types`/`levels`가 인덱스로 암묵 결합되어 있다. 길이 정합과 인덱스 정렬이 불변식이며, 한 벡터만 잘못 변형하면 전 규칙이 깨진다. 구조체-of-배열을 배열-of-구조체로 바꾸는 변경은 파급이 크다.

3. **`NO_LEVEL`(-1) 센티넬** — X9 "삭제"가 실제 제거가 아니라 `Level(-1)` 표식으로 구현되어 있어, 거의 모든 규칙이 `removed_by_x9()`로 건너뛰는 분기를 갖는다. `reordered_runs`(`:233`)는 `removed_by_x9` 항목을 UCD 테스트 호환을 위해 명시적으로 `retain`으로 제거한다(`:245`). 이 이중 표현(레벨 결과는 포함, reordered는 제외)이 미묘한 차이이므로 변경 시 주의.

4. **UBA80 미세 의미 의존** — N0의 괄호 처리(`set_bracket_pair_bc` `:874`)는 UBA80에서 추가된 결합 마크 fix-up과 스택 오버플로(깊이 63) 중단 의미에 의존한다. L3을 L2 전에 적용하는 비표준 순서(FriBidi 호환, `:371-375`)도 의도된 동작이며 임의 변경 금지.

5. **재정렬 비파괴성** — `reorder_line`/`line_runs`는 `levels` 복제본에만 L1을 적용하므로, 한 번 `resolve`한 컨텍스트로 여러 줄을 반복 질의할 수 있다. 이 비파괴 계약을 깨면 줄바꿈 처리 호출 측(`wezterm-surface`)이 오작동한다.

6. **순환 의존 없음** — leaf 크레이트이므로 내부 순환 위험은 낮다. 단, 공개 타입(`BidiClass`, `Direction`, `ParagraphDirectionHint`)이 config·surface·font·gui 다수 크레이트에 노출되어 있어 시그니처 변경의 파급 표면이 넓다.

7. **생성 테이블** — `bidi_class.rs`/`bidi_brackets.rs`는 손으로 편집하지 말 것. 두 파일 헤더가 `bidi/generate/src/main.rs`가 `bidi/data/*.txt`(DerivedBidiClass.txt, BidiBrackets.txt)로부터 생성함을 명시한다(`bidi_class.rs:1`, `bidi_brackets.rs:1`). 참고로 `generate/`·`data/`·`tests/`는 이 폴더의 `Cargo.toml` `exclude`로 패키지에서 제외되어 있다(`bidi/Cargo.toml:8`).

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `src/lib.rs` | 2089 | 크레이트 루트. `BidiContext`와 UBA 전 규칙(X/W/N/I/L), `BidiRun`/`ReorderedRun`, `Run`/`IsolatingRunSequence`, `BracketStack`/`Pair`, `paragraph_level`, `bidi_class_for_char`, `lookup_closing`, 단위/스냅샷 테스트. |
| `src/bidi_class.rs` | 2348 | [생성] `BidiClass` enum(23 변형)과 코드포인트 구간→Bidi_Class 매핑 테이블 `BIDI_CLASS`. `DerivedBidiClass.txt`로부터 생성. `bidi_class_for_char`의 이분 탐색 소비처. |
| `src/bidi_brackets.rs` | 137 | [생성] `BracketType`(Open/Close) enum과 괄호쌍 매핑 테이블 `BIDI_BRACKETS`. `BidiBrackets.txt`로부터 생성. N0(`lookup_closing`)의 소비처. |
| `src/level_stack.rs` | 78 | X1~X8용 `LevelStack`과 `Override` enum. 고정 크기(`MAX_DEPTH`) 배열 스택. |
| `src/direction.rs` | 64 | `Direction` enum과 방향 인지 이터레이터 `DirectionIter`. |
| `src/level.rs` | 58 | `Level` 뉴타입, `MAX_DEPTH(125)`, 레벨 산술(`least_greater_even/odd`, `as_bidi_class`, `removed_by_x9`). |
| `Cargo.toml` | 21 | 패키지 메타데이터. deps: `log`, `wezterm-dynamic`. dev: `k9`, `env_logger`. feature 없음. `generate/data/tests` 제외. |
