# wezterm-mux-server-impl 폴더 기능 명세

## 1. 개요 및 책임

`wezterm-mux-server-impl`은 WezTerm 멀티플렉서(mux) 서버의 **프로토콜 처리 구현체**다. 단일 책임은 다음과 같다. 클라이언트(GUI 또는 CLI)가 보낸 `codec` PDU를 수신·디코드하여 `mux` 크레이트의 상태를 조작하고, mux의 상태 변화(알림)를 PDU로 인코드하여 클라이언트로 되돌려보내는 **서버 측 세션 단(端) 로직**을 제공한다.

이 크레이트는 실행 가능한 바이너리가 아니다. 라이브러리로서 두 소비처에 동작을 제공한다.

- `wezterm-mux-server`: 독립 실행형 mux 서버 데몬(headless 멀티플렉서).
- `wezterm-gui`: GUI 프로세스가 자체적으로 mux 서버 역할을 겸할 때(GUI에 다른 클라이언트가 붙는 경우).

따라서 이 크레이트는 "서버를 어떻게 띄울 것인가"(소켓 바인딩, accept 루프의 상위 구성, TLS accept)는 부분적으로만 담당하고, 실제 **세션당 요청/응답 디스패치와 화면 변경 푸시**가 핵심이다.

Windows fork에서의 실제 책임은 다음과 같이 좁혀진다. 비Windows 플랫폼 코드는 제거되었으므로 리스너 소켓은 Windows의 Unix Domain Socket(AF_UNIX, `wezterm-uds` 경유)과 TLS 스트림(`async_ossl`)만을 대상으로 한다. PKI는 Windows 호스트명·DNS 조회를 기반으로 매 기동 시 새 CA·서버 인증서를 생성한다.

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 | 사용 목적(요약) |
|---|---|---|
| 의존(deps) | `async_ossl` | TLS 스트림(`AsyncSslStream`)을 비동기 I/O 대상으로 사용 |
| | `codec` | 클라이언트-서버 와이어 프로토콜 PDU 정의·인코딩/디코딩 |
| | `config` | 도메인 설정(`unix_domains`, `ssh_domains`, `tls_clients` 등), PKI 디렉터리, 소켓 경로 |
| | `mux` | 멀티플렉서 코어(도메인·창·탭·페인 상태, 알림 구독) |
| | `portable-pty` | (serde 기능) PTY 크기 등 직렬화 가능한 타입 |
| | `promise` | `spawn_into_main_thread` 등 비동기 태스크 스폰 |
| | `rangeset` | 변경된 라인 집합(dirty lines) 표현 |
| | `termwiz` | 터미널 표면(`SequenceNo`), serde 직렬화 |
| | `wezterm-client` | `ClientDomain`/`ClientDomainConfig`(서버가 다시 다른 서버의 클라이언트가 될 때) |
| | `wezterm-term` | 터미널 모델(`Alert`, `StableRowIndex`, `TermConfig` 연동) |
| | `wezterm-uds` | Windows용 Unix Domain Socket 리스너/스트림 |
| 피의존(usedBy) | `wezterm-gui` | GUI가 mux 서버 역할을 겸할 때 세션 처리 사용 |
| | `wezterm-mux-server` | 독립 실행형 mux 서버 데몬의 세션 처리 사용 |

이 크레이트는 계층 구조상 **`mux`(상태 코어)와 전송 계층(`wezterm-uds`/`async_ossl`) 사이의 프로토콜 어댑터**에 위치한다. 와이어 바이트와 mux 내부 상태를 양방향으로 번역하는 경계 계층이다.

## 3. 공개 API 표면

크레이트 루트(`lib.rs`)는 네 개의 하위 모듈을 `pub`으로 노출한다: `dispatch`, `local`, `pki`, `sessionhandler`.

### 루트 함수·정적 항목 (`lib.rs`)

- `pub fn update_mux_domains(config: &ConfigHandle) -> anyhow::Result<()>` — 설정에 선언된 도메인(unix/ssh/tls 클라이언트 도메인, ssh, wsl, exec, serial)을 mux에 등록한다. GUI/일반 클라이언트 관점.
- `pub fn update_mux_domains_for_server(config: &ConfigHandle) -> anyhow::Result<()>` — 동일하나 독립 실행형 mux 서버 관점. 기본 도메인 선택 시 `default_mux_server_domain`을 사용하며, 클라이언트 도메인을 기본으로 두는 것을 거부한다.
- `pub static ref PKI: pki::Pki` — `lazy_static`로 프로세스당 1회 초기화되는 PKI 핸들. 초기화 실패 시 `expect`로 패닉.

두 공개 함수는 내부 `update_mux_domains_impl(config, is_standalone_mux)`로 위임하며, 두 진입점의 유일한 차이는 기본 도메인 결정 분기다(`lib.rs:91-106`).

### `sessionhandler` 모듈

- `pub struct PduSender` — `Arc<dyn Fn(DecodedPdu) -> Result<()> + Send + Sync>`를 감싼 송신 클로저. `new<T>(f)`/`send(&self, pdu)` 제공(`sessionhandler.rs:20-36`).
- `pub struct SessionHandler` — 하나의 클라이언트 연결에 대한 세션 상태. `new(to_write_tx: PduSender)`, `process_one(&mut self, decoded: DecodedPdu)`, `schedule_pane_push(&mut self, pane_id: PaneId)`를 노출한다. `Drop` 시 등록된 `client_id`를 mux에서 해제한다(`sessionhandler.rs:206-213`).

### `dispatch` 모듈

- `pub trait AsRawDesc: AsRawSocket + AsSocket` — Windows 소켓 디스크립터 추상화. `UnixStream`과 `AsyncSslStream`에 구현.
- `pub async fn process<T>(stream: T) -> Result<()>` — 동기 스트림을 `smol::Async`로 감싼 뒤 `process_async`로 위임.
- `pub async fn process_async<T>(stream: Async<T>) -> Result<()>` — 세션 이벤트 루프 본체.

### `local` 모듈

- `pub struct LocalListener` — Unix Domain Socket 리스너 래퍼. `new(listener)`, `with_domain(unix_dom: &UnixDomain) -> Result<Self>`, `run(&mut self)`(accept 루프) 제공.

### `pki` 모듈

- `pub struct Pki` — TLS용 CA/서버/클라이언트 인증서 발급기. `init()`, `generate_client_cert() -> Result<String>`, `ca_pem_string()`, `ca_pem() -> PathBuf`, `server_pem() -> PathBuf`.

## 4. 내부 구조

모듈 분해는 책임별로 명확히 갈린다.

- `lib.rs` — 도메인 등록 정책. 설정→mux 도메인 동기화의 순수 로직.
- `local.rs` — 리스너 수립 및 accept 루프. 연결마다 `dispatch::process`를 메인 스레드 태스크로 스폰.
- `dispatch.rs` — **세션 이벤트 루프**. 단일 연결에 대해 (a) 클라이언트로부터의 읽기 가능 이벤트, (b) 송신 채널(`item_tx`)로 들어온 PDU, (c) mux 알림을 하나의 `select`(`smol::future::or`)로 다중화한다.
- `sessionhandler.rs` — **요청 디스패처**(비대 모듈, 약 1130행). `process_one`의 거대한 `match decoded.pdu` 분기가 전체 와이어 프로토콜의 서버 측 처리를 담당한다.
- `pki.rs` — 인증서 생성. 상태가 없는 보조 모듈.

### 제어/데이터 흐름

1. `LocalListener::run`이 소켓을 accept하고, 연결마다 `dispatch::process(stream)`를 `spawn_into_main_thread`로 스폰한다(`local.rs:20-38`).
2. `process_async`가 unbounded 채널 `(item_tx, item_rx)`을 만들고, `item_tx`를 캡처한 클로저로 `PduSender`를 구성해 `SessionHandler`에 주입한다. 또한 `mux.subscribe`로 mux 알림을 `Item::Notif`로 같은 채널에 밀어넣는다(`dispatch.rs:46-62`).
3. 이벤트 루프는 `item_rx.recv()`와 `stream.readable()`을 `or`로 경쟁시킨다(`dispatch.rs:64-68`).
   - `Item::Readable`: `Pdu::decode_async`로 PDU 한 개를 읽어 `handler.process_one`에 넘긴다. EOF는 조용한 정상 종료로 처리.
   - `Item::WritePdu`: PDU를 스트림에 인코딩·flush. BrokenPipe는 조용한 종료.
   - `Item::Notif(...)`: mux 알림을 종류별로 처리(아래 5절·8절).
4. `process_one`은 대부분의 요청을 `spawn_into_main_thread`로 다시 메인 스레드에 위임한다. 처리 결과는 `send_response` 클로저가 `PduSender`를 통해 채널로 보내고, 루프가 `Item::WritePdu`로 받아 실제 와이어에 쓴다. 즉 **읽기 루프와 mux 상태 조작이 채널로 분리**되어 있다.

`catch(f, send_response)` 헬퍼는 `f`의 `Result<Pdu>`를 `send_response`로 흘리며, `Err`는 `Pdu::ErrorResponse`로 변환된다(`sessionhandler.rs:258-275`).

### 화면 변경 푸시 메커니즘

`PerPane::compute_changes`(`sessionhandler.rs:52-143`)가 페인의 이전 스냅샷(커서·제목·작업 디렉터리·크기·마우스 그랩 상태·`seqno`)과 현재 상태를 비교해 변경이 있을 때만 `GetPaneRenderChangesResponse`를 만든다. 변경 라인은 `get_changed_since(seqno)`로 산출하고, 뷰포트 내 dirty 라인은 `bonus_lines`로, 커서가 있는 줄은 항상 별도로 동봉한다(예측 입력 에코의 커서 위치 보정 목적, `sessionhandler.rs:117-122`). `maybe_push_pane_changes`는 변경 푸시에 더해 config 세대(generation) 변화나 미전송 초기 팔레트를 감지하면 `Alert::PaletteChanged`를 합성하고, 큐에 쌓인 `Alert`를 `SetPalette`/`NotifyAlert` PDU로 비운다(`sessionhandler.rs:159-196`).

## 5. 핵심 데이터 구조·타입

- `PduSender`(`sessionhandler.rs:20-36`) — 송신 클로저 래퍼. 불변식: 내부 함수는 `Send + Sync`이며 채널로의 비차단 `try_send`만 수행한다(블로킹 없음).
- `PerPane`(`sessionhandler.rs:38-49`, `pub(crate)`) — 페인별 마지막으로 클라이언트에 보낸 상태의 캐시. 필드: `cursor_position`, `title`, `working_dir`, `dimensions`, `mouse_grabbed`, `sent_initial_palette`, `seqno`, `config_generation`, `notifications: Vec<Alert>`. 불변식: `seqno`는 단조 증가하는 마지막 처리 시퀀스 번호이며, `compute_changes`가 갱신하기 전까지 "클라이언트가 알고 있는 상태"를 나타낸다. `notifications`는 다음 푸시 시 모두 비워져야 하는 보류 알림 큐다.
- `SessionHandler`(`sessionhandler.rs:199-204`) — 세션 상태. `to_write_tx: PduSender`, `per_pane: HashMap<TabId, Arc<Mutex<PerPane>>>`, `client_id: Option<Arc<ClientId>>`, `proxy_client_id: Option<ClientId>`. 불변식: `client_id`가 `Some`이면 mux에 등록된 상태이며 `Drop`에서 정확히 한 번 해제된다. 프록시 세션에서는 실제 클라이언트 ID에 프록시 정보(ssh_auth_sock, "via proxy pid")를 덧씌운다(`sessionhandler.rs:298-330`). 참고: `per_pane` 맵의 키 타입은 `TabId`로 선언되어 있으나 실사용에서는 `PaneId`로 색인된다(두 타입은 동일 정수 별칭).
- `Item`(`dispatch.rs:16-21`, 비공개) — 이벤트 루프의 다중화 단위: `Notif(MuxNotification)`, `WritePdu(Box<DecodedPdu>)`, `Readable`.
- `Pki`(`pki.rs:17-20`) — `ca_cert: Certificate`, `pki_dir: PathBuf`. 불변식: 프로세스 기동마다 새 CA를 만들어 이전 키를 무효화한다(`pki.rs:7-9` 주석). 따라서 `PKI` 정적은 프로세스 수명 동안 동일한 CA를 보장한다.

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
|---|---|
| `smol` / `async-io` | 단일 스레드 친화 비동기 런타임. `smol::Async`로 동기 소켓을 비동기화하고, `smol::future::or`로 읽기/쓰기/알림을 다중화. `async-io`의 `IoSafe` 트레이트 바운드로 안전한 `Async` 래핑 보장 |
| `futures` | `FutureExt::map`으로 `readable()` 결과를 `Item::Readable`로 변환 |
| `rcgen` | TLS용 X.509 CA·서버·클라이언트 인증서를 코드로 생성(자가 서명 CA 체계) |
| `hostname` | 호스트명을 인증서 alt_name으로 사용 |
| `dns-lookup` | `getaddrinfo`로 호스트의 정규명(canonical name)을 추가 alt_name으로 수집 |
| `url` | 페인 작업 디렉터리(`Option<Url>`) 비교·전송 |
| `lazy_static` | 프로세스 단위 단일 `PKI` 정적 초기화 |
| `winapi` | (Windows 전용) `ws2def`의 `AF_UNSPEC`, `AI_CANONNAME`, `SOCK_DGRAM` 상수를 `dns-lookup` 힌트에 사용 |
| `libc` | Cargo.toml에 선언되어 있으나 소스에서 직접 참조되지 않는다. 비Windows 코드 제거 후 남은 잔여 의존성으로 보인다(9절 참조) |

## 7. 설정·기능 플래그

이 크레이트 자체에는 Cargo feature flag가 없다. 의존 크레이트에 대해서만 기능을 켠다: `portable-pty`의 `serde_support`, `termwiz`/`wezterm-term`의 `use_serde`(PDU 직렬화 목적).

관련 `config` 항목(`lib.rs`에서 직접 소비):

- `config.unix_domains` — Unix Domain 소켓 클라이언트 도메인.
- `config.ssh_domains()` — SSH 도메인. `multiplexing` 값에 따라 분기: `WezTerm`이면 클라이언트 도메인, `None`이면 `RemoteSshDomain`으로 등록.
- `config.tls_clients` — TLS 클라이언트 도메인.
- `config.wsl_domains()`, `config.exec_domains`, `config.serial_ports` — 로컬 도메인(WSL/실행/시리얼).
- `config.default_domain` / `config.default_mux_server_domain` — 기본 도메인 선택(진입점에 따라 분기).
- `config.generation()` — 팔레트 변경 감지 트리거.
- `config::pki_dir()`, `config::username_from_env()` — PKI 디렉터리·인증서 CN.
- `UnixDomain::socket_path()`, `config::create_user_owned_dirs`, `config::set_sticky_bit` — 소켓 경로 보안 수립(`local.rs`).

설정 철학상, 이 동작들은 `wezterm.lua`보다 `config` 크레이트 기본값을 직접 수정하는 방식으로 고정하는 것이 우선된다.

## 8. Windows 전용 고려사항

- **소켓 디스크립터 추상화**: `dispatch.rs:11`의 `AsRawDesc` 트레이트는 `std::os::windows::io::{AsRawSocket, AsSocket}`만 상속한다. 비Windows의 `RawFd` 경로는 제거되었다. 이 fork에서 리스너는 Windows의 AF_UNIX 소켓(`wezterm-uds`)을 사용한다.
- **소켓 경로 정리**: `local.rs:55-65`는 바인드 전 소켓 파일을 무조건 `remove_file`로 지운다. 주석대로 Windows에서는 `Path`로 UDS 존재 여부를 판별할 수 없어, 제거를 시도하고 `NotFound`만 무시하는 방식을 쓴다. `config::set_sticky_bit`은 Windows에서 사실상 무동작(no-op)일 가능성이 높다.
- **PKI의 winapi 사용**: `pki.rs:4`는 `winapi::shared::ws2def`에서 `AF_UNSPEC`, `AI_CANONNAME`, `SOCK_DGRAM`을 가져와 `dns_lookup::AddrInfoHints`를 구성한다. 이는 Winsock 상수 직접 참조다.
- **죽은/잔여 코드**: 변수명·타입명에 `unix`가 다수 남아 있으나(`unix_domains`, `UnixDomain`, `unix_name`) 이는 의미상 명칭일 뿐 비Windows 분기가 아니다. Windows의 AF_UNIX 지원을 그대로 사용한다. `cfg(unix)` 분기는 이 크레이트 소스에 존재하지 않는다. `Cargo.toml`의 `libc` 의존성은 소스 미사용으로, 플랫폼 코드 제거의 잔여물로 판단된다.

## 9. 리팩토링 주의점

- **`sessionhandler.rs`의 비대 `match`**: `process_one`의 거대한 PDU 분기(약 740행)가 단일 책임을 넘어선다. PDU 종류 추가/변경 시 이 함수와, 처리 불가 PDU를 나열한 폴백 분기(`sessionhandler.rs:991-1015`)를 동시에 갱신해야 한다. 둘 사이의 불일치는 런타임 "expected a request" 오류로만 드러난다(컴파일러가 누락을 잡지 못하는 부분이 있음).
- **채널 분리 불변식**: 읽기 루프(`dispatch.rs`)와 상태 조작(`spawn_into_main_thread`로 위임된 클로저)은 `PduSender` 채널로만 통신한다. `PduSender::send`는 `try_send`(비차단)이므로, 채널이 막히면 PDU가 유실될 수 있다(unbounded이므로 현재는 메모리 압박 형태로 나타남). 차단형으로 바꾸면 데드락 위험이 생긴다.
- **`PerPane` 락 범위**: `per_pane`는 `Arc<Mutex<PerPane>>`이며 `SendKeyDown` 등에서 락을 잡은 채 `compute_changes`→`sender.send`까지 수행한다. mux 메인 스레드 태스크 안에서 동작하므로 직렬화되지만, 락 보유 중 추가 비동기 await를 넣으면 교착 가능.
- **`spawn`/`split`/`move` 우회 헬퍼**: `schedule_domain_spawn_v2`/`schedule_split_pane`/`schedule_move_pane`은 컴파일러의 `Send` 분석을 피하려는 의도적 우회(`sessionhandler.rs:1020-1023` 주석). 시그니처를 단순화하려다 `Send` 요구가 전파되어 빌드가 깨질 수 있다.
- **`TabId` vs `PaneId` 키 혼용**: `per_pane: HashMap<TabId, ...>`인데 실제로는 페인 ID로 색인된다. 두 타입이 동일 별칭이라 동작하지만, 향후 타입을 분리하면 무음으로 깨진다.
- **PKI 매 기동 재생성**: `PKI` 정적은 프로세스마다 새 CA를 만든다. 서버 재시작 시 기존 클라이언트 인증서가 모두 무효화된다는 불변식에 의존하는 코드가 클라이언트 측에 있다.
- **mux 결합도**: 거의 모든 분기가 `Mux::get()` 전역 싱글톤과 `resolve_pane_id`/`get_pane`/`get_tab` 류 API에 강결합. mux의 ID 해석 의미가 바뀌면 이 크레이트 전반이 영향을 받는다.
- **`update_mux_domains` 멱등성**: `get_domain_by_name(...).is_some()` 검사로 중복 등록을 막지만, 도메인 설정이 변경(이름은 같고 내용이 다름)되어도 갱신하지 않는다. 설정 핫리로드 시 의도와 어긋날 수 있다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `src/lib.rs` | 114 | 크레이트 루트. 설정→mux 도메인 등록 로직(`update_mux_domains*`), `PKI` 정적, 모듈 공개 |
| `src/sessionhandler.rs` | 1128 | 세션당 요청 디스패처(`SessionHandler`/`process_one`), 페인 변경 계산·푸시(`PerPane`/`maybe_push_pane_changes`), spawn/split/move 우회 헬퍼 |
| `src/dispatch.rs` | 211 | 세션 이벤트 루프(`process`/`process_async`). 읽기·쓰기·mux 알림 다중화 및 와이어 인코딩 |
| `src/local.rs` | 73 | Unix Domain Socket 리스너(`LocalListener`)와 보안 소켓 경로 수립(`safely_create_sock_path`) |
| `src/pki.rs` | 111 | TLS용 CA/서버/클라이언트 인증서 생성기(`Pki`). 매 기동 새 CA |
| `Cargo.toml` | 36 | 크레이트 매니페스트. deps 및 `cfg(windows)` 전용 `winapi` |
