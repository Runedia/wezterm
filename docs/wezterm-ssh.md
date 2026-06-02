# wezterm-ssh 폴더 기능 명세

본 문서는 `E:\Project\wezterm\wezterm-ssh` 크레이트의 기능 명세다. WezTerm의 Windows 전용 영구 분기를 전제로 하며, 추후 리팩토링을 위한 상세 참조를 목적으로 한다. 모든 코드 인용은 `파일경로:라인` 형식을 따른다.

---

## 1. 개요 및 책임

`wezterm-ssh`는 두 개의 하위 SSH 라이브러리(`libssh-rs`, `ssh2`)를 단일하고 더 편리한 고수준 API로 추상화하는 크레이트다. `Cargo.toml:7`의 설명대로 "libssh2를 감싸는 더 편리한 고수준 래퍼"로 출발했으나, 실제로는 두 백엔드를 동시에 노출하는 추상화 계층이다.

단일 책임은 **"하나의 SSH 세션을 전용 OS 스레드에서 구동하고, 그 세션 위에서 PTY/exec 채널과 SFTP 파일시스템 연산을 비동기 채널 메시지로 중계하는 것"**이다. 구체적으로 다음을 담당한다.

- `ssh_config(5)` 형식 설정 파일의 파싱·토큰 확장·호스트 매칭 (`config.rs`).
- TCP 연결 수립, ProxyCommand 실행, 주소 패밀리 필터링 (`sessioninner.rs:320`).
- 호스트 키 검증과 사용자 인증(공개키·비밀번호·keyboard-interactive·에이전트) (`host.rs`, `auth.rs`).
- 원격 PTY 생성, 명령 실행, 채널 I/O 펌핑 (`pty.rs`, `sessioninner.rs`).
- SFTP 파일·디렉터리 연산 (`sftp/`, `sftpwrap.rs`).

Windows fork에서의 실제 책임은 위와 동일하나, 연결·에이전트 포워딩·소켓 처리 코드가 `std::os::windows::io` API에 직접 결합되어 있어(`sessioninner.rs:211`, `sessioninner.rs:343`, `sessioninner.rs:843`) 사실상 Windows 전용으로 고정되었다. ProxyCommand는 `cmd /c`로 실행된다(`sessioninner.rs:329`).

이 크레이트는 동시성 모델로 **단일 워커 스레드 + smol 채널** 패턴을 채택한다. 외부에서 보이는 `Session`/`Sftp`/`File`은 모두 메시지를 보내는 핸들이며, 실제 SSH 라이브러리 객체는 워커 스레드(`SessionInner::run`, `sessioninner.rs:64`) 안에서만 존재한다. 이는 `ssh2`/`libssh-rs` 객체가 `Send`/`Sync`를 만족하지 않거나 블로킹 호출을 요구하는 문제를, 비동기 호출자로부터 격리하기 위한 구조다.

---

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 용도 |
|---|---|
| `async_ossl` | 직접 사용되지 않음. openssl vendored 기능 선택을 워크스페이스 전체에서 일원화하기 위한 의존(`Cargo.toml:42`). |
| `filedescriptor` | `socketpair`, `FileDescriptor`, `poll`, `AsRawSocketDescriptor` 제공. 채널 I/O와 PTY 파이프의 기반. |
| `portable-pty` | `MasterPty`, `Child`, `ChildKiller`, `PtySize`, `ExitStatus` 트레이트/타입. `SshPty`/`SshChildProcess`가 구현. |
| `termwiz` | dev-dependency 전용(`Cargo.toml:50`). 본 빌드 산출물에는 포함되지 않음. |
| `wezterm-uds` | Windows에서 SSH 에이전트 소켓(`UnixStream`)에 연결하기 위한 UDS 구현(`sessioninner.rs:841`). |

이외에 직접 명시된 외부 의존은 6절 참조.

### 피의존(usedBy)

| 크레이트 | 사용 형태 |
|---|---|
| `config` | `wezterm_ssh::Config`로 ssh_config를 로드해 호스트 설정 맵을 만든다(`config/src/ssh.rs:103`). |
| `mux` | `wezterm_ssh::ConfigMap`/`Config`를 SSH 도메인 연결에 사용(`mux/src/ssh.rs:61`, `mux/src/ssh.rs:185`). |
| `ssh-funcs` | Lua API로 ssh 설정을 노출. `Config`로 호스트 목록을 열거(`lua-api-crates/ssh-funcs/src/lib.rs:22`). |
| `wezterm-client` | 원격 mux 연결 시 `wezterm_ssh::Config`를 사용(`wezterm-client/src/client.rs:827`). |
| `wezterm-gui` | SSH 도메인/연결 UI에서 소비. |

### 계층 위치

`wezterm-ssh`는 OS 소켓·하위 SSH 라이브러리 바로 위, mux/도메인 계층 바로 아래에 위치하는 **전송·세션 계층**이다. 상위 크레이트는 이 크레이트가 제공하는 `Session`/`Sftp` 핸들과 `SessionEvent` 스트림을 통해 SSH 연결의 수명주기와 PTY를 제어하며, 하위 SSH 라이브러리(`libssh-rs`/`ssh2`)의 세부를 직접 다루지 않는다.

---

## 3. 공개 API 표면

공개 항목은 `lib.rs`의 `pub use`(`lib.rs:17`-`lib.rs:29`)를 통해 평탄화되어 노출된다. 핵심 표면은 다음과 같다.

### 세션 (`session.rs`)

- `pub struct Session { tx: SessionSender }` — 클론 가능한 세션 핸들. 드롭 시 워커에 `SessionDropped`를 전송(`session.rs:81`).
  - `Session::connect(config: ConfigMap) -> anyhow::Result<(Self, Receiver<SessionEvent>)>` (`session.rs:89`) — 워커 스레드를 띄우고, 핸들과 이벤트 수신 채널을 반환. 진입점.
  - `async fn request_pty(&self, term, size: PtySize, command_line: Option<&str>, env) -> Result<(SshPty, SshChildProcess)>` (`session.rs:131`).
  - `async fn exec(&self, command_line: &str, env) -> Result<ExecResult>` (`session.rs:157`).
  - `fn sftp(&self) -> Sftp` (`session.rs:185`) — SFTP 핸들 생성. 실제 SFTP 서브시스템은 첫 연산 시 지연 초기화.
- `pub enum SessionEvent` (`session.rs:16`) — `Banner`, `HostVerify`, `Authenticate`, `HostVerificationFailed`, `Error`, `Authenticated`. 호출자가 폴링하며 인증·검증 상호작용을 수행.
- `pub struct ExecResult { stdin, stdout, stderr: FileDescriptor, child: SshChildProcess }` (`session.rs:192`).
- `pub struct DeadSession` (`session.rs:50`) — 워커가 종료된 세션에 대한 오류 타입.

### 설정 (`config.rs`)

- `pub type ConfigMap = BTreeMap<String, String>` (`config.rs:6`) — 소문자 키로 정규화된 옵션 맵. 워커가 소비하는 최종 입력.
- `pub struct Config` (`config.rs:405`) — ssh_config 파싱·해석 컨텍스트.
  - `new`/`default`, `set_option`, `add_config_string`, `add_config_file`, `add_default_config_files`(`config.rs:473`), `assign_environment`, `assign_tokens`.
  - `for_host(host) -> ConfigMap` (`config.rs:520`) — 호스트별 최종 옵션 맵 해석. 핵심 메서드.
  - `loaded_config_files() -> Vec<PathBuf>` (`config.rs:763`), `enumerate_hosts() -> Vec<String>` (`config.rs:780`).

### 인증·호스트 검증 (`auth.rs`, `host.rs`)

- `pub struct AuthenticationEvent { username, instructions, prompts: Vec<AuthenticationPrompt>, .. }` (`auth.rs:11`) + `pub struct AuthenticationPrompt { prompt, echo }` (`auth.rs:5`). `answer`/`try_answer`로 응답.
- `pub struct HostVerificationEvent { message, .. }` (`host.rs:13`) — `answer(trust_host)`/`try_answer`.
- `pub struct HostVerificationFailed { remote_address, key, file }` (`host.rs:6`) — 호스트 키 불일치 오류.

### PTY·프로세스 (`pty.rs`)

- `pub struct SshPty` — `portable_pty::MasterPty`와 `std::io::Write` 구현(`pty.rs:34`, `pty.rs:44`).
- `pub struct SshChildProcess` — `portable_pty::Child`/`ChildKiller` 구현, `async_wait`(`pty.rs:86`) 제공.

### SFTP (`sftp/`)

- `pub struct Sftp` (`sftp/mod.rs:56`) — 비동기 SFTP 연산 핸들. `open`/`open_with_mode`/`create`/`open_dir`/`read_dir`/`create_dir`/`remove_dir`/`metadata`/`symlink_metadata`/`set_metadata`/`symlink`/`read_link`/`canonicalize`/`rename`/`remove_file`.
- `pub struct File` (`sftp/file.rs:16`) — `smol::io::AsyncRead`/`AsyncWrite` 구현. `metadata`/`set_metadata`/`fsync`.
- `pub struct Dir` (`sftp/dir.rs:9`) — `read_dir`.
- 타입: `Metadata`, `FileType`, `FilePermissions`, `OpenOptions`, `OpenFileType`, `WriteMode`, `RenameOptions` (`sftp/types.rs`).
- 오류: `SftpChannelError`, `SftpChannelResult<T>`, `SftpError`, `SftpResult<T>` (`sftp/mod.rs`, `sftp/error.rs`).

### 재노출(`lib.rs:27`-`lib.rs:29`)

`Utf8Path`/`Utf8PathBuf`(camino), `FileDescriptor`(filedescriptor), `Child`/`ChildKiller`/`MasterPty`/`PtySize`(portable_pty)는 공개 API에 등장하므로 그대로 재노출된다.

---

## 4. 내부 구조

### 모듈 분해

| 모듈 | 역할 계층 |
|---|---|
| `config` | ssh_config 파싱·해석. 다른 모듈과 거의 독립. |
| `session` | 외부 핸들(`Session`) + 요청/이벤트 메시지 타입 정의. |
| `sessioninner` | 워커 스레드 본체. 이벤트 루프와 디스패처. 가장 비대(1108행). |
| `auth`, `host` | `SessionInner`에 메서드를 추가하는 impl 블록. 인증·호스트 검증 절차. |
| `pty` | PTY/프로세스 핸들 + `SessionInner::new_pty`/`resize_pty`. |
| `sessionwrap`, `channelwrap`, `filewrap`, `dirwrap`, `sftpwrap` | `ssh2`/`libssh-rs` 두 백엔드를 enum dispatch로 통합하는 어댑터 계층. |
| `sftp` | 비동기 SFTP 핸들(`Sftp`/`File`/`Dir`), 메시지 타입, 메타데이터 타입, 오류. |

### 제어 흐름

핵심은 **요청-응답 메시지 패스**다.

1. 호출자가 `Session::connect`로 워커 스레드를 띄운다(`session.rs:127`). 워커는 `SessionInner::run`(`sessioninner.rs:64`)을 실행한다.
2. 워커는 `wezterm_ssh_backend` 옵션에 따라 `run_impl_libssh`(`sessioninner.rs:110`) 또는 `run_impl_ssh2`(`sessioninner.rs:251`)로 분기. 기본값은 `libssh-rs` 기능이 켜져 있으면 `"libssh"`(`sessioninner.rs:78`).
3. 연결(`connect_to_host`) → 배너 송신 → 호스트 검증 → 인증 → `Authenticated` 이벤트 → `request_loop`(`sessioninner.rs:418`)로 진입.
4. 인증·검증 단계는 `SessionEvent`를 `tx_event`로 보내고, 호출자의 응답을 일회용 smol 채널로 `smol::block_on`하여 기다린다(`auth.rs:107`, `host.rs:56`). 즉, 워커 스레드가 호출자 응답에 대해 블로킹한다.

### 데이터 흐름 — `request_loop`

`request_loop`(`sessioninner.rs:418`)는 다음을 반복한다.

- `do_keepalive`(`sessioninner.rs:387`) — libssh 백엔드에서만 `ServerAliveInterval`마다 IGNORE 패킷 송신. ssh2는 no-op.
- `tick_io`(`sessioninner.rs:525`) — 채널별 stdin 버퍼를 원격 채널로 쓰고, 원격 stdout/stderr를 로컬 버퍼로 읽음. 모든 디스크립터가 닫힌 채널은 제거.
- `drain_request_pipe`(`sessioninner.rs:605`) — 깨우기용 소켓 파이프 비움.
- `dispatch_pending_requests`(`sessioninner.rs:610`) → `dispatch_one_request`(`sessioninner.rs:615`) — 요청 큐를 모두 처리. 각 요청 처리 동안 세션을 일시적으로 블로킹 모드로 전환.
- `connect_pending_agent_forward_channels`(`sessioninner.rs:835`) — libssh의 에이전트 포워드 채널 수락.
- 종료 조건: 채널이 없고 세션이 드롭됨(`sessioninner.rs:428`).
- `poll`(`sessioninner.rs:468`)로 깨우기 파이프 + 세션 소켓 + 모든 채널 디스크립터를 감시. 지수 백오프 슬립(`sleep_delay += sleep_delay`, `sessioninner.rs:469`)을 쓰되, 이벤트가 오면 100ms로 리셋.

### 깨우기 메커니즘

`SessionSender`(`session.rs:26`)는 smol 채널 송신과 함께, 워커가 `poll`에서 자고 있을 때 깨우기 위한 별도의 소켓 파이프에 `"x"` 1바이트를 쓴다(`session.rs:32`). 이 socketpair는 `connect`에서 생성된다(`session.rs:92`).

### 비대 모듈

- `config.rs`(1637행)는 대부분 테스트(`config.rs:802` 이후 `mod test`). 비테스트 로직은 약 800행. 스냅샷 테스트가 큰 비중을 차지.
- `sessioninner.rs`(1108행)는 디스패처(`dispatch_one_request`)가 거대한 `match`로 모든 요청 변형을 처리하여 단일 메서드가 길다(약 220행). 워커의 모든 책임이 한 파일에 집중.

---

## 5. 핵심 데이터 구조·타입

### `SessionInner` (`sessioninner.rs:41`)

워커 스레드의 전체 상태.

- `config: ConfigMap` — 해석된 옵션.
- `tx_event: Sender<SessionEvent>`, `rx_req: Receiver<SessionRequest>` — 양방향 메시지 채널.
- `channels: HashMap<ChannelId, ChannelInfo>`, `files: HashMap<FileId, FileWrap>`, `dirs: HashMap<DirId, DirWrap>` — 활성 자원 테이블.
- `next_channel_id`/`next_file_id` — 단조 증가 ID 발급기.
- `sender_read` — 깨우기 파이프의 읽기 끝.
- `session_was_dropped`, `shown_accept_env_error`, `last_keep_alive`, `keep_alive`.

불변식: `ChannelId`/`FileId`/`DirId`는 모두 `usize`(`sessioninner.rs:39`, `sftp/file.rs:9`, `sftp/dir.rs:6`)이며 `next_*_id`로만 발급되어 충돌하지 않음. 단, `open_dir`은 `next_file_id`를 공유 사용하므로(`sessioninner.rs:1003`) 파일 ID 공간과 디렉터리 ID 공간이 동일 카운터를 공유한다. `files`/`dirs`는 별도 맵이라 충돌은 없으나, ID 발급기를 공유한다는 점은 리팩토링 시 주의.

### `ChannelInfo` (`sessioninner.rs:31`) / `DescriptorState` (`sessioninner.rs:26`)

- `channel: ChannelWrap`, `exit: Option<Sender<ExitStatus>>`, `exited: bool`.
- `descriptors: [DescriptorState; 3]` — 인덱스 0=stdin, 1=stdout, 2=stderr. 각각 `fd: Option<FileDescriptor>`와 `buf: VecDeque<u8>`(용량 8192).
- 불변식: 디스크립터 3개가 모두 `None`이면 채널은 죽은 것으로 간주되어 제거됨(`sessioninner.rs:590`). PTY의 경우 stdout과 stderr가 같은 소켓을 가리킴(`pty.rs:257` `write_to_stderr = write_to_stdout.try_clone()`).

### `Session`/`SessionSender` (`session.rs:77`, `session.rs:26`)

- `Session`은 `SessionSender` 하나만 보유하는 클론 가능 핸들. 마지막 `Session`이 드롭될 때 워커가 종료되도록 `Drop`에서 `SessionDropped` 전송(`session.rs:81`).

### `SessionEvent` (`session.rs:16`)

세션 수명주기 이벤트. `HostVerify`/`Authenticate`는 응답 채널을 담은 이벤트 페이로드를 포함하며, 호출자가 반드시 응답해야 워커가 진행한다(블로킹).

### SFTP `Metadata` 및 권한 (`sftp/types.rs`)

- `Metadata { ty: FileType, permissions: Option<FilePermissions>, size, uid, gid, accessed, modified }` (`sftp/types.rs:154`).
- `FileType`(Dir/File/Symlink/Other)와 `FilePermissions`는 Unix mode 비트셋과 상호 변환(`from_unix_mode`/`to_unix_mode`). 시간은 u64 Unix epoch 초로 정규화.
- 불변식: 두 백엔드의 상이한 메타데이터 표현을 이 중립 타입으로 통일. `FilePermissions`는 9개 bool 필드로 owner/group/other × read/write/exec를 표현.

### `OpenOptions` (`sftp/types.rs:196`)

`read: bool`, `write: Option<WriteMode>`, `mode: i32`, `ty: OpenFileType`. ssh2에서는 `Ssh2OpenFlags`로(`sftp/types.rs:290`), libssh에서는 `libc` O_* 플래그 조합으로 변환(`sftpwrap.rs:43`). `WriteMode::Write`는 ssh2에서 TRUNCATE를 포함(`sftp/types.rs:299`).

### `SftpError` (`sftp/error.rs:9`)

SFTP 프로토콜 상태 코드(1~21)를 정수 값으로 갖는 enum. 14~21은 `ssh2` 전용으로 `cfg` 게이트(`sftp/error.rs:39` 이후). `from_error_code`/`to_error_code`로 코드 변환, `ssh2::Error`에서 `TryFrom` 변환.

### `SftpChannelError` (`sftp/mod.rs:29`)

SFTP 연산 최상위 오류. `Sftp(SftpError)`, `FileIo(io::Error)`, `SendFailed(anyhow::Error)`, `RecvFailed(RecvError)`, 백엔드별 `Ssh2`/`LibSsh`, `NotImplemented`.

---

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
|---|---|
| `libssh-rs` (optional, vendored) | 기본 SSH 백엔드. 에이전트 포워딩·known_hosts 자동 갱신·로그 콜백·keyboard-interactive를 지원(`sessioninner.rs:134`, `auth.rs:131`). vendored 기능으로 libssh를 정적 링크. |
| `ssh2` (optional, `openssl-on-win32`) | 대체 SSH 백엔드. libssh2 바인딩. 에이전트 인증·known_hosts 파일 직접 관리(`auth.rs:31`, `host.rs:106`). |
| `smol` | 비동기 채널(`smol::channel::bounded`)과 `block_on`. 핸들↔워커 메시징의 근간. |
| `socket2` | TCP 소켓 직접 생성·바인드·연결, 주소 패밀리 선택(`sessioninner.rs:359`). |
| `filedescriptor` | socketpair·poll·소켓 디스크립터 추상화. 채널 펌핑의 기반. |
| `portable-pty` | PTY/Child 트레이트 정의. GUI/mux가 로컬 PTY와 동일 인터페이스로 SSH PTY를 다루게 함. |
| `regex` | ssh_config 패턴(글롭→정규식)과 `${ENV}` 확장(`config.rs:41`, `config.rs:749`). |
| `filenamegen` | ssh_config `Include` 글롭 확장(`config.rs:178`). |
| `dirs-next` | 홈 디렉터리 해석(`config.rs:474`). |
| `gethostname` | `%l`/`%L` 토큰의 로컬 호스트명(`config.rs:490`). |
| `sha2` + `hex` | `%C` 토큰(해시) 계산(`config.rs:723`). |
| `base64` | ssh2 호스트 키 SHA256 지문 인코딩(`host.rs:134`). |
| `bitflags` | `FileType`/`FilePermission` 비트 플래그(`sftp/types.rs:3`). |
| `camino` | UTF-8 경로(`Utf8Path`/`Utf8PathBuf`). SFTP 경로는 UTF-8을 강제. |
| `libc` | libssh 백엔드의 O_* 파일 플래그(`sftpwrap.rs:40`). |
| `wezterm-uds` | Windows에서 SSH 에이전트 UDS 연결(`sessioninner.rs:841`). |
| `thiserror`/`anyhow` | 오류 타입 정의/전파. |
| `log` | 추적 로깅, libssh 로그 콜백 중계(`sessioninner.rs:167`). |

---

## 7. 설정·기능 플래그

### Cargo 기능 플래그 (`Cargo.toml:13`-`Cargo.toml:17`)

- `default = ["libssh-rs", "ssh2"]` — 두 백엔드 모두 컴파일.
- `libssh-rs` / `ssh2` — 각 백엔드 활성화. 둘 다 꺼지면 `compile_error!`(`lib.rs:1`).
- `vendored-openssl`, `vendored-openssl-ssh2`, `vendored-openssl-libssh-rs` — openssl vendored 선택을 백엔드별/전체로 전파.

### 런타임 백엔드 선택

- `wezterm_ssh_backend` 옵션(값 `"libssh"` 또는 `"ssh2"`)으로 런타임 분기(`sessioninner.rs:73`). 미지정 시 `libssh-rs`가 컴파일되어 있으면 `"libssh"`.
- `wezterm_ssh_verbose` = `"true"`이면 상세 트레이스/패킷 로깅 활성화(`sessioninner.rs:136`, `sessioninner.rs:286`).

### 소비되는 ssh_config 옵션 (`ConfigMap` 키)

워커가 직접 읽는 키: `hostname`, `user`, `port`, `serveraliveinterval`(`session.rs:101`), `identityagent`, `identityfile`, `userknownhostsfile`, `pubkeyacceptedtypes`, `bindaddress`, `hostkeyalgorithms`, `forwardagent`, `proxycommand`, `addressfamily`, `identitiesonly`. `Config::for_host`(`config.rs:520`)는 `port`(기본 22), `user`, `userknownhostsfile`, `identityfile`, `identityagent`의 기본값을 보강한다.

### 토큰·환경 확장 (`config.rs`)

- 토큰: `%h %n %p %r %i %u %l %L %d %C %j %T %%`. `%j`(ProxyJump)와 `%T`(터널)는 미지원으로 빈 문자열/`NONE` 처리(`config.rs:707`, `config.rs:716`).
- 옵션별 확장 대상은 `should_expand_tokens`(`config.rs:632`)·`should_expand_environment`(`config.rs:618`)로 결정. `${NAME}` 환경 변수 확장 지원(`config.rs:748`).
- `Match` 스탠자의 2단계 파싱(canonical/final)은 미지원. `needs_reparse`가 true이면 경고만 출력(`config.rs:540`). `Match Exec`도 미구현(`config.rs:112`).

---

## 8. Windows 전용 고려사항

이 fork는 비Windows 분기를 이미 제거했으며, 본 크레이트에는 명시적 `cfg(unix)`가 남아 있지 않다. 잔존 플랫폼 분기는 `pty.rs:144`의 `#[cfg(windows)]` `as_raw_handle` 한 곳뿐이다.

### Windows API 직접 결합 지점

- `connect_to_host`(`sessioninner.rs:320`): ProxyCommand를 `COMSPEC`(`cmd`) `/c`로 실행(`sessioninner.rs:329`). socketpair를 `IntoRawSocket`/`FromRawSocket`으로 변환하여 `socket2::Socket`을 구성(`sessioninner.rs:343`). 이 `unsafe` 블록은 `std::os::windows::io`를 무조건 사용하므로 비Windows에서는 컴파일되지 않는다.
- libssh 소켓 주입: `sock.into_raw_socket()`(`sessioninner.rs:211`)으로 raw 소켓을 libssh에 넘김.
- 에이전트 포워딩: `wezterm_uds::UnixStream::connect` 후 `into_raw_socket`→`FileDescriptor::from_raw_socket`(`sessioninner.rs:843`). Windows의 AF_UNIX 또는 wezterm-uds 에뮬레이션에 의존.
- `add_default_config_files`(`config.rs:473`): `~/.ssh/config`, `/etc/ssh/ssh_config`, 그리고 `%SystemDrive%/ProgramData/ssh/ssh_config`(`config.rs:478`)를 로드. 가운데 항목은 Windows에서 사실상 존재하지 않는 죽은 경로이나 무해.
- ssh2 백엔드의 `openssl-on-win32` 기능(`Cargo.toml:38`)은 Windows에서 OpenSSL을 강제 사용한다.

### 죽은/미사용 경로

- `do_keepalive`의 ssh2 분기는 no-op(`sessioninner.rs:390`). keepalive는 libssh에서만 작동.
- ssh2 백엔드의 `accept_agent_forward`/`request_auth_agent_forwarding`는 미지원(`sessionwrap.rs:101`, `channelwrap.rs:154`). 즉 에이전트 포워딩은 libssh 전용 기능.
- `send_signal`의 ssh2 분기는 no-op(`channelwrap.rs:187`). 시그널 전달은 libssh 전용.
- 주의: 위 ssh2 경로들은 `default` 기능에 의해 여전히 컴파일된다. fork가 libssh로 고정되었다면 ssh2 백엔드 전체가 잠재적 제거 후보다.

---

## 9. 리팩토링 주의점

1. **단일 워커 스레드 + 메시지 패스가 핵심 불변식**: 모든 SSH 라이브러리 객체는 `SessionInner` 안에서만 산다. `Session`/`Sftp`/`File`/`Dir` 핸들은 ID와 `SessionSender`만 보유한다. 이 경계를 무너뜨리면 `Send`/`Sync` 위반이 발생한다.

2. **워커가 호출자 응답에 블로킹**: 인증·호스트 검증 중 워커가 `smol::block_on`으로 호출자 응답을 기다린다(`auth.rs:107`, `host.rs:56`). 호출자가 `SessionEvent`를 소비·응답하지 않으면 워커는 영구 정지한다. 이는 이벤트 루프와 인증 절차 사이의 강한 시간 결합이다.

3. **이중 백엔드 enum dispatch의 중복**: `*Wrap` 5개 모듈(`sessionwrap`/`channelwrap`/`filewrap`/`dirwrap`/`sftpwrap`)이 모든 연산마다 `match` 두 갈래를 갖는다. 한쪽(ssh2)이 다수 기능에서 no-op/미구현이라 동작 비대칭이 크다. 백엔드를 하나로 정리하면 이 계층 대부분을 삭제할 수 있으나, 두 라이브러리의 메타데이터·시간·플래그 표현 차이를 흡수하던 변환 로직(`sftp/types.rs`)은 일부 보존이 필요하다.

4. **`dispatch_one_request`의 거대 match**(`sessioninner.rs:615`): 모든 요청 변형이 한 메서드에 집중. 새 SFTP 연산 추가 시 `SftpRequest`(`sftp/mod.rs:358`)와 이 match를 동시에 수정해야 하는 산탄총 수정(shotgun surgery) 구조.

5. **ID 카운터 공유**: `open_dir`이 `next_file_id`를 사용(`sessioninner.rs:1003`). 파일/디렉터리 ID 발급기 분리 또는 통합을 명시적으로 결정해야 한다.

6. **2단계 ssh_config 파싱 미지원**: `CanonicalHostname` 등 canonical/final 컨텍스트와 `Match Exec`이 미구현(`config.rs:540`, `config.rs:112`). 해당 설정을 쓰는 사용자에게 침묵하는 부정확성이 있다.

7. **`File::poll_read`의 버퍼 가정**: `poll_read`는 `buf.len()` 만큼 읽기를 요청하고 반환 데이터가 그 이하라고 가정해 `copy_from_slice`(`sftp/file.rs:179`)한다. 워커의 `read_file` 핸들러가 `max_bytes`로 truncate하므로(`sessioninner.rs:679`) 성립하지만, 두 곳이 분리되어 있어 한쪽만 바꾸면 패닉 위험이 있다.

8. **지수 백오프 슬립의 지연 특성**: `request_loop`의 `sleep_delay`가 이벤트 없을 때 두 배씩 증가(`sessioninner.rs:469`). 상한이 없으므로 장시간 유휴 후 첫 응답 지연이 커질 수 있다(다만 깨우기 파이프가 이를 즉시 단축).

9. **순환 의존 없음**: deps/usedBy를 보면 `wezterm-ssh`는 상위 크레이트에 의존하지 않는 잎(leaf)에 가까운 위치다. 단 `async_ossl` 의존은 코드가 아닌 빌드 기능 일원화 목적(`Cargo.toml:42`)이므로 제거 시 openssl vendored 일관성을 검토해야 한다.

10. **ProxyCommand 프로세스 수명**: `KillOnDropChild`(`sessioninner.rs:1097`)가 ProxyCommand 자식을 드롭 시 kill/wait한다. 그러나 `connect_to_host`가 반환한 `Option<KillOnDropChild>`는 `run_impl_*`에서 `_child`로 묶여 함수 스코프 동안만 유지된다(`sessioninner.rs:209`, `sessioninner.rs:283`) — `request_loop` 진입 전까지만 살아 있으므로, ProxyCommand 자식의 수명 관리가 의도대로인지 변경 시 확인이 필요하다.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `src/lib.rs` | 29 | 모듈 선언과 공개 재노출. 백엔드 미선택 시 `compile_error!`. |
| `src/config.rs` | 1637 | ssh_config 파싱·호스트 매칭·토큰/환경 확장. 비테스트 약 800행, 나머지는 스냅샷 테스트. |
| `src/sessioninner.rs` | 1108 | 워커 스레드 본체. 연결·이벤트 루프·요청 디스패처·채널 I/O 펌핑. 가장 비대. |
| `src/auth.rs` | 362 | 공개키·에이전트·비밀번호·keyboard-interactive 인증. ssh2/libssh 두 절차. |
| `src/sftp/mod.rs` | 428 | `Sftp` 비동기 핸들과 `SftpRequest`/`SftpChannelError` 정의. |
| `src/sftp/types.rs` | 421 | `Metadata`/`FileType`/`FilePermissions`/`OpenOptions` 등 중립 타입과 백엔드 변환. |
| `src/pty.rs` | 313 | `SshPty`/`SshChildProcess`(+`SshChildKiller`)와 `new_pty`/`resize_pty`. |
| `src/sftp/file.rs` | 314 | `File` 핸들. `AsyncRead`/`AsyncWrite` 구현과 파일 요청 타입. |
| `src/sftpwrap.rs` | 218 | SFTP 연산의 ssh2/libssh enum dispatch 어댑터. |
| `src/host.rs` | 210 | 호스트 키 검증(known_hosts). ssh2/libssh 두 절차. |
| `src/session.rs` | 198 | 외부 `Session` 핸들, `SessionEvent`/`SessionRequest`, `SessionSender`. |
| `src/channelwrap.rs` | 193 | 채널 연산(pty/exec/env/signal)의 백엔드 dispatch. |
| `src/sftp/error.rs` | 154 | `SftpError` 상태 코드 enum과 코드/`ssh2::Error` 변환. |
| `src/sessionwrap.rs` | 107 | 세션 객체의 백엔드 dispatch와 SFTP 지연 보관. |
| `src/filewrap.rs` | 74 | SFTP 파일 핸들의 백엔드 dispatch. |
| `src/sftp/dir.rs` | 72 | `Dir` 핸들과 디렉터리 요청 타입. |
| `src/dirwrap.rs` | 48 | SFTP 디렉터리 핸들의 백엔드 dispatch. |

본 크레이트에는 생성된 거대 데이터 테이블 파일이 없다.
