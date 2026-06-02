# wezterm-dynamic 폴더 기능 명세

본 문서는 `E:\Project\wezterm\wezterm-dynamic` 폴더에 포함된 두 크레이트(`wezterm-dynamic`, `wezterm-dynamic-derive`)의 기능 명세다. 추후 리팩토링을 위한 상세 참조 문서이며, 모든 기술은 실제 소스 코드에 근거한다.

본 저장소는 WezTerm의 Windows 전용 영구 분기다. 다만 이 폴더의 두 크레이트는 플랫폼 의존 코드를 거의 포함하지 않으며, 분기점은 OS가 아니라 `std`/`no_std` feature다(아래 7·8장 참조).

---

## 1. 개요 및 책임

`wezterm-dynamic`은 Rust 타입을 **JSON/Lua 유사 동적 값**(`Value`)으로 직렬화·역직렬화하는 자체 직렬화 계층이다. serde를 대체하는 워크스페이스 내부 표준이며, 단일 책임은 다음과 같다.

- Rust 타입 ↔ `Value`(동적 데이터 모델) 양방향 변환을 위한 두 트레이트(`ToDynamic`, `FromDynamic`) 정의 (`wezterm-dynamic/src/todynamic.rs:19`, `wezterm-dynamic/src/fromdynamic.rs:50`).
- 기본 타입·표준 컨테이너에 대한 트레이트 구현 제공.
- 풍부한 진단 오류 타입(`Error`)과 필드명 오타 추정 기능 제공 (`wezterm-dynamic/src/error.rs:29`).

`wezterm-dynamic-derive`는 위 두 트레이트를 사용자 정의 struct/enum에 대해 자동 생성하는 **proc-macro 파생 크레이트**다. 책임은 `#[derive(ToDynamic)]`·`#[derive(FromDynamic)]` 코드 생성과 `#[dynamic(...)]` 속성 해석에 한정된다 (`wezterm-dynamic/derive/src/lib.rs:9`, `:16`).

이 분기의 설정 철학과 직결된다. RUNE은 `wezterm.lua`보다 `config` 크레이트의 소스 기본값 수정을 우선하는데, `config`의 설정 구조체 전부가 이 크레이트의 파생 매크로로 `Value`와 연결된다. 즉 이 폴더는 **Lua 설정 값과 Rust 설정 구조체 사이의 변환 엔진**이며, 설정 파이프라인의 최하위 토대다. serde가 아닌 이 자체 계층을 쓰는 이유는 Lua 테이블처럼 키 타입이 자유롭고(문자열 외 키 허용), 알 수 없는 필드에 대한 경고/오타 추정 등 설정 친화적 진단을 직접 제어하기 위함이다.

---

## 2. 워크스페이스 내 위치

### 2.1 `wezterm-dynamic`

| 구분 | 크레이트 |
|---|---|
| 의존(deps) | wezterm-dynamic-derive |
| 피의존(usedBy) | battery, color-funcs, config, luahelper, mux, mux-lua, plugin, procinfo, serde-funcs, termwiz, termwiz-funcs, wezterm-bidi, wezterm-cell, wezterm-client, wezterm-color-types, wezterm-escape-parser, wezterm-gui, wezterm-input-types, wezterm-surface, wezterm-term, window-funcs |

### 2.2 `wezterm-dynamic-derive`

| 구분 | 크레이트 |
|---|---|
| 의존(deps) | 없음 |
| 피의존(usedBy) | wezterm-dynamic |

`wezterm-dynamic`은 워크스페이스 의존 그래프의 거의 최하층 토대(foundation) 크레이트다. 설정·색·셀·이스케이프 파서·터미널 모델·Lua API 등 광범위한 상위 크레이트가 직접 의존하며, 그 자신은 파생 매크로 하나에만 의존한다. 파생 크레이트는 `wezterm-dynamic`이 `pub use`로 재노출(`wezterm-dynamic/src/lib.rs:22`)하므로 소비처는 `wezterm-dynamic` 하나만 가져오면 트레이트와 derive를 함께 사용한다.

---

## 3. 공개 API 표면

### 3.1 `wezterm-dynamic` (재노출 — `wezterm-dynamic/src/lib.rs:16`~`:22`)

핵심 트레이트:

- `trait ToDynamic { fn to_dynamic(&self) -> Value; }` — 자기 자신을 `Value`로 직렬화 (`todynamic.rs:19`).
- `trait FromDynamic { fn from_dynamic(value: &Value, options: FromDynamicOptions) -> Result<Self, Error>; }` — `Value`에서 자기 자신을 복원 (`fromdynamic.rs:50`).
- `trait PlaceDynamic { fn place_dynamic(&self, place: &mut Object); }` — flatten 처리를 위해 자신을 대상 `Object`에 직접 적재. 파생 구현이 내부적으로 사용하며 직접 소비하지 않음 (`todynamic.rs:28`).
- `trait ObjectKeyTrait { fn key<'k>(&'k self) -> BorrowedKey<'k>; }` — 할당 없는 객체 키 조회용 (`object.rs:22`).

핵심 타입:

- `enum Value` — 동적 값 모델 (8개 variant). `variant_name()`, `coerce_unsigned()`, `coerce_signed()`, `coerce_float()` 제공 (`value.rs:19`, `:48`~`:94`).
- `struct Array` — `Vec<Value>` 래퍼 (`array.rs:12`).
- `struct Object` — `BTreeMap<Value, Value>` 래퍼. `get_by_str(&str) -> Option<&Value>` 제공 (`object.rs:74`, `:79`).
- `enum BorrowedKey<'a> { Value(&'a Value), Str(&'a str) }` — 차용 키 (`object.rs:17`).
- `enum Error` — `#[non_exhaustive]` 진단 오류 (`error.rs:29`). 공개 메서드: `warn`, `capture_warnings`, `raise_deprecated_fields`, `raise_unknown_fields`, `field_context` (`error.rs:87`~`:363`). `trait WarningCollector`도 `std`에서 노출 (`error.rs:18`).
- `struct FromDynamicOptions { unknown_fields, deprecated_fields }` + `enum UnknownFieldAction { Ignore, Warn(기본), Deny }`. `FromDynamicOptions::flatten()` 제공 (`fromdynamic.rs:21`~`:46`).

파생 매크로(재노출): `FromDynamic`, `ToDynamic`.

### 3.2 기본 구현 제공 타입

`ToDynamic`/`FromDynamic`은 다음에 대해 미리 구현된다(`todynamic.rs`, `fromdynamic.rs`): `Value`, `bool`, `str`/`String`, `char`, 정수 전체(`i8`~`i64`/`isize`, `u8`~`u64`/`usize`), `f32`/`f64`, `()`, `Option<T>`, `Box<T>`, `Arc<T>`, `Vec<T>`, `[T; N]`, `BTreeMap<K, V>`, `ordered_float::NotNan<f64>`. `std` feature에서는 추가로 `HashMap<K, V>`, `std::path::PathBuf`, `std::time::Duration`(초 단위 f64)도 구현된다.

### 3.3 `wezterm-dynamic-derive`

`proc-macro` 크레이트로 직접 호출하는 함수형 API는 없다. 외부 표면은 두 derive뿐이다:

- `#[proc_macro_derive(ToDynamic, attributes(dynamic))]`
- `#[proc_macro_derive(FromDynamic, attributes(dynamic))]`

파생된 코드는 추가로 다음 보조 항목을 생성한다: struct/enum의 `FromDynamic`에서 `possible_field_names()`(struct, `const fn`), `variants()`(enum) (`derive/src/fromdynamic.rs:127`, `:321`).

---

## 4. 내부 구조

### 4.1 `wezterm-dynamic` 모듈 분해 (`src/`)

- `lib.rs` — 크레이트 루트. `no_std` 조건부 설정과 `pub use` 재노출만 담당 (23행).
- `value.rs` — `Value` enum 정의, `Debug`, 강제 변환(coerce) 메서드.
- `array.rs` — `Array` 래퍼. `Deref`/`DerefMut`로 `Vec<Value>` 위임, 사용자 정의 `Drop`.
- `object.rs` — `Object` 래퍼와 차용 키 인프라(`BorrowedKey`, `ObjectKeyTrait`).
- `todynamic.rs` — `ToDynamic`/`PlaceDynamic` 트레이트 + 기본 타입 구현.
- `fromdynamic.rs` — `FromDynamic` 트레이트, 옵션 타입 + 기본 타입 구현.
- `error.rs` — `Error` enum, 경고 수집기, 오타 추정(`possible_matches`), 필드 컨텍스트 부착. 이 크레이트에서 가장 로직이 밀집된 모듈(371행).
- `drop.rs` — 비재귀(스택 기반) drop 헬퍼.

데이터 흐름은 두 방향이다. 직렬화(`ToDynamic`)는 Rust 값 → `Value` 트리 단방향. 역직렬화(`FromDynamic`)는 `Value` 트리 + `FromDynamicOptions` → Rust 값이며, 실패 시 `Error`가 상향 전파되면서 `field_context()`로 필드/타입 경로가 누적된다(`error.rs:303`). 이 경로 누적이 중첩 설정 구조에서 정확한 오류 위치를 가리키는 핵심 메커니즘이다.

### 4.2 `wezterm-dynamic-derive` 모듈 분해 (`derive/src/`)

- `lib.rs` — 두 derive 진입점. 파싱·코드 생성을 하위 모듈에 위임, 오류는 `to_compile_error()`로 변환.
- `attr.rs` — 속성 파서. `ContainerInfo`(컨테이너 수준), `FieldInfo`(필드 수준) 구조체와, 필드별 `ToDynamic`/`FromDynamic` 토큰 생성기(`FieldInfo::to_dynamic`, `gen_from_dynamic`). 파생 로직의 무게 중심(374행).
- `todynamic.rs` — `ToDynamic` 구현 코드 생성(struct·enum, into 변환, 변형별 처리).
- `fromdynamic.rs` — `FromDynamic` 구현 코드 생성(struct·enum, try_from, flatten 시 unknown_fields 비활성화 등).
- `bound.rs` — 제네릭 타입 파라미터마다 트레이트 바운드를 부착하는 where절 빌더(16행).

### 4.3 파생 코드 생성 흐름

derive 진입점이 `syn::DeriveInput`을 받아 struct/enum/union을 분기한다. union은 항상 오류, struct는 named field만 지원(tuple/unit struct는 오류). 컨테이너 속성(`container_info`)과 필드 속성(`field_info`)을 해석한 뒤, struct는 필드별 토큰을 합쳐 `impl` 블록을, enum은 variant별(Unit/Named/Unnamed) 매칭 코드를 합쳐 `impl`을 `quote!`로 조립한다. `#[dynamic(debug)]`가 있으면 생성 코드를 `eprintln!`으로 출력한다(`todynamic.rs:79`, `fromdynamic.rs:133`).

---

## 5. 핵심 데이터 구조·타입

### 5.1 `Value` (`value.rs:19`)

```
enum Value { Null, Bool(bool), String(String), Array(Array), Object(Object),
             U64(u64), I64(i64), F64(OrderedFloat<f64>) }
```

불변식·특성:

- `F64`가 `f64`가 아니라 `OrderedFloat<f64>`인 이유는 `Value`가 `Hash + Eq + Ord`를 파생해야 하기 때문(`value.rs:17`). `f64`는 `Eq`/`Ord`가 없으므로 `Object`의 키로 쓸 수 없다.
- `Default`는 `Null`(`value.rs:20`).
- `Debug`는 수동 구현이며 `Null`을 `nil`로 출력해 Lua 표기와 정합(`value.rs:36`).
- 숫자 강제 변환: `coerce_*`는 `U64`/`I64`/`F64` 간 무손실 변환만 허용. 부동소수의 경우 `fract()==0.0`이고 대상 정수 범위 내일 때만 정수로 강제한다(`value.rs:65`, `:78`).

### 5.2 `Array` (`array.rs:12`) / `Object` (`object.rs:74`)

- `Array` = `Vec<Value>` 단일 필드 래퍼, `Object` = `BTreeMap<Value, Value>` 단일 필드 래퍼.
- 둘 다 `Deref`/`DerefMut`로 내부 컨테이너 메서드를 그대로 노출하므로 사용처에서는 표준 컬렉션처럼 다룬다.
- **`Ord`/`PartialOrd`가 의도적으로 포인터 주소 비교다**(`array.rs:16`, `object.rs:91`). 내용 기반 정렬이 아니다. 이는 `Value::Ord` 파생을 만족시키되 컬렉션 내용 전체를 비교하는 비용을 회피하기 위한 선택이다. 결과적으로 `Array`/`Object`를 직접 `Object` 키로 쓰면 동일 내용이라도 인스턴스별로 다른 정렬 순서를 가지므로, 키로 사용하는 것은 사실상 비결정적이다. 이는 변경 시 반드시 인지해야 할 불변식이다.

### 5.3 `BorrowedKey` / `ObjectKeyTrait` (`object.rs:17`~`:71`)

`Object`에서 문자열 필드를 조회할 때 `Value::String`을 새로 할당하지 않기 위한 차용 키 패턴이다. `Borrow<dyn ObjectKeyTrait>`를 `Value`에 구현하여, `&BorrowedKey::Str(field_name) as &dyn ObjectKeyTrait`로 `BTreeMap` 조회를 수행한다(`object.rs:79`). `dyn ObjectKeyTrait`에 대해 `PartialEq`/`Eq`/`Ord`/`Hash`를 수동 구현해 trait object 비교가 `key()` 기준으로 일치하도록 보장한다. 불변식: `Value::String`의 키 표현과 `BorrowedKey::Str`의 키 표현이 동일해야 조회가 성립한다(`object.rs:27`~`:32`).

### 5.4 `Error` (`error.rs:29`)

`#[non_exhaustive]` enum. variant는 잘못된 enum variant, 미지·부적격·폐기 필드, 변환 실패, 배열 크기 불일치, char 변환 실패, enum 키 개수 오류, 필드 내부 오류(`ErrorInField`)와 중첩 필드 오류(`ErrorInNestedField`) 등. `ErrorInField`→`ErrorInNestedField` 승격(`field_context`, `error.rs:333`)으로 `a.b.c` 형태의 경로를 누적한다. leaf 오류에 한해 대상 객체를 컨텍스트로 덧붙이되, 출력이 128자 초과거나 10줄 초과면 생략한다(`error.rs:310`~`:324`).

### 5.5 `FromDynamicOptions` / `UnknownFieldAction` (`fromdynamic.rs:21`~`:46`)

`unknown_fields`·`deprecated_fields` 두 정책 필드를 갖는 `Copy` 구조체. 기본값은 둘 다 `Warn`. `flatten()`은 `unknown_fields`를 `Ignore`로 낮춘 사본을 반환한다 — flatten 시 어느 필드가 어느 하위 구조에 속하는지 알 수 없어 오탐을 막기 위함이다(`fromdynamic.rs:40`, 파생 측 `fromdynamic.rs:56`).

---

## 6. 외부 의존성

### 6.1 `wezterm-dynamic`

| 크레이트 | 용도 |
|---|---|
| `ordered-float` (`libm` feature) | `Value::F64`를 `OrderedFloat<f64>`로 보관해 `Hash`/`Eq`/`Ord` 성립. `libm`으로 `no_std`에서도 float 연산 가능. `NotNan<f64>` 변환 구현 대상이기도 함 |
| `thiserror` (2.0, `default-features=false`) | `Error` enum의 `Display`/오류 파생. `std` feature와 연동(`std=["thiserror/std", ...]`) |
| `strsim` (선택, `std` 시) | `possible_matches`에서 `jaro_winkler` 유사도로 오타 필드명 후보 추정(`error.rs:242`) |
| `log` | 경고 수집기 미설정 시 `log::warn!`으로 폴백(`error.rs:93`) |
| `wezterm-dynamic-derive` | 파생 매크로(필수, 재노출) |
| `maplit` (dev) | 테스트에서 `btreemap!` 매크로 |

### 6.2 `wezterm-dynamic-derive`

| 크레이트 | 용도 |
|---|---|
| `proc-macro2` | proc-macro 토큰 스트림 추상화 |
| `quote` | 코드 생성 템플릿(`quote!`) |
| `syn` (`extra-traits` feature) | derive 입력 파싱(`DeriveInput`, `Meta`, `Path` 등). `extra-traits`로 `Debug`/`PartialEq` 등 사용 |

---

## 7. 설정·기능 플래그

### 7.1 `std` feature (`wezterm-dynamic/Cargo.toml:9`)

이 크레이트의 유일한 feature이자 동작의 주요 분기축이다. `std = ["thiserror/std", "dep:strsim", "ordered-float/std"]`.

- 기본은 `no_std`(`lib.rs:4` `#![cfg_attr(not(feature = "std"), no_std)]`). `extern crate alloc`로 `String`/`Vec`/`BTreeMap` 등 alloc 타입 사용.
- `std` 활성 시에만 제공: `WarningCollector` 트레이트, thread-local 경고 수집기, `Error::warn`/`capture_warnings`/`set_warning_collector`/`clear_warning_collector`, `strsim` 기반 오타 추정(`possible_matches`가 빈 문자열 대신 실제 후보 반환), `HashMap`/`PathBuf`/`Duration` 트레이트 구현.

워크스페이스 루트는 `wezterm-dynamic`을 `default-features=false`로 선언한다(`Cargo.toml:205`). `std`는 소비처가 선택 활성화한다: `termwiz`, `term`, `procinfo`는 명시적으로 `features=["std"]`로 켜고, `wezterm-cell`/`wezterm-surface`/`wezterm-escape-parser`는 자신의 `std` feature가 `wezterm-dynamic/std`를 체인한다. `config`는 `workspace = true`만 사용한다.

`wezterm-dynamic-derive`는 feature가 없다.

### 7.2 `#[dynamic(...)]` 속성 (파생 매크로 설정면)

파생 코드 생성을 제어하는 사실상의 "설정 항목"이다. `attr.rs`가 해석한다.

컨테이너 수준(`container_info`, `attr.rs:11`):
- `into = "T"` — `ToDynamic`을 `Self → T → Value`로 위임.
- `try_from = "T"` — `FromDynamic`을 `Value → T → Self`(`TryFrom`)로 위임.
- `debug` — 생성 코드를 stderr에 출력.

필드 수준(`field_info`, `attr.rs:282`):
- `rename = "name"` — 직렬화 키 이름 변경.
- `skip` — 직렬화/역직렬화에서 제외(역직렬화 시 `Self::default()`로 채움 → struct에 `Default` 필요, `fromdynamic.rs:37`·`:74`).
- `flatten` — 하위 구조를 상위 객체에 평탄화. `PlaceDynamic` 사용, unknown_fields 경고 비활성화.
- `default` / `default = "fn"` — 필드 부재 시 `Default::default()` 또는 지정 함수 호출.
- `deprecated = "reason"` — 필드 사용 시 정책에 따라 경고/오류.
- `into = "T"` / `try_from = "T"` — 필드 단위 변환.
- `validate = "fn"` — 변환 후 검증 함수 호출, 실패 시 `ErrorInField`.

---

## 8. Windows 전용 고려사항

이 폴더의 두 크레이트는 **OS 플랫폼 분기를 포함하지 않는다.** `cfg(unix)`/`cfg(windows)`/Windows API 호출이 전혀 없다. 따라서 본 분기가 비Windows 코드를 제거한 작업의 영향을 직접 받지 않았다.

플랫폼 대신 `cfg(feature = "std")` / `cfg(not(feature = "std"))`로만 분기한다. Windows 빌드에서는 상위 소비처(`config`, `termwiz`, `term` 등)가 `std`를 활성화하므로 실질적으로 `std` 경로가 동작한다. `no_std` 경로(`possible_matches`가 빈 문자열 반환, 경고 수집기 부재 등)는 이 분기의 정상 GUI 빌드에서는 사용되지 않으나, alloc 전용 환경을 위한 살아 있는 대체 경로다(죽은 코드가 아님 — `wezterm-cell`/`wezterm-escape-parser` 등이 `no_std` 빌드 가능성을 보존).

요컨대 Windows 전용 분기에서 이 폴더는 플랫폼 중립이며, 관심 분기점은 `std` feature 하나다.

---

## 9. 리팩토링 주의점

1. **워크스페이스 최하층 토대.** 21개 크레이트가 직접 의존하므로 `Value`·`ToDynamic`·`FromDynamic`의 시그니처 변경은 광범위한 파급을 일으킨다. 트레이트 메서드 시그니처(특히 `from_dynamic`의 `FromDynamicOptions` 인자)는 사실상 동결로 취급해야 한다.
2. **파생 크레이트와의 강결합.** 파생 코드는 `wezterm_dynamic::` 경로의 항목명(`Error`, `Object`, `Value`, `PlaceDynamic`, `BorrowedKey`, `ObjectKeyTrait`, `raise_unknown_fields`, `raise_deprecated_fields`, `field_context` 등)을 문자열 토큰으로 직접 참조한다. `wezterm-dynamic`의 공개 항목명을 바꾸면 컴파일은 통과해도 파생 산출물이 깨진다. 두 크레이트는 항상 동반 변경해야 한다.
3. **`Array`/`Object`의 포인터 비교 `Ord`.** 5.2 참조. 내용 기반 비교로 보이지만 주소 비교다. 변경하면 `Value`의 `Ord` 의미가 바뀌고, 이를 키로 쓰는 코드의 동작이 달라진다. 성능·결정성 트레이드오프를 이해하지 않은 변경은 금지.
4. **비재귀 `Drop`.** `drop::safely`는 깊게 중첩된 `Value` 트리에서 재귀 drop으로 인한 스택 오버플로를 막는 dtolnay miniserde 유래 코드다(`drop.rs`). `Array`/`Object`의 `Drop` 구현은 `ManuallyDrop` + `ptr::read`로 내부를 빼낸 뒤 스택 기반으로 해제한다(`array.rs:42`, `object.rs:133`). 이 `unsafe` 패턴은 정확히 한 번만 내부를 읽어야 안전하다 — 임의 수정 금지.
5. **trait object 키 인프라의 정합성.** `ObjectKeyTrait`/`BorrowedKey`/`Borrow` 구현은 상호 일관성에 의존한다(`Value::String`과 `BorrowedKey::Str`의 키 동치). 한쪽만 바꾸면 `get_by_str` 조회가 조용히 실패한다.
6. **`std`/`no_std` 이중 경로.** 새 트레이트 구현이나 기능을 추가할 때 `no_std` 빌드를 깨뜨리지 않도록 `cfg(feature="std")` 게이팅을 일관되게 유지해야 한다. `std` 전용 타입(`HashMap`/`PathBuf`/`Duration`)은 반드시 게이팅 대상이다.
7. **`Error`는 `#[non_exhaustive]`.** variant 추가는 하위호환이지만, 소비처의 `match`가 와일드카드를 갖는다는 전제다. variant 제거·필드 변경은 호환성 파괴.
8. **순환 의존 없음.** `wezterm-dynamic → wezterm-dynamic-derive` 단방향. derive는 무의존. 구조적으로 건전하다.
9. **`unwrap()` 주의 지점.** `ToDynamic for isize`/`usize`는 `try_into().unwrap()`을 호출한다(`todynamic.rs:158`, `:188`). 64비트 Windows에서는 `i64`/`u64`와 폭이 같아 실패하지 않으나, 폭 가정이 코드에 암묵적이다.

---

## 10. 파일별 요약

### 10.1 `wezterm-dynamic/`

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `Cargo.toml` | 21 | 패키지·`std` feature·의존성 선언 |
| `src/lib.rs` | 23 | 크레이트 루트, `no_std` 설정, 공개 항목 재노출 |
| `src/value.rs` | 96 | `Value` enum, `Debug`, `variant_name`/`coerce_*` |
| `src/array.rs` | 104 | `Array` 래퍼(`Deref`, `Drop`, iter, 포인터 비교 `Ord`) |
| `src/object.rs` | 175 | `Object` 래퍼 + 차용 키 인프라(`BorrowedKey`/`ObjectKeyTrait`) |
| `src/todynamic.rs` | 227 | `ToDynamic`/`PlaceDynamic` 트레이트 + 기본 타입 구현 |
| `src/fromdynamic.rs` | 300 | `FromDynamic` 트레이트, 옵션 타입 + 기본 타입 구현 |
| `src/error.rs` | 371 | `Error` enum, 경고 수집, 오타 추정, 필드 컨텍스트(최대 모듈) |
| `src/drop.rs` | 35 | 비재귀 스택 기반 drop 헬퍼 |
| `tests/todynamic.rs` | 222 | `ToDynamic` 파생 동작 테스트(rename/skip/flatten/into 등) |
| `tests/fromdynamic.rs` | 291 | `FromDynamic` 파생 동작 테스트(default/flatten/try_from 등) |

### 10.2 `wezterm-dynamic/derive/`

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `Cargo.toml` | 16 | proc-macro 크레이트 선언, syn/quote/proc-macro2 의존 |
| `src/lib.rs` | 22 | 두 derive 진입점 |
| `src/attr.rs` | 374 | `#[dynamic(...)]` 속성 파싱 + 필드별 토큰 생성(파생 무게중심) |
| `src/todynamic.rs` | 213 | `ToDynamic`/`PlaceDynamic` 구현 코드 생성(struct·enum) |
| `src/fromdynamic.rs` | 334 | `FromDynamic` 구현 코드 생성(struct·enum, try_from/flatten) |
| `src/bound.rs` | 16 | 제네릭 파라미터에 트레이트 바운드 부착 where절 빌더 |

생성 데이터 테이블([생성]) 해당 파일은 이 폴더에 없다.
