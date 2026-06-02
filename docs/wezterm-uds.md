# wezterm-uds 폴더 기능 명세

## 1. 개요 및 책임

`wezterm-uds`는 **유닉스 도메인 소켓(Unix Domain Socket, UDS)의 플랫폼 독립적 추상화**를 제공하는 단일 책임 크레이트다. `UnixStream`(연결형 스트림)과 `UnixListener`(수신 리스너) 두 타입을 노출하여, mux 서버와 클라이언트가 동일 머신 내 로컬 IPC 채널로 통신하도록 한다.

이 크레이트가 별도로 존재하는 본질적 이유는 **`async-io`의 `IoSafe` 트레이트를 UDS 타입에 부착하기 위함**이다. 코드 주석(`wezterm-uds/src/lib.rs:12`)이 명시하듯, 표준 라이브러리의 `std::os::unix::net::UnixStream`에 대해서는 `async-io`가 이미 `IoSafe` 구현을 제공하지만, Windows에서 UDS를 제공하는 `uds_windows` 크레이트에는 그 구현이 없다. 따라서 본 크레이트는 `uds_windows::UnixStream`을 newtype으로 감싼 뒤 `unsafe impl async_io::IoSafe`를 직접 부여한다(`wezterm-uds/src/lib.rs:57`). 이로써 상위 계층의 비동기 I/O 래퍼(`smol::Async`, `async_ossl`)가 이 소켓을 안전하게 폴링할 수 있다.

원래 upstream 설계 의도는 "Portable unix domain sockets"(`wezterm-uds/src/Cargo.toml:6`)로, unix와 Windows 양쪽에서 동일한 타입 인터페이스를 제공해 플랫폼 차이를 최소화하는 것이었다. **Windows 전용 영구 분기에서의 실제 책임은 그 의도의 절반으로 축소**된다. 비Windows 분기는 이미 제거되었으므로, 본 크레이트는 사실상 `uds_windows`에 대한 얇은(thin) 어댑터 레이어로서, (1) `IoSafe` 부착, (2) `std::os::windows::io` 소켓 트레이트 위임, (3) `Read`/`Write` 위임이라는 세 가지를 수행한다.

## 2. 워크스페이스 내 위치

### 의존(deps)

| 의존 크레이트 | 종류 | 용도 |
|---|---|---|
| `async-io` | 외부 (workspace, `2.3`) | `IoSafe` 트레이트 제공. 비동기 I/O 안전성 마커를 UDS 타입에 부착. |
| `uds_windows` | 외부 (workspace, `1.1`) | Windows에서의 실제 UDS 구현체(`UnixStream`, `UnixListener`, `SocketAddr`) 제공. |

워크스페이스 내부 크레이트에 대한 의존은 **없다**. 본 크레이트는 워크스페이스 의존 그래프의 **잎(leaf) 노드**로, `ARCHITECTURE.md:78`에서 최하위 기반 계층(vtparse·wezterm-blob-leases·wezterm-uds·wezterm-version 등과 동일 줄)에 분류되어 있다.

### 피의존(usedBy)

| 사용 크레이트 | 사용 지점 | 사용 형태 |
|---|---|---|
| `wezterm-client` | `wezterm-client/src/client.rs:31`, `wezterm-client/src/discovery.rs:4` | `UnixStream`으로 로컬 mux 서버 소켓에 연결(클라이언트 측). |
| `wezterm-mux-server-impl` | `wezterm-mux-server-impl/src/local.rs:4`, `wezterm-mux-server-impl/src/dispatch.rs:9` | `UnixListener`로 소켓을 바인드·수신하고, 수락된 `UnixStream`을 세션 처리에 위임(서버 측). |
| `wezterm-ssh` | `wezterm-ssh/src/sessioninner.rs:841` | `UnixStream`을 SSH 세션 내부 경로에서 사용. |

### 파이프라인 내 위치

본 크레이트는 **로컬 IPC 전송 계층의 최하단**에 위치한다. mux 서버가 `UnixListener`로 소켓을 열어 클라이언트 연결을 수락하고, mux 클라이언트가 `UnixStream`으로 그 소켓에 접속하는 양방향 채널의 공통 토대를 이룬다. 수락된 스트림은 상위 `async_ossl`(TLS)·`codec`(PDU 직렬화)·`mux` 계층으로 전달된다.

## 3. 공개 API 표면

크레이트는 루트 모듈(`lib.rs`) 하나에 모든 공개 항목을 둔다. 노출 항목은 **2개의 타입과 그 인헤런트 메서드, 그리고 다수의 트레이트 구현**이다.

### `pub struct UnixStream(StreamImpl)` (`wezterm-uds/src/lib.rs:19`)

`uds_windows::UnixStream`(별칭 `StreamImpl`)을 감싼 newtype. 연결형 로컬 스트림.

- `pub fn connect<P: AsRef<Path>>(path: P) -> std::io::Result<Self>` (`wezterm-uds/src/lib.rs:60`) — 지정 경로의 소켓에 연결.
- 구현 트레이트:
  - `Read` (`:42`) / `Write` (`:48`) — 내부 스트림에 위임.
  - `IntoRawSocket` (`:21`) / `AsRawSocket` (`:26`) / `AsSocket` (`:31`) / `FromRawSocket` (`:36`) — `std::os::windows::io`의 raw 소켓 트레이트. 모두 내부 스트림에 위임. `FromRawSocket::from_raw_socket`은 `unsafe`.
  - `unsafe impl async_io::IoSafe` (`:57`) — **이 크레이트의 핵심 존재 이유**. 비동기 폴링 안전성 마커.
  - `Deref`/`DerefMut` (대상 `StreamImpl`, `:65`, `:72`) — 래핑하지 않은 `uds_windows::UnixStream`의 나머지 메서드를 그대로 노출.
  - `Debug` (`#[derive]`, `:18`).

### `pub struct UnixListener(ListenerImpl)` (`wezterm-uds/src/lib.rs:78`)

`uds_windows::UnixListener`(별칭 `ListenerImpl`)을 감싼 newtype. 수신 리스너.

- `pub fn bind<P: AsRef<Path>>(path: P) -> std::io::Result<Self>` (`:81`) — 경로에 소켓을 바인드.
- `pub fn accept(&self) -> std::io::Result<(UnixStream, SocketAddr)>` (`:85`) — 단일 연결 수락. 반환되는 스트림은 본 크레이트의 `UnixStream`으로 재래핑.
- `pub fn incoming(&self) -> impl Iterator<Item = std::io::Result<UnixStream>> + '_` (`:90`) — 연결을 순회하는 이터레이터. 내부 항목을 `UnixStream`으로 맵핑.
- `Deref`/`DerefMut` (대상 `ListenerImpl`, `:95`, `:102`).

### 재노출(re-export)

`uds_windows::SocketAddr`는 `use`로 가져와 `accept`의 반환 타입에 사용되지만 `pub use`로 재노출되지는 않는다. 소비자는 시그니처상 `uds_windows::SocketAddr`를 직접 본다.

## 4. 내부 구조

- **단일 파일·단일 모듈**: `src/lib.rs`(약 107행). 하위 모듈 없음. 비대한 모듈은 없다.
- **데이터 흐름**: 모든 공개 메서드와 트레이트 구현이 `self.0`(내부 `uds_windows` 타입)으로 **위임(delegation)**한다. 자체 로직은 newtype 래핑/언래핑(`UnixStream(stream)`, `r.map(UnixStream)`)뿐이며 상태나 분기를 갖지 않는다.
- **제어 흐름**: 없음에 가깝다. 메서드는 모두 1~2줄짜리 위임 함수다.
- **핵심 패턴**: newtype + 트레이트 위임 + `Deref` 폴백. `Deref`/`DerefMut`로 인해 명시적으로 위임하지 않은 `uds_windows` 메서드(예: `set_nonblocking`, `local_addr`, `peer_addr` 등)도 소비자가 투명하게 호출할 수 있다. 명시적 위임이 필요한 항목은 `Deref`만으로는 트레이트 구현이 자동 상속되지 않는 트레이트들(`Read`, `Write`, raw socket 트레이트, `IoSafe`)뿐이다.

## 5. 핵심 데이터 구조·타입

### `UnixStream(StreamImpl)`

- **불변식**: 내부 필드 `.0`은 살아 있는 OS 소켓 핸들을 감싼다. `from_raw_socket`으로 생성 시 호출자가 핸들의 유효성·소유권을 보증해야 하며(그래서 `unsafe`), `into_raw_socket`은 소유권을 호출자에게 이전(소유권 누수 방지를 위해 `Drop`이 닫지 않음)한다.
- **`IoSafe` 계약**: `IoSafe`는 "이 타입은 raw I/O 핸들의 소유권을 유지하며, `Read`/`Write` 중 핸들을 닫거나 교체하지 않는다"는 안전성 약속을 나타내는 unsafe 마커다. newtype이 핸들을 다른 객체로 바꾸지 않고 단순 위임만 하므로 이 계약이 성립한다.

### `UnixListener(ListenerImpl)`

- **불변식**: 바인드된 소켓 경로를 점유하는 핸들을 감싼다. `accept`/`incoming`이 산출하는 스트림은 반드시 본 크레이트의 `UnixStream`으로 재래핑되어, 호출자가 `IoSafe`를 갖춘 통일된 타입을 받는다(이것이 단순 `pub use uds_windows::*` 대신 newtype을 쓰는 이유 중 하나).

두 타입 모두 추가 필드 없이 단일 튜플 필드만 가지므로 메모리 표현은 내부 `uds_windows` 타입과 동일하다.

## 6. 외부 의존성

- **`uds_windows` (`1.1`)** — Windows에서 유닉스 도메인 소켓을 구현. `AF_UNIX` 소켓이 Windows 10 1803+에서 지원되면서 이를 표준 `std::os::unix::net` API와 유사한 형태로 노출하는 크레이트. 본 크레이트가 감싸는 실제 구현체(`UnixStream`, `UnixListener`, `SocketAddr`)의 출처다.
- **`async-io` (`2.3`)** — `smol` 비동기 런타임 계열의 reactor. 여기서는 오직 `IoSafe` 트레이트만 사용한다. 상위 계층(`wezterm-mux-server-impl/src/dispatch.rs`의 `smol::Async`)이 이 소켓을 비동기 리액터에 등록할 때 `IoSafe` 바운드를 요구하므로 필수다.

두 의존 모두 워크스페이스 루트 `Cargo.toml`에서 버전을 중앙 관리하며(`Cargo.toml:33`, `Cargo.toml:187`), 본 크레이트는 `.workspace = true`로 상속한다.

## 7. 설정·기능 플래그

- **feature flag 없음**: `Cargo.toml`에 `[features]` 섹션이 없다.
- **관련 config 항목**: 본 크레이트 자체는 `config` 크레이트에 의존하지 않으며 설정을 읽지 않는다. 다만 소비 측에서 소켓 경로가 결정된다. `wezterm-mux-server-impl/src/local.rs:46`의 `unix_dom.socket_path()`(`config::UnixDomain`)가 `bind` 대상 경로를 제공한다. 따라서 소켓 위치·권한 정책은 `config` 크레이트의 `UnixDomain` 설정 영역에 있으며, 본 크레이트는 경로 문자열을 받아 바인드/연결만 수행한다.

## 8. Windows 전용 고려사항

- **플랫폼 분기 전무**: `src/lib.rs`에 `#[cfg(...)]`가 하나도 없다. 코드는 무조건 `std::os::windows::io`의 소켓 트레이트(`AsRawSocket`, `AsSocket`, `BorrowedSocket`, `FromRawSocket`, `IntoRawSocket`, `RawSocket`)와 `uds_windows`를 사용한다(`wezterm-uds/src/lib.rs:2`, `:6`). 즉 이 파일은 이미 **Windows 전용으로 완전히 단일화**되어 있으며, 죽은 비Windows 분기가 남아 있지 않다.
- **Windows API 사용 지점**: 직접적인 Win32 호출은 없다. raw 소켓은 `RawSocket`(= Windows `SOCKET` 핸들의 Rust 표현)으로 다뤄지며, 실제 `AF_UNIX` 소켓 시스템콜은 `uds_windows`가 캡슐화한다.
- **주석의 잔재**: 타입을 "IoSafe on all platforms" / "for all platforms"로 정의한다는 주석(`wezterm-uds/src/lib.rs:12`~`:17`)은 원래의 크로스플랫폼 의도를 반영한 **역사적 흔적**이다. 현재 코드 실체는 Windows 전용이므로, 이 주석은 사실과 어긋난다(리팩토링 시 갱신 대상).
- **소비 측 결합 — 죽은 코드 가능성**: `docs/async_ossl.md:105`가 지적하듯, `wezterm-mux-server-impl/src/dispatch.rs:11`의 `AsRawDesc` 트레이트와 그에 대한 `impl AsRawDesc for UnixStream`(`dispatch.rs:13`)은 본 크레이트의 `UnixStream`이 서버 dispatch 경로에서 실제로 쓰이는지에 따라 죽은 코드일 수 있다. 본 크레이트의 raw socket 트레이트 구현을 정리할 때 이 결합 지점을 함께 검토해야 한다.

## 9. 리팩토링 주의점

- **얇은 어댑터의 정당성 재검토**: 이 크레이트는 `uds_windows`에 대한 newtype 래퍼일 뿐이다. 존재 이유는 단 하나, **`unsafe impl async_io::IoSafe`**(`:57`)다. 이 한 줄을 소비 측(예: `wezterm-mux-server-impl`) 또는 `async_ossl`로 이전하면 크레이트 자체를 제거하고 `uds_windows`를 직접 쓸 수도 있다. 단, `IoSafe`를 외부 타입(`uds_windows::UnixStream`)에 직접 구현하려면 고아 규칙(orphan rule) 위반이 되므로, **newtype 래핑은 `IoSafe`를 부착하기 위한 필수 우회책**이다. 즉 이 newtype은 단순 중복이 아니라 트레이트 구현을 가능케 하는 구조적 필요다. 크레이트 제거 시 래퍼를 소비 측 한 곳으로 옮기되 고아 규칙을 반드시 고려해야 한다.
- **`Deref`/`DerefMut`의 양면성**: `Deref`로 내부 타입을 투명 노출하므로 소비자가 어떤 `uds_windows` 메서드를 의존하는지 컴파일 시점에 추적하기 어렵다. `uds_windows` 버전을 올릴 때 API 변경이 `Deref` 경유로 소비 측에 직접 파급될 수 있다. 캡슐화를 강화하려면 `Deref`를 제거하고 필요한 메서드만 명시적으로 위임하는 방향이 안전하나, 소비 측(`set_nonblocking` 등) 호출을 전수 조사해야 한다.
- **`unsafe` 계약 유지**: `IoSafe`와 `FromRawSocket`의 `unsafe`는 newtype이 핸들을 그대로 보존한다는 전제에 의존한다. 향후 `UnixStream`에 버퍼·상태 필드를 추가하거나 `Read`/`Write`에서 핸들을 조작하는 로직을 넣으면 이 안전성 계약이 깨질 수 있다.
- **순환 의존 없음**: 잎 노드이고 워크스페이스 내부 의존이 없으므로 순환 위험은 없다.
- **주석-구현 불일치**: 8절의 "all platforms" 주석은 현 구현(Windows 전용)과 모순된다. 정리 시 갱신 필요.
- **결합 표면**: 외부 노출이 작아 변경 파급은 제한적이다. 시그니처를 바꾸면 영향받는 곳은 `wezterm-client`, `wezterm-mux-server-impl`, `wezterm-ssh` 세 크레이트의 소수 사용 지점(2절 표 참조)뿐이다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `wezterm-uds/Cargo.toml` | 13 | 패키지 메타데이터. 의존성은 `async-io`, `uds_windows` 2개(둘 다 workspace 상속). feature 없음. |
| `wezterm-uds/src/lib.rs` | 107 | 크레이트 본체. `UnixStream`/`UnixListener` newtype 정의, `uds_windows`로의 트레이트 위임, 핵심인 `unsafe impl async_io::IoSafe`. 생성 파일 아님. |

생성(generated) 파일은 없다.
