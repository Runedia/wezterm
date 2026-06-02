# pty 폴더 기능 명세

본 문서는 워크스페이스 폴더 `pty/`에 포함된 단일 크레이트 `portable-pty`(버전 0.9.0)에 대한 상세 기능 명세다. WezTerm의 Windows 전용 영구 분기라는 전제 위에서, 추후 리팩토링을 목적으로 "무엇을·왜·어떻게"를 기술한다. 모든 코드 인용은 `파일경로:라인` 형식을 따른다.

---

## 1. 개요 및 책임

`portable-pty`는 운영체제가 제공하는 의사 터미널(pseudo terminal, pty) 인터페이스를 추상화하여, 런타임에 서로 다른 구현을 선택할 수 있는 **트레이트 기반 API**를 제공하는 크레이트다(`pty/src/lib.rs:1`). 단일 책임은 "pty 한 쌍(master/slave)을 생성하고, slave 측으로 자식 프로세스를 spawn하며, master 측으로 입출력·리사이즈·종료 제어를 노출하는 것"이다.

크레이트 이름은 "portable"(이식 가능)이지만, 이 분기에서의 실제 책임은 **Windows 전용**으로 좁혀져 있다. 비Windows 구현은 이미 제거되었고, 다음 사실이 이를 입증한다.

- `lib.rs`가 무조건 `std::os::windows::prelude`를 import한다(`pty/src/lib.rs:45`). cfg 가드가 없으므로 이 크레이트는 Windows에서만 컴파일된다.
- 네이티브 pty 시스템 타입이 ConPTY 구현으로 직접 고정되어 있다: `pub type NativePtySystem = win::conpty::ConPtySystem;`(`pty/src/lib.rs:293`).
- `Cargo.toml`에 `[target."cfg(windows)".dependencies]`만 존재하고, unix 계열 의존성 섹션은 없다(`pty/Cargo.toml:24`).

따라서 이 분기에서 `portable-pty`의 실제 역할은 다음 두 가지다.

1. **ConPTY 백엔드**: Windows의 ConPTY(Pseudo Console) API를 감싸 master/slave pty 쌍을 만들고 자식 프로세스를 그 안으로 spawn한다(`pty/src/win/`).
2. **직렬 포트 백엔드**: COM 포트 등 시리얼 연결을 pty처럼 다루는 대체 구현을 제공한다(`pty/src/serial.rs`). 시리얼은 프로세스를 spawn할 수 없으므로 `new_default_prog` 명령만 허용한다.

상위 계층(`mux`, `wezterm-gui` 등)은 이 크레이트의 트레이트(`PtySystem`, `MasterPty`, `SlavePty`, `Child`)에만 의존하여 백엔드 교체 가능성을 유지한다.

---

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 용도 |
|---|---|
| `filedescriptor` | 파이프 생성(`Pipe`), 핸들 소유(`OwnedHandle`)·복제(`dup`), `FileDescriptor` 기반 읽기/쓰기 추상화. ConPTY와 시리얼 양쪽에서 핵심 |
| `anyhow` | 오류 전파(`Result`, `bail!`, `ensure!`, `Context`) |
| `downcast-rs` | 트레이트 객체(`MasterPty` 등)를 구체 타입으로 다운캐스트하기 위한 `Downcast`/`impl_downcast!` |
| `log` | 진단 로깅(`trace`/`debug`/`error`) |
| `serde`(optional) | `serde_support` 피처에서 `PtySize`·`CommandBuilder` 직렬화 |
| `serial2` | 시리얼 포트 설정·입출력(보레이트, 패리티, 흐름제어 등) |
| `shell-words` | `as_unix_command_line`에서 인자를 unix 셸 규칙으로 quoting |
| `bitflags`, `lazy_static`, `shared_library`, `winapi`, `winreg` | Windows 전용. ConPTY 동적 로딩, Win32 API 바인딩, 레지스트리 기반 환경 변수 수집 |

> 비고: `Cargo.toml`은 `bitflags`를 windows 의존성으로 선언하나(`pty/Cargo.toml:25`), 현재 `src` 내부에서 직접 사용하는 지점은 확인되지 않는다. 리팩토링 시 제거 후보다.

### 피의존(usedBy)

| 크레이트 | 사용 맥락 |
|---|---|
| `mux` | 로컬 pane(`localpane.rs`), 도메인(`domain.rs`), tmux pty 등에서 pty 생성·spawn |
| `config` | pty 관련 설정 타입 참조 |
| `codec` | 직렬화 프로토콜에서 pty 관련 타입 |
| `mux-lua` (`lua-api-crates/mux`) | Lua API에서 mux를 통한 간접 사용 |
| `wezterm`, `wezterm-gui` | CLI/GUI 진입점에서 native pty 시스템 구동 |
| `wezterm-client`, `wezterm-mux-server`, `wezterm-mux-server-impl` | 클라이언트/서버 멀티플렉서 |
| `wezterm-ssh` | SSH 채널을 pty 트레이트로 래핑(`wezterm-ssh/src/pty.rs`) |

### 계층상 위치

`portable-pty`는 워크스페이스의 **최하위 OS 추상화 계층** 중 하나다. OS의 ConPTY/시리얼 API 바로 위에 위치하며, 그 위로 `mux`(세션·pane 관리)가 얹히고, 다시 `wezterm-gui`/`wezterm`(표현·CLI)이 얹힌다. 즉 "OS pty → portable-pty → mux → gui/cli"의 파이프라인에서 OS 경계를 캡슐화하는 어댑터 역할을 한다.

---

## 3. 공개 API 표면

### 3.1 진입점 함수

- `pub fn native_pty_system() -> Box<dyn PtySystem + Send>`(`pty/src/lib.rs:289`) — 시스템 네이티브 구현(`ConPtySystem`)을 박싱하여 반환. 호출자가 백엔드를 의식하지 않고 pty를 얻는 표준 경로.
- `pub type NativePtySystem = win::conpty::ConPtySystem`(`pty/src/lib.rs:293`) — 네이티브 구현 타입 별칭.

### 3.2 핵심 트레이트

| 트레이트 | 핵심 메서드 | 용도 |
|---|---|---|
| `PtySystem: Downcast` | `openpty(&self, size: PtySize) -> anyhow::Result<PtyPair>`(`pty/src/lib.rs:214`) | master/slave 쌍 생성 |
| `MasterPty: Downcast + Send` | `resize`, `get_size`, `try_clone_reader`, `take_writer`(`pty/src/lib.rs:82`) | 제어 끝단: 리사이즈, 크기 조회, 읽기 스트림 복제, 쓰기 핸들 인출 |
| `SlavePty` | `spawn_command(&self, cmd: CommandBuilder) -> Result<Box<dyn Child + Send + Sync>>`(`pty/src/lib.rs:134`) | slave 끝단으로 자식 프로세스 spawn |
| `Child: Debug + ChildKiller + Downcast + Send` | `try_wait`, `wait`, `process_id`, `as_raw_handle`(`pty/src/lib.rs:102`) | 자식 프로세스 대기·식별 |
| `ChildKiller: Debug + Downcast + Send` | `kill`, `clone_killer`(`pty/src/lib.rs:121`) | 자식 종료 신호. `clone_killer`로 `.wait` 블로킹과 독립적으로 종료 신호 전송 가능 |

`MasterPty`, `Child`, `ChildKiller`, `PtySystem`에는 `impl_downcast!`가 적용되어 트레이트 객체를 구체 타입으로 환원할 수 있다.

`take_writer`는 한 번만 호출할 수 있다는 불변식을 가진다. 두 번째 호출은 오류를 반환한다(`pty/src/lib.rs:96`, ConPTY 구현은 `pty/src/win/conpty.rs:99`, 시리얼 구현은 `pty/src/serial.rs:227`). 쓰기 핸들 drop은 slave에 EOF를 전달한다.

### 3.3 핵심 구조체

- `PtySize { rows, cols, pixel_width, pixel_height: u16 }`(`pty/src/lib.rs:57`) — pty 표시 영역 크기. `Default`는 24행 80열(`pty/src/lib.rs:70`). `serde_support` 시 직렬화 가능.
- `PtyPair { slave, master }`(`pty/src/lib.rs:204`) — slave가 먼저 선언되어 **slave가 master보다 먼저 drop**된다(RFC 1857 drop 순서, 주석으로 명시 `pty/src/lib.rs:205`).
- `ExitStatus { code: u32, signal: Option<String> }`(`pty/src/lib.rs:141`) — 종료 상태. 생성자 `with_exit_code`/`with_signal`, 조회자 `success`/`exit_code`/`signal`. `std::process::ExitStatus`에서 `From` 변환 제공(`pty/src/lib.rs:179`). `Display` 구현 포함.
- `CommandBuilder`(`cmdbuilder` 모듈에서 re-export, `pty/src/lib.rs:48`) — 아래 3.4 참조.

### 3.4 `CommandBuilder` 공개 API (`pty/src/cmdbuilder.rs:141`)

`std::process::Command`와 유사한 인터페이스로 spawn할 명령을 준비한다.

- 생성: `new(program)`(`:151`), `from_argv(args)`(`:161`), `new_default_prog()`(`:186`) — 기본 프로그램용. `is_default_prog()`(`:196`)로 판별.
- 인자: `arg`(`:202`, default_prog에 호출 시 panic), `args`(`:226`), `replace_default_prog`(`:215`), `get_argv`/`get_argv_mut`(`:236`,`:240`).
- 환경: `env`(`:245`), `env_remove`(`:262`), `env_clear`(`:270`), `get_env`(`:274`), `iter_extra_env_as_str`(`:306`, 호출자가 명시 설정한 것만), `iter_full_env_as_str`(`:324`).
- 작업 디렉터리: `cwd`(`:288`), `clear_cwd`(`:295`), `get_cwd`(`:299`).
- 제어 터미널: `set_controlling_tty`/`get_controlling_tty`(`:176`,`:180`) — 기본 true. (Windows에서는 사실상 무의미한 플래그. 9절 참조)
- 명령행 표현: `as_unix_command_line()`(`:340`, unix 셸 quoting), `get_shell()`(`:431`, `ComSpec` 환경변수 또는 `cmd.exe`).
- `pub(crate)`: `current_directory()`(`:381`), `environment_block()`(`:410`), `cmdline()`(`:440`) — Win32 spawn 시 내부에서만 사용.

### 3.5 시리얼 백엔드 (`pty/src/serial.rs`)

- `pub struct SerialTty`(`:23`) — `PtySystem` 구현체. 생성 `new(port)`(`:33`)과 세터 `set_baud_rate`/`set_char_size`/`set_parity`/`set_stop_bits`/`set_flow_control`(`:44`–`:60`).

### 3.6 Windows 백엔드 (`pty/src/win/`)

- `pub struct ConPtySystem`(`pty/src/win/conpty.rs:9`) — 기본 `PtySystem` 구현.
- `pub struct ConPtyMasterPty`(`:75`), `pub struct ConPtySlavePty`(`:80`) — master/slave 구현.
- `pub struct WinChild`(`pty/src/win/mod.rs:21`), `pub struct WinChildKiller`(`:66`) — 자식·종료기. `WinChild`는 `Future`도 구현(`:124`).
- `pub struct PsuedoCon`(`pty/src/win/psuedocon.rs:66`) — ConPTY 핸들 래퍼. 상수 `PSUEDOCONSOLE_INHERIT_CURSOR`/`PSEUDOCONSOLE_RESIZE_QUIRK`/`PSEUDOCONSOLE_WIN32_INPUT_MODE`/`PSEUDOCONSOLE_PASSTHROUGH_MODE`(`:27`–`:31`) 노출.
- `pub struct ProcThreadAttributeList`(`pty/src/win/procthreadattr.rs:10`) — spawn 시 ConPTY를 자식에 연결하는 속성 리스트.

---

## 4. 내부 구조

모듈은 다음과 같이 분해된다.

```
lib.rs          공개 트레이트·타입 정의, native_pty_system 진입점, std::process::Child용 트레이트 impl
cmdbuilder.rs   CommandBuilder: 인자·환경·cwd 빌더, Win32용 cmdline/환경블록 생성 (가장 비대)
serial.rs       SerialTty: 시리얼 포트 기반 대체 백엔드
win/mod.rs      WinChild/WinChildKiller: 자식 프로세스 핸들 관리, GetExitCodeProcess 폴링, Future
win/conpty.rs   ConPtySystem/Master/Slave: 파이프 2쌍 + PsuedoCon으로 pty 쌍 구성
win/psuedocon.rs    PsuedoCon: ConPTY DLL 동적 로딩, CreatePseudoConsole/Resize/Close, CreateProcessW spawn
win/procthreadattr.rs   ProcThreadAttributeList: PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE 속성 설정
```

### 데이터 흐름 (ConPTY 경로)

1. `ConPtySystem::openpty`(`pty/src/win/conpty.rs:13`)가 stdin·stdout용 익명 파이프 쌍을 각각 만든다(`filedescriptor::Pipe`).
2. `PsuedoCon::new`(`pty/src/win/psuedocon.rs:80`)가 stdin의 read 끝, stdout의 write 끝을 ConPTY에 넘겨 콘솔을 생성한다.
3. `Inner` 구조체(`pty/src/win/conpty.rs:46`)가 `PsuedoCon`, 읽기 가능 끝(`readable` = stdout.read), 쓰기 가능 끝(`writable` = stdin.write), 현재 `size`를 `Arc<Mutex<>>`로 보관한다. master와 slave가 동일한 `Inner`를 공유한다(`:36`).
4. `MasterPty::try_clone_reader`는 stdout.read를 복제하여 출력 스트림을 제공하고, `take_writer`는 stdin.write를 한 번만 인출한다.
5. `SlavePty::spawn_command`(`:111`)는 잠금을 잡고 `PsuedoCon::spawn_command`(`pty/src/win/psuedocon.rs:113`)를 호출한다.

### 제어 흐름 (spawn)

`PsuedoCon::spawn_command`는 `STARTUPINFOEXW`를 구성하되 표준 핸들을 모두 `INVALID_HANDLE_VALUE`로 설정한다(`pty/src/win/psuedocon.rs:122`–`:125`). 이는 부모(예: 로그 파일로 리다이렉트된 wezterm-mux-server)의 핸들이 자식에 상속되어 출력이 엉뚱한 곳으로 가는 것을 막기 위함이다(주석 `:117`). `ProcThreadAttributeList`로 ConPTY를 자식에 바인딩한 뒤 `CreateProcessW`(`:139`)로 spawn하고, 스레드 핸들은 즉시 `OwnedHandle`로 감싸 누수를 막으며(`:168`), 프로세스 핸들은 `WinChild`에 보관한다.

### 대기·종료 흐름

`WinChild::is_complete`(`pty/src/win/mod.rs:26`)는 `GetExitCodeProcess`로 비블로킹 폴링하여 `STILL_ACTIVE`면 `None`을 반환한다. `wait`(`:92`)는 `WaitForSingleObject(INFINITE)`로 블로킹한다. `Future` 구현(`:124`)은 미완료 시 별도 스레드를 띄워 핸들 대기 후 `waker.wake()`를 호출하는 방식으로 비동기 대기를 제공한다.

### 비대 모듈

`cmdbuilder.rs`(575행)가 단일 파일로 가장 크다. 환경 변수 수집(레지스트리 포함), Win32 명령행/환경블록 인코딩, 인자 quoting(`append_quoted`)이 모두 한 파일에 모여 있다.

---

## 5. 핵심 데이터 구조·타입

### `PtySize`(`pty/src/lib.rs:57`)
불변식: 의미상 행·열은 1 이상이어야 콘솔이 정상 동작하나 타입 차원에서 강제하지 않는다. `pixel_width`/`pixel_height`는 백엔드가 무시할 수 있다(ConPTY/시리얼 모두 픽셀값을 콘솔에 반영하지 않음).

### `ExitStatus`(`pty/src/lib.rs:141`)
불변식: `signal`이 `Some`이면 항상 실패로 간주되며 `code`는 1로 고정된다(`with_signal`, `pty/src/lib.rs:153`). `success()`는 `signal == None && code == 0`일 때만 true. Windows 경로에서는 시그널 개념이 없으므로 실제로는 `code`만 채워진다.

### `EnvEntry`(`pty/src/cmdbuilder.rs:11`)
Windows의 **대소문자 무시 환경 변수**를 다루기 위한 내부 타입. 불변식: `BTreeMap`의 키는 항상 소문자화된 값(`map_key`, `:24`)이고, 실제 spawn에 쓰이는 표시는 `preferred_key`(원래 케이싱)다. `is_from_base_env`는 베이스 환경(프로세스 환경 + 레지스트리)에서 온 것인지, 호출자가 명시 설정한 것인지를 구분한다. 이 분리 덕에 `iter_extra_env_as_str`가 호출자 설정분만 골라낸다(`:306`).

### `CommandBuilder`(`pty/src/cmdbuilder.rs:141`)
불변식:
- `args`가 비어 있으면 default_prog 상태(`is_default_prog`, `:196`). 이 상태에서 `arg`/`replace_default_prog`의 호출 가능 여부가 갈린다(`arg`는 panic, `replace_default_prog`는 비-default에서 panic).
- 명령행 인코딩 시 인자에 NUL(0)이 포함되면 오류(`:460`). `cmdline`/`environment_block`은 모두 NUL 종단된 와이드 문자열을 만든다.

### `Inner`(`pty/src/win/conpty.rs:46`)
불변식: master/slave가 `Arc<Mutex<Inner>>`를 공유하므로 리사이즈·spawn·핸들 접근은 모두 동일 잠금 하에서 직렬화된다. `writable`은 `Option`이며 `take_writer`로 한 번 인출되면 `None`이 되어 재인출 불가.

### `PsuedoCon`(`pty/src/win/psuedocon.rs:66`)
불변식: `con` 핸들은 `Drop`에서 `ClosePseudoConsole`로 정확히 한 번 해제된다(`:73`). `unsafe impl Send/Sync`(`:70`)로 스레드 간 이동을 허용하나, 핸들 수명은 `Inner`의 `Arc`가 보장한다.

---

## 6. 외부 의존성

- **filedescriptor** — 이 크레이트의 골격. ConPTY 파이프(`Pipe::new`), 소유 핸들(`OwnedHandle`), 핸들 복제(`dup`/`try_clone`), `FileDescriptor` 읽기/쓰기를 제공한다. master의 reader/writer가 모두 이 타입으로 구현된다.
- **serial2** — 시리얼 백엔드의 실제 동작. `SerialPort`, `CharSize`, `Parity`, `StopBits`, `FlowControl`를 사용해 포트를 열고 raw 모드·타임아웃을 설정한다(`pty/src/serial.rs:67`–`:86`).
- **winapi** — Win32 직접 호출. `consoleapi`/`namedpipeapi`/`processthreadsapi`/`handleapi`/`synchapi`/`fileapi`/`winuser` 피처를 켜 `CreateProcessW`, `GetExitCodeProcess`, `TerminateProcess`, `WaitForSingleObject`, `InitializeProcThreadAttributeList` 등을 사용한다.
- **shared_library** — ConPTY 함수(`CreatePseudoConsole` 등)를 `kernel32.dll` 또는 사이드로딩된 `conpty.dll`에서 **런타임 동적 로딩**한다(`pty/src/win/psuedocon.rs:33`–`:60`). 컴파일 타임 링크가 아니라 런타임 로딩인 이유는, 구형 Windows에서 함수 부재를 감지해 명시적 패닉 메시지를 내고, 최신 conpty.dll을 곁들여 배포할 수 있게 하기 위함이다.
- **winreg** — 베이스 환경 구성 시 시스템(HKLM)·사용자(HKCU) 환경 변수를 레지스트리에서 읽어 들이고 `REG_EXPAND_SZ`를 확장한다(`pty/src/cmdbuilder.rs:50`–`:131`).
- **lazy_static** — 동적 로딩한 `ConPtyFuncs`를 전역 `CONPTY`로 한 번만 초기화(`pty/src/win/psuedocon.rs:62`).
- **downcast-rs** — 트레이트 객체를 구체 백엔드 타입으로 환원(상위 계층이 ConPTY 특화 동작에 접근할 때).
- **shell-words** — `as_unix_command_line`의 unix 셸 quoting.

---

## 7. 설정·기능 플래그

- **Cargo 피처**: `default = []`, `serde_support = ["serde"]`(`pty/Cargo.toml:20`–`:22`). `serde_support`를 켜면 `PtySize`(`pty/src/lib.rs:56`), `CommandBuilder`·`EnvEntry`(`pty/src/cmdbuilder.rs:10`,`:140`)가 `Serialize`/`Deserialize`를 파생한다. 이는 mux 서버/클라이언트가 명령·크기 정보를 네트워크로 직렬화할 때 필요하다.
- **런타임 환경 의존 항목**:
  - `ComSpec` — default_prog 셸 결정(`pty/src/cmdbuilder.rs:433`,`:444`). 미설정 시 `cmd.exe`.
  - `PATH`/`PATHEXT` — 실행 파일 탐색(`search_path`, `pty/src/cmdbuilder.rs:353`). `PATHEXT` 미설정 시 `.EXE`만 시도.
  - `USERPROFILE` — cwd 미지정 시 폴백 작업 디렉터리(`current_directory`, `pty/src/cmdbuilder.rs:382`).
- **ConPTY 생성 플래그**: 항상 `INHERIT_CURSOR | RESIZE_QUIRK | WIN32_INPUT_MODE`를 켠다(`pty/src/win/psuedocon.rs:87`). `PASSTHROUGH_MODE`는 정의만 되어 있고 사용되지 않는다(`#[allow(dead_code)]`, `:30`).
- 이 크레이트 자체에는 `config` 크레이트의 항목과 직접 결합된 코드가 없다. 시리얼 설정(보레이트 등)은 `SerialTty` 세터로 코드 차원에서 주입한다.

---

## 8. Windows 전용 고려사항

- **무조건 Windows 가정**: `lib.rs`가 cfg 없이 `std::os::windows::prelude`를 import하므로(`pty/src/lib.rs:45`) 비Windows에서는 컴파일 불가. 즉 이 크레이트는 cfg 분기가 거의 없는 순수 Windows 구현으로 정리되어 있다.
- **`Child::as_raw_handle`이 트레이트의 정식 메서드**: 일반적으로 Windows 전용인 프로세스 핸들 반환이 공통 트레이트에 들어가 있다(`pty/src/lib.rs:116`). upstream의 크로스플랫폼 흔적이며, 이 분기에서는 항상 유효한 핸들 경로다.
- **Win32 직접 호출 지점**:
  - 자식 생성: `CreateProcessW`(`pty/src/win/psuedocon.rs:139`), 표준 핸들 무효화로 핸들 상속 차단(`:122`).
  - 종료: `TerminateProcess`(`pty/src/win/mod.rs:43`,`:72`, 그리고 `pty/src/lib.rs:250`의 `ProcessSignaller`).
  - 대기/상태: `GetExitCodeProcess`(`pty/src/win/mod.rs:29`), `WaitForSingleObject`(`:98`,`:141`), `GetProcessId`(`:110`).
  - 속성: `InitializeProcThreadAttributeList`/`UpdateProcThreadAttribute`/`DeleteProcThreadAttributeList`(`pty/src/win/procthreadattr.rs`).
- **ConPTY 동적 로딩과 최소 버전**: `kernel32.dll`에 ConPTY 함수가 없으면 "Windows 10 October 2018 이상 필요"라는 메시지와 함께 패닉한다(`pty/src/win/psuedocon.rs:48`). 사이드로딩된 `conpty.dll`이 있으면 그것을 우선한다(`:55`).
- **대소문자 무시 환경 변수**: `EnvEntry`/`map_key`가 Windows 환경 변수 의미론을 재현한다(`pty/src/cmdbuilder.rs:8`).
- **죽은/비활성 코드**:
  - `CommandBuilder::set_controlling_tty`/`get_controlling_tty`(`pty/src/cmdbuilder.rs:176`)와 필드 `controlling_tty`는 flatpak(Linux 컨테이너) 회피용 주석을 단 채 남아 있으나, ConPTY/시리얼 spawn 경로 어디서도 이 값을 읽지 않는다. Windows에서 의미 없는 죽은 플래그다.
  - `PSEUDOCONSOLE_PASSTHROUGH_MODE` 상수(`pty/src/win/psuedocon.rs:31`).
  - `Cargo.toml`의 `bitflags` 의존성은 `src`에서 직접 사용 지점이 보이지 않는다(`pty/Cargo.toml:25`).
  - 시리얼 `Reader::read` 주석은 "unix에서는 블로킹하지 않는다"고 하나(`pty/src/serial.rs:245`), 이 분기에서는 Windows 타임아웃 경로만 유효하다.

---

## 9. 리팩토링 주의점

- **공유 잠금(`Arc<Mutex<Inner>>`) 결합**: ConPTY의 master/slave는 동일 `Inner`를 공유한다(`pty/src/win/conpty.rs:27`,`:36`). `try_clone_reader`/`take_writer`/`resize`/`spawn_command`가 모두 같은 뮤텍스를 잡으므로, `spawn_command` 같은 잠재적 장기 작업이 잠금을 보유하는 동안 다른 master 연산이 블록될 수 있다. 잠금 분리 시 핸들 수명·`writable` take 불변식을 깨지 않도록 주의.
- **`take_writer` 1회 불변식**: 트레이트 계약(`pty/src/lib.rs:95`)과 두 백엔드 구현(ConPTY `Option::take`, 시리얼 `RefCell<bool>`)이 각자 다른 방식으로 강제한다. 한쪽만 바꾸면 계약이 어긋난다.
- **`PtyPair` drop 순서 의존**: 필드 선언 순서(slave 먼저)에 동작이 의존한다(`pty/src/lib.rs:205`). 필드 재배치나 명시적 `Drop` 추가 시 순서 보장을 깨지 않아야 한다.
- **`unsafe impl Send/Sync` for `PsuedoCon`**(`pty/src/win/psuedocon.rs:70`) 및 `Future`의 `PassRawHandleToWaiterThread`(`pty/src/win/mod.rs:132`): raw 핸들을 스레드 경계로 넘기는 수동 Send 보증이다. 핸들 수명·소유권 모델을 바꾸면 데이터 레이스 위험.
- **`Future` 구현이 매 poll마다 스레드 생성**(`pty/src/win/mod.rs:139`): 미완료 상태로 반복 poll되면 스레드를 거듭 spawn할 수 있다. 폴링 빈도가 높은 호출자와 결합하면 비용이 누적된다. 재구현 시 일회성 대기 스레드/IOCP로 개선 여지.
- **`cmdbuilder.rs`의 광범위 책임**: 환경 수집(레지스트리 I/O 포함)·인코딩·quoting이 한 파일에 집중되어 단위 테스트·재사용이 어렵다. `get_base_env`는 매 `CommandBuilder::new` 호출마다 레지스트리를 읽으므로(`pty/src/cmdbuilder.rs:33`,`:154`) 다수 spawn 시 비용이 반복된다. 캐싱 시 환경 변경 반영과의 트레이드오프 고려.
- **`search_path`의 확장자 처리 한계**: 사용자가 준 경로의 확장자를 `with_extension`으로 교체하는 방식이라, 점이 포함된 경로명에서 의도와 다르게 동작할 수 있다(코드 주석 자체가 "potentially wrong"이라 명시, `pty/src/cmdbuilder.rs:364`).
- **순환 의존**: 없음. `portable-pty`는 워크스페이스 하위 크레이트를 의존하지 않고 외부 크레이트만 의존하므로 단방향이다.
- **트레이트 표면의 플랫폼 누수**: `Child::as_raw_handle`(`pty/src/lib.rs:116`)가 공통 트레이트에 Windows 핸들을 노출한다. Windows 전용 분기 정체성에 맞춰 트레이트를 정리할 때 상위 소비자(mux 등)의 호출부 파급을 확인해야 한다.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `pty/Cargo.toml` | 41 | 패키지 메타·의존성·피처(`serde_support`)·Windows 전용 의존성 선언 |
| `pty/src/lib.rs` | 293 | 공개 트레이트(`PtySystem`/`MasterPty`/`SlavePty`/`Child`/`ChildKiller`), `PtySize`/`PtyPair`/`ExitStatus`, `native_pty_system` 진입점, `std::process::Child`용 트레이트 impl, `ProcessSignaller` |
| `pty/src/cmdbuilder.rs` | 575 | `CommandBuilder`와 `EnvEntry`. 레지스트리 기반 베이스 환경 수집, Win32 cmdline/환경블록 인코딩, 인자 quoting. 단위 테스트 2건 포함. 가장 비대 |
| `pty/src/serial.rs` | 267 | 시리얼 포트 백엔드 `SerialTty`(serial2). NOP 자식(`SerialChild`)·캐리어 검출 기반 종료 감지, 타임아웃 기반 reader |
| `pty/src/win/mod.rs` | 149 | `WinChild`/`WinChildKiller`: 프로세스 핸들 보유, `GetExitCodeProcess` 폴링, `WaitForSingleObject` 대기, `Future` 구현 |
| `pty/src/win/conpty.rs` | 117 | `ConPtySystem`/`ConPtyMasterPty`/`ConPtySlavePty`/`Inner`. 파이프 2쌍 + `PsuedoCon`으로 pty 쌍 구성, 공유 `Arc<Mutex<Inner>>` |
| `pty/src/win/psuedocon.rs` | 175 | `PsuedoCon`: ConPTY DLL 동적 로딩(kernel32/conpty.dll), `CreatePseudoConsole`/`ResizePseudoConsole`/`ClosePseudoConsole`, `CreateProcessW` spawn |
| `pty/src/win/procthreadattr.rs` | 71 | `ProcThreadAttributeList`: `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`로 ConPTY를 자식 프로세스에 바인딩 |
| `pty/examples/{bash,narrow,whoami,whoami_async}.rs` | 각 73~85 | API 사용 예제(문서·검증용). 코어 빌드에 비포함 |

> 비고: 이 크레이트에는 생성(generated) 데이터 파일이 없다. 모든 `.rs`는 수기 작성 코드다.
