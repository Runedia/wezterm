# tabout 폴더 기능 명세

## 1. 개요 및 책임

`tabout`은 CLI 프로그램의 출력을 사람이 읽기 좋게 **자동 정렬·정형화(tabulate)** 하는 소형 유틸리티 크레이트다. 각 열의 너비를 코드에서 하드코딩하지 않고, 헤더 텍스트와 모든 행 데이터를 1회 스캔하여 열별 최대 폭을 산출한 뒤 패딩을 채워 정렬 출력한다(`tabout/src/lib.rs:56-113`).

단일 책임은 **"열 헤더 + 행 데이터 → 폭 정렬된 표 출력"** 변환이다. 입력 데이터를 어떻게 만드는지(정렬·필터링·색상 등)에는 관여하지 않으며, 패딩 계산과 출력 스트림으로의 쓰기만 담당한다. 폭 계산은 바이트 수가 아니라 `termwiz::cell::unicode_column_width`를 사용한 **터미널 표시 컬럼 폭**을 기준으로 한다(`tabout/src/lib.rs:31`). 따라서 CJK 전각 문자·이모지 등 다중 폭 문자가 섞여도 시각적 정렬이 유지된다.

Windows fork에서의 실제 책임도 동일하다. 이 크레이트는 플랫폼 의존 코드가 전혀 없는 순수 텍스트 처리 라이브러리이므로, fork의 Windows 전용화와 무관하게 그대로 사용된다. 실제 소비처는 `wezterm` CLI의 목록 출력(`list`, `list-clients`)과 `wezterm-gui`의 통계 덤프(`stats`)다.

## 2. 워크스페이스 내 위치

### 의존성(deps)

| 크레이트 | 용도 |
|---|---|
| `termwiz` | `unicode_column_width`(표시 폭 계산), `CellAttributes`·`Change`(터미널용 정형화 출력의 셀 속성·변경 단위) |

### 피의존(usedBy)

| 크레이트 | 사용 함수 | 사용 위치 |
|---|---|---|
| `wezterm` | `tabulate_output`, `Alignment`, `Column` | `wezterm/src/cli/list.rs:4,95`, `wezterm/src/cli/list_clients.rs:5,110` |
| `wezterm-gui` | `tabulate_output`, `Alignment`, `Column` | `wezterm-gui/src/stats.rs:11,228,252,263` |

이 크레이트는 의존 트리의 **말단(leaf) 표현 계층 유틸리티**다. `termwiz`(터미널 추상화)에만 의존하고, 상위의 명령행 도구(`wezterm`)와 GUI(`wezterm-gui`)가 자신이 수집한 데이터를 사용자에게 표 형태로 보여줄 때 호출한다.

## 3. 공개 API 표면

전체 공개 표면은 `tabout/src/lib.rs` 한 파일에 있다. 모듈 분할은 없다.

### 타입

- `pub enum Alignment { Left, Center, Right }` (`tabout/src/lib.rs:9-14`)
  - 열 정렬 방식. `Copy` 가능.
- `pub struct Column { pub name: String, pub alignment: Alignment }` (`tabout/src/lib.rs:17-23`)
  - 한 열의 헤더 텍스트와 정렬 방식.

### 함수

- `pub fn tabulate_output<S: ToString, W: io::Write>(columns: &[Column], rows: &[Vec<S>], output: &mut W) -> io::Result<()>` (`tabout/src/lib.rs:62-113`)
  - 핵심 함수. 임의의 `ToString` 셀 값을 받아 헤더 1행 + 데이터 N행을 `\n` 종결로 `output`에 기록. **워크스페이스에서 실제로 호출되는 유일한 정형화 함수다.**
- `pub fn tabulate_output_as_string<S: ToString>(columns: &[Column], rows: &[Vec<S>]) -> io::Result<String>` (`tabout/src/lib.rs:221-229`)
  - 위 함수를 `Vec<u8>` 버퍼에 쓰고 UTF-8 문자열로 변환해 반환하는 편의 래퍼. UTF-8 변환 실패 시 `io::Error::other`로 변환.
- `pub fn tabulate_for_terminal(columns: &[Column], rows: &[Vec<Vec<Change>>], spacer: CellAttributes, result: &mut Vec<Change>)` (`tabout/src/lib.rs:165-217`)
  - 평문이 아닌 `termwiz::surface::Change` 시퀀스(스타일·색상이 실린 셀 변경)를 셀 단위로 정형화. 행 종결은 `\r\n`이며, 패딩 공백에는 `spacer` 셀 속성을 적용한다. 반환값이 아니라 `result`에 누적한다.
- `pub fn unicode_column_width_of_change_slice(s: &[Change]) -> usize` (`tabout/src/lib.rs:115-125`)
  - `Change` 슬라이스에서 텍스트 변경만 골라 표시 폭의 합을 계산. `tabulate_for_terminal`의 내부 폭 계산에 쓰이며 공개도 되어 있다.

> 주의: `tabulate_for_terminal`과 `unicode_column_width_of_change_slice`는 `pub`으로 노출되어 있으나, 현재 워크스페이스(`wezterm`/`wezterm-gui`) 내부에는 호출처가 없다(2장 표 참조). 외부 크레이트(docs.rs 공개 API) 호환용으로 남아 있는 상태다.

## 4. 내부 구조

모듈 분해 없이 단일 파일(`lib.rs`, 약 263행)이다. 제어·데이터 흐름은 두 갈래로 대칭이다.

- **평문 경로**: `tabulate_output` → 내부 헬퍼 `emit_column`(`tabout/src/lib.rs:25-54`). `String` 패딩을 직접 `write!`로 출력 스트림에 기록.
- **터미널 경로**: `tabulate_for_terminal` → 내부 헬퍼 `emit_column_for_terminal`(`tabout/src/lib.rs:139-163`) → `emit_padding_for_terminal`(`tabout/src/lib.rs:127-137`). 패딩을 `Change` 시퀀스로 누적하며, 폭 계산은 `unicode_column_width_of_change_slice`로 수행.

공통 알고리즘(두 경로 모두 동일한 2-패스 구조):

1. **폭 산출(1패스)**: 헤더 폭으로 `col_widths`를 초기화(`:67-70`, `:171-174`)한 뒤, 모든 행을 순회하며 각 열의 표시 폭 최대값으로 갱신. 행이 정의된 열 수보다 많으면 `col_widths`에 새 열을 추가(`:77-81`, `:179-183`).
2. **출력(2패스)**: 헤더를 먼저 출력(`:86-93`, `:187-199`), 이어 각 데이터 행을 출력(`:95-110`, `:201-216`). 열 사이에는 공백 1칸 구분자를 둔다(인덱스 0 제외).

비대한 모듈은 없다. `tabulate_output`과 `tabulate_for_terminal`은 거의 동형 코드의 중복 구현이며(평문 vs `Change`), 패딩 비대칭 처리 주석(`:36-39`, `:151-154`)까지 동일하게 복사되어 있다.

## 5. 핵심 데이터 구조·타입

- `Alignment` (`tabout/src/lib.rs:9-14`): 단순 3-variant 열거형. 불변식 없음. `Copy`이므로 행마다 값 복사로 전달된다(`:100`, `:206`).
- `Column` (`tabout/src/lib.rs:17-23`): `name`(헤더 텍스트)과 `alignment` 보유. 불변식은 호출 측 계약 수준에 존재한다 — `columns` 슬라이스 인덱스와 각 행 `Vec`의 인덱스가 같은 열을 가리킨다고 가정한다. 행의 열 수가 `columns`보다 많으면 초과 열은 라벨 없는 **좌측 정렬**로 처리된다(`:97-101`, `:203-207`, `lib.rs:59-61` 문서화).

패딩 계산의 핵심 불변식: `max_width >= text_width`. 정렬별 패딩은 `max_width - text_width`를 기반으로 산출하므로(`:32-43`, `:147-158`), 만약 `text_width > max_width`이면 **`usize` 언더플로로 패닉**한다. 정상 경로에서는 1패스에서 `col_widths`를 모든 행의 최대 폭으로 맞추므로 이 불변식이 성립한다. 위반 가능 경로는 9장에 정리한다.

## 6. 외부 의존성

- `termwiz` (워크스페이스 의존): 두 가지 목적.
  - `unicode_column_width(text, None)` — 유니코드 표시 폭(터미널 컬럼 수) 계산. 바이트/`char` 수가 아닌 실제 화면 폭으로 정렬하기 위한 핵심 함수(`tabout/src/lib.rs:5`, `:31`, `:69`, `:76`, `:173`, `:178`).
  - `surface::Change` / `cell::CellAttributes` — 스타일이 실린 터미널 출력 단위. `tabulate_for_terminal`이 색상·속성을 보존한 채 정형화할 때 사용(`tabout/src/lib.rs:6`).

외부 의존은 `termwiz` 단 하나다. 추가 런타임 의존성은 없다.

## 7. 설정·기능 플래그

- Cargo `[features]` 섹션 없음. feature flag 미정의(`tabout/Cargo.toml`).
- `config` 크레이트와의 연동 없음. 동작에 영향을 주는 외부 설정 항목 없음.
- 동작은 호출 측이 넘기는 인자(`columns`, `rows`, `spacer`)로만 결정된다.

## 8. Windows 전용 고려사항

- 플랫폼 분기(`cfg(...)`) 없음. Windows API 직접 호출 없음. 죽은 플랫폼 경로 없음.
- 순수 텍스트/유니코드 폭 처리 라이브러리이므로 fork의 Windows 전용화와 무관하게 이식성 그대로 유지된다.
- 줄바꿈 차이: 평문 경로(`tabulate_output`)는 `\n`(`writeln!`)을, 터미널 경로(`tabulate_for_terminal`)는 `\r\n`을 사용한다(`tabout/src/lib.rs:199`, `:215`). 이는 OS가 아니라 출력 대상(파일/파이프 vs 터미널 raw 모드)에 따른 의도적 구분이다. Windows 콘솔로의 평문 출력 시에도 `\n`만 기록된다.

## 9. 리팩토링 주의점

- **`usize` 언더플로 패닉 위험**: `emit_column`/`emit_column_for_terminal`의 패딩 산출은 `max_width - text_width`에 의존한다(`tabout/src/lib.rs:33`, `:42`, `:148`, `:157`). 2패스 알고리즘이 `col_widths`를 모든 행 최대 폭으로 맞춘다는 불변식에 의해 보호된다. 다음 변경 시 깨질 수 있다 — (a) `Alignment::Right`/`Center`에서 사용한 `col_widths.get(idx).unwrap_or(col.len())` 폴백(`:97`, `:203`)은 표시 폭이 아니라 `String::len()`(바이트 수)이라, 폭 산출 패스가 다루지 못한 열이 헤더에는 있고 데이터에는 없을 때 등 경계에서 `col.len()`이 실제 표시 폭과 어긋날 수 있다. 폭 계산을 일원화하려면 두 패스의 폭 함수를 통일해야 한다.
- **중복 구현**: 평문 경로와 터미널 경로가 거의 동형이다(2-패스 폭 산출 + 정렬 패딩). `Change` 추상화와 `String` 추상화를 공통 트레이트로 묶으면 중복을 제거할 수 있으나, 출력 누적 방식(`io::Write` vs `Vec<Change>` push)과 줄바꿈(`\n` vs `\r\n`)이 달라 단순 합치기는 어렵다.
- **미사용 공개 API**: `tabulate_for_terminal`·`unicode_column_width_of_change_slice`는 워크스페이스 내 호출처가 없다(2장). fork가 upstream 동기화를 포기한 상태이므로, 외부 호환을 고려하지 않는다면 제거 후보다. 단, `pub` 제거는 SemVer상 호환성 변경이며 `docs.rs` 문서화 대상이라는 점만 유의하면 된다.
- **결합도**: `termwiz`에만 결합. 순환 의존 없음(말단 leaf). `termwiz`의 `unicode_column_width`/`Change`/`CellAttributes` 시그니처 변경에만 영향을 받는다.
- **`io::Error::other` 사용**(`:228`): Rust 1.74+ 안정화 API. 빌드 MSRV를 낮출 경우 이 호출이 컴파일되지 않는다.
- **`edition = "2018"`**: 워크스페이스 다른 크레이트 대비 구판 에디션이다. 에디션 통일 리팩토링 시 영향 범위 점검 대상.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `tabout/Cargo.toml` | 13 | 패키지 메타데이터. 의존성은 `termwiz` 단일. feature 없음. |
| `tabout/src/lib.rs` | 263 | 크레이트 전체 구현. `Alignment`/`Column` 타입, `tabulate_output`/`tabulate_output_as_string`/`tabulate_for_terminal`/`unicode_column_width_of_change_slice` 공개 함수, 내부 헬퍼(`emit_column`, `emit_column_for_terminal`, `emit_padding_for_terminal`), 단위 테스트(`test::basics`). |
| `tabout/LICENSE.md` | - | MIT 라이선스 전문. |

> 생성 파일(`[생성]`) 없음. 거대 데이터 테이블 없음.
