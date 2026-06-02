# async_ossl 폴더 기능 명세

## 1. 개요 및 책임

`async_ossl`은 OpenSSL의 동기(blocking) TLS 스트림(`openssl::ssl::SslStream<TcpStream>`)을 비동기 실행기(`async-io`/`smol`)가 폴링할 수 있는 형태로 적응(adapt)시키는 단일 책임 어댑터 크레이트다.

OpenSSL의 `SslStream`은 그 자체로는 비동기 I/O를 모른다. `std::io::Read`/`Write`를 동기적으로 수행한다. 한편 wezterm의 mux(다중화) 계층은 `smol::Async<T>`로 소켓을 감싸 reactor에 등록하고, 운영체제의 소켓 가용성(readable/writable) 이벤트에 따라 논블로킹으로 I/O를 구동한다. 이 두 세계를 잇기 위해서는 다음 두 조건을 동시에 만족하는 타입이 필요하다.

1. `std::io::Read` + `std::io::Write`를 구현하여 TLS 데이터를 평문으로 읽고 쓸 수 있어야 한다.
2. 내부 소켓의 원시 핸들(`RawSocket`)을 노출하여 `async-io`의 reactor가 해당 fd/소켓을 epoll/IOCP 류 메커니즘에 등록할 수 있어야 한다.

`AsyncSslStream`은 `SslStream<TcpStream>`을 한 겹 감싸, (1) `Read`/`Write`를 내부 스트림으로 위임하고, (2) `AsSocket`/`AsRawSocket`을 내부 `TcpStream`으로 위임하며, (3) `async_io::IoSafe` 마커를 `unsafe impl`로 부여하는 것이 전부다. 즉 이 크레이트의 책임은 "타입 결합(type plumbing)"에 한정된다. 직접적인 암호화·핸드셰이크·인증서 검증 로직은 전혀 수행하지 않는다(그 작업은 호출자가 `SslConnector`/`SslAcceptor`로 수행한 뒤 결과 `SslStream`을 본 어댑터에 주입한다).

Windows fork에서의 실제 책임도 동일하다. 본 크레이트는 처음부터 Windows 소켓 추상(`std::os::windows::io`)만 사용하도록 작성되어 있어, 플랫폼 분기나 죽은 경로가 존재하지 않는다(상세는 8절).

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 |
| --- | --- |
| 의존(deps) | (없음 — workspace 의존만: `async-io`, `openssl`) |
| 피의존(usedBy) | `wezterm-client`, `wezterm-mux-server`, `wezterm-mux-server-impl`, `wezterm-ssh` |

본 크레이트는 `docs/ARCHITECTURE.md:77` 기준 L0(최하위 기반) 계층에 위치하는 leaf 어댑터다. TLS 핸드셰이크가 완료된 `SslStream`을 비동기 mux 파이프라인(클라이언트 측 `wezterm-client`, 서버 측 `wezterm-mux-server`→`wezterm-mux-server-impl`)에 투입하는 진입 어댑터로서, TCP+TLS 전송 계층과 비동기 PDU 처리 계층 사이의 경계에 놓인다.

## 3. 공개 API 표면

크레이트 전체가 단일 파일(`async_ossl/src/lib.rs`)이며 공개 표면은 다음 두 항목뿐이다.

- `pub trait AsRawDesc: std::os::windows::io::AsRawSocket {}` (`async_ossl/src/lib.rs:4`)
  - 마커 트레이트. 슈퍼트레이트로 `AsRawSocket`을 요구한다. 본문은 비어 있다. mux 디스패치 측에서 "원시 소켓 핸들을 얻을 수 있는 스트림"이라는 제약을 표현하기 위한 이름표 역할이다.

- `pub struct AsyncSslStream` (`async_ossl/src/lib.rs:6-9`)
  - 단일 필드 `s: SslStream<TcpStream>`(비공개)를 감싸는 newtype. `#[derive(Debug)]`.
  - `pub fn new(s: SslStream<TcpStream>) -> Self` (`async_ossl/src/lib.rs:14-16`): 이미 핸드셰이크가 완료된 `SslStream`을 받아 래퍼를 생성하는 유일한 공개 생성자.

`AsyncSslStream`에 대해 구현되는 트레이트(공개적으로 관측 가능한 동작):

| 트레이트 | 정의 위치 | 동작 |
| --- | --- | --- |
| `async_io::IoSafe` | `lib.rs:11` | `unsafe impl` 마커. reactor가 이 타입을 안전하게 폴링 대상으로 등록할 수 있음을 보증. |
| `std::os::windows::io::AsRawSocket` | `lib.rs:19-23` | `as_raw_socket()` → 내부 `TcpStream`의 raw socket으로 위임. |
| `std::os::windows::io::AsSocket` | `lib.rs:25-29` | `as_socket()` → 내부 `TcpStream`의 `BorrowedSocket`으로 위임. |
| `AsRawDesc` (본 크레이트) | `lib.rs:31` | 빈 구현(마커 충족). |
| `std::io::Read` | `lib.rs:33-37` | `read()` → `SslStream::read`로 위임(TLS 복호화 후 평문 반환). |
| `std::io::Write` | `lib.rs:39-46` | `write()`/`flush()` → `SslStream`으로 위임(평문을 암호화하여 전송). |

## 4. 내부 구조

모듈 분해는 없다. 크레이트는 `lib.rs` 단일 파일 47행으로 구성되며 비대한 모듈은 존재하지 않는다.

데이터 흐름은 전적으로 위임(delegation) 패턴이다.

- I/O 흐름: 호출자 → `AsyncSslStream::read/write` → `SslStream<TcpStream>::read/write` → (OpenSSL 암복호화) → `TcpStream`.
- reactor 등록 흐름: 호출자가 `AsyncSslStream`을 `smol::Async::new(...)` 또는 `async_io::Async::new(...)`로 감쌀 때, `Async`는 `AsRawSocket::as_raw_socket()`로 얻은 소켓을 reactor에 등록한다. `IoSafe` 마커가 이 등록이 안전함을 컴파일러에 알린다.

제어 흐름상 본 크레이트에는 분기·루프·상태가 전혀 없다. 모든 메서드가 단일 위임 호출이다. 핸드셰이크와 인증서 검증은 호출 측(아래)에서 수행되고, 완료된 `SslStream`만 `new`로 주입된다.

소비처에서의 실제 결선:

- 클라이언트(`wezterm-client/src/client.rs:983`): `Box::new(Async::new(AsyncSslStream::new(connector.connect(...)?)))` — `SslConnector`로 핸드셰이크 후 래핑하고 `Async`로 감싼다.
- 서버(`wezterm-mux-server/src/ossl.rs:87`): peer cert 검증 후 `dispatch::process(AsyncSslStream::new(stream))`로 전달.
- 디스패치(`wezterm-mux-server-impl/src/dispatch.rs:32`): 제네릭 `process<T>`가 내부에서 `smol::Async::new(stream)?`로 감싼 뒤 처리한다.

## 5. 핵심 데이터 구조·타입

- `struct AsyncSslStream { s: SslStream<TcpStream> }`
  - 불변식 1: `s`는 본 타입 생성 이전에 TLS 핸드셰이크가 완료된 스트림이어야 한다. `new`는 핸드셰이크를 수행하지 않으므로, 핸드셰이크 전 스트림을 주입하면 첫 `read`/`write`에서 OpenSSL이 동작을 시도하게 된다(생성자 자체는 이를 강제하지 않음).
  - 불변식 2: 내부 전송은 항상 `TcpStream`이다(제네릭이 아닌 구체 타입으로 고정). 따라서 `AsRawSocket`/`AsSocket` 위임이 항상 유효한 OS 소켓을 가리킨다.
  - 불변식 3: `IoSafe`(`lib.rs:11`)는 `unsafe impl`이다. 이는 "이 타입을 `Async`로 감싸 폴링하는 동안, raw socket이 가리키는 fd가 타입과 같은 수명·동일성을 유지한다"는 계약을 개발자가 보증함을 뜻한다. `s`가 소유한 `TcpStream`을 외부에서 close하거나 교체하지 않는 한 이 계약은 유지된다.

- `trait AsRawDesc` (본 크레이트)
  - 슈퍼트레이트 `AsRawSocket`만 요구하는 빈 마커. 불변식이라 할 만한 상태는 없다.

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
| --- | --- |
| `openssl` | TLS 구현체. `SslStream<TcpStream>` 타입과 동기 `Read`/`Write` 제공. 본 크레이트는 이를 한 겹 래핑할 뿐이며, 빌드 시 vendored OpenSSL(Strawberry Perl + MSVC 필요)을 사용한다. |
| `async-io` | 비동기 reactor. `async_io::IoSafe` 마커 트레이트를 제공하며, 소비처가 사용하는 `async_io::Async`/`smol::Async`가 raw socket을 OS 폴링 메커니즘(Windows에서는 IOCP/AFD 계열)에 등록할 때 이 마커를 요구한다. |

`std` 외에 추가 런타임 의존은 없다. 두 의존 모두 workspace 버전(`async_ossl/Cargo.toml:12-13`)을 상속한다.

## 7. 설정·기능 플래그

본 크레이트에는 feature flag가 정의되어 있지 않다(`async_ossl/Cargo.toml`에 `[features]` 섹션 없음). 직접 참조하는 `config` 항목도 없다.

다만 본 크레이트가 처리하는 TLS 연결의 파라미터는 호출 측 설정에서 결정된다. 예: 클라이언트는 `tls_client.write_timeout`/`read_timeout`/`expected_cn`(`wezterm-client/src/client.rs:980-989`)을 적용한 뒤 핸드셰이크 결과를 본 어댑터에 주입한다. 이들은 `config` 크레이트 영역이며 본 크레이트 밖이다.

## 8. Windows 전용 고려사항

본 크레이트는 Windows fork의 다른 모듈과 달리 잔존 비Windows 분기나 죽은 경로가 없다. 소켓 추상화에 처음부터 `std::os::windows::io`를 직접 사용한다.

- `AsRawDesc`의 슈퍼트레이트: `std::os::windows::io::AsRawSocket` (`lib.rs:4`).
- `AsRawSocket` 구현: `as_raw_socket(&self) -> RawSocket` (`lib.rs:19-23`).
- `AsSocket` 구현: `as_socket(&self) -> BorrowedSocket<'_>` (`lib.rs:25-29`).

`cfg(unix)`·`cfg(windows)` 분기 자체가 존재하지 않는다. Unix였다면 `AsRawFd`/`AsFd`/`RawFd`를 사용했을 자리에 Windows 소켓 타입이 무조건적으로(unconditional) 박혀 있다. 따라서 본 크레이트는 비Windows 타깃에서 컴파일되지 않는다 — 이는 Windows 전용 영구 분기 정책에 부합하는 의도된 상태다.

Windows API를 직접 호출하지는 않는다. raw socket 핸들 획득은 표준 라이브러리 트레이트 위임으로 처리되며, 실제 IOCP/AFD 등록은 소비처가 사용하는 `async-io` 내부에서 이루어진다.

## 9. 리팩토링 주의점

- `AsRawDesc` 트레이트의 중복 정의: 동일한 이름의 마커 트레이트가 본 크레이트(`async_ossl/src/lib.rs:4`)와 `wezterm-mux-server-impl/src/dispatch.rs:11` 두 곳에 각각 정의되어 있다. 두 정의는 별개의 타입이며 슈퍼트레이트 바운드도 다르다 — 본 크레이트는 `AsRawSocket`만, dispatch 측은 `AsRawSocket + AsSocket` 둘 다 요구한다. dispatch 측은 자기 자신의 `AsRawDesc`를 `AsyncSslStream`과 `UnixStream`에 대해 다시 구현한다(`dispatch.rs:13-14`). 즉 `process<T: dispatch::AsRawDesc>`가 실제로 사용하는 제약은 dispatch 쪽 트레이트이며, 본 크레이트의 `AsRawDesc`(및 `lib.rs:31`의 구현)는 현재 어떤 소비처에서도 바운드로 쓰이지 않는 사실상 미사용 표면이다. 통합·제거를 검토할 가치가 있다. 단, 제거 시 두 트레이트가 동명이인이라는 점을 먼저 인지해야 혼동을 피할 수 있다.

- `dispatch.rs:13` 의 `impl AsRawDesc for UnixStream`: Windows fork에서 mux 서버가 `wezterm_uds::UnixStream`을 통해 어떤 경로로 작동하는지에 따라 죽은 코드일 수 있다. 이는 dispatch 크레이트 영역이나, 본 크레이트의 `AsRawDesc`를 정리할 때 함께 확인해야 할 결합 지점이다.

- `IoSafe`의 `unsafe impl`(`lib.rs:11`): 내부 구조를 바꿔 `TcpStream` 소유권 모델을 변경(예: `Arc`/`Box`로 간접화, 소켓 dup 등)하면 이 unsafe 계약의 전제가 흔들린다. raw socket의 수명·동일성 보장을 깨지 않도록 주의해야 한다.

- 전송 타입 고정: 내부가 `SslStream<TcpStream>`로 하드코딩되어 있어 TCP 이외 전송(예: 다른 소켓 타입) 위에 TLS를 얹으려면 본 타입을 제네릭화해야 한다. 현재는 단순성을 위해 의도적으로 고정되어 있다.

- 순환 의존: 없음. 본 크레이트는 leaf이며 워크스페이스 내부 크레이트에 의존하지 않는다.

- 파급 범위: 공개 표면이 극히 작아(생성자 1개 + struct 1개 + 마커 트레이트 1개) `new` 시그니처나 구현 트레이트 집합을 바꾸면 4개 소비처 전부가 영향을 받는다. 다만 각 소비처의 사용 패턴(`Async::new(AsyncSslStream::new(...))`)이 동일하여 변경 지점은 예측 가능하다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `async_ossl/Cargo.toml` | 15 | 패키지 정의. 의존: `async-io`, `openssl`(둘 다 workspace 상속). `publish = false`. |
| `async_ossl/src/lib.rs` | 47 | 크레이트 전부. `AsyncSslStream` 래퍼 + `AsRawDesc` 마커 + `Read`/`Write`/`AsRawSocket`/`AsSocket`/`IoSafe` 위임 구현. |

(생성 파일 없음.)
