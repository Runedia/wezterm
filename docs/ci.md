# ci 폴더 기능 명세

## 1. 개요 및 역할

`ci` 폴더는 이 저장소에서 **Windows 인스톨러를 생성하기 위한 패키징 스크립트**를 보관하는 단일 목적 디렉터리다. 현재 구성 파일은 다음 하나뿐이다.

- `ci/windows-installer.iss` — [Inno Setup](https://jrsoftware.org/isinfo.php) 컴파일러(ISCC)가 소비하는 설치 스크립트.

이 스크립트의 역할은 `cargo build --release`로 산출된 실행 파일·런타임 DLL을 한 곳에 모아 `WezTerm-Setup.exe`라는 단일 인스톨러로 묶고, 설치 시 다음 부수 작업을 수행하는 것이다.

- 시스템 `PATH` 환경 변수에 설치 디렉터리를 추가/제거.
- 탐색기 우클릭 컨텍스트 메뉴에 "Open WezTerm here" 항목 등록.
- 시작 메뉴/바탕화면 바로 가기 생성.
- 설치 가능한 CPU 아키텍처(x64 / arm64) 및 최소 Windows 빌드 검증.

이 폴더는 **비코드(빌드 산출물 패키징) 자산**이다. Rust 워크스페이스 컴파일에는 관여하지 않으며, 빌드가 끝난 뒤 배포 단계에서만 사용된다.

## 2. 디렉터리/파일 구성

| 경로 | 종류 | 용도 |
|------|------|------|
| `ci/windows-installer.iss` | Inno Setup 스크립트(Pascal 계열 DSL) | Windows 인스톨러 정의 |

`ci/windows-installer.iss`의 섹션 구조는 다음과 같다.

- **`#define` 매크로** (`windows-installer.iss:5`–`:9`): 앱 이름·발행자·URL·실행 파일명을 정의한다. `MyAppVersion`은 `:6`에서 주석 처리되어 있고, `:16`의 `AppVersion={#MyAppVersion}`에서 참조된다. 즉 **버전 문자열은 스크립트 내부에 박혀 있지 않으며**, ISCC 호출 시 `/DMyAppVersion=...` 형태로 외부에서 주입되어야 한다(주입되지 않으면 컴파일 실패).
- **`[Setup]`** (`:11`–`:37`): 인스톨러 전역 설정.
  - `AppId` GUID `{BCF6F0DA-...}` (`:12`)로 동일 제품 식별. 변경 시 업그레이드가 아니라 별도 제품으로 설치된다.
  - `ArchitecturesAllowed=x64 arm64` (`:13`).
  - `DefaultDirName={autopf}\WezTerm` (`:22`) — Program Files 하위.
  - `OutputDir=..` (`:28`), `OutputBaseFilename=WezTerm-Setup` (`:29`) — 저장소 루트에 `WezTerm-Setup.exe` 생성.
  - `SetupIconFile=..\assets\windows\terminal.ico` (`:30`) — 인스톨러 아이콘. 실제 파일 `assets\windows\terminal.ico`(약 162KB) 존재 확인됨.
  - `Compression=lzma`, `SolidCompression=yes` (`:32`–`:33`).
  - `MinVersion=10.0.17763` (`:36`) — ConPTY(의사 터미널) 지원을 위한 Windows 10 1809 이상 강제.
  - `ChangesEnvironment=true` (`:37`) — `PATH` 변경을 시스템에 알림.
- **`[Languages]`** (`:39`–`:40`): 영어 메시지 파일만 등록. **한국어 등 추가 언어 미설정**.
- **`[Tasks]`** (`:42`–`:43`): 바탕화면 아이콘 생성을 선택 항목(기본 해제)으로 노출.
- **`[Files]`** (`:45`–`:55`): 인스톨러에 포함할 산출물 목록. 모두 `..\target\release\` 기준 상대 경로이며 `{app}`(설치 디렉터리)로 복사된다. (3절에서 산출물 출처를 상술)
- **`[Icons]`** (`:57`–`:59`): 시작 메뉴/바탕화면 바로 가기. `AppUserModelID: "org.wezfurlong.wezterm"`로 작업 표시줄 그룹화 식별자를 고정.
- **`[Run]`** (`:61`–`:62`): 설치 완료 후 `wezterm-gui.exe`를 비대기(`nowait`)·무인설치 시 생략(`skipifsilent`) 플래그로 실행.
- **`[Registry]`** (`:64`–`:73`): 탐색기 컨텍스트 메뉴 등록.
  - Drive(`:65`–`:67`), Directory Background(`:68`–`:70`), Directory(`:71`–`:73`) 세 위치에 "Open WezTerm here" 항목을 `HKA`(설치 권한에 따라 HKLM/HKCU) 하위로 등록.
  - 명령은 `wezterm start --no-auto-connect --cwd "%V"` 형태. `%V`는 우클릭 대상 경로다. 세 위치의 경로 종단 처리(`%V\`, `%V`, `%V\\`)가 미묘하게 다른데, 이는 탐색기가 위치별로 후행 백슬래시를 다르게 전달하기 때문이다.
  - 모든 항목에 `uninsdeletekey` 플래그로 제거 시 정리.
- **`[Code]`** (`:75`–`:200`): Pascal 스크립트.
  - `IsSupportedArch()` (`:85`–`:119`): 아키텍처 적합성 검증. Build ≥ 22000이면 `GetMachineTypeAttributes`(Kernel32, delayload, `:81`–`:83`)로 AMD64 실행 가능 여부를 능력 탐지하고, 21277 ≤ Build < 22000이면 arm64의 x64 에뮬레이션을 신뢰해 허용하며, 그 이하 빌드는 x64만 허용한다.
  - `InitializeSetupCheckArchitecture` (`:121`–`:125`): `InitializeSetup` 이벤트에 위 검증을 연결.
  - `EnvAddPath`/`EnvRemovePath` (`:127`–`:188`): `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`의 `Path`를 직접 읽고 써서 설치 디렉터리를 추가·제거. 중복 등록 방지 및 `\;`·`;` 종단 양쪽을 처리한다.
  - `CurStepChanged` (`:190`–`:194`): 설치 후처리(`ssPostInstall`)에서 `EnvAddPath({app})` 호출.
  - `CurUninstallStepChanged` (`:196`–`:200`): 제거 후처리(`usPostUninstall`)에서 `EnvRemovePath({app})` 호출.

## 3. 빌드·패키징·실행과의 관계

### 산출물 출처

`[Files]` 섹션이 참조하는 9개 항목의 생성 경로는 다음과 같다.

| 인스톨러 참조 (`target\release\`) | 출처 | 비고 |
|---|---|---|
| `wezterm.exe` | `wezterm` 크레이트 (CLI) | `cargo build --release` |
| `wezterm-gui.exe` | `wezterm-gui` 크레이트 (GUI 본체) | 동상 |
| `wezterm-mux-server.exe` | `wezterm-mux-server` 크레이트 | 워크스페이스 멤버(`Cargo.toml:11`) |
| `mesa\opengl32.dll` | `assets\windows\mesa\opengl32.dll` | `wezterm-gui\build.rs:48`–`:62`가 빌드 중 복사 (소프트웨어 OpenGL 폴백) |
| `libEGL.dll`, `libGLESv2.dll` | `assets\windows\angle\` | `wezterm-gui\build.rs:32`–`:46`가 복사 (ANGLE, D3D 백엔드 OpenGL ES) |
| `conpty.dll`, `OpenConsole.exe` | `assets\windows\conhost\` | `wezterm-gui\build.rs:16`–`:30`가 복사 (ConPTY 의사 터미널) |
| `strip-ansi-escapes.exe` | **워크스페이스에 부재** | 5절 참조 — 죽은 참조 |

즉 GUI 빌드의 `build.rs`가 벤더링된 런타임 DLL을 `target\release`로 끌어오고, 인스톨러는 그 디렉터리를 패키징한다. **이 때문에 인스톨러를 만들기 전에 `wezterm-gui`가 반드시 한 번 이상 빌드되어 있어야 한다**(DLL 복사가 GUI build.rs에 종속).

### 버전 주입

버전은 `build.rs`의 `.tag` 파일 기반 버전 보고(`wezterm-gui\build.rs:64`–`:71`)와는 별개 경로다. 인스톨러 버전은 ISCC 외부 인자로 주입되어야 한다(`MyAppVersion`이 스크립트에 정의돼 있지 않음).

### CI 트리거와 실제 호출의 분리

`.github/workflows`의 `gen_windows.yml:17`, `gen_windows_continuous.yml:19`은 `ci/windows-installer.iss`를 **경로 변경 트리거**로만 나열한다. 동일 워크플로 목록에 함께 등장하는 `ci/deploy.sh`는 **현재 저장소에 존재하지 않는다**(삭제됨). 실제 ISCC 호출(인스톨러 컴파일·버전 주입·코드 서명) 로직은 이 폴더 안에 없으며, 과거 `ci/deploy.sh` 또는 외부 배포 스크립트에 있던 것으로 보인다. 따라서 이 폴더만으로는 인스톨러 빌드 절차가 완결되지 않는다.

## 4. Windows 전용 고려사항

- 이 폴더 전체가 본질적으로 Windows 전용이다. `.iss`는 Inno Setup(Windows 전용 도구)으로만 컴파일되며, 비Windows 분기가 존재하지 않는다. 이 저장소가 Windows 전용 영구 분기라는 점과 정합한다.
- `MinVersion=10.0.17763`(1809)은 ConPTY 의존 때문이며, 동시에 `conpty.dll`/`OpenConsole.exe`를 자체 동봉해 OS 내장본의 편차를 우회한다.
- ANGLE(`libEGL`/`libGLESv2`)와 Mesa(`opengl32.dll`)를 함께 동봉해 GPU 드라이버 OpenGL 미비 환경에서도 렌더링 폴백을 보장한다. 이는 Windows 그래픽 스택 다양성에 대한 대응이다.
- `PATH` 조작과 컨텍스트 메뉴 등록은 모두 레지스트리 직접 조작이다. 관리자 권한 설치 시 `HKLM`/`HKA`에 기록되므로 시스템 전역 영향이 있다.
- ISCC, 코드 서명 인증서 등은 빌드 머신(Windows + Inno Setup 설치)에 별도로 갖춰져야 한다. 빌드 도구 체인(Strawberry Perl + MSVC vcvars)은 Rust 산출물 생성에만 관여하고, 인스톨러 컴파일에는 Inno Setup이 추가로 필요하다.

## 5. 리팩토링/정리 주의점

- **`strip-ansi-escapes.exe`는 죽은 참조다.** 이 패키지는 워크스페이스 어디에도 존재하지 않으며(`Cargo.toml` 멤버 목록·`Cargo.lock` 모두에서 미발견), `cargo build`로 생성되지 않는다. 그럼에도 다음 두 곳이 이를 참조한다.
  - `ci/windows-installer.iss:54` — 존재하지 않는 파일을 패키징하려 함(인스톨러 컴파일 시 누락 오류 가능).
  - `.github/workflows`의 `gen_windows.yml:71`, `gen_windows_continuous.yml:74`, `gen_windows_tag.yml:60` — `cargo build -p strip-ansi-escapes --release`로 부재 패키지를 빌드 시도(빌드 실패 요인).
  정리 시 이 세 곳(또는 네 줄)을 함께 제거하거나, 해당 크레이트를 복원할지 결정해야 한다. **인스톨러와 워크플로를 동시에 고쳐야 한다.**
- `ci/deploy.sh`가 부재한다. 워크플로의 경로 트리거(`gen_windows.yml:16` 등)는 여전히 이를 참조한다. 실제 ISCC 호출 로직이 어디로 옮겨졌는지(또는 누락됐는지) 확인 후 트리거 목록을 정리해야 한다. 인스톨러 빌드를 재현하려면 이 호출 로직부터 복원·재작성이 필요하다.
- `MyAppVersion` 미정의: 스크립트 단독으로는 컴파일되지 않는다. 빌드 자동화를 손볼 때 버전 주입 인자(`/DMyAppVersion=`)가 보존되는지 반드시 확인할 것.
- `LicenseFile` 라인(`:24`)이 주석 처리되어 있다. `LICENSE.md`가 루트에 존재하므로, 라이선스 동의 화면이 필요하면 주석을 해제하면 된다. 현재는 라이선스 페이지가 표시되지 않는다.
- `AppId` GUID(`:12`)는 절대 임의 변경 금지. 변경 시 기존 설치본과 별개 제품으로 인식되어 업그레이드가 깨진다.
- `[Languages]`가 영어 단일이다. 이 분기의 한국어화 방침을 인스톨러까지 확장하려면 한국어 `.isl` 메시지 파일 추가가 별도로 필요하다(소스 코드 기본값 수정과는 무관한, 인스톨러 레이어 작업).
- `[Registry]` 세 항목의 `%V` 종단 처리 차이(`:67`, `:70`, `:73`)는 의도된 것이다. 일괄 통일하지 말 것 — 탐색기 위치별 전달 규약에 맞춘 값이다.
