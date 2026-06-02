# wezterm-open-url 폴더 기능 명세

## 1. 개요 및 책임

`wezterm-open-url` 크레이트는 **URL 또는 파일 경로를 운영체제의 기본 핸들러(또는 지정한 응용 프로그램)로 여는** 단일 책임을 가진다. 책임 범위는 다음으로 한정된다.

- 문자열 URL을 받아 OS 셸에 "open" 동작을 위임한다.
- 선택적으로 특정 응용 프로그램을 지정하여 해당 앱으로 URL을 연다.
- 위 동작을 호출자 스레드를 차단하지 않도록 별도 스레드에서 수행한다.

이 크레이트는 코드 출처가 명시되어 있다. `wezterm-open-url/src/lib.rs:1`–`3`에 따르면 일부는 Sebastian Thiel의 `open-rs`(<https://github.com/Byron/open-rs>)에서 파생되었다. upstream WezTerm은 플랫폼별 분기를 두었으나, 이 fork는 **Windows 전용 영구 분기**이므로 구현 전체가 Windows `ShellExecuteW` 단일 경로로 축소되어 있다. 즉 이 크레이트의 실제 책임은 "Win32 `ShellExecuteW`를 안전한 Rust 함수 두 개(`open_url`, `open_with`)로 감싼 얇은 어댑터"다.

추가 정책·로깅·이벤트 처리는 이 크레이트의 책임이 아니다. 그러한 책임(예: 사용자 정의 `open-uri` 이벤트 가로채기)은 호출자인 `wezterm-gui`가 담당한다(`wezterm-gui/src/termwindow/mod.rs:3159`–`3199` 참조).

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 | 비고 |
| --- | --- | --- |
| 의존(deps) | 없음(외부 `winapi` 1종만) | 내부 워크스페이스 크레이트 의존이 0이다. |
| 피의존(usedBy) | `spawn-funcs` | Lua `wezterm.open_with`/암묵적 `open_url` 바인딩 제공. |
| 피의존(usedBy) | `wezterm-gui` | 링크 클릭(`OpenUri`)·`open-uri` 이벤트 처리에서 호출. |
| 피의존(usedBy) | `wezterm-toast-notification` | 토스트 클릭 시 URL 열기. |

이 크레이트는 의존 그래프의 **리프(leaf) 계층**에 위치한다. 다른 워크스페이스 크레이트에 의존하지 않으며, OS API(`ShellExecuteW`)에 직접 닿는 최종 어댑터다. 상위 계층(GUI·Lua 바인딩·알림)은 사용자 의도를 해석한 뒤 최종 "열기" 행위를 이 크레이트에 위임하는 파이프라인 구조다.

## 3. 공개 API 표면

크레이트 전체 공개 표면은 함수 2개다(`wezterm-open-url/src/lib.rs`).

| 함수 | 시그니처 | 용도 |
| --- | --- | --- |
| `open_url` | `pub fn open_url(url: &str)` (`lib.rs:39`) | URL/경로를 OS 기본 핸들러로 연다. 내부적으로 `shell_execute(url, None)` 호출. |
| `open_with` | `pub fn open_with(url: &str, app: &str)` (`lib.rs:43`) | 지정한 `app`으로 URL을 연다. 내부적으로 `shell_execute(url, Some(app))` 호출. |

두 함수 모두 반환값이 없으며(`()`), 오류를 호출자에 전파하지 않는다. 실행은 비동기적으로 별도 스레드에 위임되므로 호출 시점에 즉시 반환한다.

소비처에서의 실제 사용 형태:

- `lua-api-crates/spawn-funcs/src/lib.rs:19`–`25`: Lua `wezterm.open_with(url, app?)`가 `app` 유무에 따라 `open_with` 또는 `open_url`로 분기한다.
- `wezterm-gui/src/termwindow/mod.rs:3146`, `:3191`: 링크 클릭 시 `open_url(link)` 호출.
- `wezterm-toast-notification/src/windows.rs:60`: 토스트 클릭 활성화 시 `open_url(url)` 호출.

## 4. 내부 구조

모듈 분해가 사실상 없다. `lib.rs` 단일 파일, 함수 3개로 구성된다.

- `shell_execute(url: String, with: Option<String>)` (`lib.rs:5`) — 비공개 핵심 함수. 두 공개 함수가 공유하는 단일 구현체.
- `open_url` / `open_with` — `shell_execute`로 위임하는 공개 래퍼.

제어/데이터 흐름:

1. 공개 함수가 `&str` 인자를 `String`으로 소유권 복제하여 `shell_execute`에 전달한다(`lib.rs:40`, `:44`). 스레드로 데이터를 이동(`move`)하기 위한 복제다.
2. `shell_execute`는 `std::thread::spawn`으로 새 스레드를 띄운다(`lib.rs:15`). 호출자 스레드는 즉시 반환한다.
3. 스레드 내부에서 문자열을 UTF-16 와이드 문자열(널 종료)로 변환한다. 변환은 중첩 함수 `wide_string`이 담당한다(`lib.rs:9`–`14`).
4. `with` 인자 유무에 따라 `ShellExecuteW`의 `lpFile`/`lpParameters` 인자를 구성한다(`lib.rs:21`–`24`).
   - `Some(app)`: `lpFile = app`, `lpParameters = url` (앱에 URL을 인자로 전달).
   - `None`: `lpFile = url`, `lpParameters = null` (URL을 기본 핸들러로 직접 열기).
5. `unsafe` 블록에서 `ShellExecuteW`를 "open" 동사·`SW_SHOW`로 호출한다(`lib.rs:26`–`35`).

비대한 모듈은 없다. 전체 46행이다.

## 5. 핵심 데이터 구조·타입

이 크레이트는 고유 struct/enum을 정의하지 않는다. 사용하는 타입과 불변식은 다음과 같다.

- `Option<String>` (`with` 인자): `Some`이면 "지정 앱으로 열기", `None`이면 "기본 핸들러로 열기"라는 분기 의미를 가진다. 이 분기가 `ShellExecuteW` 인자 매핑(`lib.rs:21`–`24`)을 결정하는 유일한 상태다.
- `Vec<u16>` (와이드 문자열): `wide_string`이 생성하는 UTF-16 버퍼. **불변식**: 반드시 널 종료(`chain(std::iter::once(0))`, `lib.rs:12`)되어야 한다. `ShellExecuteW`는 널 종료 문자열을 기대하므로 이 종료자가 누락되면 메모리 오버런이 발생한다.
- 포인터 수명 불변식: `app`/`path`로 넘기는 포인터(`lib.rs:22`–`23`)는 `operation`·`url`·`with` 버퍼가 `ShellExecuteW` 호출 동안 살아 있어야 유효하다. 현재 코드는 모든 버퍼를 스레드 클로저 스코프 안에 두어 호출이 끝날 때까지 생존을 보장한다(`lib.rs:16`–`35`).

## 6. 외부 의존성

| 크레이트 | 버전/지정 | 사용 이유 |
| --- | --- | --- |
| `winapi` | `{ workspace = true, features = ["shellapi"] }` (`Cargo.toml:10`) | Win32 셸 API 바인딩. `shellapi` 피처로 `ShellExecuteW`(`lib.rs:7`)를, `winuser`의 `SW_SHOW` 상수(`lib.rs:33`)를 사용한다. |

내부 워크스페이스 의존은 없다(`Cargo.toml`에 `winapi` 외 항목 없음). 표준 라이브러리에서는 `std::os::windows::ffi::OsStrExt`(와이드 변환, `lib.rs:6`), `std::ffi::OsStr`, `std::thread`, `std::ptr`, `std::iter`를 사용한다.

## 7. 설정·기능 플래그

- 크레이트 자체 feature flag는 없다.
- 의존성 측 피처는 `winapi`의 `shellapi` 하나만 활성화한다(`Cargo.toml:10`). `SW_SHOW`(`winuser` 모듈)는 별도 피처 지정 없이 winapi 기본 노출로 사용된다.
- `config` 크레이트와의 직접 연계는 없다. 관련 사용자 설정은 상위 계층에 존재한다. 예: 링크 클릭 동작은 GUI의 `open-uri` Lua 이벤트로 가로챌 수 있으며, 그 분기 로직은 `wezterm-gui/src/termwindow/mod.rs:3166`–`3192`에 있다(이 크레이트 외부).
- `publish = false`(`Cargo.toml:5`): 워크스페이스 내부 전용 크레이트로 crates.io 게시 대상이 아니다.

## 8. Windows 전용 고려사항

이 크레이트는 사실상 **전부가 Windows 전용 코드**다.

- 구현 전체가 `ShellExecuteW`(`lib.rs:27`) 단일 Win32 호출에 의존한다. `cfg(unix)` 등 비Windows 분기는 남아 있지 않다(upstream `open-rs`의 멀티플랫폼 분기가 제거된 상태).
- 사용 Windows API: `ShellExecuteW`(`winapi::um::shellapi`, `lib.rs:7`·`:27`), `SW_SHOW`(`winapi::um::winuser`, `lib.rs:33`).
- 문자열 인코딩이 Windows 와이드(UTF-16) 규약에 묶여 있다. `OsStrExt::encode_wide`(`lib.rs:11`)는 Windows에서만 의미가 있다.
- 호출자 측 Windows 고려사항: GUI는 윈도우 루프(`WndProc`) 내부에서 동기적으로 열기를 호출하면 재귀 패닉이 발생할 수 있어 비동기 디스패치 밖에서 호출하도록 설계되어 있다(`wezterm-gui/src/termwindow/mod.rs:3159`–`3164` 주석). 이 크레이트 내부의 `std::thread::spawn`(`lib.rs:15`)도 동일한 비차단·비재귀 보장을 강화한다.

## 9. 리팩토링 주의점

- **오류 무시(silent failure)**: `ShellExecuteW`의 반환값(성공/실패를 나타내는 `HINSTANCE` 의사값)을 전혀 확인하지 않는다(`lib.rs:26`–`35`). 열기 실패가 호출자에 통보되지 않는다. 진단성을 높이려면 반환 핸들 검사·로깅을 추가해야 하나, 현재 시그니처가 `()`이므로 오류 전파를 도입하면 공개 API 변경이 된다. 호출자 3곳 모두 반환값을 쓰지 않으므로 시그니처 변경 시 파급은 제한적이나 모두 갱신이 필요하다.
- **fire-and-forget 스레드**: `thread::spawn`의 `JoinHandle`을 버린다(`lib.rs:15`). 완료/실패를 알 수 없고, 스레드 누수는 없으나 작업 추적도 불가능하다.
- **입력 검증 부재**: URL/경로/앱 문자열을 검증 없이 그대로 셸에 전달한다. `open_with`는 `lpFile=app`, `lpParameters=url` 매핑(`lib.rs:22`)이라 신뢰할 수 없는 입력이 `app`에 들어오면 임의 실행 위험이 있다. 호출자 책임으로 위임된 상태이며, 신뢰 경계를 문서화하거나 검증을 강화할 여지가 있다.
- **결합도**: `winapi` 단일 의존이라 결합도는 낮다. 다만 멀티플랫폼 복원을 원한다면 사실상 재작성에 가깝다(현재는 분기 흔적이 없음).
- **순환 의존 없음**: 리프 크레이트이므로 워크스페이스 의존 사이클 위험이 없다.
- **불변식 유지**: 와이드 문자열 널 종료(`lib.rs:12`)와 포인터 생존(버퍼를 클로저 스코프 안에 유지)은 안전성의 핵심이다. 버퍼를 스레드 밖으로 옮기거나 변환 함수를 분리할 때 이 불변식을 깨지 않도록 주의해야 한다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `wezterm-open-url/Cargo.toml` | 11 | 패키지 메타·`winapi`(shellapi) 단일 의존 선언. `publish = false`. |
| `wezterm-open-url/src/lib.rs` | 46 | 크레이트 본체. 비공개 `shell_execute`와 공개 `open_url`/`open_with`. `ShellExecuteW` 래핑. |
