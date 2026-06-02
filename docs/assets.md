# assets 폴더 기능 명세

## 1. 개요 및 역할

`assets` 폴더는 WezTerm 실행 파일과 **함께 배포되거나 컴파일 시점에 바이너리에 임베드되는 번들 리소스**를 모아 둔 비코드 자산 디렉터리입니다. 폴더 자체는 Rust 크레이트가 아니며, 빌드 스크립트(`wezterm-gui/build.rs`)와 폰트 파서(`wezterm-font/src/parser.rs`), Inno Setup 설치 스크립트(`ci/windows-installer.iss`)가 이 폴더의 파일을 소비합니다.

자산의 종류는 네 가지입니다.

- **폰트**(`fonts/`): 사용자 설정이 비어 있거나 깨졌을 때 동작하는 폴백 폰트군. `include_bytes!`로 실행 파일에 임베드됩니다.
- **아이콘**(`icon/`): 애플리케이션 아이콘(창 아이콘 PNG, Windows `.ico` 생성 원본 SVG).
- **셸 통합 스크립트**(`shell-integration/`): bash/zsh용 OSC 시퀀스 통합 스크립트. **POSIX 환경 전용 산출물**입니다.
- **Windows 런타임 바이너리**(`windows/`): ConPTY 백엔드, ANGLE/Mesa OpenGL 구현, 실행 매니페스트, 애플리케이션 아이콘. 빌드 시 `target/<profile>/`로 복사되거나 리소스로 임베드됩니다.

이 저장소는 WezTerm의 **Windows 전용 영구 분기**이므로, `assets` 내부에서도 Windows 패키징·실행에 기여하는 부분(`windows/`, `icon/terminal.png`·`terminal.ico`, 임베드 폰트)이 핵심이며, `shell-integration/`과 일부 SVG는 비Windows 또는 빌드 보조 성격의 잔류 자산입니다.

## 2. 디렉터리/파일 구성

### 2.1 `fonts/` (약 14 MiB)

폴백 폰트 묶음입니다. 사용자가 폰트 설정을 잘못했거나 지정하지 않은 경우에도 합리적인 렌더링을 보장하기 위해 번들됩니다(`wezterm-font/src/parser.rs:815-818` 주석 참조).

| 파일군 | 용도 | 소비 위치 |
| --- | --- | --- |
| `JetBrainsMono-*.ttf` (16종) | 기본 고정폭 텍스트 폴백 | `parser.rs:831-846`, `vendor-jetbrains` 피처 |
| `Roboto-*.ttf` (12종) | UI/가변폭 폴백 | `parser.rs:850-861`, `vendor-roboto` 피처 |
| `NotoColorEmoji.ttf` | 컬러 이모지 폴백 | `parser.rs:864`, `vendor-noto-emoji` 피처 |
| `SymbolsNerdFontMono-Regular.ttf` | Nerd Font 심볼/아이콘 폴백 | `parser.rs:866`, `vendor-nerd-font-symbols` 피처 |
| `FiraCode-Regular.ttf` | (아래 주석 참조) | `font!` 매크로로 임베드되지 **않음** |
| `LICENSE_OFL.txt`, `LICENSE_POWERLINE_EXTRA.txt` | 폰트 라이선스 고지 | 배포·법적 고지용 |

폰트는 빌드 시 `font!` 매크로(`parser.rs:821-825`)가 `include_bytes!`로 바이트 배열을 실행 파일에 직접 임베드합니다. 즉 **설치 시 디스크로 별도 복사되지 않으며**(`ci/windows-installer.iss`에 폰트 `Source:` 항목이 없습니다), 바이너리 크기에 흡수됩니다. 각 폰트군은 `wezterm-gui/Cargo.toml:15-23`의 `vendor-*-font` 피처와 `wezterm-font/Cargo.toml:11-14`의 `vendor-*` 피처를 통해 개별 토글 가능하며, 기본 빌드에서 모두 활성화됩니다.

`FiraCode-Regular.ttf`는 현재 `font!` 매크로·설치 스크립트·설정 어디에서도 직접 참조되지 않습니다. 테스트(`config/src/lib.rs:654`의 `use_test`가 `assets/fonts` 디렉터리 전체를 `font_dirs`에 추가)와 과거 셰이핑 테스트 픽스처용으로만 존재했던 잔류 자산입니다.

### 2.2 `icon/` (약 56 KiB)

애플리케이션 아이콘 원본과 산출물입니다.

- `wezterm-icon.svg`: **마스터 아이콘**. `update.sh`가 ImageMagick `convert`로 각 플랫폼 아이콘을 생성하는 입력입니다.
- `terminal.png` (128×128): 런타임 창 아이콘. `wezterm-gui/src/termwindow/mod.rs:96`에서 `include_bytes!`로 임베드되어 `ICON_DATA`로 노출됩니다.
- `wezterm-ghifarit53-{1,2,3}.svg`: 대체 아이콘 디자인(미사용 보관 자산).
- `update.sh`: SVG에서 Linux PNG·macOS `.icns`·Windows `.ico`를 재생성하는 빌드 보조 스크립트. `convert`·`png2icns`(libicns)에 의존하며, 산출 경로에 `../macos/`를 포함하므로 **현 Windows 전용 분기에서는 대부분 죽은 경로**입니다. `set -x`로 시작하는 POSIX bash 스크립트입니다.

### 2.3 `shell-integration/` (약 24 KiB)

- `wezterm.sh`: bash/zsh용 셸 통합 스크립트. OSC 시퀀스로 시맨틱 존, OSC 7 작업 디렉터리(CWD) 보고, 사용자 변수 캡처를 활성화하며, `bash-preexec.sh`를 verbatim으로 포함합니다(`wezterm.sh:39-60`). `WEZTERM_SHELL_SKIP_ALL` 등 환경변수로 기능을 비활성화할 수 있습니다(`wezterm.sh:11-25`).

이 스크립트는 **Rust 코드·설치 스크립트 어디에서도 참조되지 않으며**, Windows 설치본에 포함되지 않습니다. POSIX 셸 사용자가 직접 source 하는 용도의 자산이므로, Windows 전용 분기에서는 실질적 잔류물입니다.

### 2.4 `windows/` (약 43 MiB)

Windows 런타임에 필요한 네이티브 바이너리와 매니페스트입니다. 가장 크고 핵심적인 하위 폴더입니다.

| 경로 | 크기 | 용도 | 출처 |
| --- | --- | --- | --- |
| `conhost/conpty.dll` | 약 106 KiB | ConPTY 구현 | MS Terminal (MIT) |
| `conhost/OpenConsole.exe` | 약 1.1 MiB | 콘솔 호스트 실행 파일 | MS Terminal (MIT) |
| `conhost/README.md` | — | 번들 사유·재빌드 절차 기록 | — |
| `angle/libEGL.dll` | 약 394 KiB | ANGLE EGL 진입점 | Google ANGLE |
| `angle/libGLESv2.dll` | 약 4.6 MiB | ANGLE GLES→D3D 변환 | Google ANGLE |
| `mesa/opengl32.dll` | 약 36 MiB | Mesa 소프트웨어 OpenGL(llvmpipe) | mesa.fdossena.com |
| `mesa/README.md` | — | 출처·라이선스 기록 | — |
| `terminal.ico` | 약 162 KiB | 실행 파일 리소스 아이콘 | `update.sh` 생성물 |
| `manifest.manifest` | — | wezterm-gui용 실행 매니페스트 | — |
| `console.manifest` | — | 콘솔 도구용 실행 매니페스트 | — |

**`conhost/`**: Windows 기본 ConPTY가 마우스 리포팅을 지원하지 않아, 오픈소스 MS Terminal 빌드의 ConPTY/OpenConsole을 동봉해 마우스 리포팅을 활성화합니다(`conhost/README.md:7-15`). 동일 README에 ms-terminal 재빌드 절차(`razzle.cmd`, `bcz rel`)가 기록되어 있습니다.

**`angle/`·`mesa/`**: GPU 렌더링 백엔드. ANGLE은 OpenGL ES를 Direct3D로 변환하고, Mesa `opengl32.dll`은 GPU가 없거나 드라이버가 부적합한 환경을 위한 소프트웨어 폴백입니다.

**매니페스트**: 두 매니페스트 모두 `activeCodePage=UTF-8`(코드 페이지 UTF-8 강제)과 Windows 10/11 `supportedOS`, `asInvoker` 권한(UAC 상승 없음)을 선언합니다. `manifest.manifest`는 추가로 `dpiAwareness=PerMonitorV2`(모니터별 DPI 인식)와 Common-Controls 6.0 의존성을 선언하며, GUI 실행 파일(`wezterm-gui`)에 임베드됩니다.

## 3. 빌드·패키징·실행과의 관계

세 가지 소비 경로가 있습니다.

### 3.1 컴파일 시 임베드(`include_bytes!`)

- 폴백 폰트: `wezterm-font/src/parser.rs:828-867`이 `font!` 매크로로 바이너리에 임베드.
- 창 아이콘 PNG: `wezterm-gui/src/termwindow/mod.rs:96`의 `ICON_DATA`.
- 이 경로의 자산은 별도 파일로 배포되지 않고 실행 파일에 흡수됩니다.

### 3.2 빌드 스크립트의 DLL 복사 및 리소스 컴파일(`wezterm-gui/build.rs`)

`build.rs`는 `cfg(windows)` 빌드에서 다음을 수행합니다.

- `assets/windows/conhost/`의 `conpty.dll`·`OpenConsole.exe`를 `target/<profile>/`로 복사(`build.rs:16-30`).
- `assets/windows/angle/`의 `libEGL.dll`·`libGLESv2.dll`를 `target/<profile>/`로 복사(`build.rs:32-46`).
- `assets/windows/mesa/opengl32.dll`을 `target/<profile>/mesa/`로 복사(`build.rs:48-62`).
- `resource.rc`를 생성해 `manifest.manifest`를 `RT_MANIFEST`로, `terminal.ico`를 `IDI_ICON`(0x101)으로 임베드하고 버전 정보를 기록한 뒤, MSVC `cl.exe` 환경을 탐지해 `embed_resource::compile`로 컴파일(`build.rs:96-150`).

`terminal.ico` 변경 시 재빌드를 트리거하도록 `rerun-if-changed`가 설정되어 있습니다(`build.rs:98`). 아이콘 리소스 ID(0x101)는 `window/src/os/windows/window.rs:429`의 창 아이콘 로딩 코드와 결합되어 있습니다.

### 3.3 설치본 패키징(`ci/windows-installer.iss`)

Inno Setup이 `build.rs`가 복사해 둔 `target/release/`의 DLL·EXE를 설치 디렉터리로 포함합니다(`opengl32.dll`은 `{app}\mesa\`, 나머지는 `{app}` 루트). 설치 프로그램 아이콘은 `..\assets\windows\terminal.ico`를 직접 참조합니다(`windows-installer.iss:30`). **폰트와 셸 통합 스크립트는 설치본에 포함되지 않습니다**(전자는 임베드, 후자는 비Windows 자산).

### 3.4 CI 트리거

`.github/workflows/gen_windows.yml`과 `gen_windows_continuous.yml`은 `assets/fonts/**`, `assets/icon/*`, `assets/windows/**` 변경을 빌드 트리거 경로에 포함합니다. `shell-integration/`은 트리거 경로에 없습니다.

## 4. Windows 전용 고려사항

- **DLL 검색 경로**: `build.rs`가 ANGLE·ConPTY DLL을 실행 파일과 같은 디렉터리에 복사하고, Mesa는 `mesa/` 하위에 둡니다. 실행 시 이 상대 배치가 유지되어야 GPU 백엔드와 ConPTY가 정상 로드됩니다. 설치본의 `Source:` 매핑(`windows-installer.iss:49-53`)도 이 배치를 그대로 따릅니다.
- **UTF-8 코드 페이지**: 두 매니페스트의 `activeCodePage=UTF-8`은 Windows에서 ANSI API의 인코딩을 UTF-8로 고정해 비ASCII 경로·인자 처리를 안정화합니다. 제거하면 한글 등 비ASCII 처리에 회귀가 발생할 수 있습니다.
- **DPI 인식**: `manifest.manifest`의 `PerMonitorV2`는 고해상도·다중 모니터 환경의 스케일링 정확도를 보장합니다. GUI 실행 파일에만 적용되며, `console.manifest`에는 DPI 선언이 없습니다(콘솔 보조 도구용이므로 불필요).
- **MSVC 의존**: 리소스(`.rc`) 컴파일은 `cl.exe` 탐지를 요구합니다(`build.rs:144-149`). 빌드에는 MSVC vcvars 환경이 전제됩니다(저장소 공통 빌드 요건과 일치).
- **ConPTY 동봉 사유**: Windows 기본 ConPTY의 마우스 리포팅 미지원을 우회하기 위한 의도적 번들이므로(`conhost/README.md:7-15`), 시스템 ConPTY로의 대체는 마우스 기능 회귀를 동반합니다.

## 5. 리팩토링/정리 주의점

- **임베드 자산의 크기 영향**: `fonts/`의 모든 폰트는 실행 파일에 임베드되어 약 14 MiB가 바이너리 크기에 직접 가산됩니다. 폰트군 제거는 `Cargo.toml`의 `vendor-*` 피처를 끄는 방식으로 안전하게 가능하며, 파일만 지우면 `include_bytes!`가 컴파일 오류를 냅니다.
- **`FiraCode-Regular.ttf`는 제거 후보**: `font!` 매크로·설치·설정 어디에서도 직접 참조되지 않으며, 테스트가 `assets/fonts` 디렉터리 전체를 로드하는 경로(`config/src/lib.rs:654`)로만 우연히 적재됩니다. 제거 시 해당 테스트가 이 폰트에 의존하지 않는지 먼저 확인해야 합니다.
- **`shell-integration/wezterm.sh`**: Windows 전용 분기에서 코드·설치·CI 어디에서도 소비되지 않는 POSIX 자산입니다. 다만 외부 사용자의 수동 source 용도가 남아 있을 수 있으므로, 제거는 배포 정책 확인 후 결정해야 합니다.
- **`icon/wezterm-ghifarit53-{1,2,3}.svg`**: 어떤 빌드 경로에서도 참조되지 않는 대체 디자인 보관물입니다. 안전한 제거 후보입니다.
- **`icon/update.sh`와 `update.sh` 내 macOS/Linux 산출 경로**: `../macos/`·Linux PNG 생성 단계는 현 분기에서 죽은 경로입니다. Windows `.ico` 재생성 단계(`update.sh:26`)만 유효하므로, 스크립트를 단순화하려면 macOS/Linux 단계를 제거하고 Windows 단계만 남길 수 있습니다.
- **`terminal.png`(런타임 아이콘)와 `terminal.ico`(리소스 아이콘)의 분리**: 둘 다 `wezterm-icon.svg`에서 파생되지만 소비 경로가 다릅니다(전자는 `include_bytes!` 런타임, 후자는 `.rc` 임베드·설치 아이콘). 한쪽만 갱신하면 창 아이콘과 실행 파일 아이콘이 어긋나므로, 아이콘 변경 시 `update.sh`로 양쪽을 함께 재생성해야 합니다.
- **DLL 갱신**: `windows/` 내 네이티브 바이너리는 외부 프로젝트(MS Terminal, ANGLE, Mesa) 산출물입니다. 갱신 시 각 README에 기록된 출처·버전·라이선스 정보를 함께 갱신하고, ANGLE/Mesa의 ABI 호환성(EGL/GLES 버전, `opengl32.dll` 익스포트)을 검증해야 합니다.
- **라이선스 고지 유지**: 폰트(`LICENSE_OFL.txt` 등)와 네이티브 DLL은 각각 OFL·MIT·MITish 라이선스를 따릅니다. 자산을 제거·교체하더라도 배포물에 대응 고지가 누락되지 않도록 주의해야 합니다.
