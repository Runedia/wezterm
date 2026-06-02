# wezterm-toast-notification 폴더 기능 명세

본 문서는 WezTerm의 Windows 전용 영구 분기에서 `wezterm-toast-notification` 크레이트의 기능을 리팩토링 목적의 상세 명세로 기술한다. 모든 기술은 실제 소스 코드를 근거로 한다.

---

## 1. 개요 및 책임

`wezterm-toast-notification`은 WezTerm이 운영체제의 네이티브 토스트 알림(toast notification)을 띄우는 단일 책임을 갖는 크레이트다. 상위 계층(GUI, 폰트 로더)이 알림의 제목·본문·선택적 URL·타임아웃만 넘기면, 이 크레이트가 플랫폼 알림 시스템 호출을 캡슐화한다.

이 크레이트의 책임은 다음으로 한정된다.

- 알림 데이터 모델(`ToastNotification`)을 정의한다.
- 그 데이터를 OS 네이티브 알림으로 표시한다.
- 알림에 URL이 포함된 경우, 사용자가 "Show" 액션을 클릭하면 해당 URL을 연다(`wezterm-open-url`에 위임).

Windows fork에서의 실제 책임은 **WinRT의 `Windows.UI.Notifications` API를 통한 토스트 표시**다. 원본 WezTerm은 macOS(`NSUserNotification`)·Linux(D-Bus/`notify`) 백엔드를 가졌으나, 본 분기는 Windows 백엔드만 유지한다. `lib.rs:17`에서 백엔드가 `crate::windows`로 무조건 고정되어 있어 비Windows 경로는 존재하지 않는다. `lib.rs:19-26`의 `nop` 모듈은 비Windows 환경용 폴백이었으나 현재 `#[allow(dead_code)]`로 표시된 죽은 코드다.

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 | 용도 |
|------|----------|------|
| 의존(deps) | `wezterm-open-url` | 토스트의 "Show" 액션 클릭 시 URL 열기 |
| 의존(deps) | `log` | 오류 로깅 |
| 의존(deps, Windows 한정) | `windows` | WinRT 토스트 API 바인딩 |
| 의존(deps, Windows 한정) | `xml-rs` | 토스트 XML 페이로드 텍스트 이스케이프 |
| 피의존(usedBy) | `wezterm-font` | 폰트 로딩 경고/오류를 사용자에게 알림 (`wezterm-font/src/lib.rs:23,432`) |
| 피의존(usedBy) | `wezterm-gui` | 터미널 알림(OSC), 업데이트 안내, 다운로드 완료, 패닉/치명 오류, Lua `toast_notification` API 등 다수 경로 |

이 크레이트는 **leaf에 가까운 출력(side-effect) 계층**에 위치한다. 알림을 발생시키는 비즈니스 로직(예: `term`의 `Alert::ToastNotification`, 자동 업데이트 검사)은 상위 크레이트에 있고, 이 크레이트는 그 결과를 OS에 전달하는 어댑터다. 자신은 워크스페이스 내 단 하나의 크레이트(`wezterm-open-url`)에만 의존하므로 의존성 그래프상 말단에 가깝다.

## 3. 공개 API 표면

전부 `lib.rs`에 선언된다.

- `pub struct ToastNotification` — 알림 데이터 모델. 필드 `title: String`, `message: String`, `url: Option<String>`, `timeout: Option<std::time::Duration>` (`lib.rs:3-9`).
- `impl ToastNotification { pub fn show(self) }` — 인스턴스를 곧바로 표시하는 편의 메서드. 내부적으로 자유 함수 `show`에 위임 (`lib.rs:11-15`).
- `pub fn show(notif: ToastNotification)` — 핵심 진입점. 백엔드의 `show_notif`를 호출하고 실패 시 `log::error!`로만 보고한다(에러를 호출자에게 전파하지 않음) (`lib.rs:28-32`).
- `pub fn persistent_toast_notification_with_click_to_open_url(title: &str, message: &str, url: &str)` — 클릭 시 URL을 여는 무기한(`timeout: None`) 알림 생성 헬퍼 (`lib.rs:34-41`).
- `pub fn persistent_toast_notification(title: &str, message: &str)` — URL 없는 무기한 알림 생성 헬퍼 (`lib.rs:43-50`).

소비처는 주로 두 헬퍼 함수와 `show` 자유 함수를 사용한다. 예: `wezterm-gui/src/update.rs:197`(업데이트 안내), `wezterm-gui/src/main.rs:797`(치명 오류), `wezterm-gui/src/scripting/guiwin.rs:87`(Lua `toast_notification` API에서 `ToastNotification`을 직접 구성해 `show` 호출).

## 4. 내부 구조

모듈은 2개뿐이며 단순하다.

- **`lib.rs`** — 공개 데이터 모델·API와 백엔드 디스패치. 백엔드를 `use crate::windows as backend`로 별칭 고정(`lib.rs:17`). 오류를 로그로 소거하는 경계가 이곳이다.
- **`windows.rs`** — Windows WinRT 구현. 파일 전체가 `#![cfg(windows)]`로 게이트된다(`windows.rs:1`).

제어/데이터 흐름:

1. 호출자가 `show(notif)` 또는 헬퍼를 통해 `ToastNotification`을 전달.
2. `show` → `backend::show_notif(notif)` 호출.
3. `show_notif`(`windows.rs:89-100`)는 **새 스레드를 스폰**한 뒤 즉시 `Ok(())`를 반환한다. 별도 스레드를 쓰는 이유는 호출자가 Windows 메시지 루프 디스패치 내부일 수 있고, 그 경우 메시지를 펌프할 수 없기 때문이다(`windows.rs:90-92` 주석).
4. 스폰된 스레드에서 `show_notif_impl`(`windows.rs:21-87`)이 실제 작업을 수행: XML 페이로드 구성 → `ToastNotification` 생성 → `Activated` 이벤트 핸들러 등록 → notifier 생성 → `Show` 호출.

비대한 모듈은 없다. 두 파일 모두 100행 안팎이다.

## 5. 핵심 데이터 구조·타입

- **`ToastNotification`** (`lib.rs:3-9`) — `#[derive(Debug, Clone)]`.
  - 불변식/특성: 모든 필드는 단순 소유 데이터. `url`이 `Some`이면 백엔드가 "Show" 액션 버튼을 추가하고, 클릭 시 그 URL을 연다(`windows.rs:24-32`, `58-62`). `timeout` 필드는 **구조체에 존재하나 Windows 백엔드에서 전혀 소비되지 않는다**(아래 9절 참조). 두 헬퍼 함수는 항상 `timeout: None`으로 생성한다.

`windows.rs` 내부에서 다루는 WinRT 타입(소유 아님, 외부 정의):

- `XmlDocument` — 토스트 XML 페이로드 적재(`windows.rs:22,34`).
- `windows::UI::Notifications::ToastNotification` — 실제 토스트 객체. 본 크레이트의 동명 타입과 충돌을 피하려 `lib.rs`의 타입을 `use crate::ToastNotification as TN`로 별칭한다(`windows.rs:3`).
- `ToastActivatedEventArgs` — 액션 클릭 시 전달되는 인자. `Arguments()`가 XML의 `arguments="show"`와 매칭(`windows.rs:54-58`).
- `ToastNotificationManager` — notifier 팩토리. AppUserModelID로 `"org.wezfurlong.wezterm"` 고정(`windows.rs:80-82`).

## 6. 외부 의존성

- **`windows` (0.33.0, workspace 고정)** — WinRT `Windows.UI.Notifications` 바인딩. 활성화한 feature: `Data_Xml_Dom`, `Foundation`, `UI_Notifications`, `Win32_Foundation`(`Cargo.toml:15-21`). 토스트 표시의 핵심.
- **`xml-rs` (0.8)** — `xml::escape::escape_str_pcdata`로 제목·본문을 XML PCDATA로 안전하게 이스케이프(`windows.rs:4,44-45`). 사용자 제공 문자열이 토스트 XML 구조를 깨뜨리거나 주입되는 것을 방지하는 목적.
- **`wezterm-open-url`** — `open_url(url)` 호출로 "Show" 클릭 시 기본 브라우저 실행(`windows.rs:60`). 내부 구현은 `ShellExecuteW` 기반(`wezterm-open-url/src/lib.rs:39-41`).
- **`log` (0.4)** — 실패 보고 전용(`lib.rs:30`, `windows.rs:95`).

## 7. 설정·기능 플래그

- **Cargo feature flag**: 이 크레이트 자체에는 사용자 정의 feature가 없다. `windows` 크레이트의 feature만 선언한다(`Cargo.toml:16-21`).
- **`config` 크레이트 연동**: 없음. 이 크레이트는 `config`에 의존하지 않으며, 알림 활성화/내용 결정은 상위 호출자(`wezterm-gui` 등)가 담당한다. 따라서 알림 동작을 소스 기본값으로 고정하려면 본 크레이트가 아니라 호출 측을 수정해야 한다.
- **하드코딩 상수**: AppUserModelID `"org.wezfurlong.wezterm"`(`windows.rs:81`), 토스트 `duration="long"`(`windows.rs:35`), 액션 라벨 `content="Show"`(`windows.rs:27`)가 설정 불가능하게 코드에 고정되어 있다.

## 8. Windows 전용 고려사항

- `windows.rs` 전체가 `#![cfg(windows)]`(`windows.rs:1`). 비Windows 빌드에서는 `backend` 모듈이 비게 되나, `lib.rs`는 항상 `crate::windows`를 백엔드로 참조하므로 사실상 Windows 전용 크레이트다.
- **죽은 코드**: `lib.rs:19-26`의 `nop` 모듈은 비Windows 폴백 백엔드였다. 현재 `lib.rs`가 무조건 `windows`를 백엔드로 쓰므로 `nop::show_notif`는 호출되지 않으며 `#[allow(dead_code)]`로 경고만 억제된 상태다. 리팩토링 시 제거 후보.
- **주석 처리된 핸들러**: `Dismissed`/`Failed` 이벤트 핸들러가 통째로 주석 처리되어 있다(`windows.rs:68-78`). 알림이 무시되거나 OS가 알림을 비활성화한 경우를 감지하지 못한다.
- **Windows API 사용 지점**:
  - `ToastNotificationManager::CreateToastNotifierWithId`로 notifier 생성(`windows.rs:80`).
  - `notifier.Show(&notif)`로 표시(`windows.rs:84`).
  - `TypedEventHandler`로 `Activated` 콜백 등록(`windows.rs:51`).
  - `E_POINTER` HRESULT로 `Option`이 `None`일 때 WinRT 오류 생성(`windows.rs:9,17`).
- **스레딩**: 메시지 루프 데드락 회피를 위해 표시 작업을 별도 스레드로 분리(`windows.rs:93`). 이 스레드는 분리(detach)되며 결과를 호출자가 회수할 수 없다.

## 9. 리팩토링 주의점

- **`timeout` 필드 미사용**: `ToastNotification::timeout`은 정의되어 있으나 Windows 백엔드에서 읽히지 않는다. 토스트는 항상 `duration="long"`으로 고정(`windows.rs:35`). 필드를 신뢰해 타임아웃 동작을 기대하면 안 된다. 제거하거나 실제로 WinRT `duration`에 매핑하는 결정이 필요하다.
- **오류 소거 경계**: `show`(`lib.rs:28-32`)와 스폰 스레드(`windows.rs:94-96`) 두 곳에서 오류를 로그로만 처리하고 전파하지 않는다. 따라서 호출자는 알림 표시 성공/실패를 알 수 없다. 신뢰성이 필요한 경로(예: 사용자에게 반드시 보여야 하는 치명 오류)는 이 한계를 인지해야 한다.
- **fire-and-forget 스레드**: `show_notif`가 즉시 반환하므로 알림 실패 시점이 호출 시점과 분리된다. 또한 `Activated` 클로저가 `toast`(특히 `url`)를 `move`로 캡처하므로 알림 객체 수명 동안 데이터가 유지되어야 한다.
- **타입명 충돌**: 자체 `ToastNotification`과 WinRT `ToastNotification`이 동명이므로 `windows.rs`는 별칭(`TN`)으로 구분한다. 모듈 분리·재명명 시 이 별칭 규칙을 유지해야 혼동을 막는다.
- **XML 주입 방어 의존성**: 제목·본문 이스케이프를 `escape_str_pcdata`에 전적으로 의존한다(`windows.rs:44-45`). `xml-rs` 제거/교체 시 동등한 이스케이프를 반드시 보존해야 한다. 단, `url`은 별도 XML 텍스트에 삽입되지 않고 액션 인자 매칭(`"show"`)에만 쓰이므로 XML 이스케이프 대상이 아니다.
- **하드코딩 의존**: AppUserModelID·액션 라벨·duration이 상수다(8·7절). 이들이 Windows 알림 등록/표시와 결합되어 있어 변경 시 OS 측 알림 그룹화·표시에 영향을 줄 수 있다.
- **순환 의존**: 없음. 단방향(상위 → 본 크레이트 → `wezterm-open-url`).
- **죽은 코드 정리**: `nop` 모듈(8절)은 안전하게 제거 가능한 후보다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|------|-----------|------|
| `Cargo.toml` | 23 | 패키지 정의. workspace deps(`log`, `wezterm-open-url`)와 Windows 한정 deps(`windows` feature 4종, `xml-rs`) 선언 |
| `build.rs` | 2 | 빈 빌드 스크립트(본문 없음). `Cargo.toml:6`에서 `build = "build.rs"`로 참조되나 현재 무동작 |
| `src/lib.rs` | 51 | 공개 API. `ToastNotification` 모델, `show`/`persistent_*` 헬퍼, 백엔드 디스패치, 죽은 `nop` 모듈 |
| `src/windows.rs` | 101 | Windows WinRT 토스트 백엔드. XML 페이로드 구성, `Activated` 핸들러, notifier 생성·표시, 메시지 루프 회피용 스레드 스폰 |

생성 파일은 없다.
