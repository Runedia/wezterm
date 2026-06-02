# frecency 폴더 기능 명세

## 1. 개요 및 책임

`frecency` 크레이트는 "frequency(빈도) + recency(최근성)"를 단일 점수로 결합해 산출하는 순수 알고리즘 라이브러리다. 어떤 항목이 **얼마나 자주** 그리고 **얼마나 최근에** 접근되었는지를 하나의 `f64` 점수로 환산하며, 그 점수는 시간이 지남에 따라 지수적으로 감쇠(decay)한다. 단일 책임은 명확하다: **접근 통계의 누적·시간 감쇠·점수 산출**이다.

이 크레이트는 UI나 영속화(persistence) 로직을 전혀 포함하지 않는다. 파일 I/O, 정렬, 표시 정책은 모두 소비처의 책임이다. `frecency`는 오로지 `Frecency` 값 객체 하나와 그 상태 전이 함수만 제공한다.

Windows fork에서의 실제 책임은 GUI의 "최근 사용 항목" 목록 순위 매기기다. 구체적으로 두 곳에서 소비된다.
- 문자 선택기(char select)의 최근 사용 이모지 정렬 (`wezterm-gui/src/termwindow/charselect.rs`)
- 명령 팔레트의 최근 사용 명령 정렬 (`wezterm-gui/src/termwindow/palette.rs`)

이 크레이트 자체에는 플랫폼 종속 코드가 전혀 없다. 시간(`chrono::Utc`)과 부동소수 연산만 사용하므로 Windows 전용 분기와 무관하게 동작한다.

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 | 비고 |
|------|----------|------|
| 의존(deps) | 없음 (워크스페이스 내부) | 외부 크레이트만 사용: `chrono`, `serde`, `serde_with`, (dev) `serde_json` |
| 피의존(usedBy) | `wezterm-gui` | `Cargo.toml:52`에서 `frecency.workspace = true`로 선언 |

이 크레이트는 워크스페이스 의존성 그래프의 **최하위 리프(leaf) 노드**다. 내부 크레이트에 전혀 의존하지 않으며, 오직 `wezterm-gui`의 UI 순위 매기기 계층에만 소비된다. 따라서 계층상 "유틸리티/순수 알고리즘" 층에 위치하며, 변경 시 워크스페이스 내 파급 범위는 `wezterm-gui` 한 곳으로 한정된다.

## 3. 공개 API 표면

크레이트는 단일 모듈(`lib.rs`)로 구성되며, 공개 항목은 `Frecency` 타입과 그 메서드뿐이다. 헬퍼 함수 `duration_secs_f64`는 비공개(private)다.

핵심 타입:
- `pub struct Frecency` (`frecency/src/lib.rs:10`) — 접근 통계 값 객체. `Serialize`, `Deserialize`, `Clone`, `Debug`, `Default` 파생.

생성자:
- `pub fn new() -> Self` (`lib.rs:29`) — 현재 시각(`Utc::now()`) 기준으로 접근 이력 0인 인스턴스 생성. `Default::default()`도 내부적으로 이를 호출한다(`lib.rs:21-25`).
- `pub fn new_at_time(now: DateTime<Utc>) -> Self` (`lib.rs:35`) — 기준 시각을 외부에서 주입해 생성. 테스트·결정론적 동작에 사용.

상태 전이(쓰기):
- `pub fn register_access(&mut self)` (`lib.rs:45`) — 현재 시각에 접근 1회 기록.
- `pub fn register_access_at_time(&mut self, now: DateTime<Utc>)` (`lib.rs:50`) — 지정 시각에 접근 1회 기록. `register_access`가 위임하는 실제 구현.

조회(읽기):
- `pub fn num_accesses(&self) -> u64` (`lib.rs:58`) — 누적 접근 횟수.
- `pub fn last_accessed(&self) -> &DateTime<Utc>` (`lib.rs:63`) — 마지막 접근 시각 참조.
- `pub fn score(&self) -> f64` (`lib.rs:68`) — 현재 시각 기준 frecency 점수.
- `pub fn score_at_time(&self, now: DateTime<Utc>) -> f64` (`lib.rs:73`) — 지정 시각 기준 점수. `score`가 위임하는 실제 구현.

용도 요약: 소비처는 (1) 항목 생성 시 `Frecency::new()` + `register_access()`로 초기화하고, (2) 재접근 시 `register_access()`를 호출하며, (3) 정렬 시 `score()` 내림차순으로 항목을 배열한다. 영속화는 `serde`를 통해 JSON 직렬화로 처리한다.

## 4. 내부 구조

단일 파일(`lib.rs`) 구조이며 비대한 모듈은 없다(총 134행, 테스트 모듈 포함). 모듈 분해 없이 평면 구조다.

제어/데이터 흐름:
1. **초기화**: `new()` → `new_at_time(Utc::now())`. `half_life = 3일`, `frecency = 0.0`, `last_accessed = now`, `num_accesses = 0`으로 고정 초기화(`lib.rs:36-41`).
2. **접근 기록**: `register_access_at_time(now)`의 핵심 알고리즘(`lib.rs:50-55`):
   - `prior = score_at_time(now)` — 현재 시각으로 감쇠된 직전 점수를 먼저 계산한다.
   - `last_accessed = now`로 갱신한다.
   - `set_frecency_at_time(1.0 + prior, now)` — 감쇠된 직전 점수에 1.0을 더한 값을 새 기준으로 "역감쇠"하여 저장 필드 `frecency`에 기록한다.
   - `num_accesses += 1`.
3. **점수 산출**: `score_at_time(now)`(`lib.rs:73-76`):
   - `elapsed = (now - last_accessed)`(초 단위).
   - `frecency / 2^(elapsed / half_life)` — 마지막 접근 이후 경과 시간을 반감기로 나눈 지수로 저장값을 감쇠시킨다.

핵심 설계는 **저장 필드 `frecency`가 "마지막 접근 시점의 점수"를 보존**한다는 점이다. `set_frecency_at_time`(비공개, `lib.rs:78-81`)은 `score_at_time`의 역연산으로, 외부에서 "이 시점에 점수가 value였어야 한다"는 목표값을 받아 내부 저장값으로 환산한다. 이 역대칭 구조 덕분에 점수 조회는 항상 저장값을 경과 시간만큼 감쇠시키는 단순 연산이 된다.

`duration_secs_f64`(`lib.rs:84-86`)는 `chrono::Duration`을 밀리초 정밀도의 `f64` 초로 변환하는 단일 헬퍼다. `num_milliseconds() / 1000.0`을 사용하므로 밀리초 미만 정밀도는 버려진다.

## 5. 핵심 데이터 구조·타입

`Frecency` (`lib.rs:10-19`) — 4개 필드를 가진 값 객체.

| 필드 | 타입 | 의미 |
|------|------|------|
| `half_life` | `chrono::Duration` | 반감기. 마지막 접근 이후 이 기간이 지나면 점수가 절반으로 감쇠. 기본 3일(`Duration::days(3)`). |
| `last_accessed` | `DateTime<Utc>` | 마지막 접근 시각. 감쇠 계산의 기준점. |
| `frecency` | `f64` | "마지막 접근 시점의 점수" 저장값. 외부에 직접 노출되지 않음. |
| `num_accesses` | `u64` | 누적 접근 횟수. 점수 계산에는 쓰이지 않으며 순수 통계용. |

불변식(invariant):
- `frecency`는 항상 `last_accessed` 시점 기준의 점수를 의미한다. 따라서 `score_at_time(last_accessed) == frecency`가 성립한다(경과 0 → 2^0 = 1).
- `register_access`마다 (감쇠 후) 점수에 정확히 `1.0`이 가산된다(`1.0 + prior`, `lib.rs:53`). 접근 직후의 점수 증가량은 누적 빈도에 무관하게 항상 1.0이다.
- `half_life > 0`이어야 한다. `duration_secs_f64(half_life)`가 분모(지수의 분모)에 들어가므로 0이면 0 나눗셈으로 NaN/Inf가 발생한다. 현재 생성자는 항상 3일로 고정하므로 정상 경로에서는 위반 불가하나, 직렬화 데이터를 통해 0 또는 음수 half_life가 역직렬화되면 불변식이 깨진다(아래 9절 참조).
- `num_accesses`는 단조 증가한다(감소 경로 없음).

직렬화 표현(`serde_with` 적용, `lib.rs:8`, `13-16`):
- `half_life`는 `DurationSeconds<i64>`로 → 정수 초.
- `last_accessed`는 `TimestampSeconds<i64>`로 → Unix epoch 초.
- 테스트(`lib.rs:128-133`)가 정확한 JSON 형태를 고정: `{"half_life":259200,"last_accessed":1661984160,"frecency":0.0,"num_accesses":0}`. `259200`은 3일의 초 수.

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
|----------|-----------|
| `chrono` | 시각(`DateTime<Utc>`)·기간(`Duration`) 표현 및 산술. 감쇠 알고리즘의 시간축 전체를 담당. |
| `serde` (`derive` feature) | `Frecency`의 직렬화/역직렬화 파생. 소비처가 JSON 영속화에 사용. |
| `serde_with` | `chrono` 타입을 사람이 읽기 쉽고 안정적인 정수 형태(초/타임스탬프)로 직렬화. `#[serde_as]` 어트리뷰트로 `Duration`→초, `DateTime`→Unix epoch 변환. chrono 기본 직렬화의 장황함·버전 취약성을 회피. |
| `serde_json` (dev only) | 직렬화 단위 테스트(`lib.rs:128`)에서만 사용. 빌드 산출물에는 미포함. |

GPU·셰이핑·OS API 등 무거운 의존은 전혀 없다. 순수 계산 라이브러리다.

## 7. 설정·기능 플래그

이 크레이트에는 Cargo feature flag가 없다(`Cargo.toml`에 `[features]` 섹션 부재). `serde`의 `derive` feature만 의존성 수준에서 활성화한다.

반감기 등 알고리즘 파라미터는 **하드코딩**되어 있다. `half_life`는 생성자에서 `Duration::days(3)`로 고정되며(`lib.rs:37`), 이를 변경하는 공개 setter나 config 항목은 없다. 즉 `config` 크레이트와 연동되는 설정 항목은 존재하지 않는다. 동작을 바꾸려면 소스 기본값(`lib.rs:37`)을 직접 수정해야 한다 — 이는 이 fork의 "소스 기본값 수정" 철학과 일치하나, 현재 어떤 식으로도 외부 노출되어 있지 않다.

## 8. Windows 전용 고려사항

이 크레이트에는 플랫폼 분기(`cfg(windows)`/`cfg(unix)`)가 전혀 없다. 죽은 코드 경로도 없다. Windows API를 직접 호출하는 지점도 없다. `chrono::Utc::now()`가 내부적으로 OS 시계를 읽지만, 이는 크로스플랫폼 추상화 뒤에 있어 fork의 Windows 전용 분기와 무관하다.

영속화 측면의 Windows 관련 사항은 크레이트 외부(소비처)에 있다. 직렬화된 데이터는 `config::DATA_DIR` 아래 파일로 저장된다 — 문자 선택기는 `recent-emoji.json`(`charselect.rs:98`), 명령 팔레트도 유사 경로다. 이 경로 결정 로직은 `config` 크레이트의 책임이며 `frecency` 크레이트는 관여하지 않는다.

## 9. 리팩토링 주의점

- **결합도**: 매우 낮다. 워크스페이스 내부 의존이 0이고, 피의존도 `wezterm-gui` 한 곳뿐이다. 순환 의존 위험 없음.
- **직렬화 호환성이 진짜 위험 지점**: `serialize` 테스트(`lib.rs:128-133`)가 정확한 JSON 바이트열을 고정하고 있다. 필드명·필드 순서·`serde_with` 변환 타입을 바꾸면 기존 `recent-emoji.json` 등 디스크 데이터와의 호환이 깨진다. 소비처는 역직렬화 실패 시 빈 목록으로 폴백하므로(`charselect.rs:110` `unwrap_or_else(|_| vec![])`) 데이터 손실은 조용히 발생한다 — 사용자 입장에서는 최근 항목이 초기화된다.
- **half_life 불변식 미강제**: 생성자는 3일로 고정하지만, 역직렬화 경로에는 `half_life > 0` 검증이 없다. 손상되거나 조작된 JSON으로 0/음수 half_life가 들어오면 `score_at_time`에서 NaN 또는 Inf가 산출되고, 소비처의 `partial_cmp(...).unwrap()` 정렬(`charselect.rs:105`, `palette.rs:58`)이 **패닉**할 수 있다. NaN 점수는 `partial_cmp`가 `None`을 반환하기 때문이다. 리팩토링 시 반감기 설정을 노출하려면 이 검증을 함께 추가해야 한다.
- **정밀도 부채**: `duration_secs_f64`가 밀리초 단위로 절단(`lib.rs:85`)하므로 밀리초 미만 정밀도는 손실된다. frecency 용도에서는 무해하나, 더 미세한 시간 척도로 재사용할 경우 한계다.
- **점수 단조성 가정**: 소비처는 `score()` 내림차순 정렬에 의존한다. `register_access`의 "+1.0 가산" 의미(빈도 가중치 균일)를 바꾸면 순위 동작 전반이 바뀐다.
- **테스트가 알고리즘을 잠근다**: `it_works` 테스트(`lib.rs:102-125`)가 특정 입력에 대한 정확한 점수값(예: 1일 후 `0.7937005259840997`)을 골든 값으로 고정한다. 감쇠 공식을 수정하면 이 값들을 함께 갱신해야 한다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|------|-----------|------|
| `frecency/Cargo.toml` | 16 | 크레이트 매니페스트. 의존성: `chrono`, `serde`(derive), `serde_with`, (dev) `serde_json`. `publish = false`. |
| `frecency/src/lib.rs` | 134 | 크레이트 전부. `Frecency` 구조체, 생성자·접근 기록·점수 산출 API, 비공개 헬퍼(`set_frecency_at_time`, `duration_secs_f64`), 단위 테스트(`it_works`, `serialize`) 포함. |

생성 파일 없음.
