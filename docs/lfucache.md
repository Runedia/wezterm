# lfucache 폴더 기능 명세

## 1. 개요 및 책임

`lfucache`는 LFU(Least-Frequently-Used) 축출 정책을 구현한 단일 책임의 인메모리 캐시 크레이트다. 키-값 항목을 고정 용량 안에서 보관하며, 용량 초과 시 "빈도가 가장 낮은 항목"을 제거한다. 순수 LFU의 고질적 문제(과거에 매우 뜨거웠으나 현재 사용되지 않는 항목이 테이블을 점거하는 현상)를 완화하기 위해 LRU(최근 사용) 정보를 결합한 **빈도 감쇠(decay)** 메커니즘을 추가로 구현한다.

이 크레이트는 GUI 렌더링 파이프라인의 고비용 중간 산물을 캐싱하는 데 쓰인다. `config` 크레이트에만 의존하는 이유는, 캐시 용량을 런타임 설정값(`ConfigHandle`)에서 함수로 끌어와 설정 변경 시 동적으로 재조정하기 위함이다(2번·7번 항목 참조).

Windows fork에서의 실제 책임은 upstream과 동일하다. 본 크레이트에는 플랫폼 분기 코드가 전혀 없으며, 순수 자료구조 로직이므로 fork 분기로 인한 변형이 없다(8번 항목 참조).

본질적 특징:
- 자료구조는 모두 `intrusive-collections` 기반 침입형(intrusive) 컨테이너이며, 단일 `Entry`가 해시 버킷·LRU·LFU 세 색인에 동시에 연결된다.
- 항목 소유권은 `Rc<Entry>`로 공유되며, 침입형 컨테이너들이 같은 노드를 가리킨다.
- `Cell`/`RefCell`을 통한 내부 가변성을 사용해 `&self` 경로(`get` 제외)에서도 빈도·tick을 갱신한다.

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 | 용도 |
| --- | --- | --- |
| 의존(deps) | `config` | `ConfigHandle`에서 용량을 산출하는 `CapFunc` 타입 정의 및 설정 연동 |
| 의존(deps, 외부) | `ahash`, `fnv`, `intrusive-collections`, `metrics` | 6번 항목 참조 |
| 피의존(usedBy) | `wezterm-gui` | 글리프/셰이핑/라인 렌더 캐시 백엔드 |

`lfucache`는 GUI 렌더링 파이프라인의 **저수준 캐시 유틸리티 계층**에 위치한다. `wezterm-gui`가 텍스트 셰이핑 결과·라인 쿼드·디코드된 이미지 등 매 프레임 재계산이 비싼 산물을 이 캐시에 적재해 재사용한다. 즉, 데이터 흐름상 GUI 렌더러의 하위 종속물이며, 자신은 `config` 외 워크스페이스 크레이트에 의존하지 않는 말단 라이브러리다.

## 3. 공개 API 표면

전체 공개 표면은 `lfucache/src/lib.rs` 한 파일에 집중되어 있다.

- `pub type CapFunc = fn(&ConfigHandle) -> usize;` (`lfucache/src/lib.rs:35`)
  - 설정 핸들로부터 캐시 용량을 산출하는 함수 포인터 타입. 캐시 생성 시 주입되며, `update_config` 시 재호출되어 동적 용량 변경을 가능케 한다.

- `pub struct LfuCache<K, V, S = BuildHasherDefault<AHasher>>` (`lfucache/src/lib.rs:40`)
  - 핵심 타입. 기본 해셔는 `ahash` 기반이다. 제약: `K: Hash + Eq + Clone + Debug`, `S: Default + BuildHasher`(`lfucache/src/lib.rs:60`).
  - 공개 메서드:
    - `pub fn new(hit: &'static str, miss: &'static str, cap_func: CapFunc, config: &ConfigHandle) -> Self` (`lfucache/src/lib.rs:89`): hit/miss 메트릭 이름과 용량 함수, 설정을 받아 캐시를 구성. 초기 버킷 수는 `(cap/10).next_power_of_two()`.
    - `pub fn len(&self) -> usize` (`lfucache/src/lib.rs:124`)
    - `pub fn is_empty(&self) -> bool` (`lfucache/src/lib.rs:128`)
    - `pub fn update_config(&mut self, config: &ConfigHandle)` (`lfucache/src/lib.rs:150`): 새 용량을 산출해 줄어든 경우 초과분을 즉시 축출.
    - `pub fn clear(&mut self)` (`lfucache/src/lib.rs:220`): 모든 색인과 길이를 초기화.
    - `pub fn get<'a, Q>(&'a mut self, k: &Q) -> Option<&'a V>` (`lfucache/src/lib.rs:229`): 조회 시 빈도 +1, LRU 갱신, hit/miss 메트릭 기록. `K: Borrow<Q>` 경계를 통해 빌림 키 조회 지원.
    - `pub fn put(&mut self, k: K, v: V)` (`lfucache/src/lib.rs:278`): 기존 동일 키 제거 후 삽입. 용량 초과 시 축출, 적재율이 일정 수준 넘으면 버킷 확장.

- `pub type LfuCacheU64<V> = LfuCache<u64, V, fnv::FnvBuildHasher>;` (`lfucache/src/lib.rs:329`)
  - 정수 키 전용 별칭. `fnv` 해셔를 써 u64 키에 최적화. 문서 주석은 u64 키일 때 이 별칭 사용을 권장한다(`lfucache/src/lib.rs:38`).

비공개(내부 전용) 항목: `Entry`, 세 침입형 어댑터(`RecencyAdapter`/`FrequenceAdapter`/`HashAdapter`), `bucket_for_key`, `grow_hash`, `decay_least_recent`, `evict_one`. `with_capacity`는 `#[cfg(test)]` 한정 생성자다(`lfucache/src/lib.rs:61`).

## 4. 내부 구조

단일 모듈(`lib.rs`)이다. 제어/데이터 흐름은 세 침입형 색인의 협조로 구성된다.

- **해시 색인(`buckets`)**: `Vec<LinkedList<HashAdapter>>`. 키 기반 O(1) 평균 조회용. `bucket_for_key`(`lfucache/src/lib.rs:118`)가 해셔로 버킷 인덱스를 산출한다. 충돌 시 해당 버킷의 연결 리스트를 선형 탐색한다.
- **빈도 색인(`frequency_index`)**: `RBTree<FrequenceAdapter>`. `Entry::freq`를 키로 정렬(`KeyAdapter`, `lfucache/src/lib.rs:28`). 축출은 `lower_bound_mut(Bound::Included(&0))`로 최소 빈도 항목을 찾아 제거(`evict_one`, `lfucache/src/lib.rs:202`).
- **최근성 색인(`recency_index`)**: `LinkedList<RecencyAdapter>`. front=MRU, back=LRU(`lfucache/src/lib.rs:51`). 조회 시 해당 항목을 front로 이동.

흐름 요약:
- `get`(`lfucache/src/lib.rs:229`): 버킷 선형 탐색 → 일치 시 LRU front 이동 → `tick += 1` → `last_tick` 갱신 → RBTree에서 항목 제거 후 `freq.saturating_add(1)` 재삽입 → hit 기록. 불일치 시 miss 기록 후 `None`.
- `put`(`lfucache/src/lib.rs:278`): `tick += 1` → 동일 키 선삭제(있으면) → 용량 초과 동안 `evict_one` → `Rc<Entry>` 생성 후 세 색인 모두에 등록 → 적재 조건 만족 시 `grow_hash`.
- `evict_one`(`lfucache/src/lib.rs:202`): 먼저 `decay_least_recent`로 LRU 항목 빈도를 감쇠 → 최소 빈도 항목을 RBTree에서 제거하고 버킷·LRU 색인에서도 제거.
- `decay_least_recent`(`lfucache/src/lib.rs:165`): LRU(back) 항목의 `freq`가 0이거나 `delta = (tick - last_tick)/10 <= 1`이면 무동작. 그렇지 않으면 `freq /= delta`로 감쇠하고, 해당 항목을 LRU front로 옮겨 다음 감쇠 호출 시 즉시 재방문되지 않게 한다.

비대한 모듈은 없다. 코드 본체는 약 330행이며 나머지(약 500행)는 `#[cfg(test)]` 테스트 모듈이다.

## 5. 핵심 데이터 구조·타입

- `struct Entry<K, V>` (`lfucache/src/lib.rs:13`)
  - 필드: `hash_link`/`recency_link`(`LinkedListLink`), `frequency_link`(`RBTreeLink`), `freq: RefCell<u16>`, `last_tick: RefCell<u32>`, `key: K`, `value: V`.
  - 불변식:
    - 모든 살아 있는 `Entry`는 세 색인(해시 버킷·LRU·LFU) 모두에 동시에 연결되어 있어야 한다. 축출/삭제 시 세 색인에서 모두 제거되어야 색인 간 일관성이 깨지지 않는다.
    - 항목은 `Rc<Entry>`로 공유 소유되며, 같은 노드를 세 침입형 컨테이너가 가리킨다. `cursor_mut_from_ptr`를 쓰는 `unsafe` 블록은 해당 포인터가 실제로 그 색인에 연결되어 있다는 전제에 의존한다.
    - `freq`는 `saturating_add(1)`로 증가하므로 `u16::MAX`에서 포화한다. 감쇠는 정수 나눗셈(`/= delta`)이라 0으로 수렴 가능.

- `struct LfuCache<K, V, S>` (`lfucache/src/lib.rs:40`)
  - 불변식:
    - `len`은 세 색인에 등록된 살아 있는 항목 수와 항상 일치해야 한다(`put`/`evict_one`/`put`의 선삭제에서 `len` 증감으로 유지).
    - `tick`은 단조 증가하며 `get`/`put`마다 +1. `last_tick`과의 차이가 감쇠 계산의 기준이 된다.
    - `buckets.len()`은 항상 2의 거듭제곱(`next_power_of_two`/`grow_hash`의 ×2 증식)이어야 `% buckets.len()` 분포가 균등하다.

- 세 어댑터: `RecencyAdapter`/`FrequenceAdapter`/`HashAdapter` (`lfucache/src/lib.rs:23-25`). `intrusive_adapter!` 매크로로 생성되며, 같은 `Rc<Entry>`를 각기 다른 링크 필드로 컨테이너에 매핑한다.

- `EntryData<'a, K, V>` (`lfucache/src/lib.rs:337`): 테스트 전용 스냅샷 표현 구조체.

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
| --- | --- |
| `intrusive-collections` | 핵심. `LinkedList`/`RBTree`/`LinkedListLink`/`RBTreeLink`/`KeyAdapter`/`Bound`와 `intrusive_adapter!` 매크로 제공. 하나의 `Entry`가 세 색인에 노드 재할당 없이 동시 참여하게 해, 항목당 별도 할당·중복 보관을 제거한다. |
| `ahash` | 범용 키용 기본 해셔(`AHasher`). `LfuCache`의 기본 타입 파라미터 `S = BuildHasherDefault<AHasher>`. |
| `fnv` | `LfuCacheU64` 별칭의 해셔(`FnvBuildHasher`). u64 정수 키에 대해 ahash보다 단순·빠른 해시를 제공. |
| `metrics` | `histogram!`로 hit/miss율을 기록(`lfucache/src/lib.rs:249,274`). 메트릭 이름은 생성 시 `&'static str`로 주입. |
| `config` | `ConfigHandle` 타입을 `CapFunc` 시그니처와 `new`/`update_config`에서 사용. 용량 설정 연동의 유일한 통로. |
| `k9` (dev) | 테스트 스냅샷(`k9::snapshot!`/`k9::assert_equal!`). |

## 7. 설정·기능 플래그

feature flag는 없다. 설정 연동은 `CapFunc`를 통한 간접 방식이다. 캐시 스스로는 어떤 config 필드도 직접 읽지 않으며, 호출 측이 주입한 클로저가 필드를 읽는다.

`wezterm-gui`가 주입하는 용량 함수와 대응 config 필드(`config/src/config.rs`):
- `shape_cache_size`(기본 1024, `config/src/config.rs:829,2041`)
- `line_state_cache_size`(기본 1024, `config/src/config.rs:831,2045`)
- `line_quad_cache_size`(기본 1024, `config/src/config.rs:833,2049`)
- `line_to_ele_shape_cache_size`(기본 1024, `config/src/config.rs:835,2053`)
- `glyph_cache_image_cache_size`(기본 256, `config/src/config.rs:837,2037`) — `image_cache: LfuCache`에 사용(`wezterm-gui/src/glyphcache.rs:564,581`).

각 캐시는 hit/miss 메트릭 이름 쌍을 함께 받는다. 예: `"shape_cache.hit.rate"`/`"shape_cache.miss.rate"`(`wezterm-gui/src/termwindow/mod.rs:731-734`).

설정 철학상, 사용자는 `wezterm.lua`로도 이 값들을 조정할 수 있으나, 이 fork의 기본값 고정 방식에서는 `config` 크레이트 소스의 `default_*` 함수를 직접 수정하는 경로가 우선된다.

## 8. Windows 전용 고려사항

플랫폼 분기·`cfg(unix)`/`cfg(windows)`·Windows API 호출 지점이 전혀 없다. 이 크레이트는 순수 자료구조/알고리즘 코드이며 OS에 무관하다. 따라서 Windows fork에서도 upstream 대비 변형이 없고, 죽은 플랫폼 경로도 없다. 리팩토링 시 플랫폼 관점에서 주의할 사항은 없다.

## 9. 리팩토링 주의점

- **`unsafe` 포인터 커서 의존성**: `cursor_mut_from_ptr`를 쓰는 모든 `unsafe` 블록(`lfucache/src/lib.rs:181,208,239,256,292`)은 대상 `Entry`가 해당 색인에 실제로 연결되어 있다는 불변식에 의존한다. 색인 등록·제거 순서를 바꾸면 미정의 동작이 발생할 수 있다. 세 색인의 등록/제거는 항상 짝을 이뤄야 한다.
- **세 색인 일관성**: 삭제 경로가 셋(`evict_one`, `put`의 선삭제, `clear`) 존재한다. 새 삭제/이동 경로 추가 시 해시·LRU·LFU 세 곳 모두에서 제거하고 `len`을 갱신하지 않으면 색인 불일치·`len` 오차가 누적된다.
- **빈도 색인 갱신 비용**: `freq` 변경 시 RBTree에서 노드를 제거 후 재삽입해야 한다(`KeyAdapter`가 `freq`를 키로 쓰므로). `get`·`decay`마다 O(log n) 재배치가 발생한다. 빈도 표현(`u16`)·감쇠 공식을 바꾸면 정렬 키 의미가 바뀌어 축출 순서가 달라진다.
- **버킷 확장 임계**: `put`의 `self.buckets.len() < self.cap && self.len > self.buckets.len() / 2`(`lfucache/src/lib.rs:321`)는 한 번 확장 시 ×2(`grow_hash`)로만 늘린다. 용량이 매우 클 때 초기 버킷 수(`cap/10`)와 확장 임계의 상호작용을 검토해야 부하 분산이 의도대로 동작한다.
- **포화·정수 나눗셈 경계**: `freq.saturating_add(1)`(상한 포화)와 `freq /= delta`(0 수렴)는 의도된 동작이다. 감쇠 분모 `delta = (tick - last_tick)/10`이 `<=1`이면 감쇠를 건너뛴다(`lfucache/src/lib.rs:174`). 이 상수(10)를 바꾸면 캐시 거동이 전반적으로 변한다.
- **`get`이 `&mut self`**: 조회가 빈도/LRU/메트릭을 변경하므로 가변 차용을 요구한다. `wezterm-gui`는 이 때문에 캐시를 `RefCell`로 감싸 사용한다(`wezterm-gui/src/termwindow/mod.rs:426`). 동시성/재진입 경로를 추가하면 `RefCell` 패닉 위험이 있다.
- **결합도**: `config`에 대한 의존은 `CapFunc`/`ConfigHandle`로 최소화되어 있어 결합도가 낮다. 순환 의존은 없다(말단 라이브러리).
- **테스트 결합**: 동작 검증이 `k9::snapshot!`의 정확한 빈도/tick 값에 강하게 묶여 있다(`lfucache/src/lib.rs:371-826`). tick 증가 시점·감쇠 공식·축출 순서를 바꾸면 다수 스냅샷이 동시에 깨지므로, 알고리즘 변경 시 스냅샷 일괄 재생성을 동반해야 한다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `lfucache/Cargo.toml` | 18 | 크레이트 매니페스트. deps(ahash·config·fnv·intrusive-collections·metrics)·dev-dep(k9) 선언. `publish = false`. |
| `lfucache/src/lib.rs` | 827 | 전체 구현. `LfuCache`/`LfuCacheU64`/`CapFunc`/`Entry`·세 침입형 어댑터·공개 메서드 정의. 약 1~330행이 본체, 331~827행이 `#[cfg(test)]` 스냅샷 테스트(`decay`/`eviction`/`basic`). |

생성 파일은 없다.
