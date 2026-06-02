# wezterm-version 폴더 기능 명세

## 1. 개요 및 책임

`wezterm-version`은 **빌드 시점에 확정된 버전 문자열과 타깃 트리플을 런타임에 정적 문자열로 제공**하는 단일 책임 크레이트다. 책임 범위는 두 개의 함수로 한정된다.

- `wezterm_version()` — 빌드 당시 결정된 버전 태그(예: `20240101-120000-abcdef12`)를 반환한다.
- `wezterm_target_triple()` — 컴파일된 대상 플랫폼 트리플(예: `x86_64-pc-windows-msvc`)을 반환한다.

핵심은 `build.rs`(빌드 스크립트)에 있다. 컴파일 시점에 버전 정보를 결정하여 `WEZTERM_CI_TAG`·`WEZTERM_TARGET_TRIPLE` 두 환경 변수를 `rustc`에 주입하고, `lib.rs`는 `env!` 매크로로 그 값을 정적 문자열 상수로 인라인한다. 따라서 런타임에는 어떤 계산도 발생하지 않으며, 반환되는 `&'static str`은 바이너리에 박혀 있는 불변 데이터다.

Windows fork에서의 실제 책임은 변하지 않는다. 버전 표시(`wezterm -h`, `TERM_PROGRAM_VERSION` 환경 변수, Lua `wezterm.version`·`wezterm.target_triple`, mux/ssh 핸드셰이크 시 자기 식별 등)의 단일 진실 출처(single source of truth)다. 이 크레이트 자체에는 플랫폼 분기가 없다. 타깃 트리플 값은 빌드 환경의 `TARGET` 환경 변수에서 자동으로 채워지므로, Windows에서 빌드하면 자연히 `*-windows-*` 트리플이 박힌다.

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 | 비고 |
| --- | --- | --- |
| 의존(deps) | 없음 | 런타임 의존성 없음. `build.rs`만 `git2`를 빌드 의존성으로 사용 |
| 빌드 의존(build-deps) | `git2` (`default-features = false`) | 빌드 시점 git 정보 조회용. 런타임 바이너리에는 링크되지 않음 |
| 피의존(usedBy) | `env-bootstrap` | `bootstrap()`에서 두 함수를 호출해 `config`로 값 전달 |

이 크레이트는 의존성 그래프의 **최하단 잎(leaf) 노드**다. 다른 크레이트를 전혀 끌어오지 않으며, 버전·트리플이라는 빌드 메타데이터를 그래프 상위로 단방향 공급한다. 소비 경로는 `wezterm-version` → `env-bootstrap::bootstrap()` → `config::assign_version_info()`로 이어지며, 이후 `config` 크레이트가 워크스페이스 전역(GUI·mux·ssh·Lua)에 버전 정보를 재배포한다(`config/src/version.rs:6`).

## 3. 공개 API 표면

`src/lib.rs` 전체가 공개 표면이며, 함수 2개로 끝난다.

```rust
pub fn wezterm_version() -> &'static str
pub fn wezterm_target_triple() -> &'static str
```

- `wezterm_version()` (`wezterm-version/src/lib.rs:1`) — `env!("WEZTERM_CI_TAG")`를 반환. 빌드 시점에 인라인된 버전 태그 문자열.
- `wezterm_target_triple()` (`wezterm-version/src/lib.rs:6`) — `env!("WEZTERM_TARGET_TRIPLE")`를 반환. 빌드 대상 트리플 문자열.

두 함수 모두 인자가 없고 부작용이 없으며 `&'static str`을 반환한다. `env!`는 컴파일 타임 매크로이므로 해당 환경 변수가 빌드 시점에 정의되어 있지 않으면 **컴파일이 실패**한다. 즉 런타임 `None`/오류 경로는 존재하지 않으며, 값의 존재가 타입 시스템이 아니라 빌드 단계에서 보장된다.

## 4. 내부 구조

모듈 분해가 사실상 없다. 두 개의 파일이 전부다.

- **`build.rs`** — 빌드 스크립트. 제어 흐름:
  1. `cargo:rerun-if-changed=build.rs` 출력으로 자기 자신 변경 추적 등록.
  2. 워크스페이스 루트의 `../.tag` 파일을 우선 읽는다. 존재하면 그 내용(trim)을 버전 태그로 사용하고 `cargo:rerun-if-changed=../.tag`를 등록한다. — **CI/릴리스 경로**.
  3. `.tag`가 없으면 git에서 파생한다. `git2::Repository::discover(".")`로 저장소를 찾고, `HEAD` 참조를 resolve하여 해당 ref 파일 경로를 `rerun-if-changed`로 등록한다(커밋이 바뀌면 재빌드 트리거). 그 후 외부 `git` 프로세스를 `git -c core.abbrev=8 show -s --format=%cd-%h --date=format:%Y%m%d-%H%M%S`로 실행해 `날짜-단축해시` 형태의 태그를 만든다. — **개발 빌드 경로**.
  4. `TARGET` 환경 변수를 읽되 없으면 `"unknown"`으로 대체.
  5. `cargo:rustc-env=WEZTERM_TARGET_TRIPLE=...`, `cargo:rustc-env=WEZTERM_CI_TAG=...` 두 줄을 출력해 컴파일러에 환경 변수를 주입.
- **`src/lib.rs`** — `env!`로 주입된 두 환경 변수를 정적 문자열로 노출하는 래퍼 2함수.

데이터 흐름은 단방향이다. 빌드 스크립트(빌드 타임) → `rustc-env` → `env!`(컴파일 타임 인라인) → `&'static str`(런타임). 비대한 모듈은 없다.

## 5. 핵심 데이터 구조·타입

선언된 struct/enum이 없다. 데이터 모델은 두 개의 `&'static str` 값으로 환원된다.

불변식(invariant):
- `WEZTERM_CI_TAG`, `WEZTERM_TARGET_TRIPLE` 두 환경 변수는 컴파일 시점에 **반드시** 존재해야 한다(`build.rs`가 항상 두 값을 출력하므로 보장됨). 빠지면 `env!`가 컴파일 에러를 낸다.
- 반환값은 빌드 타임에 고정되어 런타임 내내 불변. `'static` 수명을 가진다.
- `.tag` 파일이 git 파생 경로보다 우선한다(if/else 분기). 두 경로가 동시에 적용되는 경우는 없다.
- `.tag`도 없고 git 조회도 모두 실패하면 `ci_tag`는 빈 문자열(`String::new()`)로 남으며, 버전 문자열이 빈 값이 될 수 있다(런타임 오류가 아니라 빈 표시).

## 6. 외부 의존성

- **`git2`** (빌드 의존성, `default-features = false`) — `build.rs`에서 `Repository::discover`로 저장소 위치를 찾고 `HEAD` ref를 resolve해 `rerun-if-changed` 경로를 등록하는 데 사용. 즉 **재빌드 트리거 정밀화** 용도이며, 실제 태그 문자열 생성은 외부 `git` 프로세스 호출이 담당한다. `default-features = false`로 SSH/HTTPS 등 무거운 백엔드를 끄고 최소 빌드를 유지한다. 런타임 바이너리에는 링크되지 않는다.

런타임 `[dependencies]`는 비어 있다(`wezterm-version/Cargo.toml:13`). 외부 크레이트 의존이 런타임에 0인 점이 이 크레이트의 특징이다.

## 7. 설정·기능 플래그

- Cargo feature flag: 없음.
- `config` 크레이트의 설정 항목과는 직접 연결되지 않는다. 다만 산출된 버전 값은 `config`를 통해 다음으로 전파된다:
  - `TERM_PROGRAM_VERSION` 환경 변수 주입 (`config/src/config.rs:1565`)
  - Lua `wezterm.version` / `wezterm.target_triple` (`config/src/lua.rs:324-325`)
- 빌드 입력으로 동작하는 외부 인자:
  - `../.tag` 파일(존재 시 최우선 버전 소스). CI/릴리스 파이프라인이 이 파일을 생성하는 방식으로 버전을 고정한다.
  - `TARGET` 환경 변수(Cargo가 자동 설정). 트리플 값의 출처.

## 8. Windows 전용 고려사항

- 이 크레이트에는 `cfg(windows)`/`cfg(unix)` 등 플랫폼 분기가 **없다**. 따라서 죽은 비Windows 코드도 없다.
- `build.rs`는 외부 `git` 실행 파일을 `std::process::Command::new("git")`로 호출한다. Windows 빌드 환경에서 `git`이 `PATH`에 있어야 git 파생 경로가 동작한다(없으면 `.tag` 미존재 시 버전이 빈 문자열). 릴리스 빌드는 `.tag` 우선 경로를 쓰므로 `git` 부재의 영향을 받지 않는다.
- 타깃 트리플은 `TARGET`에서 그대로 가져오므로 Windows MSVC 빌드 시 `x86_64-pc-windows-msvc` 등이 자동으로 박힌다. 별도 처리가 필요 없다.
- 빌드 의존 `git2`는 Windows에서 정상 빌드되며, `default-features=false`로 외부 라이브러리 요구를 최소화한다(이 fork의 vendored 빌드 정책과 충돌하지 않음).

## 9. 리팩토링 주의점

- **결합도는 매우 낮다.** 런타임 의존성이 0이고 공개 API가 함수 2개뿐이라 변경 파급은 직접 호출처(`env-bootstrap`)로 국한된다. 시그니처(`-> &'static str`)를 바꾸면 `env-bootstrap/src/lib.rs:159-160`과 그 하류(`config::assign_version_info`)가 즉시 영향을 받는다.
- **`env!`의 컴파일 타임 계약을 깨지 말 것.** `build.rs`가 `rustc-env`로 두 변수를 항상 출력한다는 전제가 `lib.rs`의 `env!`를 떠받친다. 빌드 스크립트에서 둘 중 하나라도 출력하지 않으면 크레이트 전체가 컴파일되지 않는다.
- **외부 `git` 프로세스 의존.** 개발 빌드 경로는 셸이 아닌 `Command::new("git").args([...])` 형태(인자 배열, shell 미사용)로 안전하지만, `git` 부재 시 조용히 빈 버전이 된다. 리팩토링 시 이 무음 실패를 경고로 승격할지 검토할 가치가 있다.
- **버전 결정 우선순위 불변식.** `.tag` > git 파생 순서를 유지해야 CI 릴리스 태그가 개발 파생값에 덮이지 않는다.
- 순환 의존: 없음. 이 크레이트는 잎 노드라 순환 위험이 구조적으로 차단된다.
- 기술 부채는 미미하다. `unwrap()`이 `build.rs:25`(`canonicalize().unwrap()`)에 한 군데 있으나 빌드 스크립트 한정이라 런타임 영향은 없다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `wezterm-version/Cargo.toml` | 14 | 패키지 정의. 런타임 의존 0, 빌드 의존 `git2`만 선언 |
| `wezterm-version/build.rs` | 53 | 빌드 스크립트. `.tag` 또는 git에서 버전 태그를 산출하고 `TARGET`을 읽어 두 환경 변수를 `rustc`에 주입 |
| `wezterm-version/src/lib.rs` | 10 | `env!`로 주입 변수를 정적 문자열로 노출하는 공개 함수 2개 |

생성 파일 없음.
