# wezterm-client 폴더 기능 명세

> 대상 경로: `E:\Project\wezterm\wezterm-client`
> 크레이트: `wezterm-client` (단일 크레이트)
> 본 문서는 추후 리팩토링을 위한 상세 기능 명세다. 모든 인용은 `파일경로:라인` 형식을 따른다.

---

## 1. 개요 및 책임

`wezterm-client`는 WezTerm GUI(또는 CLI)가 **원격 또는 로컬 mux 서버(wezterm-mux-server)에 클라이언트로 접속**할 때 사용하는 크레이트다. 단일 책임은 다음과 같다.

- mux 서버와의 **연결 수립·유지·재접속**(Unix 소켓 / TLS / SSH 세 가지 전송).
- `codec` 크레이트가 정의한 PDU(Protocol Data Unit)를 **직렬화·전송하고 응답을 promise로 매칭**하는 RPC 계층 제공.
- 서버 측 mux 토폴로지(window/tab/pane)를 **로컬 mux로 미러링**하고, remote↔local ID 매핑을 유지하는 `ClientDomain` 구현.
- 원격 pane 1개를 로컬에서 표현하는 `ClientPane`을 통해, 렌더 가능한 화면 데이터를 **지연 페치·캐시·예측 에코(local echo)** 방식으로 제공.

핵심 설계 의도는 네트워크 지연(latency)을 흡수하는 것이다. 서버에서 화면 변경 알림(unilateral PDU)을 받으면 변경 행을 비동기로 페치하고, 사용자가 입력하면 응답이 도착하기 전에 **예측 에코**를 화면에 그려 체감 지연을 줄인다(`wezterm-client/src/pane/renderable.rs:157` 이하).

Windows fork에서의 실제 책임은 동일하나, 전송 계층의 하부 구현(`UnixStream`)이 `wezterm-uds`를 경유해 Windows 소켓 위에서 동작한다는 점, 그리고 로컬 GUI 소켓 발견(discovery)을 위해 **Windows 공유 메모리 + 명명 뮤텍스**를 직접 사용한다는 점이 특징이다(`wezterm-client/src/discovery.rs:19` 이하).

---

## 2. 워크스페이스 내 위치

### 의존(deps)

| 크레이트 | 사용 이유 |
|---|---|
| `async_ossl` | TLS 스트림을 async I/O로 감싸는 `AsyncSslStream`(`client.rs:983`) |
| `codec` | 클라이언트↔서버 간 모든 PDU 정의·인코딩·디코딩의 단일 원천 |
| `config` | 도메인 설정(`UnixDomain`/`TlsDomainClient`/`SshDomain`), `RUNTIME_DIR`, `pki_dir`, 팔레트 등 |
| `filedescriptor` | SSH 프록시용 socketpair·파일 디스크립터 추상화(`client.rs:462`) |
| `mux` | `Domain`/`Pane` 트레이트, `Mux` 싱글턴, `ConnectionUI`, ID 타입 — 통합 지점 |
| `portable-pty` | `CommandBuilder`(spawn 요청), `Child` 트레이트 |
| `promise` | `spawn`/`spawn_into_main_thread`/`block_on` — 비동기 태스크 디스패치 |
| `rangeset` | dirty 행 집합·페치 대상 행 집합 표현(`RangeSet<StableRowIndex>`) |
| `ratelim` | 행 프리페치 스로틀링(`RateLimiter`) |
| `termwiz` | `Line`/`Cell`/`KeyEvent`/`ImageCell` 등 터미널 셀·입력 원형 |
| `wezterm-dynamic` | `ClientPane::get_metadata`의 `Value` 직렬화(`clientpane.rs:261`) |
| `wezterm-ssh` | SSH 부트스트랩 세션(`Config`, `ssh_connect_with_ui` 경유) |
| `wezterm-term` | `TerminalSize`/`ColorPalette`/`Alert`/`Progress` 등 터미널 모델 |
| `wezterm-uds` | `UnixStream` — Windows에서의 도메인 소켓 추상화 |
| 기타 | `anyhow`, `async-io`, `async-trait`, `futures`, `smol`, `lru`, `lazy_static`, `log`, `metrics`, `openssl`, `parking_lot`, `textwrap`, `thiserror`, `url` |

### 피의존(usedBy)

| 크레이트 | 사용 형태 |
|---|---|
| `wezterm` (CLI) | `wezterm-client/src/cli/*` 전반에서 `client::Client`로 RPC 호출. `proxy.rs`는 `unix_connect_with_retry`·`ClientDomainConfig`도 사용 |
| `wezterm-gui` | `domain::ClientDomain`(main.rs:30), `discovery::{discover_gui_socks, resolve_gui_sock_path, publish_gui_sock_path}`, `client::Client::new_unix_domain` |
| `wezterm-mux-server-impl` | `domain::{ClientDomain, ClientDomainConfig}` — 서버가 다른 mux에 클라이언트로 연결할 때 |

### 계층상 위치

`wezterm-client`는 **`mux`(데이터 모델·통합 허브)와 `codec`(와이어 프로토콜) 사이의 클라이언트 측 어댑터**다. 위로는 `wezterm-gui`/`wezterm` CLI가 `Client`·`ClientDomain`을 통해 원격 mux를 마치 로컬 도메인처럼 다루게 하고, 아래로는 `codec` PDU를 전송 스트림(UDS/TLS/SSH)에 실어 보낸다.

---

## 3. 공개 API 표면

모듈 트리는 단순하다(`wezterm-client/src/lib.rs:1`): `client`, `discovery`, `domain`, `pane`.

### 3.1 `client` 모듈 (`client.rs`)

- `pub struct Client` (`client.rs:51`): 단일 mux 연결의 클라이언트 핸들. `Clone` 가능(내부 `Sender` 공유). 필드: `client_id: ClientId`, `is_reconnectable`, `is_local`(공개), `local_domain_id`·`sender`·`client_domain_config`(비공개).
  - 생성자: `new_unix_domain`, `new_default_unix_domain`, `new_tls`, `new_ssh`(`client.rs:1221`~`1266`).
  - `pub async fn send_pdu(&self, pdu: Pdu) -> anyhow::Result<Pdu>` (`client.rs:1268`): 모든 RPC의 토대. PDU를 reader 스레드로 보내고 promise 채널로 응답을 기다린다.
  - `pub async fn verify_version_compat(&self, ui) -> Result<GetCodecVersionResponse>` (`client.rs:1112`): codec 버전 호환성 검증 + `SetClientId` 전송. 60초 타임아웃.
  - `pub async fn resolve_pane_id(&self, Option<PaneId>) -> Result<PaneId>` (`client.rs:1281`): `--pane-id` 미지정 시 `$WEZTERM_PANE` 또는 포커스된 pane으로 해석.
  - `pub fn into_client_domain_config(self) -> ClientDomainConfig` (`client.rs:1108`).
  - `rpc!` 매크로로 생성된 **약 30개 비동기 RPC 메서드**(`client.rs:1308`~`1358`): `ping`, `list_panes`, `spawn_v2`, `split_pane`, `move_pane_to_new_tab`, `write_to_pane`, `send_paste`, `key_down`, `mouse_event`, `resize`, `set_zoomed`, `activate_pane_direction`, `get_pane_render_changes`, `get_lines`, `get_dimensions`, `get_codec_version`, `get_tls_creds`, `search_scrollback`, `kill_pane`, `set_client_id`, `list_clients`, `set_window_workspace`, `set_focused_pane_id`, `get_image_cell`, `set_configured_palette_for_pane`, `set_tab_title`, `set_window_title`, `rename_workspace`, `erase_scrollback`, `get_pane_direction`, `adjust_pane_size`. 각 메서드는 `metrics` 히스토그램/카운터를 기록한다.
- `pub fn unix_connect_with_retry(target: &UnixTarget, just_spawned: bool, max_attempts: Option<u64>) -> Result<UnixStream>` (`client.rs:433`): 재시도 백오프와 프록시 커맨드(`UnixTarget::Proxy`)를 지원하는 UDS 접속 헬퍼. CLI `proxy.rs`가 직접 사용.
- `pub trait AsyncReadAndWrite` (`client.rs:510`): `Unpin + AsyncRead + AsyncWrite + Debug + Send` + `wait_for_readable()`. 세 전송을 단일 트레이트 객체로 추상화.
- `pub struct IncompatibleVersionError` (`client.rs:71`): 버전 불일치 오류(`thiserror`).
- `pub fn tls_connect(...)` (`client.rs:774`): `Reconnectable`의 메서드이나 `pub`. (단, `Reconnectable` 자체는 비공개라 외부 호출 경로는 제한적.)

### 3.2 `discovery` 모듈 (`discovery.rs`)

- `pub use windows::NameHolder` (`discovery.rs:249`): 발행한 경로를 프로세스 수명 동안 살아 있게 하는 핸들.
- `pub fn publish_gui_sock_path(path: &Path, class_name: &str) -> Result<NameHolder>` (`discovery.rs:253`): GUI 소켓 경로를 desktop-local 공유 메모리에 발행.
- `pub fn resolve_gui_sock_path(class_name: &str) -> Result<PathBuf>` (`discovery.rs:260`): 마지막으로 발행된 경로 조회(실행 중 보장 없음).
- `pub fn discover_gui_socks() -> Vec<PathBuf>` (`discovery.rs:269`): `RUNTIME_DIR`의 `gui-sock-*` 항목을 스캔, 죽은 소켓 정리 후 오래된 순으로 정렬해 반환.

### 3.3 `domain` 모듈 (`domain.rs`)

- `pub struct ClientDomain` (`domain.rs:253`): `mux::Domain` 트레이트 구현체. `pub fn new(ClientDomainConfig)`(`domain.rs:399`).
- `pub enum ClientDomainConfig` (`domain.rs:178`): `Unix`/`Tls`/`Ssh` variant. 접근자 `name`/`local_echo_threshold_ms`/`overlay_lag_indicator`/`label`/`connect_automatically`.
- `pub struct ClientInner` (`domain.rs:20`): 연결된 도메인의 가변 상태(클라이언트 핸들 + 3종 ID 매핑 맵 + 포커스 pane).
- `ClientDomain`의 주요 공개 메서드: `connect_automatically`, `perform_detach`, `remote_to_local_pane_id`, `remote_to_local_window_id`, `local_to_remote_window_id`, `local_to_remote_tab_id`, `get_client_inner_for_domain`(연관 함수), `reattach`(연관 async), `resync`, `process_remote_window_title_change`, `process_remote_tab_title_change`.
- `impl Domain for ClientDomain` (`domain.rs:740`): `spawn`, `split_pane`, `move_pane_to_new_tab`, `attach`, `detach`, `state`, `domain_id`, `domain_name`, `domain_label`. (`spawn_pane`은 미구현 — `bail!`.)

### 3.4 `pane` 모듈 (`pane/mod.rs`)

- `pub use clientpane::ClientPane` (`pane/mod.rs:1`)만 공개. `mousestate`·`renderable`은 crate-private.
- `pub struct ClientPane` (`pane/clientpane.rs:34`): `mux::Pane` 트레이트 구현체. `pub fn new(...)`, `pub async fn process_unilateral(&self, Pdu)`, `pub fn remote_pane_id()`, `pub fn ignore_next_kill()`. 공개 필드 `remote_pane_id`, `remote_tab_id`, `renderable`.

---

## 4. 내부 구조

### 4.1 연결·RPC 흐름 (`client.rs`)

1. `Client::new`(`client.rs:1005`)는 `Reconnectable`을 받아 **전용 OS 스레드**를 띄운다. 이 스레드는 `client_thread`→`client_thread_async`(`client.rs:344`)를 `block_on`으로 돌린다.
2. `client_thread_async`는 단일 루프에서 두 future를 `smol::future::or`로 경쟁시킨다: (a) 송신 채널 `rx.recv()`, (b) `stream.wait_for_readable()`.
   - `SendPdu` 수신 시: 단조 증가 `serial` 부여 → `promises` 맵에 promise 등록 → `pdu.encode_async` + flush.
   - `Readable` 시: `Pdu::decode_async`. `serial == 0`이면 **unilateral PDU**(`process_unilateral`로 분기), 그 외엔 `serial`로 promise 매칭.
3. 연결 단절 시 재접속: `is_reconnectable`이고 `local_domain_id`가 있을 때만 지수 백오프(1초→최대 10초)로 재시도하고, 성공 시 `ClientDomain::reattach`를 메인 스레드에 디스패치(`client.rs:1042`~).
4. `Reconnectable`(`client.rs:529`)은 설정에 따라 `unix_connect`/`tls_connect`/`ssh_connect`를 호출해 `Box<dyn AsyncReadAndWrite>` 스트림을 만든다.

### 4.2 unilateral PDU 처리 (`client.rs:190`)

서버가 자발적으로 보내는 알림을 종류별로 분기한다. 워크스페이스/타이틀 변경류(`WindowWorkspaceChanged`, `WindowTitleChanged`, `RenameWorkspace`, `TabTitleChanged`)는 메인 스레드 태스크로 처리하고, `TabResized`/`TabAddedToWindow`는 **resync**를 유발한다. pane 단위 PDU는 `process_unilateral_inner_async`(`client.rs:120`)가 remote→local pane 해석 후 `ClientPane::process_unilateral`로 위임하며, 매핑이 없으면 `resync` 후 재시도한다.

### 4.3 도메인 동기화 (`domain.rs`)

- `process_pane_list`(`domain.rs:504`)는 **mark-and-sweep GC 패턴**이다. 현재 remote ID 집합을 "표시"한 뒤 서버의 `ListPanesResponse`를 순회하며 살아 있는 ID를 표시 해제하고, 남은(=사라진) 매핑을 마지막에 일괄 삭제한다. 이 함수가 window/tab/pane 매핑 갱신, 로컬 tab/pane 생성(`ClientPane::new`), 워크스페이스 일치 검사까지 담당하는 **이 크레이트에서 가장 비대한 동기화 로직**이다.
- `mux_notify_client_domain`(`domain.rs:269`)은 로컬 mux 이벤트를 서버로 역전파한다(타이틀·워크스페이스 변경). `WindowTitleChanged`는 PDU 핑퐁 사이클을 막기 위해 1초 디바운스 후 현재 타이틀을 전송한다(`domain.rs:357`~).
- `attach`(`domain.rs:931`)는 별도 스레드에서 `Client`를 생성→버전 검증→`list_panes`→`finish_attach` 순으로 진행하며, 진행 상황을 `ConnectionUI`에 출력한다.

### 4.4 pane 렌더링 파이프라인 (`pane/`)

- `ClientPane`(`clientpane.rs`)은 `mux::Pane` 트레이트의 모든 메서드를 구현하되, 대부분의 동작(`key_down`, `resize`, `send_paste`, `kill`, `search`, `erase_scrollback` 등)을 RPC로 서버에 전달한다. 입력 계열은 예측 에코를 먼저 적용한 뒤 RPC를 디스패치한다.
- `RenderableState`/`RenderableInner`(`renderable.rs`)는 행 단위 LRU 캐시(`lines: LruCache<StableRowIndex, LineEntry>`)와 폴링·페치 로직을 보유한다. `get_lines`(`renderable.rs:727`)가 GUI 렌더 시 호출되며, 캐시 상태에 따라 즉시 반환하거나 페치 대상으로 표시한다. 이 파일도 비대(859행)하며 예측 에코·페치 상태기계가 집중돼 있다.
- `MouseState`(`mousestate.rs`)는 마우스 이벤트를 큐에 모으고 연속 이동/휠을 **합치며(coalesce)**, 한 번에 하나씩 in-flight로 전송한다(`pending` 플래그).
- `hydrate_lines`(`renderable.rs:643`)는 직렬화된 라인의 이미지 셀을 해시 기반 LRU(`IMAGE_LRU`, 128 entries)로 캐시하면서 `get_image_cell` RPC로 보충한다.

---

## 5. 핵심 데이터 구조·타입

### `Client` (`client.rs:51`)
연결 1개당 핸들. 불변식: `sender`는 reader 스레드가 살아 있는 한 유효하며, 스레드 종료 시 채널이 닫혀 `send_pdu`가 `ChannelSendError`로 실패한다. `Clone`은 동일 연결을 공유한다.

### `Reconnectable` (`client.rs:529`)
`config`, `Option<Box<dyn AsyncReadAndWrite>>` 스트림, `Option<GetTlsCredsResponse>`. 불변식: `reconnectable()`는 `Tls`만 `true`(Unix·Ssh는 `false`, `client.rs:606`). `take_stream`은 reader 스레드가 스트림 소유권을 가져갈 때 1회 호출.

### `ClientInner` (`domain.rs:20`)
3개의 `Mutex<HashMap>`(remote↔local window/tab/pane)와 `focused_remote_pane_id`. 불변식:
- 매핑은 단방향(remote→local) 저장이며 역방향 조회(`local_to_remote_*`)는 선형 탐색이다.
- `expire_stale_mappings`(`domain.rs:37`)·`process_pane_list`의 sweep 단계가 실제 mux 상태와 매핑의 일관성을 보존한다. 매핑이 mux 실태와 어긋나면 unilateral 처리에서 resync가 트리거된다.

### `ClientDomainConfig` (`domain.rs:178`)
3-variant enum. 불변식: variant가 곧 전송 종류이며, `reconnectable()`/`is_local()`/credential 경로가 이 variant로 분기한다.

### `ClientPane` (`clientpane.rs:34`)
원격 pane 1개의 로컬 대리자. 다수의 `parking_lot::Mutex` 필드로 상태 보호. 불변식:
- `palette`는 애플리케이션이 escape로 팔레트를 바꾸지 않은 동안에만 설정 팔레트를 추종한다(`application_palette` 플래그, `clientpane.rs:613`).
- `ignore_next_kill`은 윈도우 닫힘 시 detach를 위한 1회성 kill 억제(`clientpane.rs:250`, `484`).
- `kill`은 도메인이 `Detached` 상태면 서버로 kill을 보내지 않는다(`clientpane.rs:500`~).

### `LineEntry` (`renderable.rs:31`)
4-state 열거형: `Line`(최신·렌더됨), `Fetching(Instant)`(다운로드 중), `LineAndFetching(Line, Instant)`(구버전 보유 + 신버전 페치 중), `Stale(Line)`(재페치 필요). 불변식: `put_line`은 페치 시작 `Instant`가 일치할 때만 `Fetching`/`LineAndFetching`을 `Line`으로 승격한다(`renderable.rs:457`~). 그 사이 상태가 바뀌었으면 덮어쓰지 않는다(중간 갱신 보호).

### `RenderableInner` (`renderable.rs:56`)
폴링 간격(`poll_interval`, 20ms~30s 지수 증가), `last_send_time`/`last_recv_time`/`last_input_rtt`, `input_serial`, `seqno`. 불변식: `is_tardy`(`renderable.rs:127`)는 최근 송신이 수신보다 늦고 폴 간격(최소 3초) 초과 시 true → 지연 인디케이터 표시. 커서 위치는 가장 최근 `input_serial` 응답만 채택해 커서 흔들림을 방지한다(`renderable.rs:347`).

---

## 6. 외부 의존성 (주목할 항목)

- **openssl / async_ossl**: TLS 전송. `SslConnector`로 클라이언트 인증서·CA를 로드하고(`client.rs:914`~), `AsyncSslStream`으로 async 래핑. 자격증명은 SSH 부트스트랩(`wezterm cli tlscreds`)으로 받아 `pki_dir`에 파일로 저장 후 openssl에 주입한다.
- **wezterm-ssh**: SSH 전송 및 TLS 부트스트랩 세션. 원격에서 `wezterm cli ... proxy`를 실행해 stdin/stdout을 mux 스트림으로 사용한다(`client.rs:651`~).
- **smol / async-io / futures**: reader 스레드의 이벤트 루프(`future::or`), 타이머, `Async<T>` I/O 래핑.
- **lru**: 행 캐시(`LineEntry`)와 이미지 데이터 캐시(`IMAGE_LRU`). 행 캐시 용량은 `scrollback_lines.max(128)`(`renderable.rs:108`).
- **ratelim**: 행 프리페치 폭주 방지(`ratelimit_mux_line_prefetches_per_second` 설정 연동, `clientpane.rs:74`).
- **metrics**: RPC 메서드별 지연 히스토그램·호출 카운터(`client.rs:82`).
- **textwrap**: 붙여넣기 예측 시 줄바꿈 계산(`renderable.rs:275`).
- **parking_lot**: `ClientPane`의 다수 Mutex 및 `MappedMutexGuard`(writer 노출, `clientpane.rs:360`).

---

## 7. 설정·기능 플래그

`Cargo.toml`에 자체 `[features]`는 없다. 의존성 측 feature만 활성화한다: `portable-pty`의 `serde_support`, `wezterm-term`의 `use_serde`(`wezterm-client/Cargo.toml:26`, `37`).

관련 config 항목(전부 도메인별로 `UnixDomain`/`TlsDomainClient`/`SshDomain`에 존재):

| 항목 | 위치 | 클라이언트에서의 효과 |
|---|---|---|
| `connect_automatically` | `config/src/{unix,tls,ssh}.rs` | GUI 시작 시 자동 attach 여부(`domain.rs:415`) |
| `local_echo_threshold_ms` | 동일 | 이 RTT(ms) 이상일 때만 예측 에코 활성(`renderable.rs:138`) |
| `overlay_lag_indicator` | 동일 | 지연 시 상단 행에 "⏳since last response" 오버레이(`renderable.rs:764`) |
| `no_serve_automatically` | `config/src/unix.rs:26` | 접속 실패 시 서버 자동 spawn 금지(`client.rs:721`) |
| `ratelimit_mux_line_prefetches_per_second` | `config/src/config.rs:392` | 행 프리페치 스로틀 한도 |
| `read_timeout`/`write_timeout` | 도메인별 | 소켓·TCP 타임아웃(`client.rs:767`, `980`) |
| `scrollback_lines` | `config` | 행 LRU 캐시 용량 산정 |
| `remote_wezterm_path`/`override_proxy_command` | ssh/tls | 원격 wezterm 바이너리 경로·프록시 명령 |

런타임 환경변수: `WEZTERM_UNIX_SOCKET`(소켓 경로 강제, `client.rs:1190`), `WEZTERM_PANE`(pane id 해석, `client.rs:1285`).

---

## 8. Windows 전용 고려사항

- **discovery는 전적으로 Windows API 기반이다.** `discovery.rs:19`의 `mod windows`는 `CreateFileMappingW`/`MapViewOfFile`/`OpenFileMappingW`(공유 메모리)와 `CreateMutexW`/`WaitForSingleObject`/`ReleaseMutex`(명명 뮤텍스)를 직접 호출한다. `Local\\wezterm-sock-*` 네임스페이스를 사용해 **데스크톱 단위**로 GUI 소켓 경로를 발행/해석한다. 이는 Windows에서 심볼릭 링크가 까다롭다는 판단에 따른 구현이다(주석 `discovery.rs:6`~`18`). `winapi`는 `cfg(windows)` 의존성으로만 선언됨(`Cargo.toml:40`).
- **소켓 타입은 Windows 네이티브다.** `client.rs:26`에서 `std::os::windows::io::{AsRawSocket, AsSocket, BorrowedSocket, RawSocket}`를 import하고, `SshStream`이 이를 구현한다(`client.rs:548`~). `unix_connect_with_retry`의 `UnixTarget::Proxy` 분기는 `FromRawSocket`/`IntoRawSocket`로 socketpair를 `UnixStream`으로 변환한다(`client.rs:497`~). 즉 "Unix domain"이라는 이름과 달리 실제 핸들은 Windows 소켓이며, 추상화는 `wezterm-uds`가 담당한다.
- **죽은 코드 / 잔존 분기:** 본 크레이트 소스에는 `cfg(unix)`/`cfg(target_os)` 분기가 남아 있지 않다. 다만 `process::Command::spawn`을 사용하는 서버 자동 기동(`client.rs:739`) 및 SSH 프록시 실행(`client.rs:459`)은 플랫폼 중립 API다. `UnixDomain`/`UnixTarget`이라는 명명은 upstream에서 유래한 잔재이나 현재 Windows에서도 유효 경로다.
- **시간 단위 폴백:** `discover_gui_socks`는 `created` 미지원 파일시스템을 위해 `modified` 폴백을 둔다(`discovery.rs:281`).

---

## 9. 리팩토링 주의점

- **`mux`와의 강결합.** `Client`/`ClientDomain`/`ClientPane` 모두 `Mux::get()`/`Mux::try_get()` 전역 싱글턴에 의존하며, 다수 경로가 메인 스레드 태스크(`spawn_into_main_thread`)에 의존한다. mux API 시그니처 변경은 광범위한 파급을 부른다.
- **`process_pane_list`(domain.rs:504)의 복잡도.** mark-and-sweep + 3종 ID 매핑 + tab 재생성 + 워크스페이스 일치 검사가 한 함수에 응축돼 있다. 매핑 불변식(§5)을 깨면 화면 미러링이 조용히 어긋나며, 증상은 "resync 무한 반복" 또는 "pane not found"로 나타난다. 분해 시 sweep 단계와 window 배치 로직을 우선 분리할 것.
- **`LineEntry` 상태기계의 미묘함.** `put_line`/`apply_lines`/`make_stale`/`get_lines`가 `Instant` 동일성으로 페치 경합을 조정한다(`renderable.rs:457`, `533`, `437`, `727`). `Instant` 비교 의미를 바꾸면 갱신 손실 또는 무한 페치가 발생한다.
- **예측 에코의 휴리스틱.** `apply_prediction`은 `"sword"` 포함 행을 비밀번호 프롬프트로 간주해 에코를 억제하는 등(`renderable.rs:159`) 임시방편적 규칙을 담는다. 주석(`renderable.rs:144`~)이 미해결 엣지케이스를 명시한다.
- **스레드 경계.** reader 스레드는 `block_on`으로 도는 별도 OS 스레드이고, 응답 처리·RPC 디스패치는 promise 런타임 위에서 돈다. `Client`는 `Clone`이라 여러 호출자가 동일 채널을 공유한다. 동시성 모델 변경 시 promise serial 매칭(`client.rs:382`~)을 깨지 않도록 주의.
- **순환 의존 없음(크레이트 단위).** deps/usedBy 목록상 순환은 없다. 단, `domain`↔`pane`↔`client`는 crate 내부에서 상호 참조한다(`ClientPane`→`ClientInner`→`Client`, `ClientDomain`→`ClientPane`).
- **미구현·FIXME.** `Domain::spawn_pane`은 `bail!`로 미구현(`domain.rs:760`). `is_alt_screen_active`는 항상 `false` 반환(서버에서 미조회, `clientpane.rs:548`). unilateral 구독 협상 미구현 FIXME(`client.rs:196`). `key_up` 처리 미정(`clientpane.rs:479`).
- **TLS 자격증명을 디스크에 평문 저장.** `tls_creds_*_path`에 PEM을 파일로 쓴다(`client.rs:887`~). 주석이 "이상적으로는 메모리 유지"라 명시(`client.rs:884`). 보안 리팩토링 후보.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `src/lib.rs` | 4 | 모듈 선언만(`client`/`discovery`/`domain`/`pane`) |
| `src/client.rs` | 1359 | 연결 수립(Unix/TLS/SSH)·재접속·reader 스레드·RPC 매크로·`Client`·`Reconnectable`·unilateral 분기. 가장 큰 단일 책임 집합 |
| `src/domain.rs` | 1008 | `ClientDomain`(`mux::Domain` 구현)·`ClientInner`(ID 매핑)·`ClientDomainConfig`·`process_pane_list`(동기화 GC)·mux 이벤트 역전파 |
| `src/discovery.rs` | 321 | Windows 공유 메모리/명명 뮤텍스 기반 GUI 소켓 발행·해석·스캔 |
| `src/pane/mod.rs` | 5 | `pane` 서브모듈 선언 및 `ClientPane` 재노출 |
| `src/pane/clientpane.rs` | 662 | `ClientPane`(`mux::Pane` 구현)·`process_unilateral`·`PaneWriter`·입력/제어 RPC 위임·팔레트 추종 |
| `src/pane/renderable.rs` | 859 | 행 LRU 캐시·`LineEntry` 상태기계·폴링/페치·예측 에코·`hydrate_lines`(이미지 셀 보충)·지연 인디케이터 |
| `src/pane/mousestate.rs` | 106 | 마우스 이벤트 큐 합치기(coalesce)·단일 in-flight 직렬 전송 |

> 생성 파일: 없음. 본 크레이트에는 생성된 거대 데이터 테이블이 존재하지 않는다.
