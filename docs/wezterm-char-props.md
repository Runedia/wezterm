# wezterm-char-props 폴더 기능 명세

본 문서는 WezTerm Windows 전용 영구 분기에 속한 단일 크레이트 `wezterm-char-props`의 상세 기능 명세이다. 추후 리팩토링을 위해 "무엇을·왜·어떻게"를 코드 근거와 함께 기술한다. 코드 인용은 `파일경로:라인` 형식을 따른다.

---

## 1. 개요 및 책임

`wezterm-char-props`는 **유니코드 문자 속성 조회를 담당하는 순수 데이터 크레이트**이다. 단일 책임은 다음 세 가지 질의에 상수 시간 또는 로그 시간으로 답하는 것이다.

- 어떤 코드포인트(또는 그래핌)가 화면에서 차지하는 **셀 폭**은 몇 칸인가(단일폭·이중폭·결합문자 등).
- 어떤 문자 또는 그래핌의 기본 **표현(presentation)**이 텍스트인가 이모지인가, 그리고 변형 선택자(variation selector)에 의해 어떻게 바뀌는가.
- Nerd Fonts 심볼 이름을 대응하는 **문자 코드포인트**로 매핑한다.

이 크레이트는 비즈니스 로직을 거의 갖지 않는다. 핸드라이트 코드는 폭 분류 로직(`widechar_width.rs`)과 표현 판정 로직(`emoji.rs`), 그리고 Nerd Fonts 룩업 맵 빌더(`nerdfonts.rs`)뿐이며, 나머지는 전부 `ucd-generate` 및 자체 codegen으로 생성된 유니코드/이모지/너드폰트 테이블이다.

Windows fork에서의 실제 책임에는 플랫폼 분기가 없다. 이 크레이트는 OS 독립적이며, `#![cfg_attr(not(feature = "std"), no_std)]`(`wezterm-char-props/src/lib.rs:1`)로 `no_std` 환경까지 지원한다. 따라서 Windows fork에서도 변경 없이 그대로 동작하는, 계층 최하단의 안정적 데이터 공급원이다.

---

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 | 사용 항목 |
|------|----------|-----------|
| 의존(deps) | `termwiz` | (dev-dependency) 벤치마크 `wcwidth`에서만 사용. 본 크레이트의 런타임 의존이 아님 |
| 피의존(usedBy) | `termwiz` | `nerdfonts` 모듈 재노출(`termwiz/src/lib.rs:56`) |
| 피의존(usedBy) | `wezterm-cell` | `emoji::Presentation`, `emoji_variation::WCWIDTH_TABLE`, `widechar_width::WcWidth`, `white_space::WHITE_SPACE` 사용 |
| 피의존(usedBy) | `wezterm-surface` | `emoji::Presentation` 사용(`wezterm-surface/src/line/cellref.rs:3`, `wezterm-surface/src/cellcluster.rs:5`) |

참고: 과업 명세는 `termwiz`를 deps로 분류하나, `Cargo.toml`상 `termwiz`는 `[dev-dependencies]`이며(`wezterm-char-props/Cargo.toml:26`) 벤치마크 전용이다. 실제 런타임 의존성은 `phf`·`serde`(선택)·`ucd-trie` 셋뿐이다.

이 크레이트는 **셀/표면 처리 파이프라인의 최하단 데이터 계층**이다. `wezterm-cell`이 셀 폭 계산과 그래핌 표현 판정을 위해 직접 소비하고, `termwiz`는 Nerd Fonts 룩업을 그 상위로 재노출하며, 상위 GUI·셰이핑 계층은 이를 간접적으로 사용한다.

---

## 3. 공개 API 표면

크레이트 루트에서 노출되는 모듈은 `emoji`, `emoji_presentation`, `emoji_variation`, `nerdfonts`, `white_space`, `widechar_width`이며 `nerdfonts_data`만 비공개 모듈이다(`wezterm-char-props/src/lib.rs:2-8`).

### 3.1 `widechar_width` — 셀 폭 분류

- `pub enum WcWidth`(`widechar_width.rs:25`): 폭 분류 결과. variant는 `One`, `Two`, `NonPrint`, `Combining`, `Ambiguous`, `PrivateUse`, `Unassigned`, `WidenedIn9`, `NonCharacter`. `#[repr(u8)]`.
- `WcWidth::from_char(c: char) -> WcWidth`(`widechar_width.rs:1528`): 정적 범위 테이블에 대한 이진 탐색으로 분류. 전 코드포인트 범위를 다룬다.
- `WcWidth::width_unicode_8_or_earlier(self) -> u8`(`widechar_width.rs:1565`) / `width_unicode_9_or_later(self) -> u8`(`widechar_width.rs:1577`): 분류 결과를 실제 폭(0/1/2)으로 변환. 둘의 차이는 `WidenedIn9`이 1칸인지 2칸인지뿐이다.
- `pub struct WcLookupTable { pub table: [WcWidth; 65536] }`(`widechar_width.rs:1592`): BMP(0x0000–0xFFFF) 전 영역을 미리 채운 64KB 룩업 테이블.
- `WcLookupTable::new() -> Self`(`widechar_width.rs:1598`): 테이블을 동적으로 구성. `Default`도 구현(`widechar_width.rs:1669`).
- `WcLookupTable::classify(&self, c: char) -> WcWidth`(`widechar_width.rs:1660`): BMP 내는 O(1) 인덱싱, 그 외는 `from_char`로 폴백. 주석 기준 BMP 내 약 1.5ns 상수 시간, 외부는 20–75ns(`widechar_width.rs:1585-1591`).

### 3.2 `emoji` — 표현(presentation) 판정

- `pub enum Presentation { Text, Emoji }`(`emoji.rs:7`): `use_serde` feature 시 `Serialize`/`Deserialize` 파생.
- `Presentation::for_grapheme(s: &str) -> (Self, Option<Self>)`(`emoji.rs:16`): 그래핌의 (기본 표현, 변형 선택자에 의한 명시적 표현) 쌍을 반환. 먼저 `VARIATION_MAP`을 조회하고, 없으면 구성 문자를 순회해 이모지 문자가 하나라도 있으면 `Emoji`로 판정.
- `Presentation::for_char(c: char) -> Self`(`emoji.rs:35`): `EMOJI_PRESENTATION` trie에 포함되면 `Emoji`, 아니면 `Text`.

### 3.3 `nerdfonts` — Nerd Fonts 심볼 룩업

- `pub static NERD_FONTS: LazyLock<HashMap<&'static str, char>>`(`nerdfonts.rs:7`): 심볼 이름→코드포인트 해시맵. `std` feature 한정. 생성 데이터 슬라이스로부터 lazy 구성(`nerdfonts.rs:12-16`).
- `pub use crate::nerdfonts_data::NERD_FONT_GLYPHS`(`nerdfonts.rs:9`): `&[(&str, char)]` 형태 원본 슬라이스 재노출. `no_std`에서도 접근 가능.

### 3.4 생성 데이터 모듈의 공개 항목

- `emoji_presentation::EMOJI_PRESENTATION: &ucd_trie::TrieSet`(`emoji_presentation.rs:13`), `BY_NAME`(`emoji_presentation.rs:10`).
- `white_space::WHITE_SPACE: &ucd_trie::TrieSet`(`white_space.rs:13`), `BY_NAME`(`white_space.rs:10`).
- `emoji_variation::VARIATION_MAP: phf::Map<&'static str, (Presentation, Presentation)>`(`emoji_variation.rs:6`): 변형 선택자가 붙은 그래핌→(기본, 명시) 표현 쌍.
- `emoji_variation::WCWIDTH_TABLE: WcLookupTable`(`emoji_variation.rs:866`): 컴파일 타임에 전부 베이크된 `const` 룩업 테이블. `wezterm-cell`이 폭 계산의 1차 경로로 사용(`wezterm-cell/src/lib.rs:865`).

---

## 4. 내부 구조

모듈 분해는 다음과 같다.

- `lib.rs`(8행): 모듈 선언과 `no_std` 스위치만 담는다(`lib.rs:1-8`).
- `widechar_width.rs`(1688행): 핸드라이트 분류 로직 + 9개 정적 범위 테이블(`ASCII_TABLE`, `PRIVATE_TABLE`, `NONPRINT_TABLE`, `COMBINING_TABLE`, `COMBININGLETTERS_TABLE`, `DOUBLEWIDE_TABLE`, `AMBIGUOUS_TABLE`, `UNASSIGNED_TABLE`, `NONCHAR_TABLE`, `WIDENED_TABLE`). 파일 본문 대부분은 `(u32, u32)` 범위 쌍 데이터이며 widecharwidth 프로젝트에서 생성된 것이다(헤더 `widechar_width.rs:1-19`).
- `emoji.rs`(42행): 표현 판정 로직. `emoji_variation`과 `emoji_presentation`에 의존.
- `nerdfonts.rs`(16행): 룩업 맵 빌더. `nerdfonts_data`에 의존.
- 생성 데이터 모듈(아래 표 참조): `emoji_variation.rs`, `nerdfonts_data.rs`, `emoji_presentation.rs`, `white_space.rs`.

제어/데이터 흐름은 두 갈래다.

1. **폭 분류 흐름**: `WcWidth::from_char`는 ASCII→PrivateUse→NonPrint→NonChar→Combining→CombiningLetters→DoubleWide→Ambiguous→Unassigned→WidenedIn9 순서로 각 테이블을 이진 탐색(`in_table`, `widechar_width.rs:1515`)하며, 어디에도 없으면 `One`으로 귀결(`widechar_width.rs:1530-1560`). `WcLookupTable::new`는 이 우선순위를 **역순으로** 채워 동일 우선순위를 보장한다(`widechar_width.rs:1600-1648`). 상위 호출자(`wezterm-cell`)는 일반적으로 `from_char`를 매번 호출하지 않고, 베이크된 `WCWIDTH_TABLE.classify`로 O(1) 조회한다.

2. **표현 판정 흐름**: `Presentation::for_grapheme`가 `VARIATION_MAP`(phf 완전 해시) 우선 조회 후, 미스 시 문자별 `EMOJI_PRESENTATION` trie 조회로 폴백한다(`emoji.rs:16-33`).

비대 모듈: `emoji_variation.rs`(약 66,405행)와 `nerdfonts_data.rs`(약 10,756행)는 전부 생성된 데이터다. `widechar_width.rs`도 1,688행 중 다수가 범위 테이블이다.

---

## 5. 핵심 데이터 구조·타입

- **`WcWidth` (enum, `widechar_width.rs:25`)**: 9개 분류 카테고리. `#[repr(u8)]`이므로 `WcLookupTable`의 `[WcWidth; 65536]` 배열이 정확히 64KB를 차지한다. 불변식: 각 코드포인트는 정확히 하나의 분류를 가지며, 룩업 테이블 구성 순서가 `from_char`의 탐색 순서와 역순이어야 동일 우선순위가 성립한다(`widechar_width.rs:1600-1603`의 주석이 명시).

- **`WcLookupTable` (struct, `widechar_width.rs:1592`)**: `pub table: [WcWidth; 65536]`. 불변식: 인덱스 `i`는 BMP 코드포인트 `i`의 분류여야 한다. `classify`는 `c <= 0xffff`일 때만 인덱싱하고 그 외는 `from_char` 폴백(`widechar_width.rs:1660-1666`). `WCWIDTH_TABLE`은 이 구조체를 `const`로 컴파일 타임에 베이크한 인스턴스다.

- **`Presentation` (enum, `emoji.rs:7`)**: `{ Text, Emoji }`. `for_grapheme` 반환 튜플의 두 번째 요소 `Option<Self>`는 변형 선택자가 표현을 명시적으로 지정했을 때만 `Some`이다.

- **`VARIATION_MAP` (phf::Map, `emoji_variation.rs:6`)**: 키는 변형 선택자를 포함한 그래핌 문자열, 값은 `(Presentation, Presentation)` = (기본 표현, 명시 표현). phf 완전 해시이므로 런타임 충돌 없음.

- **`TrieSet` (ucd-trie, `emoji_presentation.rs:13` / `white_space.rs:13`)**: `contains_u32`로 집합 멤버십을 O(1)에 가깝게 질의. `EMOJI_PRESENTATION`과 `WHITE_SPACE`가 이 형태다.

- **`NERD_FONT_GLYPHS` (`&[(&str, char)]`, `nerdfonts_data.rs:4`)**: 정렬된(코드상 알파벳순) 이름→코드포인트 슬라이스. `NERD_FONTS` 해시맵의 원본.

---

## 6. 외부 의존성

| 크레이트 | 용도 | 근거 |
|----------|------|------|
| `phf` | 컴파일 타임 완전 해시 맵. `VARIATION_MAP`(변형 선택자 그래핌→표현)에 사용. 충돌 없는 정적 룩업 | `Cargo.toml:14`, `emoji_variation.rs:6` |
| `ucd-trie` | 유니코드 코드포인트 집합을 압축한 trie. `EMOJI_PRESENTATION`·`WHITE_SPACE` 멤버십 질의 | `Cargo.toml:16`, `emoji_presentation.rs:13` |
| `serde` (선택) | `Presentation`의 직렬화/역직렬화. `use_serde` feature로 활성 | `Cargo.toml:15`, `emoji.rs:2-6` |
| `termwiz` (dev) | 벤치마크에서 `grapheme_column_width`·`UnicodeVersion` 사용. 런타임 비의존 | `Cargo.toml:26`, `benches/wcwidth.rs:2` |
| `criterion` (dev) | `wcwidth` 벤치마크 하네스 | `Cargo.toml:24,28-30` |
| `k9` (dev) | 테스트 어서션 | `Cargo.toml:25` |

이 크레이트에는 harfbuzz·wgpu 같은 무거운 외부 의존성이 없다. 의도적으로 가벼운 데이터 계층으로 유지된다.

---

## 7. 설정·기능 플래그

`Cargo.toml`(`wezterm-char-props/Cargo.toml:18-21`)에 정의된 feature:

- `default = ["std"]`: 기본 활성.
- `std = ["serde/std", "ucd-trie/std", "phf/std"]`: 표준 라이브러리 사용. 이 feature가 꺼지면 `lib.rs:1`의 `no_std`가 적용되고, `nerdfonts::NERD_FONTS`(`LazyLock<HashMap>`)와 `build_map`이 `#[cfg(feature = "std")]`로 컴파일에서 제외된다(`nerdfonts.rs:1-16`).
- `use_serde = ["serde"]`: `Presentation`에 serde 파생을 부여(`emoji.rs:5-6`).

워크스페이스 루트는 이 크레이트를 `default-features=false`로 참조한다(`Cargo.toml:201`). 따라서 상위 크레이트가 `std`를 명시적으로 켜지 않는 한 `no_std` 경로로 들어간다. `wezterm-cell`은 자신의 `std` feature에서 `wezterm-char-props/std`를 전파한다(`wezterm-cell/Cargo.toml:27`).

관련 config 항목: 이 크레이트 자체에는 `config` 크레이트 연동 설정이 없다. 유니코드 버전·모호 폭 처리 같은 정책은 상위 `termwiz`/`wezterm-cell`의 `UnicodeVersion`이 `width_unicode_8_or_earlier`/`width_unicode_9_or_later`를 선택 호출하는 방식으로 결정된다(이 크레이트는 두 함수를 제공만 한다).

---

## 8. Windows 전용 고려사항

이 크레이트에는 **플랫폼 분기(`cfg(unix)`/`cfg(windows)` 등)가 전혀 없다.** 유니코드 문자 속성은 OS 독립적이므로 Windows fork에서 제거·수정할 죽은 플랫폼 경로가 없다. Windows API 호출 지점도 없다.

유일한 조건부 컴파일은 `feature = "std"` 기반이며(플랫폼이 아닌 환경 스위치), 이는 `no_std` 지원을 위한 것이지 Windows와 무관하다(`lib.rs:1`, `nerdfonts.rs:1-11`).

결론적으로 이 크레이트는 Windows fork에서 가장 손댈 일이 적은 안정 계층이다. Windows 전용 변경의 영향은 이 크레이트가 아니라 이를 소비하는 `wezterm-cell`·`termwiz` 측에 국한된다.

---

## 9. 리팩토링 주의점

- **생성물 직접 편집 금지**: `emoji_variation.rs`(66K행)·`nerdfonts_data.rs`(10K행)·`emoji_presentation.rs`·`white_space.rs`·`widechar_width.rs`의 테이블은 전부 생성물이다. 각 파일 상단 헤더가 생성 커맨드를 명시한다(`ucd-generate ...` / `cd ../codegen ; cargo run`). 유니코드 버전 업데이트는 codegen 재실행으로 처리해야 하며, 수기 편집은 다음 생성 시 소실된다.

- **우선순위 불변식**: `WcWidth::from_char`의 테이블 탐색 순서(`widechar_width.rs:1530-1560`)와 `WcLookupTable::new`의 채움 순서(역순, `widechar_width.rs:1604-1648`)는 짝을 이룬다. 한쪽만 변경하면 BMP 내/외 코드포인트의 분류가 어긋난다. 동기화 유지가 필수.

- **이중 폭 계산 경로**: 폭 계산이 두 경로로 존재한다. (1) `widechar_width.rs`의 `WcLookupTable::new`로 런타임 구성, (2) `emoji_variation.rs`의 `WCWIDTH_TABLE`로 컴파일 타임 베이크. 두 테이블은 동일 데이터에서 나와야 일관성이 유지되나, 생성 파이프라인이 다르므로(전자는 핸드라이트 로직, 후자는 codegen 산출 const) 유니코드 버전 갱신 시 둘 다 재생성·검증해야 파급 위험을 막는다.

- **표현 판정 결합**: `emoji::Presentation`은 `wezterm-cell`·`wezterm-surface`가 직접 의존한다(`wezterm-cell/src/lib.rs:12`, `wezterm-surface/src/cellcluster.rs:5`). `Presentation` enum이나 `for_grapheme` 시그니처 변경은 상위 셀/표면 계층에 직접 파급된다. `use_serde` feature로 직렬화되므로 wire/저장 포맷 호환성도 고려해야 한다.

- **`no_std` 제약**: `std` feature가 꺼진 빌드에서는 `nerdfonts::NERD_FONTS`(HashMap)를 쓸 수 없고 `NERD_FONT_GLYPHS` 슬라이스만 사용 가능하다. 새 API 추가 시 `std` 게이팅을 일관되게 적용해야 한다.

- **순환 의존 없음**: deps는 `phf`/`ucd-trie`/`serde`뿐으로 워크스페이스 내부 크레이트에 의존하지 않는다. 계층 최하단이므로 순환 의존 위험이 없다. 단, dev-dependency `termwiz`는 역방향(termwiz가 본 크레이트에 의존)이므로 벤치마크 빌드 시에만 사실상의 dev 순환이 형성된다 — 런타임에는 무해.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|------|-----------|------|
| `src/lib.rs` | 8 | 크레이트 루트. 모듈 선언, `no_std` 스위치 |
| `src/widechar_width.rs` | 1,688 | 핸드라이트 폭 분류 로직(`WcWidth`, `WcLookupTable`, `from_char`, `classify`) + 9개 범위 테이블. 테이블 본문은 widecharwidth 생성물 |
| `src/emoji.rs` | 42 | 핸드라이트 표현 판정(`Presentation`, `for_grapheme`, `for_char`) |
| `src/nerdfonts.rs` | 16 | 핸드라이트 Nerd Fonts 룩업 맵 빌더(`NERD_FONTS` LazyLock, `NERD_FONT_GLYPHS` 재노출) |
| `src/emoji_variation.rs` | 66,405 | [생성] 변형 선택자 그래핌→표현 phf 맵(`VARIATION_MAP`)과 컴파일 타임 베이크 폭 테이블(`WCWIDTH_TABLE`). codegen 산출. 소비처: `emoji.rs`, `wezterm-cell` |
| `src/nerdfonts_data.rs` | 10,756 | [생성] Nerd Fonts 심볼 이름→코드포인트 슬라이스(`NERD_FONT_GLYPHS`). codegen 산출. 소비처: `nerdfonts.rs` |
| `src/emoji_presentation.rs` | 116 | [생성] `Emoji_Presentation` 속성 trie(`EMOJI_PRESENTATION`). ucd-generate 산출(Unicode 16.0.0). 소비처: `emoji.rs` |
| `src/white_space.rs` | 88 | [생성] `White_Space` 속성 trie(`WHITE_SPACE`). ucd-generate 산출(Unicode 16.0.0). 소비처: `wezterm-cell` |
| `benches/wcwidth.rs` | 101 | [dev] criterion 벤치마크. `WcWidth::from_char` vs `WcLookupTable::classify` vs termwiz `grapheme_column_width` 성능 비교 |

생성 파이프라인: `ucd-generate property-bool ...`(emoji_presentation·white_space), widecharwidth `generate.py`(widechar_width 테이블), 자체 codegen `cd ../codegen ; cargo run`(emoji_variation·nerdfonts_data). 모두 Unicode 16.0.0 기준.
