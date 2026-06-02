# ratelim 폴더 기능 명세

## 1. 개요 및 책임

`ratelim` 크레이트는 단일 책임을 가진 얇은 래퍼다. 외부 토큰 버킷 라이브러리 `governor`를 감싸, **설정값 기반의 초당 처리량 제한기(`RateLimiter`)** 하나를 제공한다. 핵심 부가가치는 두 가지다.

1. **설정 핫리로드 연동**: 제한 용량을 상수가 아니라 `config` 크레이트의 `ConfigHandle`에서 추출하는 클로저로 받는다. 설정 세대(generation)가 바뀌고 추출값이 변하면 제한기를 재생성해 새 용량을 즉시 반영한다.
2. **부분 수용(partial admission) 검사**: 요청량을 한 번에 다 수용할 수 없을 때, 절반씩 줄여가며 수용 가능한 최대량을 찾아 반환하는 `admit_check`를 제공한다.

Windows fork에서의 실제 책임은 변하지 않는다. 이 크레이트는 플랫폼 분기 코드를 포함하지 않으며, 순수 로직 크레이트다. 유일한 소비처는 `wezterm-client`로, mux 서버에서 화면 라인을 선반입(prefetch)할 때 초당 요청 횟수를 제한하는 데 사용한다(`config.ratelimit_mux_line_prefetches_per_second`). 즉 원격 mux 연결에서 라인 페치 폭주를 억제하는 흐름 제어 부품이다.

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 | 비고 |
| --- | --- | --- |
| 의존(deps) | `config` | 설정 핸들·세대 조회 (`configuration()`, `ConfigHandle`) |
| 의존(deps) | `governor` | 토큰 버킷 알고리즘 본체 (워크스페이스 의존, `Cargo.toml`에 미명시였으나 코드에서 사용) |
| 피의존(usedBy) | `wezterm-client` | mux 라인 선반입 흐름 제어 |

> 주: `ratelim/Cargo.toml:10-12`은 `config`, `governor`를 워크스페이스 의존으로 선언한다.

계층상 이 크레이트는 `config`(설정 계층) 위에 얹힌 **유틸리티 계층**에 위치하며, 상위의 네트워크 클라이언트 계층(`wezterm-client`)이 흐름 제어 부품으로 소비한다. 의존 방향은 단방향이며 순환은 없다.

## 3. 공개 API 표면

공개 표면은 단일 타입 `RateLimiter`와 그 메서드 3개로 구성된다(`ratelim/src/lib.rs:7-88`).

- `pub struct RateLimiter` — 제한기 본체.
- `pub fn new<F: Fn(&ConfigHandle) -> u32 + 'static + Send>(get_limit_value: F) -> Self` (`lib.rs:21`)
  - 설정 핸들에서 초당 용량을 추출하는 클로저를 받아 제한기를 생성한다. 생성 시점의 설정 세대를 기록한다. 추출값이 0이면 `expect`로 패닉한다("RateLimiter capacity to be non-zero").
- `pub fn non_blocking_admittance_check(&mut self, amount: u32) -> bool` (`lib.rs:52`)
  - `amount`개를 즉시 전부 수용 가능한지 비차단으로 검사한다. 호출 시 설정 리로드를 먼저 점검한다. `amount`가 0이면 `expect`로 패닉한다.
  - 소스에 `#[allow(dead_code)]`가 붙어 있으나 **실제로는 사용 중**이다(`wezterm-client/src/pane/renderable.rs:409`). 이 속성은 현시점에서 부정확하다(§9 참조).
- `pub fn admit_check(&mut self, mut amount: u32) -> Result<u32, Duration>` (`lib.rs:64`)
  - `amount`까지 수용을 시도한다. 성공 시 **실제 수용된 양**(요청보다 적을 수 있음)을 `Ok(u32)`로 반환한다. 한 개도 즉시 수용 불가하면 재시도까지 대기해야 할 `Duration`을 `Err`로 반환한다. `amount==0`이면 `Ok(0)`을 즉시 반환한다.

`check_config_reload`(`lib.rs:36`)는 비공개 헬퍼다.

## 4. 내부 구조

모듈 분해는 없다. 단일 파일 `lib.rs`(89행)에 전부 담긴다. 비대한 모듈은 없다.

제어 흐름:
- **생성**: `new` → 현재 설정 조회 → 세대·용량 캐시 → `governor`의 `Limiter::direct(Quota::per_second(NonZeroU32))` 생성.
- **수용 검사 진입점 공통 전처리**: `non_blocking_admittance_check`·`admit_check` 모두 진입 즉시 `check_config_reload()`를 호출한다. 이것이 설정 변경을 검사 경로에 끌어들이는 지점이다.
- **`check_config_reload`**: 현재 설정 세대를 캐시된 세대와 비교한다. 다르면 클로저로 용량을 재추출하고, 그 값이 캐시된 용량과 다를 때만 `Limiter`를 새로 만들어 교체한다(이때 토큰 카운터가 사실상 리셋된다). 세대만 갱신될 뿐 용량이 같으면 제한기는 유지된다.
- **`admit_check`의 이분 축소 루프**(`lib.rs:66-86`): `governor`의 `check_n`을 호출해 전량 수용을 시도하고, 실패 시 `amount`를 절반으로 줄여 재시도한다. `amount==1`까지 줄였는데도 비순응이면 `over.wait_time_from(now)`로 산출한 대기 시간을 `Err`로 반환한다. 주석에 따르면 32k 버퍼가 100% 찬 최악의 경우 약 15회 반복으로 단일 바이트 또는 대기 결정에 도달한다.

데이터 흐름: 설정값(u32) → `Quota` → `governor` 내부 토큰 버킷 상태(`InMemoryState`) → 수용 판정. 시계는 `DefaultClock`을 사용한다.

## 5. 핵심 데이터 구조·타입

유일한 구조체 `RateLimiter`(`lib.rs:7-12`):

| 필드 | 타입 | 역할 |
| --- | --- | --- |
| `lim` | `Limiter<NotKeyed, InMemoryState, DefaultClock>` | governor 직접(direct, 키 없는) 제한기 본체 |
| `get_limit_value` | `Box<dyn Fn(&ConfigHandle) -> u32 + 'static + Send>` | 설정에서 용량을 추출하는 클로저 |
| `generation` | `usize` | 마지막으로 관측한 설정 세대 |
| `capacity_per_second` | `u32` | 현재 적용 중인 초당 용량(캐시) |

불변식:
- `capacity_per_second`는 항상 0이 아니다. 0이면 `NonZeroU32::new(...).expect(...)`에서 패닉하므로, 0은 구조체에 절대 저장되지 않는다.
- `generation`은 `check_config_reload` 후 항상 가장 최근 관측 세대와 일치한다.
- `lim`은 `capacity_per_second`와 정합한다. 즉 `lim`은 `capacity_per_second`(초당)을 용량으로 갖는 버킷이다. 둘은 항상 함께 갱신된다.
- `get_limit_value`는 `Send`이며 `'static`이다. `RateLimiter`를 스레드 경계로 넘길 수 있게 한다.

`governor` 타입 별칭: `Quota::per_second`는 "초당 N개" 버킷을, `NegativeMultiDecision::BatchNonConforming(_, over)`는 비순응 시 초과분과 회복 정보를 담는다.

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
| --- | --- |
| `governor` | 토큰 버킷/leaky bucket 기반 속도 제한 알고리즘 본체. `RateLimiter`(별칭 `Limiter`), `Quota`, `Clock`/`DefaultClock`, `NegativeMultiDecision`을 직접 사용한다. 키 없는 인메모리 변종(`direct` + `InMemoryState`)만 쓴다. |
| `config` | 워크스페이스 전역 설정 접근. `configuration()`으로 현재 핸들을, `ConfigHandle::generation()`으로 핫리로드 세대를 얻는다. 이 연동이 이 크레이트가 단순 `governor` 래퍼를 넘어서는 유일한 부가가치다. |

표준 라이브러리는 `std::num::NonZeroU32`(용량의 비영(非零) 보장), `std::time::Duration`(재시도 대기 반환)을 사용한다.

## 7. 설정·기능 플래그

- **feature flag**: 없다. `Cargo.toml`에 `[features]` 섹션이 없다.
- **관련 config 항목**: `ratelimit_mux_line_prefetches_per_second: u32` (`config/src/config.rs:391-392`). 기본값 50(`config/src/config.rs:1626-1628`, `default_ratelimit_line_prefetches_per_second`). 이 크레이트 자체는 특정 config 필드를 알지 못한다. 어떤 필드를 읽을지는 전적으로 소비처가 `new`에 넘기는 클로저로 결정된다. `wezterm-client`는 `|config| config.ratelimit_mux_line_prefetches_per_second`를 넘긴다(`wezterm-client/src/pane/clientpane.rs:74`).

## 8. Windows 전용 고려사항

- 플랫폼 분기 코드 없음. `cfg(...)` 사용 없음. Windows API 직접 호출 없음. 죽은 플랫폼 경로 없음.
- 이 크레이트는 플랫폼 중립 순수 로직이므로 Windows fork 전환의 영향을 받지 않는다.
- 단, `#[allow(dead_code)]`가 붙은 `non_blocking_admittance_check`는 플랫폼과 무관하게 실제 사용 중이며(아래 §9), fork 정리 작업에서 죽은 코드로 오인해 제거하지 않도록 주의해야 한다.

## 9. 리팩토링 주의점

- **`#[allow(dead_code)]` 오기**(`lib.rs:51`): `non_blocking_admittance_check`에 죽은 코드 속성이 붙어 있으나, `wezterm-client/src/pane/renderable.rs:409`에서 호출된다. 이 속성은 사실과 어긋난다. 제거하고, 컴파일러가 죽은 코드 경고를 내지 않음을 확인하는 것이 옳다. fork 정리 정책(미사용 코드 전수 제거)과 직접 충돌하는 함정이다.
- **0 용량 패닉**: 용량이 0이면 `new`·`check_config_reload`·`non_blocking_admittance_check`의 `expect`에서 패닉한다. 설정에서 `ratelimit_mux_line_prefetches_per_second`를 0으로 설정하면 클라이언트 페인 초기화 시 프로세스가 패닉할 수 있다. 견고성을 높이려면 `NonZeroU32` 변환 실패를 패닉 대신 합리적 하한(예: 1)으로 클램프하는 방안을 검토할 수 있다.
- **카운터 리셋 부작용**: `check_config_reload`가 용량 변경을 감지하면 `Limiter`를 통째로 재생성한다. 이는 누적된 토큰 상태를 버린다. 설정을 자주 바꾸는 시나리오에서 의도치 않은 버스트 허용이 생길 수 있다. 불변식(§5)상 의도된 동작이나, 호출 측이 이를 인지해야 한다.
- **세대-값 이중 비교**: 세대가 바뀌어도 추출값이 같으면 제한기를 유지한다. `generation`은 항상 갱신되고 `capacity_per_second`는 값이 변할 때만 갱신된다. 두 캐시 필드의 갱신 조건이 다르다는 점을 변경 시 깨뜨리지 말아야 한다.
- **결합도**: `config` 크레이트의 전역 `configuration()`와 `generation()` 시맨틱에 강하게 결합된다. 설정 핫리로드 구조가 바뀌면 `check_config_reload` 로직을 재검토해야 한다.
- **순환 의존**: 없다. `config` → `ratelim` → `wezterm-client` 단방향.
- **`admit_check` 효율**: 이분 축소 루프는 최악의 경우 약 15회 반복(주석 기준). 주석 스스로 "완벽히 효율적이지 않다"고 명시한다. 매우 큰 버퍼에서 성능이 문제되면 governor의 잔여 용량 직접 조회 API로 단일 산술 결정으로 대체하는 리팩토링을 고려할 수 있다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `ratelim/Cargo.toml` | 13 | 패키지 메타·의존(`config`, `governor`) 선언. `publish = false`, edition 2018. |
| `ratelim/src/lib.rs` | 89 | 크레이트 전체. `RateLimiter` 구조체와 `new`/`non_blocking_admittance_check`/`admit_check` 공개 메서드, 비공개 `check_config_reload`. |

생성 파일 없음.
