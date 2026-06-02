# procinfo 폴더 기능 명세

본 문서는 `E:\Project\wezterm\procinfo` 폴더에 포함된 크레이트 `procinfo`의 상세 기능 명세다. 추후 리팩토링을 위한 근거 자료로서, 코드(`procinfo/src/lib.rs`, `procinfo/src/windows.rs`)를 직접 확인하여 작성하였다.

---

## 1. 개요 및 책임

`procinfo` 크레이트의 단일 책임은 **운영체제로부터 프로세스 트리 정보를 조회하여 플랫폼 독립적인 자료구조(`LocalProcessInfo`)로 제공**하는 것이다. 구체적으로는 다음을 수행한다.

- 임의의 루트 PID를 기준으로 한 **프로세스 트리 전체 스냅샷** 구성 (`with_root_pid`)
- 특정 PID의 **현재 작업 디렉터리(cwd)** 조회 (`current_working_dir`)
- 특정 PID의 **실행 이미지 경로** 조회 (`executable_path`)
- 프로세스 트리에서 **실행 파일 베이스 네임 집합** 추출 (`flatten_to_exe_names`)

이 정보들은 WezTerm이 터미널 탭/페인의 "전경 프로세스"를 식별하고, 셸이 보고하지 않을 때 cwd를 추론(divine)하며, 탭 제목·종료 확인 정책 등을 결정하는 데 사용된다.

Windows fork에서의 실제 책임은 명확하다. 본래 upstream의 `procinfo`는 macOS·Linux·FreeBSD 등 다수 플랫폼별 구현을 보유했으나, 이 분기에서는 **Windows 구현(`windows.rs`)만 실제로 동작**한다. 데이터 수집은 전적으로 Toolhelp32 스냅샷과 `NtQueryInformationProcess` + `ReadProcessMemory`를 통한 PEB(Process Environment Block) 직접 판독에 의존한다. 즉 이 크레이트는 Windows 커널 자료구조를 안전한 Rust 타입으로 변환하는 **하위 수준 시스템 어댑터** 역할을 한다.

---

## 2. 워크스페이스 내 위치

### 의존성 (deps)

| 크레이트 | 사용 목적 |
|---|---|
| `luahelper` | `impl_lua_conversion_dynamic!` 매크로로 `LocalProcessInfo`를 Lua 값으로 상호 변환 (feature `lua`에서만) |
| `wezterm-dynamic` | `FromDynamic`/`ToDynamic` derive 제공 — 동적 직렬화 계층 (feature `lua`에서만, `std` 기능 활성) |
| `libc` | `wcslen`으로 와이드 문자열 길이 계산 (`cmd_line_to_argv`) |
| `log` | 진단용 `trace` 로깅 |
| `ntapi` *(cfg(windows))* | PEB/`RTL_USER_PROCESS_PARAMETERS`(32/64비트) 구조체, `NtQueryInformationProcess` 선언 |
| `winapi` *(cfg(windows))* | Toolhelp32, 프로세스 핸들·메모리·시간·셸 API (features: handleapi, memoryapi, psapi, processthreadsapi, shellapi, tlhelp32, winbase) |

### 피의존 (usedBy)

| 크레이트 | 사용 형태 |
|---|---|
| `mux` | `mux/src/localpane.rs`에서 `LocalProcessInfo`로 페인의 root/foreground 프로세스 트리를 보관, `with_root_pid`·`flatten_to_exe_names`로 전경 프로세스 식별 및 cwd 추론 |
| `procinfo-funcs` (`lua-api-crates/procinfo-funcs`) | Lua API 함수 `with_root_pid`·`current_working_dir_for_pid`·`executable_path_for_pid`로 노출 |

이 크레이트는 **계층의 최하단(시스템 인접) 어댑터**에 위치한다. OS 프로세스 정보를 수집하여 상위 `mux`(터미널 멀티플렉서)와 Lua 설정 계층(`procinfo-funcs`)에 공급하며, 그 자체는 워크스페이스 내 다른 도메인 크레이트에 의존하지 않는다(luahelper/wezterm-dynamic는 직렬화 보조).

---

## 3. 공개 API 표면

크레이트가 외부에 노출하는 항목은 모두 `procinfo/src/lib.rs`에 정의된 두 타입과 그 메서드다. `windows.rs`의 모든 헬퍼(`Snapshot`, `ProcHandle`, `ProcParams` 등)는 비공개이며, `impl LocalProcessInfo` 블록을 통해서만 공개 메서드를 추가한다.

### `pub enum LocalProcessStatus`
- 정의: `procinfo/src/lib.rs:11-24`
- 변형: `Idle, Run, Sleep, Stop, Zombie, Tracing, Dead, Wakekill, Waking, Parked, LockBlocked, Unknown`
- 용도: 프로세스 상태의 플랫폼 독립 표현. Windows 구현은 항상 `Run`만 채운다(§5 불변식 참조). 나머지 변형은 다른 플랫폼 호환용으로 사실상 죽은 값이다.

### `pub struct LocalProcessInfo`
- 정의: `procinfo/src/lib.rs:28-59`
- 단일 프로세스 노드이자, `children`을 통해 재귀적으로 **프로세스 트리**를 표현한다.

### `impl LocalProcessInfo`의 공개 메서드

| 시그니처 | 위치 | 용도 |
|---|---|---|
| `pub fn flatten_to_exe_names(&self) -> HashSet<String>` | `lib.rs:67` | 서브트리를 순회하여 실행 파일 베이스 네임의 유일 집합 반환. 경로가 달라도 동일 파일명이면 1개로 합침 |
| `pub fn current_working_dir(pid: u32) -> Option<PathBuf>` | `windows.rs:346` | 단일 PID의 cwd 조회 (트리 미구성, 단발 조회) |
| `pub fn executable_path(pid: u32) -> Option<PathBuf>` | `windows.rs:353` | 단일 PID의 실행 이미지 경로 조회 |
| `pub fn with_root_pid(pid: u32) -> Option<Self>` | `windows.rs:359` | 루트 PID부터 자식까지 프로세스 트리 전체를 구성. 스냅샷에서 해당 PID를 못 찾으면 `None` |

`flatten_to_exe_names`는 플랫폼 무관(`lib.rs`)이고, 나머지 셋은 Windows 전용(`windows.rs`, `#![cfg(windows)]`)이다.

---

## 4. 내부 구조

크레이트는 2개 모듈로 구성된다.

- `lib.rs` — 플랫폼 독립 타입 정의(`LocalProcessStatus`, `LocalProcessInfo`)와 트리 순회 로직(`flatten_to_exe_names`), Lua 변환 바인딩. `windows` 모듈을 `mod windows;`로 포함.
- `windows.rs` — Windows 한정(`#![cfg(windows)]`) 실제 데이터 수집 구현. 본 크레이트의 무게중심으로, 비공개 인프라 타입 3종과 모든 시스템 콜 래퍼를 담는다.

### 데이터 흐름 (트리 구성, `with_root_pid`)

1. `Snapshot::entries()` (`windows.rs:43`) — `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS)`로 전체 프로세스 목록(`PROCESSENTRY32W` 벡터)을 한 번에 확보. `ProcIter`가 `Process32FirstW`/`Process32NextW`를 순회하며 RAII로 핸들을 닫는다.
2. 루트 PID에 해당하는 엔트리를 선형 탐색(`windows.rs:413`).
3. `build_proc` (`windows.rs:364`) 재귀 — 스냅샷 슬라이스에서 `th32ParentProcessID == 현재 PID`인 항목을 자식으로 삼아 트리를 내려가며 구성한다.
4. 각 노드마다 `ProcHandle::new`로 프로세스를 열고(`OpenProcess`), 다음을 채운다.
   - `executable()` — `QueryFullProcessImageNameW`
   - `get_params()` — PEB 판독으로 argv·cwd·console 추출
   - `start_time()` — `GetProcessTimes`의 생성 시각
5. 핸들 열기/판독 실패 시 폴백: `executable`은 스냅샷의 `szExeFile`로, `name`은 그 파일명으로 대체(`windows.rs:393-397`). 즉 정보가 일부 누락되어도 노드는 항상 생성된다.

### 제어 흐름 (PEB 판독, `get_params`)

`get_params` (`windows.rs:186`)는 먼저 `get_peb32_addr`로 **WOW64(32비트 on Win64) 여부를 판별**한다.

- WOW64 프로세스: `get_params_32` → `RTL_USER_PROCESS_PARAMETERS32`를 직접 읽음.
- 네이티브 64비트: `get_params_64` → `PROCESS_BASIC_INFORMATION` → `PEB` → `RTL_USER_PROCESS_PARAMETERS` 순으로 포인터를 따라가며 읽음.

양쪽 모두 `read_process_wchar`로 대상 프로세스 주소공간에서 CommandLine·CurrentDirectory 와이드 문자열을 복사한 뒤, `cmd_line_to_argv`(`CommandLineToArgvW`)로 argv를 파싱한다.

비대한 모듈은 `windows.rs`(약 416행)이며, 시스템 콜 래핑·메모리 판독·트리 구성이 모두 이 한 파일에 집중되어 있다.

---

## 5. 핵심 데이터 구조·타입

### `LocalProcessInfo` (`lib.rs:28`)

| 필드 | 타입 | 의미 |
|---|---|---|
| `pid` | `u32` | 프로세스 식별자 |
| `ppid` | `u32` | 부모 프로세스 식별자 |
| `name` | `String` | COMM 이름. 본 Windows 구현에서는 실행 이미지의 파일명에서 파생 |
| `executable` | `PathBuf` | 실행 이미지 경로 |
| `argv` | `Vec<String>` | 인자 벡터 |
| `cwd` | `PathBuf` | 현재 작업 디렉터리. 접근 실패 시 빈 경로 |
| `status` | `LocalProcessStatus` | 프로세스 상태 |
| `start_time` | `u64` | 시스템 의존 단위의 생성 시각 |
| `console` | `u64` *(cfg(windows) 전용)* | 연결된 콘솔 핸들 값 |
| `children` | `HashMap<u32, LocalProcessInfo>` | 자식 프로세스(키=pid) — 트리 표현 |

#### 불변식
- `children`의 키는 항상 자식 노드의 `pid`와 일치한다(`build_proc`가 `kid.th32ProcessID`로 삽입).
- Windows 구현에서 `status`는 **항상 `LocalProcessStatus::Run`**으로 설정된다(`windows.rs:407`). Toolhelp32는 상태를 제공하지 않으므로, `Run` 이외 변형은 이 분기에서 산출되지 않는다.
- `console` 필드는 `#[cfg(windows)]` 게이트 하에만 존재한다. 비Windows 빌드에서는 구조체 레이아웃이 달라지므로, 이 필드를 채우는 코드는 Windows 한정이다.
- `name`은 `executable.file_name()`에서 파생되며, 추출 실패 시 빈 문자열(`windows.rs:394-397`). COMM 이름과의 직접 대응은 보장하지 않는다(주석 `lib.rs:33-37` 참조).

### `LocalProcessStatus` (`lib.rs:11`)
열거형으로, 12개 변형 중 Windows 경로에서 실제 사용되는 것은 `Run` 1개뿐이다(나머지는 타 플랫폼 호환을 위한 명목상 값).

### 비공개 인프라 타입 (`windows.rs`)
- `Snapshot(HANDLE)` — Toolhelp32 스냅샷 핸들의 RAII 래퍼. `Drop`에서 `CloseHandle`. 불변식: 생성 성공 시 핸들은 non-null.
- `ProcHandle { pid, proc: HANDLE }` — `OpenProcess`로 연 핸들의 RAII 래퍼. `Drop`에서 `CloseHandle`. **자기 자신의 PID는 거부**(`windows.rs:108`, 자기 검사 시 교착 회피).
- `ProcParams { argv, cwd, console }` — PEB 판독 결과를 묶는 임시 값.

---

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
|---|---|
| `ntapi` | 문서화되지 않은 NT 내부 구조체에 접근하기 위함. `PEB`, `RTL_USER_PROCESS_PARAMETERS`/`...32`, `NtQueryInformationProcess`, `ProcessBasicInformation`/`ProcessWow64Information` 클래스. cwd·argv는 공개 Win32 API로 얻을 수 없어 PEB를 직접 읽어야 하므로 필수다. |
| `winapi` | 공개 Win32 표면: Toolhelp32 스냅샷(`tlhelp32`), 프로세스 열기/시간(`processthreadsapi`), 메모리 판독(`memoryapi`의 `ReadProcessMemory`), 핸들 해제(`handleapi`), `CommandLineToArgvW`(`shellapi`), `QueryFullProcessImageNameW`·`LocalFree`(`winbase`). |
| `libc` | 와이드 NUL 종단 문자열 길이 측정(`wcslen`)에만 사용. `CommandLineToArgvW`가 반환한 각 인자의 길이를 구하는 용도(`windows.rs:330`). |
| `luahelper` / `wezterm-dynamic` | `LocalProcessInfo`를 Lua 설정/스크립트로 노출하기 위한 동적 직렬화. `lua` feature가 꺼지면 컴파일에서 제외. |
| `log` | `trace` 레벨 진단 로깅(핸들 열기/해제, 트리 구성 단계). |

GPU·셰이핑 등 무거운 서드파티는 사용하지 않는다. 본 크레이트의 외부 의존은 전부 OS 시스템 API 접근과 직렬화에 한정된다.

---

## 7. 설정·기능 플래그

`Cargo.toml`에 정의된 feature는 다음과 같다.

| feature | 효과 |
|---|---|
| `default = ["lua"]` | 기본으로 `lua` 활성 |
| `lua` | `luahelper`·`wezterm-dynamic`(std) 의존을 켜고, `LocalProcessStatus`/`LocalProcessInfo`에 `FromDynamic`/`ToDynamic` derive 부여 및 `impl_lua_conversion_dynamic!(LocalProcessInfo)` 적용(`lib.rs:60-61`) |

`lua` feature를 끄면 직렬화 derive와 Lua 변환이 제거되고, 순수 데이터 수집 라이브러리로 축소된다. 이 크레이트 자체에는 사용자 대면 config 항목이 없다. 다만 산출 데이터는 상위 `mux`에서 cwd 추론 정책 등 설정 동작에 소비된다.

---

## 8. Windows 전용 고려사항

- **모듈 게이트**: `windows.rs`는 `#![cfg(windows)]`로 전체가 Windows에서만 컴파일된다. 비Windows에서는 `LocalProcessInfo`의 `with_root_pid`·`current_working_dir`·`executable_path`가 존재하지 않는다(즉 이 분기에서 비Windows 빌드는 사실상 미지원).
- **`console` 필드**: `LocalProcessInfo.console`은 `#[cfg(windows)]`로 게이트되어 플랫폼 간 구조체 레이아웃이 달라진다(`lib.rs:55-56`).
- **NT 내부 의존**: PEB·`RTL_USER_PROCESS_PARAMETERS` 판독은 문서화되지 않은 커널 ABI에 의존한다. Windows 버전에 따라 오프셋·레이아웃이 바뀔 수 있는 취약 지점이다. 방어책으로 `read_process_wchar`는 `MAX_PATH * 4` 초과 길이를 거부하여(`windows.rs:253`) 잘못된 오프셋 판독으로 인한 과대 할당을 막는다.
- **WOW64 분기**: 32비트 프로세스를 64비트 호스트에서 검사할 때 `RTL_USER_PROCESS_PARAMETERS32`(32비트 포인터 폭) 경로를 별도로 탄다(`get_params_32`, `windows.rs:232`). 이는 죽은 코드가 아니라 실제로 필요한 분기다.
- **자기 PID 회피**: `ProcHandle::new`는 자기 자신을 여는 것을 거부한다(`windows.rs:108`). 자기 PEB를 `ReadProcessMemory`로 읽을 때의 교착 위험 회피.
- **권한 제약**: `OpenProcess`는 `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ`를 요청한다. 권한 부족(예: 상위 권한 프로세스)이나 보호 프로세스에서는 핸들 획득이 실패하며, 이때 §4의 스냅샷 폴백으로 부분 정보만 채운다.
- **죽은 코드**: `LocalProcessStatus`의 `Run`을 제외한 11개 변형은 비Windows 플랫폼 표현을 위한 것으로, 이 분기에서 산출되지 않는다.
- **`libc` 사용**: 비Windows 함수가 아닌 `wcslen`만 호출하므로 Windows에서 정상 동작한다.

---

## 9. 리팩토링 주의점

- **`build_proc`의 O(N²) 트리 구성**: `with_root_pid`는 각 노드마다 전체 스냅샷 슬라이스를 선형 순회해 자식을 찾는다(`windows.rs:367-371`). 프로세스 수가 많으면 이차 비용이 발생한다. 부모 PID로 미리 인덱싱하면 개선 가능하나, 트리 구조 계약(`children: HashMap`)은 유지해야 한다.
- **노드당 `OpenProcess` 비용**: 트리의 모든 프로세스에 대해 핸들을 열고 PEB를 판독한다. 권한 실패가 흔한 환경에서는 대량 실패 로그·시스템 콜 오버헤드가 누적된다. 호출자(`mux/src/localpane.rs`)가 `CachePolicy`로 빈도를 제어하므로, 호출 빈도 변경 시 이 비용을 함께 고려해야 한다.
- **NT ABI 결합**: `ntapi` 구조체 레이아웃에 직접 의존하므로 Windows 빌드/버전 변경에 취약하다. `read_struct`/`read_process_wchar`는 `unsafe`로 원시 메모리를 `T`로 재해석한다. 오프셋 가정이 깨지면 `assume_init`이 비정상 값을 산출할 수 있다. 길이 상한 외의 검증은 없다.
- **`status` 항상 `Run`**: 상위 코드가 `status`를 신뢰하면 안 된다. 현재 호출자(`mux`)는 주로 `executable`·`flatten_to_exe_names`·`cwd`를 사용한다. 상태 기반 로직을 추가하려면 별도 수집 경로가 필요하다.
- **`#[cfg(windows)]` 필드와 직렬화의 상호작용**: `console` 필드가 플랫폼 게이트되어 있어, 비Windows에서 동일 타입을 `FromDynamic`으로 역직렬화할 때 필드 차이가 발생한다. 본 분기는 Windows 전용이므로 실문제는 없으나, 타입을 공유 크레이트로 옮길 때 주의해야 한다.
- **단발 조회 vs 트리 구성 중복**: `current_working_dir`/`executable_path`(단일 PID)와 `with_root_pid`(트리)가 동일한 `ProcHandle` 경로를 별도로 사용한다. 호출자가 트리를 이미 구성했다면 단발 API 재호출은 중복 비용이다.
- **순환 의존 없음**: 의존은 단방향(luahelper/wezterm-dynamic/libc/log/ntapi/winapi → procinfo)이며 워크스페이스 내 순환은 확인되지 않았다.
- **`flatten_to_exe_names`의 재귀**: 깊은 프로세스 트리에서 스택 재귀를 사용한다(`lib.rs:70`). 실무상 트리 깊이는 얕아 문제되지 않으나, 신뢰 불가 깊이를 다룰 일이 생기면 반복 구현을 고려해야 한다.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `procinfo/Cargo.toml` | 29 | 패키지·feature(`lua`)·의존성 정의. Windows 타깃 한정 `ntapi`/`winapi` 선언 |
| `procinfo/src/lib.rs` | 85 | 플랫폼 독립 타입(`LocalProcessStatus`, `LocalProcessInfo`), 트리 순회(`flatten_to_exe_names`), Lua/dynamic 변환 바인딩, `windows` 모듈 포함 |
| `procinfo/src/windows.rs` | 416 | Windows 전용 데이터 수집 핵심. Toolhelp32 스냅샷, `OpenProcess`·PEB 판독(32/64·WOW64), `GetProcessTimes`, argv 파싱, 프로세스 트리 구성(`with_root_pid`) 및 단발 조회(`current_working_dir`/`executable_path`) |

생성 파일(`[생성]`)은 없다. 두 소스 파일 모두 수작업 구현이다.
