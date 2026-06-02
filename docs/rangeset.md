# rangeset 폴더 기능 명세

## 1. 개요 및 책임

`rangeset` 크레이트는 정수 집합을 **연속 구간(`Range<T>`)의 정렬된 배열**로 압축 저장하는 단일 자료구조 `RangeSet<T>`를 제공한다. 인접하거나 겹치는 정수를 자동으로 하나의 구간으로 병합(collapse)함으로써, 희소하지 않고 대체로 연속적인 정수 집합을 메모리·연산 양면에서 효율적으로 다룬다.

이 크레이트의 단일 책임은 다음과 같다: **정수 구간 집합의 표현과 집합 연산(추가·제거·합집합·교집합·차집합) 제공**. 그 이상의 책임(직렬화, 동시성, 도메인 의미)을 갖지 않는, 순수 알고리즘 라이브러리이다.

Windows fork에서의 실제 책임은 상위 크레이트들이 **"어떤 행(row)/스크롤백 라인이 변경(dirty)되었는가"**를 추적하는 데 쓰는 기반 자료구조이다. 터미널 렌더링 파이프라인은 매 프레임 전체 화면을 다시 그리지 않고, 변경된 라인 범위만 추적해 서버↔클라이언트 간 전송량과 GPU 렌더 비용을 줄인다. `RangeSet`은 그 "변경된 라인 인덱스 집합"을 대표하는 자료구조로 소비된다. 이 책임은 플랫폼에 무관하므로 Windows 전용 분기와 직접적 관련이 없고, 크레이트 내부에 `cfg` 플랫폼 분기도 존재하지 않는다.

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 |
|------|----------|
| 의존(deps) | 없음 (외부 크레이트 `num`만 사용) |
| 피의존(usedBy) | `codec`, `mux`, `wezterm-client`, `wezterm-font`, `wezterm-gui`, `wezterm-mux-server-impl` |

이 크레이트는 워크스페이스 의존 그래프의 **최하단 잎(leaf) 계층**에 위치한다. 내부 의존이 전혀 없어 가장 먼저 컴파일되며, mux(터미널 다중화)·codec(클라이언트-서버 프로토콜)·렌더링(`wezterm-client`, `wezterm-gui`)·폰트(`wezterm-font`) 계층이 모두 이를 공통 기반 유틸리티로 끌어 쓴다. 즉 변경 추적 파이프라인의 데이터 원시 자료형을 제공하는 위치이다.

## 3. 공개 API 표면

전체 API는 단일 모듈(`src/lib.rs`)에 평탄하게 노출된다. 모든 항목은 `Integer + Copy`(일부는 추가로 `Debug + ToPrimitive`) 경계를 갖는 제네릭 정수 타입 `T`에 대해 동작한다.

### 핵심 타입

- `pub struct RangeSet<T: Integer + Copy>` — 본 크레이트의 유일한 자료구조. `Debug, Default, Clone, PartialEq, Eq` 파생.

### 자유 함수 (구간 단위 원시 연산)

- `pub fn range_is_empty<T: Integer>(range: &Range<T>) -> bool` — `start == end` 여부.
- `pub fn intersects_range<T>(r1, r2) -> bool` — 두 구간 교차 여부.
- `pub fn range_intersection<T>(r1, r2) -> Option<Range<T>>` — 교집합 구간(없으면 `None`).
- `pub fn range_subtract<T>(r1, r2) -> (Option<Range<T>>, Option<Range<T>>)` — `r1 - r2`. r2가 r1 내부를 가르면 좌·우 두 조각이 반환될 수 있음.
- `pub fn range_union<T: Integer>(r1, r2) -> Range<T>` — 두 구간의 합(빈 구간은 상대 구간을 그대로 반환).

### `RangeSet<T>` 메서드

생성·조회:
- `new() -> Self`, `is_empty() -> bool`, `len() -> T`(모든 구간 길이의 합), `contains(value: T) -> bool`.

수정:
- `add(value: T)`, `add_range(range: Range<T>)` — 추가 시 인접/중첩 구간을 자동 병합.
- `add_range_unchecked(range: Range<T>)` — 병합·정렬 없이 즉시 push, `needs_sort` 플래그만 세움(고속 일괄 적재용).
- `add_set(&Self)` — 다른 집합 전체 추가.
- `remove(value: T)`, `remove_range(range: Range<T>)`, `remove_set(&Self)`.
- `sort_if_needed()` — 지연 정렬을 강제 수행.

집합 대수:
- `difference(&Self) -> Self` — 차집합(self에는 있고 other에는 없는 값). 구현은 `O(n²)`(주석 명시).
- `intersection(&Self) -> Self` — 교집합.
- `intersection_with_range(range: Range<T>) -> Self` — 단일 구간과의 교집합.

순회:
- `iter() -> impl Iterator<Item = &Range<T>>` — 구간 단위 순회.
- `iter_values() -> impl Iterator<Item = T>` — 개별 정수 값 순회(거대 구간 주의 주석 존재).

변환:
- `impl From<RangeSet<T>> for Vec<Range<T>>` — 내부 구간 벡터를 그대로 추출.

## 4. 내부 구조

모듈 분해는 없다. 크레이트 전체가 `src/lib.rs` 단일 파일(약 421행, 그중 `#[cfg(test)]` 테스트 약 86행)로 구성된다. 비대한 모듈은 없다.

데이터 흐름의 핵심은 **불변식 유지를 위한 add/remove 경로**이다.

- `add_range`(`src/lib.rs:217`): 빈 구간·빈 집합을 단축 처리한 뒤 `sort_if_needed`로 정렬을 보장하고, `intersection_helper`로 삽입 위치 주변의 교차 구간을 찾는다.
  - 두 인접 구간(`b == a + 1`)을 잇는 경우: 둘째 구간을 제거하고 합쳐 재귀적으로 `add_range`(`src/lib.rs:230-239`).
  - 한 구간과만 교차: `merge_into_range`로 `range_union` 적용(`src/lib.rs:240`).
  - 교차 없음: `insertion_point`로 정렬 위치를 찾아 삽입(`src/lib.rs:242-246`).
- `remove_range`(`src/lib.rs:175`): 모든 구간에 대해 `range_subtract`를 적용해 제거 대상 인덱스와 추가 대상 조각을 수집한 뒤, 인덱스를 **역순으로** 제거(앞 인덱스 무효화 방지)하고 남은 조각을 `add_range`로 재삽입한다.
- 탐색은 `binary_search_ranges`(`src/lib.rs:299`)가 담당하며, 커스텀 비교자로 "구간 포함" 의미의 이진 탐색을 수행한다.

지연 정렬(lazy sort) 기제: `add_range_unchecked`로 정렬되지 않은 데이터를 적재할 수 있으며, 이때 `needs_sort = true`가 세팅된다. `intersection_helper`·`insertion_point`는 `needs_sort`가 켜진 상태로 호출되면 `panic!`하여(`src/lib.rs:268-270`, `314-316`) 불변식 위반을 조기 검출한다. 정렬은 `sort_if_needed`에서 `start` 키 기준으로만 수행된다.

## 5. 핵심 데이터 구조·타입

### `RangeSet<T>` (`src/lib.rs:10`)

```rust
pub struct RangeSet<T: Integer + Copy> {
    ranges: Vec<Range<T>>,
    needs_sort: bool,
}
```

불변식(코드가 의존하는 전제):
1. **정규형(canonical) 불변식** — `needs_sort == false`일 때, `ranges`는 `start` 오름차순으로 정렬되어 있고, 어떤 두 구간도 서로 겹치거나 인접(`r[i].end == r[i+1].start`)하지 않는다. `add_range`/`remove_range` 경로가 병합으로 이를 보장한다.
2. **비어 있지 않은 구간** — `add_range`는 `range_is_empty`인 구간을 적재하지 않는다. 따라서 정상 경로로 만들어진 집합에는 빈 구간이 없다.
3. **정렬 지연 불변식** — `needs_sort == true`인 동안에는 1번 불변식이 깨져 있을 수 있으며, 이 상태에서 탐색 의존 메서드를 호출하면 `panic`한다. `add_range_unchecked`만 이 상태를 의도적으로 만든다.

위 불변식 중 정규형이 핵심이다. `binary_search_ranges`의 커스텀 비교자(`src/lib.rs:300-310`)는 구간이 정렬·비중첩이라는 전제 하에서만 정합적이며, 비교 분기의 `else => unreachable!()`(`src/lib.rs:308`)는 그 전제가 깨지면 패닉으로 드러난다.

## 6. 외부 의존성

- `num` (workspace 버전 고정, `Cargo.toml:9`) — 제네릭 정수 추상화. `Integer`, `ToPrimitive` 트레이트로 임의 정수 타입에 대한 일반화를 제공하고, `num::zero()`/`num::one()`로 타입 중립적 상수를, `num::range()`로 `iter_values`의 값 순회 이터레이터를 얻는다. `RangeSet`이 특정 정수 폭(`usize`, `u64` 등)에 묶이지 않도록 하는 유일한 이유이다.
- `criterion` (dev-dependency, `Cargo.toml:12`) — 벤치마크 하니스. `benches/rangeset.rs`에서 연속(contig)·희소(sparse) 적재 성능을 100/10000/1000000 규모로 측정한다.

런타임 외부 의존은 `num` 단 하나뿐이며, harfbuzz·wgpu 등 무거운 네이티브 의존은 전혀 없다.

## 7. 설정·기능 플래그

- Cargo feature flag 없음.
- `config` 크레이트 연동 설정 항목 없음.
- `[package] publish = false`(`Cargo.toml:6`) — 워크스페이스 내부 전용, crates.io 비공개.
- `[[bench]] harness = false`(`Cargo.toml:14-16`) — 기본 libtest 하니스를 끄고 criterion 하니스를 사용.
- edition = 2018(`Cargo.toml:5`).

## 8. Windows 전용 고려사항

이 크레이트에는 플랫폼 분기가 없다. `cfg(windows)`/`cfg(unix)`/`cfg(target_os = ...)` 지시문, Windows API 직접 호출, 죽은 비Windows 경로가 **전혀 존재하지 않는다**. 순수 제네릭 정수 자료구조이므로 Windows fork의 플랫폼 코드 제거 작업의 영향을 받지 않았고, 향후에도 받지 않는다. 이 크레이트는 OS 중립적이며 그 자체로는 Windows 전용 고려사항이 없다.

## 9. 리팩토링 주의점

- **불변식 결합도가 높다.** `binary_search_ranges`·`intersection_helper`·`insertion_point`는 모두 "정렬·비중첩 정규형"을 암묵 전제로 한다. 어느 한 변이(mutation) 메서드라도 병합을 빠뜨리면 이진 탐색의 `unreachable!()`/패닉으로 즉시 깨진다. add/remove 로직 수정 시 정규형 불변식 보존을 최우선으로 검증해야 한다.
- **`needs_sort` 상태 머신.** `add_range_unchecked`만 `needs_sort=true`를 만들고, 탐색 의존 메서드는 이 상태에서 패닉한다. `add_range_unchecked` 호출 후에는 반드시 `sort_if_needed`(또는 이를 내부 호출하는 `add_range`)를 거쳐야 한다. 이 계약을 깨면 런타임 패닉이 발생한다. 또한 `sort_if_needed`는 `start`로만 정렬할 뿐 **중첩 구간 병합을 하지 않는다** — `add_range_unchecked`로 겹치는 구간을 넣은 뒤 정렬만 하면 정규형이 보장되지 않는다.
- **알고리즘 복잡도.** `difference`는 주석에 명시된 대로 `O(n²)`이고, `intersection`은 이중 루프(`other` × `self`)이며, `contains`/`len`은 선형 스캔이다. 코드 주석은 "스크롤백 = 큰 연속 구간 1개 + 뷰포트 변경 = 작은 구간 1개"라는 사용 가정 하에서 충분하다고 본다(`src/lib.rs:127-133`). 구간 수가 커지는 새 호출처를 추가할 경우 이 가정이 깨질 수 있으니 주의해야 한다.
- **`add_range`의 재귀.** 인접 구간 병합 시 자기 자신을 재귀 호출한다(`src/lib.rs:238`). 한 번의 병합으로 종료되지만, 향후 병합 규칙 변경 시 재귀 종료성을 함께 검증해야 한다.
- **`From<RangeSet<T>> for Vec<Range<T>>`가 내부 표현을 그대로 노출**한다(`src/lib.rs:87-91`). 정규형이므로 정렬·비중첩 벡터가 나오지만, 내부 표현 변경(예: `BTreeMap` 전환) 시 이 변환의 의미가 바뀐다.
- **순환 의존 위험 없음.** 내부 의존이 전무한 잎 크레이트이므로, 변경의 파급은 단방향(이 크레이트 → 6개 소비처)이다. 단, 6개 크레이트가 모두 의존하므로 공개 API 시그니처 변경은 광범위한 재컴파일·수정을 유발한다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|------|-----------|------|
| `Cargo.toml` | 17 | 패키지 메타데이터. `num` 의존, `criterion` dev-dep, 커스텀 bench 하니스 선언, `publish=false`. |
| `src/lib.rs` | 421 | 크레이트 전체 구현. `RangeSet<T>` 타입, 5개 구간 자유 함수, add/remove/집합 대수/순회 메서드, 이진 탐색·지연 정렬 로직. 하단 약 86행은 `#[cfg(test)]` 단위 테스트(add/remove/difference 시나리오). |
| `benches/rangeset.rs` | 44 | criterion 벤치마크. 연속·희소 정수 적재를 100/10000/1000000 규모로 측정. |

생성 파일([생성]) 없음. 모든 파일이 수기 작성된 소스이다.
