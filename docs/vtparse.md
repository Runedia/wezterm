# vtparse 폴더 기능 명세

## 1. 개요 및 책임

`vtparse`는 단일 크레이트 폴더이며, 단일 책임을 가진다. DEC ANSI 파서([vt100.net/emu/dec_ansi_parser](https://vt100.net/emu/dec_ansi_parser))의 상태 기계를 구현하여, 바이트 스트림을 ANSI/ECMA-48 이스케이프 시퀀스의 **구조적 분류**로 변환하는 것이다. UTF-8 멀티바이트 시퀀스를 인식하도록 원본 상태 기계를 확장했다.

이 크레이트는 시퀀스를 **분류만** 한다. 의미를 부여하지 않는다. 예를 들어 `CSI 1 m`을 보면 "CSI 디스패치, 최종 바이트 `m`, 파라미터 `[1]`"로 콜백을 호출할 뿐, 그것이 "굵게(bold)"라는 SGR 의미는 해석하지 않는다(`vtparse/src/lib.rs:6-8`). 의미 부여는 상위 크레이트(`termwiz::escape::parser`, `wezterm-escape-parser`)의 책임이다.

설계상 특징:
- **무할당 경로 지원**: `no_std`/`alloc`/`std` 피처로 할당 정책을 분리한다. 콜백 기반(`VTActor` 트레이트)이므로 코어 파싱은 입력 버퍼 외에 동적 할당을 강제하지 않는다.
- **푸시 기반(push) 파서**: 호출자가 바이트를 밀어 넣으면(`parse_byte`/`parse`) 파서가 인식한 이벤트마다 `VTActor`의 메서드를 호출한다. 파서가 입력을 당겨오지(pull) 않는다.
- **컴파일 타임 전이 테이블**: 상태 전이는 `const fn`으로 빌드 시점에 `[[u16; 256]; 15]` 배열로 펼쳐진다. 런타임 초기화 비용이 없고, 룩업은 2차원 배열 인덱싱 한 번이다.

### Windows fork에서의 실제 책임

이 크레이트는 순수 알고리즘 코드다. 플랫폼 분기(`cfg(unix)`, `cfg(windows)`)나 Windows API 호출이 전무하다. Windows 전용 분기에서도 책임 변화가 없다. 터미널 PTY로부터 읽은 바이트 스트림을 파싱하는 최하위 계층으로서, upstream과 동일한 형태로 유지된다.

## 2. 워크스페이스 내 위치

### 의존성(deps)

| 크레이트 | 종류 | 용도 |
|---|---|---|
| `utf8parse` | 외부 | UTF-8 멀티바이트 시퀀스 점진적 디코딩(`Parser`, `Receiver`) |
| `heapless` | 외부(optional) | `no_std` 피처에서 고정 용량 `Vec` 제공 |
| `k9` | 외부(dev) | 테스트 단언(`assert_equal`) |

### 피의존(usedBy)

| 크레이트 | 의존 선언 위치 | 활성 피처 |
|---|---|---|
| `termwiz` | `termwiz/Cargo.toml:39` (`vtparse.workspace = true`) | 워크스페이스 기본값 |
| `wezterm-escape-parser` | `wezterm-escape-parser/Cargo.toml:27` | `features=["alloc"]` |

워크스페이스 루트(`Cargo.toml:196`)에서 `vtparse = { version="0.7", path="vtparse", default-features=false }`로 선언되어 있다. 즉 워크스페이스 차원에서는 `default`(=`std`) 피처가 꺼진 상태이며, 소비 크레이트가 필요한 피처를 명시적으로 켠다. `wezterm-escape-parser`는 `alloc`을, 그 `std` 피처가 켜지면 `vtparse/std`를 전파한다(`wezterm-escape-parser/Cargo.toml:51`).

**계층상 위치**: 이스케이프 시퀀스 파이프라인의 **최하위(어휘 분석) 계층**이다. PTY 바이트 → `vtparse`(상태 기계 분류) → `wezterm-escape-parser`/`termwiz`(의미 해석) 순으로 흐른다. 이 크레이트는 워크스페이스 내부 의존이 전혀 없는 리프(leaf) 노드이므로, 변경 시 컴파일 파급은 자신과 직접 소비자 2개로 한정된다.

## 3. 공개 API 표면

모든 공개 항목은 `vtparse/src/lib.rs`에 노출된다. `enums.rs`와 `transitions.rs`의 항목은 `pub(crate)` 또는 모듈 내부로 캡슐화되어 외부에 노출되지 않는다.

### 트레이트 `VTActor` (`vtparse/src/lib.rs:90-184`)

호스트 애플리케이션이 구현하여 파싱 이벤트를 수신하는 콜백 인터페이스. 상태 기계의 각 액션에 대응한다.

| 메서드 | 시그니처 요약 | 용도 |
|---|---|---|
| `print` | `(&mut self, b: char)` | GL 영역(또는 UTF-8 디코딩된) 출력 문자. 잘못된 시퀀스는 U+FFFD로 전달 |
| `execute_c0_or_c1` | `(&mut self, control: u8)` | C0/C1 제어 기능 실행 |
| `dcs_hook` | `(&mut self, mode: u8, params: &[i64], intermediates: &[u8], ignored_excess_intermediates: bool)` | DCS 시작: 최종 바이트와 파라미터로 핸들러 선택 |
| `dcs_put` | `(&mut self, byte: u8)` | DCS 데이터 바이트 전달 |
| `dcs_unhook` | `(&mut self)` | DCS 종료(ST/CAN/SUB/ESC) |
| `esc_dispatch` | `(&mut self, params: &[i64], intermediates: &[u8], ignored_excess_intermediates: bool, byte: u8)` | 단순 이스케이프 시퀀스 디스패치 |
| `csi_dispatch` | `(&mut self, params: &[CsiParam], parameters_truncated: bool, byte: u8)` | CSI 시퀀스 디스패치 |
| `osc_dispatch` | `(&mut self, params: &[&[u8]])` | OSC 종료: 세미콜론 분리된 바이트 문자열 파라미터 배열 전달 |
| `apc_dispatch` | `(&mut self, data: Vec<u8>)` | APC 종료. `std`/`alloc` 피처에서만 존재(`cfg` 게이트) |

### enum `VTAction` (`vtparse/src/lib.rs:189-215`)

`VTActor` 구현을 직접 작성하지 않고 이벤트를 값으로 캡처하기 위한 대안. `Print`, `ExecuteC0orC1`, `DcsHook{...}`, `DcsPut`, `DcsUnhook`, `EscDispatch{...}`, `CsiDispatch{...}`, `OscDispatch`, `ApcDispatch` 변형을 가진다. `std`/`alloc` 게이트.

### struct `CollectingVTActor` (`vtparse/src/lib.rs:221-309`)

`VTActor`를 구현하여 모든 이벤트를 내부 `Vec<VTAction>`에 적재하는 헬퍼. `into_iter()`로 순회하거나 `into_vec()`로 추출한다(`vtparse/src/lib.rs:239`). `Default` 파생. `std`/`alloc` 게이트.

### enum `CsiParam` (`vtparse/src/lib.rs:398-426`)

CSI 파라미터 한 개를 표현. `Integer(i64)`(10진 정수 파라미터) 또는 `P(u8)`(구분자/프라이빗 마커 등 파라미터 바이트). `as_integer(&self) -> Option<i64>` 제공. `Copy`, `Clone`, `PartialEq`, `Eq`, `Hash` 파생, `Debug`/`Display` 수동 구현, `Default`는 `Integer(0)`.

### struct `VTParser` (`vtparse/src/lib.rs:359-377`, `442-752`)

파서 본체. 공개 메서드:

| 메서드 | 시그니처 | 용도 |
|---|---|---|
| `new` | `() -> Self` | Ground 상태로 초기화 |
| `is_ground` | `(&self) -> bool` | 보류 상태 없이 Ground에 있는지 |
| `parse_byte` | `(&mut self, byte: u8, actor: &mut dyn VTActor)` | 1바이트 처리, `#[inline(always)]` |
| `parse` | `(&mut self, bytes: &[u8], actor: &mut dyn VTActor)` | 바이트 슬라이스 처리(불완전 시퀀스 허용) |

`actor`가 `&mut dyn VTActor` 트레이트 객체로 전달되므로, 액터 구현이 단형화(monomorphization)되지 않고 동적 디스패치된다.

## 4. 내부 구조

3개 모듈로 구성된다. 비대한 모듈은 없으며, 가장 큰 `lib.rs`(1162행)도 절반 이상이 테스트(`vtparse/src/lib.rs:755-1162`)다.

- **`enums.rs`** (62행): `Action`(19개 변형), `State`(17개 변형) 정의와 `from_u16` 변환 헬퍼.
- **`transitions.rs`** (372행): 상태별 전이 함수(`const fn`)와 컴파일 타임 테이블(`TRANSITIONS`, `ENTRY`, `EXIT`).
- **`lib.rs`** (1162행): 공개 API, 파서 본체, UTF-8 처리, 룩업 함수, 테스트.

### 제어 흐름

핵심 루프는 `parse_byte`(`vtparse/src/lib.rs:720-743`)다.

1. 현재 상태가 `Utf8Sequence`면 VT 테이블을 우회하고 `next_utf8`로 위임한다(`vtparse/src/lib.rs:725-728`).
2. 그 외에는 `lookup(state, byte)`(`vtparse/src/lib.rs:30-38`)로 `(Action, State)` 쌍을 얻는다. 이 룩업은 `TRANSITIONS[state][byte]`를 `get_unchecked`로 인덱싱하고, 상위 8비트를 `Action`, 하위 8비트를 `State`로 분해한다.
3. 상태가 바뀌면 **exit 액션 → 전이 액션 → entry 액션** 순으로 실행한다(`vtparse/src/lib.rs:732-738`). 단, 목표 상태가 `Utf8Sequence`이면 exit 액션을 생략한다. 상태가 그대로면 전이 액션만 실행한다.
4. 액션 디스패치는 `action`(`vtparse/src/lib.rs:520-657`)에서 `match`로 처리하고, 필요 시 `actor`의 메서드를 호출한다.

### 데이터 흐름 (파라미터 누적)

- **CSI/DCS 파라미터**: `Action::Param`에서 `current_param`에 숫자를 자릿수 단위로 누적(`saturating_mul(10).saturating_add`)하고, 구분자/마커 바이트가 오면 `finish_param`으로 `params` 배열에 확정한다(`vtparse/src/lib.rs:549-580`, `492-499`).
- **인터미디에이트 승격**: `?`(DECSET 등) 같은 인터미디에이트 범위 바이트가 파라미터 위치에 올 경우, `promote_intermediates_to_params`가 이를 `CsiParam::P`로 승격해 파라미터로 취급한다(`vtparse/src/lib.rs:506-518`).
- **정수 변환**: `dcs_hook`/`esc_dispatch`는 `as_integer_params`(`vtparse/src/lib.rs:479-490`)로 `CsiParam` 배열을 `[i64; MAX_PARAMS]`로 평탄화한다. `csi_dispatch`는 `CsiParam` 슬라이스를 그대로 넘긴다(콜론 구분 RGB 등 표현을 보존하기 위함).
- **OSC**: `OscState`(`vtparse/src/lib.rs:315-356`)가 버퍼와 파라미터 경계 인덱스를 누적하고, `OscEnd`에서 경계로 슬라이스를 잘라 `&[&[u8]]`로 디스패치한다(`vtparse/src/lib.rs:619-637`).

### UTF-8 처리 (`next_utf8`, `vtparse/src/lib.rs:667-715`)

VT 테이블이 `Action::Utf8`을 내보내면 `Utf8Sequence` 상태로 진입하고, 이후 바이트마다 `utf8parse::Parser`에 위임한다. 코드포인트가 완성되면:
- 값이 0xFF 이하이고 그것이 상태 전이를 유발하는 바이트라면(UTF-8로 인코딩된 C1 제어의 특수 처리), 정상 전이 경로(exit→action→entry)를 수행한다(`vtparse/src/lib.rs:692-706`).
- 그 외에는 `utf8_return_state`에 따라 `Ground`면 `print`, `OscString`이면 `osc.put`을 호출하고, 다른 상태면 `panic!`(불변식 위반)한다(`vtparse/src/lib.rs:708-713`).

잘못된 시퀀스는 `Receiver::invalid_sequence`에서 U+FFFD(REPLACEMENT_CHARACTER)로 대체된다(`vtparse/src/lib.rs:677-679`).

## 5. 핵심 데이터 구조·타입

### `State` (`vtparse/src/enums.rs:34-55`)
17개 변형, `#[repr(u16)]`. 0~14는 DEC 파서의 실제 상태(`Ground`, `Escape`, `CsiEntry`, `DcsPassthrough`, `OscString`, `ApcString` 등)이며 각각 `TRANSITIONS`에 한 테이블을 가진다. 15(`Anywhere`)와 16(`Utf8Sequence`)은 **특수 상태**로 테이블이 없다.
- **불변식**: discriminant 순서가 `TRANSITIONS`/`ENTRY`/`EXIT` 배열 인덱스와 일대일 대응해야 한다. `from_u16`이 `transmute`를 쓰므로(`vtparse/src/enums.rs:58-61`) 범위 밖 값이 들어오면 미정의 동작이다. 룩업 결과의 하위 8비트가 항상 0~16 범위 안에 있도록 전이 테이블이 보장해야 한다.

### `Action` (`vtparse/src/enums.rs:3-25`)
19개 변형, `#[repr(u16)]`. `None`/`Print`/`Execute`/`Clear`/`Collect`/`Param`/`CsiDispatch`/`Hook`/`Put`/`OscPut`/`Utf8`/`Apc*` 등.
- **불변식**: `from_u16`(`vtparse/src/enums.rs:27-32`)이 `transmute`를 쓰므로 0~18 범위 밖 값은 미정의 동작. 전이 테이블의 상위 8비트가 항상 이 범위 안이어야 한다.

### `VTParser` (`vtparse/src/lib.rs:359-377`)
파서의 전 상태를 담는다. 고정 크기 배열을 사용하여 무할당 경로를 지원한다.
- `intermediates: [u8; 2]`, `num_intermediates`: 인터미디에이트는 최대 2개(`MAX_INTERMEDIATES`). 초과 시 `ignored_excess_intermediates = true`.
- `params: [CsiParam; 256]`, `num_params`, `current_param`, `params_full`: CSI 파라미터(`MAX_PARAMS = 256`). 가득 차면 무시.
- `osc: OscState`, `apc_data: Vec<u8>`(std/alloc 게이트).
- `utf8_parser`, `utf8_return_state`: UTF-8 시퀀스 처리 중 복귀할 상태 저장.
- **불변식**: `num_*` 카운터는 대응 배열 용량을 초과하지 않는다(`finish_param`/`Action::Collect`/`promote_intermediates_to_params`에서 경계 검사). `Action::Clear`(`vtparse/src/lib.rs:525-540`)가 시퀀스 시작 시 모든 누적 상태를 초기화한다.

### `OscState` (`vtparse/src/lib.rs:315-356`)
- `buffer`: std/alloc에선 `Vec<u8>`, no_std에선 `heapless::Vec<u8, {MAX_OSC*16}>`(=1024바이트).
- `param_indices: [usize; 64]`(`MAX_OSC = 64`), `num_params`, `full`.
- **불변식**: `param_indices[i]`는 버퍼 내 i번째 파라미터 경계의 누적 오프셋이다. `OscEnd`에서 이 오프셋들이 단조 증가하고 버퍼 길이 이내라야 슬라이스 분할이 안전하다. 파라미터가 `MAX_OSC`를 넘으면 `full=true`로 막는다(`vtparse/src/lib.rs:328-336`).

### `CsiParam` (`vtparse/src/lib.rs:398-426`)
`Integer(i64)` 또는 `P(u8)`. 콜론/세미콜론 구분 구조(예: `4:3` 커리 밑줄, `38:2::128:64:192` RGB)를 손실 없이 보존하기 위해 정수와 파라미터 바이트를 분리해 표현한다.

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
|---|---|
| `utf8parse` | UTF-8 멀티바이트 점진적 디코더. `Parser::advance(receiver, byte)`로 바이트를 밀어 넣고 `Receiver::codepoint`/`invalid_sequence`로 결과를 수신(`vtparse/src/lib.rs:667-684`). 부분 시퀀스를 가로질러 상태를 유지하므로, vtparse가 직접 UTF-8 디코딩 로직을 구현할 필요가 없다 |
| `heapless` | `no_std` 피처에서만 사용. 힙 없는 고정 용량 `Vec`을 OSC 버퍼에 제공(`vtparse/src/lib.rs:319`). `dep:heapless`로 명시적 optional 의존(`vtparse/Cargo.toml:17,20`) |
| `k9` (dev) | 테스트의 `assert_equal` 매크로. 디프 가독성 향상 목적. 프로덕션 빌드에 미포함 |

GPU/셰이핑 등 무거운 의존은 없다. 이 크레이트는 표준 라이브러리(또는 그 일부)와 위 두 런타임 의존만 사용하는 경량 파서다.

## 7. 설정·기능 플래그

`vtparse/Cargo.toml:13-17`에 정의된 피처:

| 피처 | 효과 |
|---|---|
| `default = ["std"]` | 기본으로 `std` 활성 |
| `std` | 표준 라이브러리 사용. `alloc` 경로와 동일하게 `Vec`/`apc_dispatch` 노출 |
| `alloc` | `no_std`이되 `alloc` 크레이트 사용(`extern crate alloc`, `vtparse/src/lib.rs:20-24`). `Vec`/`VTAction`/`CollectingVTActor`/`apc_dispatch` 노출 |
| `no_std` | `heapless` 의존을 켜고 고정 용량 컬렉션만 사용. `Vec` 기반 API 비활성 |

피처 게이트 결과:
- `VTAction`, `CollectingVTActor`, `VTActor::apc_dispatch`, `VTParser::apc_data`는 `cfg(any(feature = "std", feature = "alloc"))` 게이트(`vtparse/src/lib.rs:182,189,221,372`). 순수 `no_std`에서는 APC 디스패치와 이벤트 캡처가 제공되지 않는다.
- `#![cfg_attr(not(feature = "std"), no_std)]`(`vtparse/src/lib.rs:14`)로 std 외 빌드에서 표준 라이브러리를 끊는다.

`config` 크레이트와 연동되는 런타임 설정 항목은 없다. 이 크레이트는 컴파일 타임 피처만 가진다.

## 8. Windows 전용 고려사항

- **플랫폼 분기 부재**: 소스 전체에 `cfg(unix)`, `cfg(windows)`, `cfg(target_os = ...)` 분기가 없다. 모든 `cfg`는 피처(`std`/`alloc`)와 `cfg(test)`에 한정된다.
- **Windows API 미사용**: OS API 호출 지점이 전무하다. 순수 알고리즘·자료구조 코드다.
- **죽은 코드 측면**: Windows fork에서 비활성화될 코드 경로는 없다. 단, 워크스페이스가 `default-features=false`로 이 크레이트를 가져오므로(`Cargo.toml:196`) `std`가 자동으로 켜지지 않는다. 실제 활성 피처는 소비 크레이트(`wezterm-escape-parser`는 `alloc`, 그 `std`가 켜지면 `vtparse/std`)에 의해 결정된다. `no_std`/`heapless` 경로는 워크스페이스 내에서 활성화되지 않으므로 현재 fork에서는 사실상 사용되지 않는 경로다.

## 9. 리팩토링 주의점

- **`transmute` 기반 enum 변환**: `Action::from_u16`/`State::from_u16`(`vtparse/src/enums.rs:27-32,57-62`)이 `unsafe transmute`를 쓴다. enum에 변형을 추가/삭제하거나 discriminant 값을 바꾸면, 전이 테이블이 내보내는 모든 값이 유효 범위 안임을 반드시 보장해야 한다. 위반 시 미정의 동작. enum 정의와 `transitions.rs`의 `pack`/per-state 함수는 강하게 결합되어 있다.
- **`get_unchecked` 룩업**: `lookup`/`lookup_entry`/`lookup_exit`의 비테스트 경로가 경계 검사 없는 인덱싱을 한다(`vtparse/src/lib.rs:32-66`). `State` discriminant가 0~16, 바이트가 0~255 범위라는 불변식에 의존한다. `ENTRY`/`EXIT` 배열 길이(17)와 `State` 변형 수가 어긋나면 메모리 안전성이 깨진다. 테스트 빌드는 `.get(...).unwrap_or_else(panic)`로 안전 검증한다.
- **테이블 일관성 회귀 테스트**: `transitions.rs`의 `test_transitions`(`vtparse/src/transitions.rs:351-371`)가 `TRANSITIONS` 전체를 직렬화해 길이와 3종 해시(djb2/sdbm 등)를 하드코딩된 값과 비교한다. 전이 규칙을 의도적으로 바꾸면 이 해시도 함께 갱신해야 한다. 의도치 않은 테이블 변경을 잡는 안전장치다.
- **`Utf8Sequence`의 상태 우회**: `parse_byte`가 UTF-8 상태에서 VT 테이블을 건너뛰는 분기(`vtparse/src/lib.rs:725-728`)와 `next_utf8` 내부의 C1 특수 처리(`vtparse/src/lib.rs:692-706`)는 상호 의존적이다. `utf8_return_state` 관리가 미묘하다. 변경 시 `osc_utf8`, `osc_fedora_vte`, `utf8_control`, `print_utf8` 테스트로 회귀를 확인해야 한다.
- **콜백 의미 안정성**: `csi_dispatch`가 `CsiParam` 슬라이스를, `dcs_hook`/`esc_dispatch`가 평탄화된 `[i64]`를 넘기는 비대칭은 소비자(`termwiz`, `wezterm-escape-parser`)가 의존한다. 이 표현을 바꾸면 두 소비 크레이트가 깨진다. 콜론 구분 구조 보존(`CsiParam::P`)은 SGR 확장(커리 밑줄, true-color) 파싱의 전제다.
- **고정 한계값**: `MAX_INTERMEDIATES=2`, `MAX_OSC=64`, `MAX_PARAMS=256`(`vtparse/src/lib.rs:311-313`)은 스택 배열 크기와 직결된다. 늘리면 `VTParser`/`OscState` 크기가 커지고, no_std 경로의 `heapless::Vec` 용량(`MAX_OSC*16`)도 함께 영향받는다.
- **순환 의존 없음**: 워크스페이스 내부 의존이 0이므로 순환 위험이 없다. 외부 의존(`utf8parse`, `heapless`)만 가진다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `vtparse/Cargo.toml` | 24 | 패키지·피처(`std`/`alloc`/`no_std`)·의존(`utf8parse`, `heapless`, dev: `k9`) 선언 |
| `vtparse/src/lib.rs` | 1162 | 공개 API(`VTActor`, `VTAction`, `CollectingVTActor`, `CsiParam`, `VTParser`), 파서 본체, UTF-8 처리, 룩업 함수. 약 400행이 단위 테스트(`vtparse/src/lib.rs:755-1162`) |
| `vtparse/src/enums.rs` | 62 | `Action`(19변형)·`State`(17변형) enum과 `from_u16` 변환. `#[repr(u16)]` |
| `vtparse/src/transitions.rs` | 372 | DEC 파서 상태 전이 테이블. 상태별 `const fn`과 `define_table!` 매크로로 컴파일 타임에 `TRANSITIONS[[u16;256];15]`/`ENTRY`/`EXIT`를 생성. 끝에 테이블 해시 회귀 테스트 |

`transitions.rs`는 거대 생성 데이터 파일이 아니다. 사람이 작성한 규칙 함수이며, **테이블은 컴파일 타임에 그 함수로부터 펼쳐진다**(런타임 생성물이 아닌 `const fn` 산출물). 따라서 규칙 변경은 코드 수정으로 이뤄지고, `unicode_names.rs` 류의 외부 생성 데이터와 성격이 다르다.
