# filedescriptor 폴더 기능 명세

## 1. 개요 및 책임

`filedescriptor`는 플랫폼 수준의 원시 핸들/디스크립터를 소유권·수명 관리가 가능한 형태로 감싸는 저수준 유틸리티 크레이트다. 단일 책임은 다음과 같다.

- 원시 핸들을 RAII 방식으로 소유(`OwnedHandle`)하고, 읽기·쓰기 가능한 디스크립터(`FileDescriptor`)로 래핑한다.
- 핸들의 복제(`dup`/`try_clone`), 파이프 생성(`Pipe`), 연결된 소켓 쌍 생성(`socketpair`), 준비도 검사(`poll`), 표준 입출력 리다이렉션(`redirect_stdio`)을 제공한다.
- `RawFd`/`RawHandle`를 직접 `cfg` 분기로 다루지 않도록 플랫폼 독립 타입 별칭(`RawFileDescriptor`, `SocketDescriptor`)과 트레이트를 노출한다.

upstream 본래 의도는 posix/windows 양쪽을 추상화하는 "이식 가능한" 래퍼였다(`lib.rs:1`~`lib.rs:3` 문서 주석에 그 흔적이 남아 있음). 그러나 본 Windows 전용 분기에서는 posix 구현 파일(`unix.rs`)이 제거되었고, 현재 실제 구현은 `windows.rs` 하나뿐이다. 따라서 이 fork에서의 실제 책임은 **Win32 `HANDLE`/winsock `SOCKET`을 동일 추상 타입으로 묶어, 상위 크레이트(mux, portable-pty, termwiz 등)가 핸들과 소켓을 구분 없이 다루도록 하는 것**이다. 핸들 유형(문자/디스크/파이프/소켓)을 런타임에 탐지(`probe_handle_type`)하여, 읽기·쓰기·해제·논블로킹 전환 시 적절한 Win32 API와 winsock API를 자동 선택하는 점이 핵심 가치다.

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 목록 |
| --- | --- |
| 의존(deps) | 없음 (외부 크레이트 `thiserror`, `libc`, `winapi`만 사용) |
| 피의존(usedBy) | `mux`, `portable-pty`, `termwiz`, `wezterm`, `wezterm-client`, `wezterm-ssh` |

이 크레이트는 워크스페이스 의존 그래프의 최하위 기반(foundation) 계층에 위치한다. 내부 크레이트에 전혀 의존하지 않으면서 PTY·먹스·SSH·터미널 처리 계층 다수에 핸들/소켓 추상화를 공급하는 잎(leaf) 노드이므로, 변경 시 파급 범위가 넓다.

## 3. 공개 API 표면

공개 API는 `lib.rs`에 정의되고, `windows.rs`의 플랫폼 항목이 `pub use crate::windows::*`(`lib.rs:98`)로 재노출된다.

### 타입 별칭 (windows.rs)
- `pub type RawFileDescriptor = RawHandle;` (`windows.rs:30`) — 플랫폼 독립 파일 디스크립터 별칭.
- `pub type SocketDescriptor = SOCKET;` (`windows.rs:35`) — 플랫폼 독립 소켓 디스크립터 별칭.
- `pub use winapi::um::winsock2::{POLLERR, POLLHUP, POLLIN, POLLOUT, WSAPOLLFD as pollfd};` (`windows.rs:25`) — `poll`에 쓰이는 이벤트 상수와 `pollfd` 구조체를 재노출.

### 핵심 트레이트 (lib.rs)
- `AsRawFileDescriptor::as_raw_file_descriptor(&self) -> RawFileDescriptor` (`lib.rs:154`) — 비소유 참조 반환.
- `IntoRawFileDescriptor::into_raw_file_descriptor(self) -> RawFileDescriptor` (`lib.rs:160`) — 소유권 이전.
- `FromRawFileDescriptor::from_raw_file_descriptor(fd) -> Self` (`unsafe`, `lib.rs:179`) — 원시값에서 소유 객체 생성.
- 소켓 대응 3종: `AsRawSocketDescriptor` (`lib.rs:182`), `IntoRawSocketDescriptor` (`lib.rs:185`), `FromRawSocketDescriptor` (`unsafe`, `lib.rs:188`).

이 트레이트들은 `windows.rs:53`~`windows.rs:87`에서 표준 라이브러리의 `AsRawHandle`/`IntoRawHandle`/`FromRawHandle` 및 `AsRawSocket`/`IntoRawSocket`/`FromRawSocket`를 구현한 모든 타입에 대해 blanket impl로 자동 제공된다.

### 핵심 타입과 메서드
- `OwnedHandle` (`lib.rs:205`) — RAII 핸들 소유자.
  - `new<F: IntoRawFileDescriptor>(f) -> Self` (`lib.rs:214`)
  - `try_clone(&self) -> Result<Self>` (`lib.rs:228`)
  - `dup<F: AsRawFileDescriptor>(f: &F) -> Result<Self>` (`lib.rs:239`)
- `FileDescriptor` (`lib.rs:267`) — `OwnedHandle` 위에 `Read`/`Write`를 얹은 래퍼.
  - `new`, `dup`, `try_clone` (`lib.rs:282`~`lib.rs:306`)
  - `as_stdio(&self) -> Result<std::process::Stdio>` (`lib.rs:312`)
  - `as_file(&self) -> Result<std::fs::File>` (`lib.rs:319`)
  - `set_non_blocking(&mut self, bool) -> Result<()>` (`lib.rs:329`)
  - `redirect_stdio<F>(f: &F, stdio: StdioDescriptor) -> Result<Self>` (`lib.rs:338`)
- `StdioDescriptor` enum: `Stdin`/`Stdout`/`Stderr` (`lib.rs:272`).
- `Pipe { read, write }` (`lib.rs:359`) — 커널 파이프 양 끝.
  - `Pipe::new() -> Result<Pipe>` (`windows.rs:418`)
- 자유 함수:
  - `poll(pfd: &mut [pollfd], duration: Option<Duration>) -> Result<usize>` (`lib.rs:395`)
  - `socketpair() -> Result<(FileDescriptor, FileDescriptor)>` (`lib.rs:402`)
- `Error` enum (`lib.rs:103`, `#[non_exhaustive]`)과 `Result<T>` 별칭 (`lib.rs:149`).

`#[doc(hidden)] pub` 항목으로 `socketpair_impl` (`windows.rs:483`), `poll_impl` (`windows.rs:552`)이 노출되나, 공개 자유 함수의 위임 대상이며 직접 호출 대상이 아니다.

## 4. 내부 구조

모듈 분해는 단순하다.

- `lib.rs` — 공개 API 정의 계층. 트레이트 선언, `Error`/`Result`, `OwnedHandle`·`FileDescriptor`·`Pipe`·`StdioDescriptor`의 구조체 정의 및 플랫폼 독립 메서드 시그니처(문서 주석 포함)를 담는다. 모든 메서드는 본문에서 `*_impl` 함수로 위임하여 플랫폼 구현과 분리한다(예: `as_stdio` → `as_stdio_impl`).
- `windows.rs` — 유일한 플랫폼 구현 계층. 위 `*_impl` 메서드, 트레이트 blanket impl, `Drop`, `Read`/`Write`, `Pipe::new`, `socketpair_impl`, `poll_impl`, winsock 초기화를 담는다.

제어/데이터 흐름:
1. 핸들 진입 시 `OwnedHandle::new`/`from_raw_handle`가 `probe_handle_type`(`windows.rs:100`)으로 유형을 1회 판정해 `handle_type` 필드에 캐시한다.
2. 이후 모든 분기(`Read`/`Write`/`Drop`/`set_non_blocking`)는 `is_socket_handle`(`windows.rs:141`)로 소켓 여부를 확인해 winsock API(`recv`/`send`/`closesocket`/`ioctlsocket`)와 Win32 파일 API(`ReadFile`/`WriteFile`/`CloseHandle`)를 선택한다.
3. `socketpair`는 winsock으로 루프백 TCP 서버를 만들고 자기 자신에게 연결하는 방식으로 연결된 소켓 쌍을 합성한다(`windows.rs:483`~`windows.rs:549`).

비대한 모듈은 없다. 두 파일 합계 약 985행으로 규모가 작다. 다만 `windows.rs`의 `probe_handle_type`과 `socketpair_impl`이 가장 절차적이고 unsafe 밀도가 높은 지점이다.

## 5. 핵심 데이터 구조·타입

- `OwnedHandle { handle: RawFileDescriptor, handle_type: HandleType }` (`lib.rs:205`)
  - 불변식: `handle`은 소유된(유일 책임) 핸들이다. `INVALID_HANDLE_VALUE` 또는 null이면 "비어 있음"으로 취급되어 `Drop`에서 닫지 않는다(`windows.rs:151`). `handle_type`은 생성 시 판정된 유형의 캐시이며, `Unknown`인 경우 사용 시점에 재판정한다.
  - `Send`/`Sync`를 unsafe로 구현(`windows.rs:89`~`windows.rs:90`).
  - `IntoRawHandle::into_raw_handle`는 `std::mem::forget(self)`로 `Drop`을 억제해 이중 해제를 방지한다(`windows.rs:219`).
- `FileDescriptor { handle: OwnedHandle }` (`lib.rs:267`)
  - 불변식: `OwnedHandle`을 정확히 하나 보유하는 얇은 래퍼. 소켓 변환(`as_raw_socket`/`into_raw_socket`)은 `debug_assert!(self.handle.is_socket_handle())`로만 보호되며, 주석(`windows.rs:306`, `windows.rs:314`)이 "보장된 변환이 아님(FIXME)"을 명시한다.
- `HandleType` enum: `Char`/`Disk`/`Pipe`/`Socket`/`Unknown` (`windows.rs:43`, `pub(crate)`)
  - `#[default] Unknown` (`windows.rs:48`). 외부 비노출이며 분기 선택의 핵심 상태.
- `StdioDescriptor` enum: `Stdin`/`Stdout`/`Stderr` (`lib.rs:272`).
- `Error` enum (`lib.rs:103`) — 파이프/소켓/바인드/연결/poll/dup 등 실패 원인을 변종으로 구분. `Dup { fd }`, `Dup2 { src_fd, dest_fd }`, `IllegalFdValue`, `FdValueOutsideFdSetSize`는 데이터 필드를 동반한다. `#[from] std::io::Error`로 `Io` 변종을 통한 자동 변환을 지원(`lib.rs:146`).

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
| --- | --- |
| `thiserror` | `Error` enum의 `#[derive(Error)]`·`#[error("...")]`로 오류 메시지·소스 체이닝을 선언적으로 생성 (`lib.rs:100`). |
| `libc` | `poll_impl`의 타임아웃 인자 캐스팅에 `libc::c_int` 한 곳만 사용 (`windows.rs:558`). 사실상 최소 의존. |
| `winapi` | Win32/winsock 호출 전반. 활성화 feature: `winuser`, `handleapi`, `fileapi`, `namedpipeapi`, `processthreadsapi`, `winsock2`, `processenv` (`Cargo.toml:18`~`Cargo.toml:26`). 핸들 복제(`DuplicateHandle`), 파이프(`CreatePipe`/`GetNamedPipeInfo`), 파일 IO(`ReadFile`/`WriteFile`/`GetFileType`), 표준 핸들(`GetStdHandle`/`SetStdHandle`), 소켓 전반(`WSASocketW`/`bind`/`connect`/`accept`/`WSAPoll` 등)을 제공. |

내부 워크스페이스 크레이트 의존은 없다.

## 7. 설정·기능 플래그

- 자체 정의 cargo feature flag는 없다.
- 유일한 조건부 의존은 `[target."cfg(windows)".dependencies]`의 `winapi`(`Cargo.toml:17`)다. 이 fork는 Windows 전용이므로 항상 활성이다.
- `config` 크레이트와의 연동 설정 항목은 없다. 본 크레이트는 설정 대상이 아니라 순수 시스템 래퍼다.

## 8. Windows 전용 고려사항

- 플랫폼 분기 자체가 사실상 소거되었다. 원래 posix 경로를 담던 `unix.rs`는 제거되어 존재하지 않으며, `lib.rs:97`은 `mod windows;`만 선언한다. `lib.rs`의 문서 주석(`lib.rs:66`~`lib.rs:73`, `lib.rs:376`~`lib.rs:384`)에 남은 "posix/macOS/`poll(2)`/`select(2)`" 서술은 **현 fork에서 도달 불가능한 죽은 설명**이다.
- Windows API 사용 지점(전부 `windows.rs`):
  - 핸들 유형 판정: `GetFileType`, `GetNamedPipeInfo`, winsock `getsockopt`(`SO_ERROR`/`WSAENOTSOCK`로 WSL↔win32 파이프와 소켓을 구분, `windows.rs:121`~`windows.rs:134`).
  - 핸들 복제: `DuplicateHandle`(`windows.rs:189`, 비상속 옵션).
  - 해제: 소켓이면 `closesocket`, 아니면 `CloseHandle`(`windows.rs:154`~`windows.rs:158`).
  - 표준 IO 리다이렉션: `GetStdHandle`/`SetStdHandle`(`windows.rs:272`, `windows.rs:276`). 상수 `STD_INPUT/OUTPUT/ERROR_HANDLE`를 `u32` 2의 보수 표기(`-10`/`-11`/`-12`)로 하드코딩(`windows.rs:37`~`windows.rs:39`).
  - 파이프: `CreatePipe`(`windows.rs:426`), `SECURITY_ATTRIBUTES.bInheritHandle = 0`으로 비상속.
  - 소켓 IO: `recv`/`send`(`windows.rs:341`, `windows.rs:382`)를 `ReadFile`/`WriteFile` 대신 쓰는데, winsock 함수만이 논블로킹 모드를 존중하기 때문(`windows.rs:337`~`windows.rs:339` 주석).
  - 소켓 생성: `WSASocketW`에 `WSA_FLAG_NO_HANDLE_INHERIT`(`windows.rs:467`). winsock은 `init_winsock`의 `WSAStartup`(버전 2.2, `Once`로 1회)으로 초기화(`windows.rs:447`~`windows.rs:457`).
  - 준비도 검사: `WSAPoll`(`windows.rs:554`). 따라서 `poll`은 **소켓에만** 동작한다(파일/파이프 핸들 불가, `lib.rs:383`~`lib.rs:384` 주석과 일치).
- 특이 동작: `Read` 구현에서 파일/파이프 경로의 `ERROR_BROKEN_PIPE`를 EOF(`Ok(0)`)로 변환한다(`windows.rs:366`~`windows.rs:368`).

## 9. 리팩토링 주의점

- **광범위한 피의존**: mux·portable-pty·termwiz·wezterm·wezterm-client·wezterm-ssh가 모두 의존하므로, 공개 시그니처·타입 별칭·`Error` 변종 변경은 워크스페이스 전반에 파급된다. `Error`는 `#[non_exhaustive]`이므로 변종 추가는 비교적 안전하나 제거·이름 변경은 위험하다.
- **soundness 경계**: `as_raw_socket`/`into_raw_socket`의 핸들→소켓 변환이 `debug_assert!`로만 보호된다(`windows.rs:306`~`windows.rs:317`). release 빌드에서 비소켓 핸들을 소켓으로 잘못 다루면 정의되지 않은 동작 위험. 주석의 FIXME가 미해결 기술 부채임을 명시.
- **유형 캐시 정합성**: `OwnedHandle.handle_type`은 캐시다. `Pipe::new`·`socket`·`socketpair_impl`은 구조체를 직접 리터럴로 만들며 유형을 `Pipe`/`Socket`으로 못박는다(`windows.rs:431`, `windows.rs:474`, `windows.rs:543`). `probe_handle_type`을 우회하므로, 유형 판정 로직을 바꿀 때 이 직접 생성 경로들과의 일관성을 함께 유지해야 한다.
- **null/INVALID 핸들 의미론**: `dup_impl`(`windows.rs:177`)과 `Drop`(`windows.rs:152`)이 `INVALID_HANDLE_VALUE`/null을 "빈 핸들"로 특수 취급한다. 이 불변식을 깨면 이중 해제 또는 누수가 발생한다.
- **죽은 문서·죽은 분기 정리**: posix/macOS를 언급하는 문서 주석(`lib.rs` 다수)은 Windows fork 기준 사실과 어긋나므로 정리 대상이다. 다만 코드 동작에는 영향이 없다.
- **순환 의존 없음**: 내부 의존이 0이므로 순환 위험은 없다. 잎 노드이므로 본 크레이트만 단독 변경·테스트 가능.
- **표준 라이브러리 대체 검토**: 현 std에는 `OwnedHandle`/`BorrowedHandle`/`OwnedSocket` 등 I/O safety 타입이 존재한다. 장기적으로 본 크레이트의 일부 책임을 std 타입으로 이관할 여지가 있으나, 핸들/소켓 통합 추상화와 `poll`/`socketpair` 합성 로직은 std로 직접 대체되지 않는다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `Cargo.toml` | 27 | 패키지 메타·의존(`thiserror`/`libc`/`winapi`)·`cfg(windows)` winapi feature 정의. |
| `src/lib.rs` | 404 | 공개 API 계층. 트레이트·`Error`/`Result`·`OwnedHandle`/`FileDescriptor`/`Pipe`/`StdioDescriptor` 정의, `*_impl` 위임 메서드와 문서 주석, 자유 함수 `poll`/`socketpair` 선언. |
| `src/windows.rs` | 581 | 유일한 플랫폼 구현. blanket trait impl, 핸들 유형 판정, `Drop`, `Read`/`Write`, 핸들 복제, stdio 리다이렉션, `Pipe::new`, winsock 초기화, `socketpair_impl`, `poll_impl`, 단위 테스트(`socketpair`). |

생성 파일(`[생성]`)은 없다. 모든 소스가 수기 작성된 일반 코드다.
