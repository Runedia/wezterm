# wezterm-gui-subcommands 폴더 기능 명세

## 1. 개요 및 책임

`wezterm-gui-subcommands` 크레이트는 WezTerm의 **GUI 관련 서브커맨드에 대한 명령행 인자(CLI argument) 정의**를 담는 단일 책임 크레이트다. 구체적으로 `clap`의 `derive` 매크로(`#[derive(Parser)]`)를 적용한 인자 구조체들과, 그 인자 파싱을 보조하는 소수의 자유 함수·상수만을 제공한다.

이 크레이트는 **실행 로직을 전혀 포함하지 않는다.** 즉 GUI를 띄우거나, SSH 세션을 열거나, 폰트를 나열하는 등의 동작은 여기서 구현되지 않는다. 이 크레이트는 오직 "어떤 플래그·인자가 존재하며 어떻게 파싱되는가"라는 **인터페이스 선언**만 담당한다. 실제 동작은 이 구조체를 소비하는 `wezterm`(CLI 프런트엔드), `wezterm-gui`(실제 GUI 프로세스), `wezterm-mux-server`(멀티플렉서 서버)에서 수행된다.

존재 이유는 **인자 정의의 단일 출처(single source of truth) 공유**다. `wezterm`과 `wezterm-gui`는 동일한 서브커맨드 표면(`start`, `ssh`, `serial`, `connect`, `ls-fonts`, `show-keys`)을 노출해야 한다. `wezterm`은 사용자가 입력한 인자를 파싱한 뒤 GUI 프로세스(`wezterm-gui`)로 전달하고, `wezterm-gui`는 자신이 직접 호출될 때 동일한 인자를 다시 파싱한다. 두 크레이트가 인자 정의를 중복 작성하면 표류(drift)가 발생하므로, 정의를 이 크레이트 한 곳에 모아 두 측이 동일 타입을 임포트하도록 한다.

Windows 전용 분기에서의 실제 책임도 동일하다. 다만 일부 인자(예: `--position`의 Wayland 주석, `serial`의 `/dev/ttyUSB0` 예시)는 비Windows 플랫폼을 전제한 문서 문자열·예시를 그대로 보유하고 있으며, 이는 사용자에게 노출되는 도움말 텍스트에만 영향을 미친다(섹션 8 참조).

## 2. 워크스페이스 내 위치

이 크레이트는 워크스페이스 의존성 계층에서 **하단 근처의 잎(leaf)에 가까운 위치**에 있다. 자신은 `config`(및 `clap`)에만 의존하며, 상위의 세 실행 바이너리 크레이트가 이를 공유 임포트한다.

### 의존(deps)

| 크레이트 | 사용 이유 |
| --- | --- |
| `config` | 인자 파싱 결과 타입으로 사용하는 `GuiPosition`(창 위치)·`SshParameters`(SSH 대상 파라미터)를 제공받기 위함. `--position`, `user_at_host_and_port` 필드가 이 타입을 직접 사용한다. |
| `clap` (`derive` feature) | `#[derive(Parser)]` 기반 인자 정의·파싱 매크로. 워크스페이스 공통 버전 4.0. |

### 피의존(usedBy)

| 크레이트 | 사용 형태 |
| --- | --- |
| `wezterm` | CLI 프런트엔드. `use wezterm_gui_subcommands::*;`로 전체 임포트하여 자신의 `SubCommand` enum 변형(`Start`, `BlockingStart`, `Ssh`, `Serial`, `Connect`, `LsFonts`, `ShowKeys`)에 이 구조체를 담는다. `DEFAULT_WINDOW_CLASS`도 사용한다(`wezterm/src/cli/mod.rs:179`). |
| `wezterm-gui` | 실제 GUI 프로세스. 동일하게 전체 임포트하여 자신의 서브커맨드 enum을 구성하고, `run_ssh`/`run_serial`/`run_terminal_gui`/`run_show_keys`/`run_ls_fonts` 등 실행 함수의 인자 타입으로 사용한다(`wezterm-gui/src/main.rs:136~`). `DEFAULT_WINDOW_CLASS`는 창 클래스 초기값으로 쓰인다(`wezterm-gui/src/termwindow/mod.rs:92`). |
| `wezterm-mux-server` | `use wezterm_gui_subcommands::*;`로 임포트하나, 실사용은 사실상 `DEFAULT_WINDOW_CLASS` 등 일부 항목에 한정된다(`wezterm-mux-server/src/main.rs:12`). |

파이프라인상 위치: 이 크레이트는 **CLI 진입점 계층의 입력 계약(input contract)** 을 정의한다. 사용자 입력 → (`wezterm`/`wezterm-gui`의 `clap` 파싱, 본 크레이트 구조체로 역직렬화) → 실행 함수 호출이라는 흐름에서, 본 크레이트는 첫 번째 화살표가 산출하는 데이터 형식 그 자체를 규정하는 역할을 한다.

## 3. 공개 API 표면

본 크레이트의 공개 표면은 `src/lib.rs` 단일 파일에 모두 존재한다. 모듈 분할은 없다.

### 상수

- `pub const DEFAULT_WINDOW_CLASS: &str = "org.wezfurlong.wezterm";` (`wezterm-gui-subcommands/src/lib.rs:7`)
  - 기본 윈도잉 시스템 클래스 식별자. 각 서브커맨드의 `--class` 미지정 시 적용되는 폴백 값으로, 소비처에서 참조한다.

### 자유 함수

- `pub fn name_equals_value(arg: &str) -> Result<(String, String), String>` (`wezterm-gui-subcommands/src/lib.rs:10`)
  - `name=value` 형식 문자열을 `(name, value)` 튜플로 파싱하는 헬퍼. 첫 `=` 기준으로 분할하고 양변을 `trim()`한다. `=`가 없거나, 분할 후 좌·우 어느 한쪽이라도 빈 문자열이면 `Err(String)`을 반환한다. `SshCommand::config_override`의 `value_parser`로 사용된다.

### 인자 구조체 (모두 `pub`, `#[derive(Parser)]`)

| 타입 | 대응 서브커맨드 | 비고 |
| --- | --- | --- |
| `StartCommand` | `start`(및 `blocking-start`) | `Default`·`Clone` 추가 파생. `trailing_var_arg=true`. |
| `SshCommand` | `ssh` | `trailing_var_arg=true`. |
| `SerialCommand` | `serial` | trailing 인자 없음(`port`는 단일 위치 인자). |
| `ConnectCommand` | `connect` | `trailing_var_arg=true`. |
| `LsFontsCommand` | `ls-fonts` | 폰트 진단용. 실행 인자 없음. |
| `ShowKeysCommand` | `show-keys` | 키 바인딩 덤프용. |

각 구조체의 필드 시그니처 요약은 섹션 5에 정리한다. 이들 타입은 모두 `Debug + Clone`을 파생하며, `StartCommand`만 추가로 `Default`를 파생한다(`wezterm/src/main.rs:739`에서 `StartCommand::default()`를 인자 없는 `start` 폴백으로 사용하기 때문).

## 4. 내부 구조

내부 구조는 평탄하다. `src/lib.rs`(약 274행) 하나가 전부이며 하위 모듈이 없다. 비대한 모듈은 존재하지 않는다.

제어/데이터 흐름은 다음과 같다.

1. 소비 크레이트가 자신의 최상위 `#[derive(Parser)]` enum(예: `wezterm`의 `SubCommand`)의 변형 안에 본 크레이트의 구조체를 페이로드로 둔다.
2. `clap`이 명령행을 파싱하며 본 구조체의 `#[arg(...)]` 속성에 따라 필드를 채운다. 이때 `value_parser`(예: `name_equals_value`)와 `FromStr` 구현(예: `GuiPosition`, `SshParameters`)이 호출되어 문자열이 도메인 타입으로 변환된다.
3. 채워진 구조체가 소비 크레이트의 실행 함수로 전달된다.

본 크레이트 내부에서 유일하게 능동적인 코드 경로는 `name_equals_value` 함수와 구조체에 부착된 `clap` 검증 제약(`conflicts_with`, `requires` 등)이며, 후자는 매크로가 생성한 파서가 런타임에 강제한다. 그 외 파싱 로직(`GuiPosition::from_str`, `SshParameters`)은 모두 `config` 크레이트에 위임된다.

## 5. 핵심 데이터 구조·타입

아래는 각 구조체의 필드와 그에 결부된 불변식(`clap` 속성으로 강제되는 제약)이다.

### `StartCommand` (`wezterm-gui-subcommands/src/lib.rs:29`)

| 필드 | 타입 | 플래그 | 의미·제약 |
| --- | --- | --- | --- |
| `no_auto_connect` | `bool` | `--no-auto-connect` | `connect_automatically` 도메인 자동 연결 억제. |
| `always_new_process` | `bool` | `--always-new-process` | 기존 GUI 인스턴스에 위임하지 않고 항상 새 프로세스에서 GUI 기동. |
| `new_tab` | `bool` | `--new-tab` | 새 창 대신 활성 창에 새 탭. **불변식: `always_new_process`와 상호 배타(`conflicts_with`).** |
| `cwd` | `Option<PathBuf>` | `--cwd` | 초기 프로그램의 작업 디렉터리. `ValueHint::DirPath`. |
| `_cmd` | `bool` | `-e`(hidden) | `-e`를 소비하는 더미 인자. 호환성 목적. trailing 인자 파싱을 위한 폴백. |
| `class` | `Option<String>` | `--class` | 윈도 클래스 오버라이드. 미지정 시 `DEFAULT_WINDOW_CLASS`. |
| `workspace` | `Option<String>` | `--workspace` | 워크스페이스 이름 오버라이드(기본 `"default"`). |
| `position` | `Option<GuiPosition>` | `--position` | 초기 창 위치. `config::GuiPosition`의 `FromStr`로 파싱. |
| `domain` | `Option<String>` | `--domain` | 연결할 멀티플렉서 도메인 이름. |
| `attach` | `bool` | `--attach` | **불변식: `domain` 필요(`requires = "domain"`).** 실행 중 페인이 있으면 attach만 하고 PROG를 스폰하지 않음. |
| `prog` | `Vec<OsString>` | (위치/trailing) | 셸 대신 실행할 프로그램과 인자. `num_args=1..`, `CommandWithArguments`. |

### `SshCommand` (`wezterm-gui-subcommands/src/lib.rs:113`)

| 필드 | 타입 | 플래그 | 의미·제약 |
| --- | --- | --- | --- |
| `user_at_host_and_port` | `SshParameters` | (위치) | `[username@]host[:port]` 형식. `config::SshParameters`로 파싱. |
| `config_override` | `Vec<(String, String)>` | `-o`/`--ssh-option` | ssh_config 스타일 옵션 오버라이드. `value_parser = name_equals_value`, `number_of_values = 1`. |
| `verbose` | `bool` | `-v` | SSH 프로토콜 추적을 stderr로 출력. |
| `class` | `Option<String>` | `--class` | 윈도 클래스 오버라이드. |
| `position` | `Option<GuiPosition>` | `--position` | 초기 창 위치. |
| `prog` | `Vec<OsString>` | (trailing) | 원격에서 실행할 프로그램. |

### `SerialCommand` (`wezterm-gui-subcommands/src/lib.rs:172`)

| 필드 | 타입 | 플래그 | 의미 |
| --- | --- | --- | --- |
| `baud` | `Option<usize>` | `--baud` | 보레이트(기본 9600). |
| `class` | `Option<String>` | `--class` | 윈도 클래스 오버라이드. |
| `position` | `Option<GuiPosition>` | `--position` | 초기 창 위치. |
| `port` | `String` | (위치) | 시리얼 장치명. Windows에서는 `COM0` 등. |

### `ConnectCommand` (`wezterm-gui-subcommands/src/lib.rs:205`)

| 필드 | 타입 | 플래그 | 의미 |
| --- | --- | --- | --- |
| `domain_name` | `String` | (위치) | 연결할 멀티플렉서 도메인 이름. |
| `new_tab` | `bool` | `--new-tab` | 새 창 대신 새 탭. |
| `class` | `Option<String>` | `--class` | 윈도 클래스 오버라이드. |
| `workspace` | `Option<String>` | `--workspace` | 워크스페이스 이름 오버라이드. |
| `position` | `Option<GuiPosition>` | `--position` | 초기 창 위치. |
| `prog` | `Vec<OsString>` | (trailing) | 셸 대신 실행할 프로그램. |

### `LsFontsCommand` (`wezterm-gui-subcommands/src/lib.rs:247`)

| 필드 | 타입 | 플래그 | 제약 |
| --- | --- | --- | --- |
| `list_system` | `bool` | `--list-system` | 시스템 가용 폰트 전체 나열. |
| `text` | `Option<String>` | `--text` | 텍스트 렌더 시 사용 폰트 설명. **불변식: `list_system`·`codepoints`와 상호 배타.** |
| `codepoints` | `Option<String>` | `--codepoints` | 콤마 구분 hex 코드포인트의 사용 폰트 설명. **불변식: `list_system`과 배타.** |
| `rasterize_ascii` | `bool` | `--rasterize-ascii` | ascii 블록으로 글리프 래스터화 표시. **불변식: `text` 필요(`requires = "text"`).** |

### `ShowKeysCommand` (`wezterm-gui-subcommands/src/lib.rs:266`)

| 필드 | 타입 | 플래그 | 의미 |
| --- | --- | --- | --- |
| `lua` | `bool` | `--lua` | 키 바인딩을 lua config 문으로 출력. |
| `key_table` | `Option<String>` | `--key-table` | lua 모드에서 지정 키 테이블만 출력. |

**전역 불변식**: 모든 구조체가 `--class`/`--position`을 중복 보유한다(공통 창 속성). `StartCommand`/`SshCommand`/`ConnectCommand`는 `#[command(trailing_var_arg = true)]`로 `prog` 뒤 인자를 그대로 흡수한다. `name_equals_value`는 양변이 비어 있지 않음을 보장한다(빈 name/value 거부).

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
| --- | --- |
| `clap` (4.0, `derive`) | 본 크레이트의 존재 근거. `Parser`/`ValueHint`/`ValueParser`/`#[arg]`/`#[command]` 매크로로 인자 정의·파싱·검증을 선언적으로 구성한다. `value_hint`(`DirPath`, `CommandWithArguments`)는 셸 자동완성 메타데이터 제공 목적. |
| `config` (워크스페이스 내부 크레이트) | `GuiPosition`(창 위치, `FromStr`로 `x,y`/`origin:x,y` 파싱)·`SshParameters`(SSH 대상 파싱) 타입 제공. CLI 인자를 도메인 타입으로 직접 역직렬화하기 위함. |

표준 라이브러리에서는 `std::ffi::OsString`(비UTF-8 가능한 프로그램 인자 보존)과 `std::path::PathBuf`(`--cwd`)를 사용한다. 외부 시스템 라이브러리(harfbuzz/wgpu 등)에 대한 직접 의존은 **없다**. 본 크레이트는 순수 인자 정의 계층이므로 무거운 네이티브 의존을 끌어오지 않는다.

## 7. 설정·기능 플래그

- 본 크레이트의 `Cargo.toml`에는 `[features]` 섹션이 **없다.** feature flag를 정의하지 않는다.
- 끌어오는 유일한 feature는 워크스페이스 공통 `clap`의 `derive`다(`Cargo.toml:51`).
- 관련 config 항목: 인자 자체는 런타임에 config를 오버라이드하는 매개가 되며(`--workspace`, `--domain`, `--class`, `--position`), 이들은 소비 크레이트에서 `config::ConfigHandle`에 반영된다. `class` 미지정 시의 기본값이 본 크레이트의 `DEFAULT_WINDOW_CLASS` 상수다. 또한 `GuiPosition`/`SshParameters`의 기본값·파싱 규칙은 `config` 크레이트(`config/src/units.rs`, `config/src/ssh.rs`)가 소유한다.

## 8. Windows 전용 고려사항

본 크레이트에는 `#[cfg(...)]` 플랫폼 분기 코드가 **전혀 없다.** 모든 구조체와 함수가 플랫폼 무관하게 컴파일된다. 따라서 코드 경로 수준의 죽은 코드는 없다.

다만 사용자에게 노출되는 **도움말(doc comment) 텍스트에 비Windows 잔재**가 남아 있다. 이는 Windows 전용 분기에서 부정확하거나 불필요한 안내가 될 수 있다.

- `--class`의 doc comment는 "Under X11 and Windows this changes the window class. Under Wayland this changes the app_id."로, X11/Wayland 언급을 6개 구조체에 걸쳐 반복 보유한다(`StartCommand`/`SshCommand`/`SerialCommand`/`ConnectCommand`).
- `--position`의 doc comment 말미에 "Note that Wayland does not allow applications to control window positioning."가 남아 있다(`StartCommand`, `wezterm-gui-subcommands/src/lib.rs:87`).
- `SerialCommand::port`의 doc comment는 Windows 예시(`COM0`)와 함께 posix 예시(`/dev/ttyUSB0`)를 병기한다(`wezterm-gui-subcommands/src/lib.rs:196`).

Windows API를 직접 호출하는 지점은 없다. 시리얼 장치명 검증 역시 본 크레이트에서 수행하지 않고 단순 `String`으로 받으므로, `COM`/`/dev` 구분은 실행 단계(`wezterm-gui`)에 위임된다.

## 9. 리팩토링 주의점

- **인자 정의 중복**: `--class`/`--position` 두 필드가 4개 구조체에 동일 doc comment와 함께 중복된다. `#[command(flatten)]`으로 공통 옵션 구조체를 추출하면 중복을 제거할 수 있으나, 이는 각 서브커맨드의 도움말 출력 형식과 필드 접근 경로를 바꾸므로 소비 크레이트(`wezterm`/`wezterm-gui`)의 필드 접근(`opts.class`, `opts.position`)을 함께 수정해야 한다.
- **소비처 결합(타입 호환성)**: `wezterm`과 `wezterm-gui`가 동일 구조체를 임포트하므로, 필드 추가/삭제/이름 변경은 두 바이너리에 동시 파급된다. 특히 `wezterm`은 파싱한 인자를 `wezterm-gui`로 프로세스 간 전달하는 흐름이므로, 인자 표면이 두 측에서 비트 단위로 일치해야 한다.
- **`StartCommand: Default` 불변식**: `wezterm/src/main.rs:739`가 인자 없는 `start`의 폴백으로 `StartCommand::default()`를 사용한다. `Default` 파생을 제거하면 빌드가 깨진다.
- **`_cmd`(`-e`) 더미 인자**: 값을 사용하지 않는 호환성 전용 필드다. 제거하면 `wezterm -e <prog>` 호환 사용처가 깨질 수 있으므로, trailing_var_arg 흐름과 함께 신중히 다뤄야 한다.
- **doc comment 정합성**: 섹션 8의 비Windows 잔재 텍스트는 기능에는 영향이 없으나 사용자 도움말의 정확성을 떨어뜨린다. Windows 전용 분기 정책에 맞춰 정리 대상이다.
- **순환 의존 없음**: 본 크레이트는 `config`/`clap`만 의존하고 소비 크레이트를 역참조하지 않으므로 순환 의존 위험은 없다. 계층상 안전한 잎 크레이트다.
- **검증 위임**: `name_equals_value`를 제외한 모든 도메인 검증(`GuiPosition`, `SshParameters`)은 `config`에 위임되어 있다. 파싱 규칙 변경 시 본 크레이트가 아니라 `config`를 수정해야 한다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `Cargo.toml` | 13 | 패키지 매니페스트. `publish=false`, edition 2018, deps는 `clap`·`config`만. |
| `src/lib.rs` | 274 | 크레이트 전체. `DEFAULT_WINDOW_CLASS` 상수, `name_equals_value` 헬퍼, 6개 서브커맨드 인자 구조체(`StartCommand`/`SshCommand`/`SerialCommand`/`ConnectCommand`/`LsFontsCommand`/`ShowKeysCommand`). 하위 모듈 없음. |

생성 파일은 없다.
