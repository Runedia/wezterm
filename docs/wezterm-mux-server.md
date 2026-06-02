# wezterm-mux-server 폴더 기능 명세

## 1. 개요 및 책임

`wezterm-mux-server`는 WezTerm 멀티플렉서(mux) 서버의 **독립 실행 바이너리**다. GUI 없이 헤드리스로 동작하는 터미널 다중화 데몬이며, 단일 책임은 다음과 같다.

- 프로세스 부팅(`env_bootstrap`), 설정 로드, mux 상태(`Mux`) 초기화.
- 로컬 도메인(`LocalDomain`)에 초기 셸/프로그램을 띄우고, 클라이언트가 접속해 attach 할 수 있도록 **리스너를 기동**한다.
- 두 종류의 리스너를 연다: 유닉스 도메인 소켓 기반 로컬 리스너(`unix_domains`)와 TLS 리스너(`tls_servers`).
- 데몬화(`--daemonize`) 요청 시 자신을 백그라운드 프로세스로 재기동한다.

실제 프로토콜 처리·세션 핸들링·PKI는 본 크레이트에 없다. 그것은 `wezterm-mux-server-impl`에 위임된다. 본 크레이트는 **얇은 진입점(entry point)**으로, 부팅 시퀀스 조립과 리스너 스폰만 담당한다(`wezterm-mux-server/src/main.rs:60`, `wezterm-mux-server/src/main.rs:254`).

### Windows fork에서의 실제 책임

이 저장소는 WezTerm의 Windows 전용 영구 분기다. 그 결과 본 크레이트의 동작에는 다음 특이점이 있다.

- **데몬화**: Windows에는 `fork()`가 없으므로 진짜 데몬화가 불가능하다. 대신 `std::process::Command`로 자기 자신을 `DETACHED_PROCESS` 플래그와 함께 재기동한다(`wezterm-mux-server/src/main.rs:91`~`130`).
- **유닉스 도메인 소켓**: 코드 경로는 `unix_domains`를 그대로 순회하나(`wezterm-mux-server/src/main.rs:256`), 이는 `wezterm-mux-server-impl::local::LocalListener`에 위임되며 Windows에서는 named pipe 기반 구현으로 매핑된다(소켓 추상화는 impl 크레이트 책임).
- **TLS 피어 인증**: `ossl.rs`의 인증 로직은 `$USER` 환경변수와 인증서 CN을 비교한다(`wezterm-mux-server/src/ossl.rs:46`). Windows에서 `USER`는 표준 변수가 아니므로(`USERNAME`이 표준) 이 경로는 환경 설정에 의존하는 **사실상 잠재적 죽은 경로**다.

---

## 2. 워크스페이스 내 위치

### 의존성(deps)

| 크레이트 | 역할 |
|---|---|
| `async_ossl` | OpenSSL `SslStream`을 async I/O로 감싸는 어댑터(`AsyncSslStream`). TLS 리스너에서 사용. |
| `config` | 설정 로드·핸들(`ConfigHandle`), `TlsDomainServer`·`DaemonOptions` 등 설정 타입, `common_init`/`configuration`. |
| `env-bootstrap` | 프로세스 부팅(`bootstrap()`): 로깅·환경·패닉 훅 등 전역 초기화. |
| `mux` | 멀티플렉서 코어. `Mux`, `Domain`/`LocalDomain`, `Activity`. |
| `portable-pty` | `CommandBuilder`로 초기 프로그램 명령 구성. |
| `promise` | `SimpleExecutor`/`spawn` 기반 비동기 런타임(GUI 없는 단일 스레드 이벤트 루프). |
| `umask` | `UmaskSaver`로 umask 저장·복원(유닉스 잔재). |
| `wezterm-blob-leases` | blob lease 스토리지 등록·해제(임시 디렉터리 기반). |
| `wezterm-gui-subcommands` | CLI 인자 파서 헬퍼(`name_equals_value`) 및 공용 서브커맨드 정의. |
| `wezterm-mux-server-impl` | **핵심 위임 대상**. 리스너 구현(`local`), 세션 디스패치(`dispatch`), PKI, 도메인 갱신. |
| `wezterm-term` | 터미널 모델(간접 의존, 초기 size 등). |
| `anyhow`, `clap`, `libc`, `log`, `mlua`, `openssl`, `winapi` | 오류, CLI, FFI, 로깅, Lua 이벤트, TLS, Windows API. |

### 피의존(usedBy)

| 사용처 | 비고 |
|---|---|
| 없음 | 최상위 실행 바이너리. 다른 크레이트가 라이브러리로 의존하지 않는다. |

### 계층상 위치

본 크레이트는 **mux 파이프라인의 서버 측 최상위 진입점**이다. `config`/`mux`/`promise` 같은 코어 위에 앉아 부팅을 조립하고, 프로토콜·세션 처리 일체를 하위 라이브러리 `wezterm-mux-server-impl`에 위임한다. GUI(`wezterm-gui`)와 동급의 별도 실행 바이너리이며, GUI가 클라이언트로 이 서버에 접속하는 구조다.

---

## 3. 공개 API 표면

본 크레이트는 바이너리이므로 외부 라이브러리 소비를 위한 공개 표면은 사실상 없다. 바이너리 내부에서 의미를 갖는 `pub` 항목은 다음 하나뿐이다.

- `pub fn spawn_listener() -> anyhow::Result<()>` (`wezterm-mux-server/src/main.rs:254`)
  - 설정의 `unix_domains` 각각에 대해 `LocalListener`를 만들어 전용 스레드에서 `run()` 하고, `tls_servers` 각각에 대해 `ossl::spawn_tls_listener`를 호출한다. `pub`이지만 외부 크레이트에서 호출되지 않으며 같은 바이너리 `run()`에서만 쓰인다.

`ossl` 모듈 내 공개 항목:

- `pub fn spawn_tls_listener(tls_server: &TlsDomainServer) -> Result<(), Error>` (`wezterm-mux-server/src/ossl.rs:112`)
  - `SslAcceptor::mozilla_modern`으로 acceptor를 구성하고 인증서/키/루트 CA를 적재한 뒤, `TcpListener`를 바인딩하고 수락 루프 스레드를 스폰한다. 모듈은 `main.rs:252`에서 `mod ossl;`로만 선언되어 바이너리 내부에서만 가시.

CLI 표면(외부 사용자 대상 인터페이스):

- `struct Opt` (clap `Parser`, `wezterm-mux-server/src/main.rs:21`): `--skip-config(-n)`, `--config-file`, `--config(name=value)`, `--daemonize`, `--cwd`, 그리고 trailing `prog`(실행할 프로그램).

---

## 4. 내부 구조

모듈 구성은 단순하다(`main.rs`, `ossl.rs` 2개).

**제어 흐름(`main.rs`)**:

1. `main()` (`:60`): `run()` 호출. 실패 시 `wezterm_blob_leases::clear_storage()` 후 `exit(1)`. 성공 시에도 종료 전 스토리지 정리.
2. `run()` (`:69`):
   - `env_bootstrap::bootstrap()` → `config::designate_this_as_the_main_thread()` → `UmaskSaver` 설치.
   - `Opt::parse()` → `config::common_init(...)` 로 설정 로드.
   - `update_ulimit()`, `default_ssh_auth_sock` 처리.
   - `--daemonize`면 자기 자신을 인자 그대로 재구성해 `DETACHED_PROCESS`로 스폰하고 즉시 반환(`:91`~`130`).
   - 방해되는 환경변수 제거(`OLDPWD`, `PWD`, `SHLVL`, `WEZTERM_PANE`, `WEZTERM_UNIX_SOCKET`, `_` 및 `mux_env_remove`)(`:137`~`149`).
   - blob lease 스토리지 등록(`SimpleTempDir`)(`:151`).
   - `prog`/`cwd` 유무로 `CommandBuilder` 구성(`:155`~`169`).
   - `LocalDomain::new("local")` → `Mux::new` → `Mux::set_mux`(`:171`~`173`).
   - `SimpleExecutor` 생성, `spawn_listener()` 호출(`:175`~`180`).
   - `Activity` 생성 후 `async_run(cmd)`를 스폰(`detach`), 이후 `executor.tick()` 무한 루프(`:182`~`194`).
3. `async_run()` (`:205`): `update_mux_domains_for_server(&config)`로 도메인 동기화, 설정 reload 구독 등록, `mux-startup` Lua 이벤트 emit, 해당 도메인에 패널이 없으면 빈 윈도우 생성 후 `attach` + 초기 `spawn`.
4. `trigger_mux_startup()` (`:197`): Lua가 있으면 `mux-startup` 이벤트를 emit.
5. `terminate_with_error()` (`:247`): 로그 후 `exit(1)`하는 `!` 반환 헬퍼.

**제어 흐름(`ossl.rs`)**:

- `spawn_tls_listener()` (`:112`): acceptor 구성 → `TcpListener::bind` → `OpenSSLNetListener::run`을 스레드 스폰.
- `OpenSSLNetListener::run()` (`:72`): `incoming()` 수락 루프. 각 연결마다 `accept` → `verify_peer_cert` → `AsyncSslStream`으로 감싸 `dispatch::process`를 메인 스레드로 스폰.
- `verify_peer_cert()` (`:34`): 피어 인증서 CN을 `$USER` 또는 `user:<name>/` 접두사와 대조.

비대한 모듈은 없다(`main.rs` 270행, `ossl.rs` 189행).

---

## 5. 핵심 데이터 구조·타입

- **`struct Opt`** (`main.rs:21`): clap CLI 인자 컨테이너. `skip_config`와 `config_file`는 상호 배타(`conflicts_with`). `prog`는 `num_args=1..`로 trailing var arg.
- **`struct OpenSSLNetListener`** (`ossl.rs:12`): `{ acceptor: Arc<SslAcceptor>, listener: TcpListener }`.
  - 불변식: `acceptor`는 `set_verify(PEER | FAIL_IF_NO_PEER_CERT)`로 구성되어, 수락된 연결은 항상 피어 인증서를 제출해야 한다. 추가로 `verify_peer_cert`가 CN을 검증해 통과한 연결만 `dispatch::process`로 진행한다.
  - 수락 루프는 `verify_peer_cert` 실패 시 `break`(루프 종료, 리스너 사망), `accept` 실패 시 로그 후 다음 연결로 진행, `incoming()` 오류 시 `return`(루프 종료). 즉 인증 실패가 리스너 전체를 멈추는 강한 결합이 존재한다.

본 크레이트 자체에는 enum이나 복합 상태 머신이 없다. 주요 상태(`Mux`, `Domain`)는 `mux` 크레이트 소유이며 본 크레이트는 그것을 조립·구동만 한다.

---

## 6. 외부 의존성

- **`openssl` / `async_ossl`**: TLS 서버 종단. `SslAcceptor::mozilla_modern`으로 modern cipher suite를 강제하고, `async_ossl::AsyncSslStream`으로 동기 `SslStream`을 async 디스패처에 연결.
- **`portable-pty`**: 초기 프로그램의 명령행(`CommandBuilder`)을 플랫폼 독립적으로 구성. `serde_support` feature 활성(`Cargo.toml:22`).
- **`promise`**: GUI가 없는 환경에서의 단일 스레드 비동기 실행기(`SimpleExecutor`). `executor.tick()` 루프가 메인 이벤트 펌프.
- **`mlua`**: `mux-startup` Lua 이벤트 발화. 설정 측의 Lua 컨텍스트와 연결.
- **`wezterm-blob-leases`** (`simple_tempdir` feature): 대용량 blob(이미지 등)을 임시 디렉터리 기반 lease로 관리. 시작 시 등록, 종료 시 `clear_storage`.
- **`winapi`** (`winuser` feature): 데몬화 시 `DETACHED_PROCESS` 등 Windows 프로세스 생성 플래그.
- **`libc`, `umask`**: 유닉스 계열 잔재(umask). Windows에서는 실질적 효과가 제한적.

---

## 7. 설정·기능 플래그

빌드 feature flag는 본 크레이트에 정의되지 않는다. 의존성 feature만 고정한다.

- `portable-pty` → `serde_support`
- `wezterm-blob-leases` → `simple_tempdir`
- `winapi` → `winuser`

관련 `config` 항목(설정 철학상 `config` 크레이트 기본값 수정으로 고정 권장):

| config 항목 | 용도 |
|---|---|
| `unix_domains` | 로컬 리스너(named pipe/소켓) 대상 도메인 목록(`main.rs:256`). |
| `tls_servers` (`TlsDomainServer`) | TLS 리스너 바인드 주소·인증서·키·CA 목록(`ossl.rs:112`). |
| `daemon_options` (`DaemonOptions`) | 데몬화 시 stdout/stderr 리다이렉트(`main.rs:122`~`123`). |
| `mux_env_remove` | 서버 시작 시 제거할 추가 환경변수(`main.rs:147`). |
| `default_ssh_auth_sock` | `SSH_AUTH_SOCK` 설정(`main.rs:87`). |
| `default_mux_server_domain` | 스탠드얼론 mux의 기본 도메인 지정(impl 측 `update_mux_domains_for_server`에서 적용). |

`TlsDomainServer`의 핵심 필드(`config/src/tls.rs:6`): `bind_address`, `pem_private_key`, `pem_cert`, `pem_ca`, `pem_root_certs`. 미지정 시 PKI 자동 생성 인증서(`PKI.server_pem()`, `PKI.ca_pem()`)로 대체된다.

---

## 8. Windows 전용 고려사항

- **데몬화 분기**: `main.rs:120`~`129` 블록은 `use std::os::windows::process::CommandExt`와 `creation_flags(DETACHED_PROCESS)`를 사용하는 **Windows 전용 경로**다. 주석(`:92`~`95`)은 유닉스의 fork/re-exec 차이를 설명하나, 실제 코드는 Windows 경로만 남아 있다. 유닉스 분기 코드는 제거되었다.
- **빌드 스크립트(`build.rs`)**: 전체가 `#[cfg(windows)]`로 감싸여 있다(`build.rs:4`). `assets/windows/console.manifest`를 `RT_MANIFEST`로 임베드하고, MSVC 레지스트리에서 `cl.exe` 툴 환경을 끌어와 rc 컴파일러가 헤더를 찾게 한다(`build.rs:31`~`36`). 비Windows에서는 `main()`이 사실상 no-op.
- **`$USER` 의존 (잠재적 죽은 경로)**: `ossl.rs:46`의 `std::env::var("USER")`는 유닉스 관례다. Windows의 표준 변수는 `USERNAME`이므로, `USER`가 별도로 설정되지 않은 환경에서는 `verify_peer_cert`가 즉시 오류를 반환하고 리스너가 `break`로 종료될 수 있다(`ossl.rs:81`~`84`). TLS 서버를 Windows에서 실사용하려면 이 부분의 사용자명 소스 재검토가 필요하다.
- **umask/libc**: `umask::UmaskSaver`와 `libc`는 유닉스 의미론이다. Windows에서는 실질 효과가 미미한 잔재.
- **유닉스 도메인 소켓**: `WEZTERM_UNIX_SOCKET` 환경변수와 `unix_domains` 처리는 유지되나(`main.rs:257`), 실제 전송은 impl 크레이트의 플랫폼별 구현(Windows named pipe)에 매핑된다.

---

## 9. 리팩토링 주의점

- **얇은 셸·강한 위임**: 본 크레이트는 부팅 조립과 리스너 스폰만 한다. 프로토콜·세션·PKI 변경은 모두 `wezterm-mux-server-impl`에서 이뤄져야 한다. 본 크레이트만 보고 동작을 판단하면 안 된다.
- **인증 실패의 파급(불변식 위험)**: `OpenSSLNetListener::run`은 `verify_peer_cert` 실패 시 `break`로 수락 루프 전체를 종료한다(`ossl.rs:82`~`84`). 한 클라이언트의 인증 실패가 TLS 리스너 전체를 죽이는 결함성 설계다. `continue`로 바꾸는 것이 합리적이나, 동작 변경이므로 의도 확인 필요.
- **로그 레벨 오용**: `ossl.rs:86`("Making new AsyncSslStream"), `ossl.rs:173`("listening with TLS on ...")는 정상 흐름인데 `log::error!`로 기록된다. 운영 로그 잡음의 원인. 정리 시 `info!`/`debug!`로 격하 검토.
- **`$USER` 결합**: 8절 참조. 인증의 사용자명 소스가 유닉스 관례에 묶여 있어 Windows 이식성이 깨진다.
- **무한 루프 이벤트 펌프**: `run()` 말미의 `loop { executor.tick()? }`(`main.rs:192`)는 정상 종료 경로가 없다. 종료는 `terminate_with_error`의 `exit(1)`이나 데몬화 분기의 조기 `return`으로만 발생. 그레이스풀 셧다운이 필요하면 신규 설계 필요.
- **데몬화 인자 재구성 누락 위험**: `--daemonize` 재기동 시 `Opt`의 일부 인자만 수동으로 재전달한다(`main.rs:98`~`118`). `Opt`에 새 인자를 추가하면 이 블록도 함께 갱신해야 한다(컴파일러가 잡아주지 않는 암묵 결합).
- **순환 의존**: 본 크레이트는 최상위 바이너리로 피의존이 없어 순환 위험은 낮다. 다만 `wezterm-mux-server-impl` 측 `update_mux_domains_for_server`가 `wezterm-client`/`mux`를 끌어오므로, 도메인 갱신 시그니처 변경은 GUI 측 동등 호출(`update_mux_domains`)과 함께 검토해야 한다.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `wezterm-mux-server/Cargo.toml` | 36 | 패키지·의존성 정의. Windows 타깃 전용 `winapi`/`embed-resource`/`cc` 분기. |
| `wezterm-mux-server/build.rs` | 38 | (Windows 전용) `console.manifest`를 RT_MANIFEST로 임베드, MSVC `cl.exe` 환경 주입. |
| `wezterm-mux-server/src/main.rs` | 270 | 진입점. 부팅·설정·mux 초기화, 데몬화, 환경 정리, 리스너 스폰(`spawn_listener`), 비동기 부트(`async_run`), 이벤트 펌프 루프. |
| `wezterm-mux-server/src/ossl.rs` | 189 | TLS 리스너. acceptor 구성·인증서 적재(`spawn_tls_listener`), 수락 루프와 피어 CN 인증(`OpenSSLNetListener`/`verify_peer_cert`), `dispatch::process`로 위임. |

생성 파일은 없다(데이터 테이블 생성물 없음). `build.rs`가 빌드 시 `OUT_DIR/resource.rc`를 생성하나 이는 빌드 산출물이며 소스 트리에 커밋되지 않는다.
