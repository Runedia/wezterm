# luahelper 폴더 기능 명세

## 1. 개요 및 책임

`luahelper`는 WezTerm의 **Rust 타입 ↔ Lua 값 변환 브리지** 크레이트다. 단일 책임은 명확하다. `wezterm-dynamic`의 동적 값 모델(`wezterm_dynamic::Value`)을 경유하여, Rust 측 구조체/열거형과 `mlua`의 Lua 값(`mlua::Value`) 사이를 양방향으로 변환하는 것이다.

이 크레이트는 직접 직렬화/역직렬화 로직을 가진 것이 아니라, 이미 존재하는 추상화(`wezterm-dynamic`의 `FromDynamic`/`ToDynamic`)를 Lua 런타임 경계에 접착한다. 구체적으로 다음 세 가지를 제공한다.

1. **변환 함수**: `to_lua` / `from_lua` 및 그 하위 원시 함수(`dynamic_to_lua_value` / `lua_value_to_dynamic`).
2. **변환 트레이트 자동 구현 매크로**: `impl_lua_conversion_dynamic!`. 임의의 `FromDynamic + ToDynamic` 타입에 `mlua::IntoLua` / `mlua::FromLua`를 한 줄로 부여한다.
3. **열거형 생성자 메커니즘**: `enumctor::Enum<T>`. `wezterm.action.QuickSelectArgs{...}` 같은 Lua 측 enum 생성 문법을 메타테이블로 구현한다.

Windows fork에서의 실제 책임은 upstream과 동일하다. 이 크레이트에는 플랫폼 분기가 전혀 없다. `cfg(unix)`, `cfg(windows)`, 또는 OS별 코드 경로가 존재하지 않으며, 순수하게 두 인메모리 데이터 모델 간 변환만 수행한다. 따라서 Windows 전용 분기로 인해 이 크레이트에 발생한 변경이나 죽은 코드는 없다.

이 크레이트는 WezTerm Lua 설정 시스템의 **기반 계층**이다. `config` 크레이트가 `wezterm.lua`를 해석할 때, GUI가 콜백 인자를 Lua로 넘길 때, mux가 도메인 정의를 읽을 때 모두 이 변환 함수를 통과한다.

## 2. 워크스페이스 내 위치

### 의존성 (deps)

| 크레이트 | 용도 |
| --- | --- |
| `wezterm-dynamic` | 동적 값 모델(`Value`, `FromDynamic`, `ToDynamic`, `FromDynamicOptions`)을 제공. 변환의 중간 표현(IR). |
| `mlua` | Lua 5.4 바인딩. `Value`, `Lua`, `Table`, `UserData`, `MetaMethod` 등 Lua 측 타입. `pub use mlua`로 재노출. |
| `log` | `ValuePrinterHelper`의 맵 항목 조회 실패 시 오류 로깅(`luahelper/src/lib.rs:389`). |

> 주의: 작업 컨텍스트가 제공한 deps 목록에는 `wezterm-dynamic`만 있었으나, 실제 `luahelper/Cargo.toml:10-13`은 `log`, `mlua`, `wezterm-dynamic` 세 개를 선언한다. `mlua`는 이 크레이트의 핵심 의존이며 `pub use mlua`로 재노출되므로 누락 시 명세가 부정확해진다.

### 피의존 (usedBy)

| 크레이트 | 사용 방식(확인된 진입점) |
| --- | --- |
| `config` | `impl_lua_conversion_dynamic!`(keyassignment, frontend, font, config, color, background, wsl, ssh, exec_domain), `dynamic_to_lua_value`(config.rs:1139), `lua_value_to_dynamic`(lib.rs:323), `enumctor::Enum::<KeyAssignment>`(lua.rs:343) |
| `mux` | `from_lua_value_dynamic`(domain.rs:338,664), `impl_lua_conversion_dynamic!`(renderable.rs) |
| `lua-api-crates/serde-funcs` | `lua_value_to_dynamic`, `ValuePrinter` |
| `lua-api-crates/plugin` | `to_lua` |
| `lua-api-crates/logging` | `ValuePrinter`(로그 출력 포맷) |
| `lua-api-crates/mux` | `dynamic_to_lua_value`(window.rs:87, tab.rs:101), `impl_lua_conversion_dynamic!`(pane.rs, lib.rs) |
| `lua-api-crates/color-funcs` | `impl_lua_conversion_dynamic!`(image_colors.rs) |
| `lua-api-crates/termwiz-funcs` | `impl_lua_conversion_dynamic!` |
| `lua-api-crates/window-funcs` | `impl_lua_conversion_dynamic!` |
| `lua-api-crates/battery` | `impl_lua_conversion_dynamic!` |
| `procinfo` | `impl_lua_conversion_dynamic!` |
| `wezterm-gui` | `dynamic_to_lua_value`(scripting/mod.rs, guiwin.rs:158), `ValuePrinter`(overlay/debug.rs), `impl_lua_conversion_dynamic!`(palette.rs) |

> 주의: 작업 컨텍스트의 usedBy 목록에 `mux-lua`가 있으나, 코드 검색상 `mux-lua`의 직접 참조는 확인되지 않았다. Lua API 표면은 `lua-api-crates/mux`에서 제공된다.

**계층 위치**: `luahelper`는 `wezterm-dynamic` 바로 위, 모든 Lua API 크레이트(`config`, `lua-api-crates/*`, `wezterm-gui` 스크립팅) 바로 아래에 위치하는 **얇은 접착 계층**이다. Lua 경계를 넘는 모든 Rust 데이터는 이 크레이트의 변환을 통과한다.

## 3. 공개 API 표면

모든 공개 항목은 `luahelper/src/lib.rs`와 `luahelper/src/enumctor.rs`에 있다.

### 재노출

- `pub use mlua;` (`lib.rs:3`) — 소비 크레이트가 동일 버전 `mlua`에 접근하도록 재노출. 매크로가 `$crate::mlua::...`로 참조하기 위함.

### 변환 함수 (`lib.rs`)

| 시그니처 | 용도 |
| --- | --- |
| `pub fn to_lua<'lua, T: ToDynamic>(lua: &'lua Lua, value: T) -> Result<Value<'lua>, Error>` | Rust 타입 → `to_dynamic()` → Lua 값. |
| `pub fn from_lua<'lua, T: FromDynamic>(value: Value<'lua>) -> Result<T, Error>` | Lua 값 → 동적 값 → `from_dynamic()`. 변환 실패 시 `FromLuaConversionError`로 래핑. |
| `pub fn dynamic_to_lua_value<'lua>(lua: &'lua Lua, value: DynValue) -> Result<Value<'lua>>` | 동적 값 → Lua 값(재귀). 원시 변환기. |
| `pub fn lua_value_to_dynamic(value: Value) -> Result<DynValue>` | Lua 값 → 동적 값. 순환 참조 방지를 위해 내부적으로 `visited: HashSet<usize>` 사용. |
| `pub fn from_lua_value_dynamic<T: FromDynamic>(value: Value) -> Result<T>` | `lua_value_to_dynamic` + `from_dynamic`의 조합. `from_lua`와 유사하나 기본 옵션으로 직접 `from_dynamic` 호출. |

### 매크로

- `#[macro_export] macro_rules! impl_lua_conversion_dynamic!($struct:ident)` (`lib.rs:40-61`) — 지정한 타입에 `IntoLua`/`FromLua`를 구현. 내부적으로 `to_lua`/`from_lua` 호출. **`FromDynamic + ToDynamic` 선구현이 전제**다. 워크스페이스 전반에서 20개 파일이 사용한다.

### 공개 타입 (`lib.rs`)

- `pub struct ValueLua { pub value: wezterm_dynamic::Value }` (`lib.rs:251-255`) — `FromDynamic`/`ToDynamic` 파생 + `impl_lua_conversion_dynamic!` 적용. 임의의 동적 값을 Lua 경계로 옮기는 래퍼.
- `pub struct ValuePrinter<'lua>(pub Value<'lua>)` (`lib.rs:257`) — `Debug` 구현체. Lua 값을 사람이 읽는 안정적 문자열로 포맷(맵 키 정렬, 순환 감지, userdata `__wezterm_to_dynamic`/`tostring` 호출). 로깅·디버그 오버레이·serde-funcs에서 사용.

### 열거형 생성자 (`enumctor.rs`)

- `pub struct Enum<T>` (`enumctor.rs:96-98`) — `T: FromDynamic + ToDynamic + Debug + 'static`에 대해 `UserData` 구현. 메타메서드 `__call`과 `__index`로 Lua 측 enum 생성 문법 제공.
  - `pub fn new() -> Self` / `impl Default`.

## 4. 내부 구조

모듈 구성은 단 두 개다. 크레이트 규모가 작아(약 645행) 모듈 분해의 여지는 제한적이다.

- **`lib.rs`** (약 445행) — 변환 함수, 매크로, `ValuePrinter`. 이 크레이트에서 가장 비대한 단일 파일이며, 대부분의 부피는 `ValuePrinterHelper`의 `Debug` 구현(`lib.rs:333-444`)이 차지한다.
- **`enumctor.rs`** (약 200행) — `Enum<T>` / `EnumVariant<T>` 생성자 로직.

### 제어/데이터 흐름

**Rust → Lua (`to_lua` / `dynamic_to_lua_value`)**: `ToDynamic`로 동적 값 획득 → `dynamic_to_lua_value`가 `DynValue` 트리를 재귀 순회하며 Lua 원시값/테이블로 변환. `Array`는 1-기반 인덱스 테이블, `Object`는 키-값 테이블로 매핑(`lib.rs:74-90`).

**Lua → Rust (`from_lua` / `lua_value_to_dynamic`)**: `lua_value_to_dynamic`가 `visited` 집합으로 테이블 포인터를 추적하며 재귀(`lib.rs:99-239`). 이미 방문한 테이블은 `Null`로 절단해 무한 루프를 방지. 테이블은 **첫 인덱스(`1`) 존재 여부**로 배열/객체를 판별(`lib.rs:192`). 배열 판별 시 모든 키가 `1..=len` 정수인지 검증한다.

**userdata 처리**: Lua userdata는 메타테이블의 `__wezterm_to_dynamic` 함수를 우선 호출, 없으면 `__tostring`을 호출해 그 결과를 재귀 변환(`lib.rs:127-167`). light userdata 중 널 포인터는 특수 `Null` 표현으로 매핑(`lib.rs:119`).

**enum 생성자 흐름**(`enumctor.rs`):
- `Enum<T>::__call`: 전달된 테이블을 `from_dynamic`으로 검증(미지 필드 거부, deprecated 경고)한 뒤 다시 Lua 값으로 환원(`enumctor.rs:126-137`). 구버전 호환 경로.
- `Enum<T>::__index`: 3단계. (1) 필드명이 단위 변형(unit variant)이면 문자열 그대로 반환(`enumctor.rs:142-152`). (2) 빈 객체로 기본 생성 가능하면 메타테이블(`__call`) 부착 테이블 반환(`enumctor.rs:161-188`). (3) 인자가 필수면 `EnumVariant<T>` userdata를 반환해 후속 `__call`을 대기(`enumctor.rs:192-196`).
- `EnumVariant<T>::call_impl`: `{변형명 = 페이로드}` 객체를 구성해 `from_dynamic` 검증 후 Lua 값으로 환원(`enumctor.rs:33-49`).

## 5. 핵심 데이터 구조·타입

| 타입 | 위치 | 불변식 / 핵심 속성 |
| --- | --- | --- |
| `ValueLua` | `lib.rs:251` | `value` 필드 하나만 가진 투명 래퍼. `FromDynamic`/`ToDynamic` 자동 파생과 `impl_lua_conversion_dynamic!`로 Lua 변환 가능. |
| `ValuePrinter<'lua>` | `lib.rs:257` | Lua 값을 감싸는 newtype. `Debug`만 구현. 출력은 결정적이어야 한다(맵 키를 동적 값 순서로 정렬, `lib.rs:394`). |
| `ValuePrinterHelper<'lua>` | `lib.rs:271` (비공개) | `visited: Rc<RefCell<HashSet<usize>>>`로 순환을 감지. `is_cycle`이 참이면 포인터 주소만 출력. `Ord`/`Eq`는 `lua_value_to_dynamic`로 환산한 동적 값 비교에 의존(`lib.rs:291-297`). 키 타입이 내부 가변성(`Rc<RefCell>`)을 가지므로 `BTreeMap` 대신 `Vec` 후 정렬 방식을 사용(`mutable_key_type` 린트 회피, 주석 `lib.rs:365-369`). |
| `Enum<T>` | `enumctor.rs:96` | `PhantomData<T>`만 보유, 무상태. `unsafe impl Send`(PhantomData 전용이므로 안전, 주석 `enumctor.rs:100-101`). `T: FromDynamic+ToDynamic+Debug+'static` 경계. |
| `EnumVariant<T>` | `enumctor.rs:10` (비공개) | `variant: String`과 `PhantomData<T>` 보유. 인자가 필요한 변형의 지연 생성자. `unsafe impl Send`(동일 근거). |

핵심 불변식:
- **순환 안전성**: `lua_value_to_dynamic`와 `ValuePrinterHelper`는 테이블 포인터(`to_pointer() as usize`) 집합으로 순환을 차단한다. 이 추적이 깨지면 무한 재귀 위험.
- **배열/객체 판별 규칙**: 키 `1`의 존재(`lua_value_to_dynamic`) 또는 모든 키가 연속 정수(`is_array_style_table`, `lib.rs:307-331`)로 판단. 두 함수의 판별 기준이 미묘하게 다르다(전자는 키 `1`만 검사, 후자는 연속성 전체 검사).
- **enum 검증 옵션**: 생성자 경로는 `UnknownFieldAction::Deny`(미지 필드 거부) + deprecated 필드는 경고 또는 무시로 고정(`enumctor.rs:41-46`, `144-147`, `163-166`).

## 6. 외부 의존성

- **`mlua`** — Lua 5.4 인터프리터 바인딩. 이 크레이트 존재 이유의 절반. `Value`/`Table`/`UserData`/`MetaMethod`/`Function` 등 Lua 측 모든 타입을 제공. `pub use`로 재노출하여 매크로와 소비 크레이트가 버전 일관성을 유지.
- **`wezterm-dynamic`** — 변환의 중간 표현(IR). `Value`(Null/Bool/String/U64/F64/I64/Array/Object), `FromDynamic`/`ToDynamic` 트레이트, `FromDynamicOptions`, `UnknownFieldAction`, `DynError`. Lua와 Rust 사이의 공통 데이터 어휘.
- **`log`** — 진단 로깅. `ValuePrinterHelper`가 맵 항목 조회에 실패할 때만 사용(`lib.rs:389`).

GPU·폰트 셰이핑·네트워크 등 무거운 외부 크레이트 의존은 없다. 순수 데이터 변환 크레이트다.

## 7. 설정·기능 플래그

- **feature flag 없음**: `luahelper/Cargo.toml`에 `[features]` 섹션이 없다. 조건부 컴파일 분기도 코드에 존재하지 않는다.
- **관련 config 항목**: 이 크레이트 자체는 설정값을 정의하지 않는다. 다만 `config` 크레이트의 모든 Lua 직렬화 가능 타입(`Config`, `KeyAssignment`, 색상·폰트·배경 등)이 `impl_lua_conversion_dynamic!`를 통해 이 크레이트에 의존한다. 즉 "소스 기본값 수정" 철학에서 설정 타입을 Lua로 노출/수용하는 **하부 변환 메커니즘**이 이 크레이트다.
- **enum 검증 정책**: 코드에 하드코딩된 정책. enum 생성자는 미지 필드를 거부(`Deny`)하므로, Lua 설정에서 오타나 존재하지 않는 필드는 즉시 오류가 된다.

## 8. Windows 전용 고려사항

- **플랫폼 분기 없음**: `cfg(windows)`, `cfg(unix)`, `cfg(target_os = ...)` 등의 조건부 컴파일이 소스에 전혀 없다.
- **Windows API 사용 없음**: `winapi`, `windows`, FFI 호출, 경로·파일 시스템 접근이 없다. 순수 인메모리 변환.
- **죽은 코드 없음**: Windows fork 전환으로 발생한 죽은 경로가 이 크레이트에는 존재하지 않는다. upstream과 코드가 동일하게 유지될 가능성이 높은 안정적 크레이트다.
- 결론: 이 크레이트는 Windows 전용 분기와 무관하다. 플랫폼 정리 작업에서 손댈 부분이 없다.

## 9. 리팩토링 주의점

- **광범위한 피의존**: `impl_lua_conversion_dynamic!` 매크로는 20개 파일에서 사용된다(섹션 2 검색 결과). 매크로 시그니처나 `to_lua`/`from_lua`의 동작·오류 형식을 바꾸면 `config`, `mux`, 모든 `lua-api-crates`, `wezterm-gui`로 파급된다. **이 크레이트의 공개 API는 사실상 동결 인터페이스로 취급해야 한다.**
- **순환 의존 위험은 낮음**: 의존이 `wezterm-dynamic`/`mlua`/`log`로 단순하고, 소비 크레이트를 역참조하지 않으므로 순환은 없다.
- **순환 참조 처리의 불변식**: `lua_value_to_dynamic`(`visited` 인자)와 `ValuePrinterHelper`(`Rc<RefCell<HashSet>>`)가 서로 다른 방식으로 순환을 추적한다. 리팩토링 시 두 경로 모두 포인터 기반 방문 추적을 유지해야 무한 재귀를 막을 수 있다.
- **배열/객체 판별의 불일치**: `lua_value_to_dynamic`(`lib.rs:192`, 키 `1` 검사)와 `is_array_style_table`(`lib.rs:307`, 연속성 검사)의 판별 기준이 다르다. 전자는 변환 경로, 후자는 출력 포맷 경로에 쓰인다. 통합하려면 빈 테이블·희소 배열·혼합 키 케이스에서 의미 차이를 정밀 검증해야 한다.
- **`unsafe impl Send`**: `Enum<T>`/`EnumVariant<T>`의 `Send` 구현은 `T`가 `PhantomData`에만 등장한다는 전제(`enumctor.rs:15-17`, `100-101`)에 의존한다. 필드에 `T` 인스턴스를 추가하면 이 전제가 깨지므로 동시성 안전성을 재검토해야 한다.
- **`from_lua` vs `from_lua_value_dynamic` 중복**: 두 함수 모두 `lua_value_to_dynamic` + `from_dynamic`을 수행하나 오류 래핑·옵션이 미묘하게 다르다. 통합 시 호출처(`mux/domain.rs`는 후자, 매크로는 전자)의 오류 메시지 형식 변화에 주의.
- **`ValuePrinter`의 결정성**: 테스트가 정렬된 맵 출력에 의존한다(주석 `lib.rs:365-369`). 정렬 로직(`Ord` via 동적 값 비교)을 바꾸면 스냅샷성 테스트가 깨질 수 있다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `luahelper/Cargo.toml` | 13 | 패키지 정의. deps: `log`, `mlua`, `wezterm-dynamic`. `publish = false`, edition 2018. |
| `luahelper/src/lib.rs` | 445 | 핵심 변환 함수(`to_lua`/`from_lua`/`dynamic_to_lua_value`/`lua_value_to_dynamic`/`from_lua_value_dynamic`), `impl_lua_conversion_dynamic!` 매크로, `ValueLua`, `ValuePrinter`(+ 비공개 `ValuePrinterHelper`). `mlua` 재노출. |
| `luahelper/src/enumctor.rs` | 200 | Lua enum 생성자. `Enum<T>`(공개) / `EnumVariant<T>`(비공개). 메타메서드 `__call`/`__index`로 `wezterm.action.X{...}` 문법 구현. |

생성 파일(생성 데이터 테이블)은 이 크레이트에 없다.
