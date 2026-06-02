# licenses 폴더 기능 명세

## 1. 개요 및 역할

`licenses/` 폴더는 WezTerm 바이너리에 함께 배포되는 **서드파티 구성 요소의 라이선스 원문**을 보관하는 비코드 디렉터리입니다. 빌드·실행 로직에는 일절 참여하지 않으며, 법적 고지(attribution) 의무를 충족하기 위한 정적 텍스트 자산만을 담습니다.

폴더의 의도는 `licenses/README.md:1-2`에 명시되어 있습니다.

> WezTerm bundles some components provided by third parties.
> Their respective licenses are included in this directory.

즉 WezTerm은 일부 구성 요소를 제3자가 제공한 형태로 번들하며, 해당 구성 요소들의 라이선스 텍스트를 이 디렉터리에 모읍니다. 본 폴더는 *컴파일러나 패키저가 자동으로 소비하는 입력*이 아니라, 배포물에 동봉되는 사람이 읽는 고지 문서의 보관소입니다.

이 폴더의 라이선스 의무는 Rust 의존성에 대한 라이선스 검증과는 별개의 트랙입니다. Rust 크레이트 의존성의 라이선스 정책은 `deny.toml`의 `[licenses]` 섹션(`deny.toml:87-105`)에서 `cargo deny check licenses`로 기계적으로 검증됩니다. 반면 `licenses/` 폴더는 Cargo 의존성 그래프에 속하지 않고 **사전 빌드된 바이너리 형태로 번들되는 네이티브 구성 요소**(예: ANGLE DLL)의 라이선스를 다룹니다. 두 메커니즘은 대상과 작동 방식이 다릅니다.

## 2. 디렉터리/파일 구성

폴더의 현재 구성은 다음 두 파일이 전부입니다.

| 파일 | 바이트 | 성격 | 내용 |
| --- | --- | --- | --- |
| `licenses/README.md` | 117 | 안내 텍스트 | 폴더의 목적 설명. 2줄. |
| `licenses/ANGLE.md` | 1643 | 라이선스 원문 | ANGLE 프로젝트의 BSD 3-Clause 변형 라이선스 전문. |

### 2.1 `README.md`

`licenses/README.md`는 폴더의 존재 이유를 설명하는 2줄짜리 안내문입니다(1절 인용 참조). 별도의 색인이나 파일 목록을 유지하지 않으므로, 향후 라이선스 파일이 추가되어도 이 README를 갱신할 필요는 없는 구조입니다.

### 2.2 `ANGLE.md`

`licenses/ANGLE.md`는 ANGLE(Almost Native Graphics Layer Engine) 프로젝트의 라이선스 전문입니다. 헤더는 `// Copyright 2018 The ANGLE Project Authors.`(`licenses/ANGLE.md:1`)로 시작하며, 본문은 BSD 3-Clause 계열의 재배포 조건을 담습니다.

- 소스/바이너리 재배포 시 저작권 고지와 조건 목록, 면책 조항 유지 의무(`licenses/ANGLE.md:8-14`).
- TransGaming Inc., Google Inc., 3DLabs Inc. Ltd. 및 기여자 이름을 사전 서면 허가 없이 제품 보증·홍보에 사용 금지(`licenses/ANGLE.md:16-19`).
- 표준 "AS IS" 무보증·책임 면제 조항(`licenses/ANGLE.md:21-32`).

원문이 C/C++ 주석(`//`) 형식으로 보존되어 있는 것은 ANGLE 소스 헤더에서 그대로 복사한 흔적입니다. 이 파일은 코드가 아니므로 주석 형식은 무의미하며, 사람이 읽는 고지 텍스트로만 기능합니다.

## 3. 빌드·패키징·실행과의 관계

### 3.1 ANGLE의 실제 소비처

`licenses/ANGLE.md`가 고지하는 ANGLE 바이너리는 `assets/windows/angle/` 아래에 사전 빌드된 DLL로 동봉되어 있습니다.

- `assets/windows/angle/libEGL.dll` (약 394 KiB)
- `assets/windows/angle/libGLESv2.dll` (약 4.6 MiB)

이 두 DLL은 ANGLE이 제공하는 EGL/OpenGL ES 구현체로, Windows에서 Direct3D 백엔드 위에 OpenGL ES API를 노출합니다. WezTerm의 OpenGL 렌더링 경로(`window` 크레이트의 EGL 로더, `window/src/egl.rs`)는 런타임에 `libloading`으로 EGL 라이브러리를 동적 적재합니다(`window/src/egl.rs:110-121`, `eglGetProcAddress` 심볼 해석). 즉 ANGLE은 정적 링크 대상이 아니라 **런타임 동적 적재 대상**이며, 그렇기 때문에 별도의 DLL 파일과 별도의 라이선스 고지가 필요합니다.

### 3.2 설치 패키지 동봉

ANGLE DLL은 Inno Setup 설치 스크립트를 통해 설치 디렉터리로 복사됩니다(`ci/windows-installer.iss:50-51`).

```
Source: "..\target\release\libEGL.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\libGLESv2.dll"; DestDir: "{app}"; Flags: ignoreversion
```

배포물에 ANGLE 바이너리가 포함되므로 BSD 3-Clause의 "바이너리 재배포 시 라이선스 고지 동반" 조건(`licenses/ANGLE.md:11-14`)이 발동합니다. `licenses/ANGLE.md`는 이 의무를 충족하는 고지 텍스트입니다.

다만 **현재 `ci/windows-installer.iss`는 `licenses/` 폴더 자체를 설치 산출물에 포함하지 않습니다**. 설치 스크립트의 `[Files]` 섹션(`ci/windows-installer.iss:45-54`)은 실행 파일과 DLL만 나열하며, `licenses/ANGLE.md`를 `{app}`로 복사하는 항목이 없습니다. 또한 `LicenseFile` 지시자도 주석 처리되어 있습니다(`ci/windows-installer.iss:24`). 따라서 라이선스 고지는 **소스 트리에는 존재하나 설치된 바이너리 배포물에는 동봉되지 않는** 상태입니다. 이는 5절에서 다룹니다.

### 3.3 빌드 과정과의 무관성

`licenses/` 폴더의 파일들은 Cargo 빌드(`build.rs`, `Cargo.toml`)나 어떤 코드 경로에서도 참조되지 않습니다. 전수 검색 결과 `licenses/ANGLE.md`를 입력으로 읽거나 경로로 지정하는 코드·스크립트는 존재하지 않습니다. 이 폴더는 빌드 의존성 그래프 바깥의 순수 문서 자산입니다.

### 3.4 고지가 누락된 다른 번들 구성 요소

WezTerm은 ANGLE 외에도 다음 네이티브 구성 요소를 번들하지만, 이들의 라이선스 고지는 `licenses/`가 아니라 각 자산 폴더의 README에 흩어져 있습니다.

- **Mesa** (`assets/windows/mesa/opengl32.dll`, 약 36 MiB): 소프트웨어 OpenGL 폴백. 라이선스 고지는 `assets/windows/mesa/README.md:4-6`에서 "largely MITish licenses"로 외부 URL(docs.mesa3d.org/license.html)을 참조할 뿐, 전문이 `licenses/`에 보관되어 있지 않습니다.
- **Microsoft Terminal conhost** (`assets/windows/conhost/conpty.dll`, `OpenConsole.exe`): MIT 라이선스로 제공되는 ConPTY 구현. 고지는 `assets/windows/conhost/README.md:3-5`에 서술되어 있습니다.

즉 본 폴더의 고지 정책은 현재 일관적이지 않습니다. ANGLE만 `licenses/`에 전문이 있고, Mesa·conhost는 자산 폴더의 README로 대체되어 있습니다. 이 비대칭은 5절의 정리 대상입니다.

## 4. Windows 전용 고려사항

- 이 저장소는 WezTerm의 Windows 전용 영구 분기이며, `licenses/ANGLE.md`가 고지하는 ANGLE DLL과 그 소비처(EGL 동적 적재, 설치 스크립트)는 모두 Windows 경로에 한정됩니다. ANGLE은 본래 크로스플랫폼 라이브러리이나, 본 분기에서는 Windows에서 Direct3D 백엔드를 통한 OpenGL ES 제공 용도로만 의미를 가집니다.
- 번들되는 모든 그래픽 관련 네이티브 구성 요소(ANGLE, Mesa)는 `assets/windows/` 하위에 위치하며, 비Windows 플랫폼용 라이선스 대상은 이미 제거된 상태입니다. 따라서 이 폴더에 비Windows 구성 요소의 라이선스가 추가될 이유는 없습니다.
- `ci/windows-installer.iss`는 Inno Setup 기반 Windows 전용 설치 스크립트이며, 라이선스 동봉 정책의 유일한 패키징 게이트입니다. 다른 플랫폼의 패키징 경로는 존재하지 않습니다.

## 5. 리팩토링/정리 주의점

1. **고지 위치의 비일관성 해소 검토.** 현재 ANGLE만 `licenses/`에 전문이 있고 Mesa·conhost는 각 자산 폴더 README에 분산되어 있습니다(3.4절). 번들 구성 요소의 라이선스를 한곳에 모으는 정책으로 통일하려면 `licenses/`로 Mesa·Microsoft Terminal 라이선스를 이전하고 README들을 교차 참조하도록 정리하는 방안을 고려할 수 있습니다. 단, 함부로 자산 폴더 README의 출처·빌드 절차 설명까지 옮기지 말아야 합니다. 그 README들은 라이선스뿐 아니라 바이너리의 출처와 재빌드 방법(`assets/windows/conhost/README.md:17-24` 등)을 담은 별개 목적의 문서입니다.

2. **설치 배포물에 라이선스 미동봉 상태 확인 필요.** `ci/windows-installer.iss`가 `licenses/` 폴더를 설치 산출물에 포함하지 않으므로(3.2절), 엄밀히 보면 바이너리 재배포 시 라이선스를 동반하라는 BSD 3-Clause 조건이 설치본 차원에서는 충족되지 않습니다. 정리 작업 시 `[Files]` 섹션에 `licenses/` 동봉 항목을 추가하거나 `LicenseFile` 지시자를 활성화하는 보강을 검토해야 합니다. 이는 본 폴더 자체의 변경이 아니라 패키징 스크립트의 변경 대상입니다.

3. **이 폴더는 코드 의존성이 없으므로 안전하게 이동·재구성 가능합니다.** 어떤 빌드 스크립트도 `licenses/ANGLE.md`를 경로로 참조하지 않습니다(3.3절). 다만 ANGLE DLL을 번들에서 제거하지 않는 한 `licenses/ANGLE.md`를 삭제해서는 안 됩니다. 라이선스 고지 삭제는 곧 BSD 3-Clause 위반입니다. 파일 삭제의 전제 조건은 `assets/windows/angle/` DLL의 제거입니다.

4. **`ANGLE.md`의 주석 형식은 보존 권장.** 파일이 `//` 주석 형태인 것은 ANGLE 원본 소스 헤더의 형식을 그대로 보존한 것입니다. 가독성을 이유로 주석 기호를 제거하면 원문과의 동일성 추적이 어려워질 수 있으므로, 라이선스 텍스트는 원형 그대로 유지하는 것이 안전합니다.

5. **README의 색인화 여부는 선택 사항.** `licenses/README.md`는 현재 파일 목록을 나열하지 않습니다. 라이선스 파일이 여러 개로 늘어날 경우 README에 구성 요소-파일 매핑 표를 추가하면 추적성이 향상되나, 현 시점에서는 단일 라이선스만 있어 필수는 아닙니다.
