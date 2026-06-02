# WezTerm (Windows 전용 Fork) — 기능 명세 문서

이 디렉터리는 워크스페이스의 **폴더별 기능 명세**를 담는다. 추후 리팩토링을 위해
각 폴더(크레이트)의 책임·공개 API·내부 구조·의존 관계·Windows 전용 고려사항·
리팩토링 주의점을 상세히 기술한다.

- 전체 아키텍처·의존성 계층·데이터 플로우는 **[ARCHITECTURE.md](./ARCHITECTURE.md)** 참조.
- 아래 인덱스는 폴더를 역할별로 분류한 것이다. 각 항목은 해당 폴더의 명세 문서로 연결된다.

---

## 1. 실행 바이너리 · 진입점

| 폴더 | 크레이트 | 역할 |
|---|---|---|
| [wezterm](./wezterm.md) | `wezterm` | CLI 프런트엔드 및 서브커맨드 |
| [wezterm-gui](./wezterm-gui.md) | `wezterm-gui` | GPU 기반 GUI 터미널 본체(조립 지점) |
| [wezterm-mux-server](./wezterm-mux-server.md) | `wezterm-mux-server` | 헤드리스 멀티플렉서 서버 데몬 |
| [wezterm-gui-subcommands](./wezterm-gui-subcommands.md) | `wezterm-gui-subcommands` | GUI 관련 CLI 서브커맨드 정의 |
| [env-bootstrap](./env-bootstrap.md) | `env-bootstrap` | 환경 초기화 및 Lua API 모듈 등록 허브 |

## 2. 터미널 코어 — 파싱 · 모델

| 폴더 | 크레이트 | 역할 |
|---|---|---|
| [vtparse](./vtparse.md) | `vtparse` | 저수준 escape 시퀀스 상태 기계 |
| [wezterm-escape-parser](./wezterm-escape-parser.md) | `wezterm-escape-parser` | VT/escape 시퀀스 → 액션 파싱 |
| [term](./term.md) | `wezterm-term` | 가상 터미널 모델(화면 상태 적용) |
| [wezterm-cell](./wezterm-cell.md) | `wezterm-cell` | 터미널 셀(Cell) 모델 |
| [wezterm-surface](./wezterm-surface.md) | `wezterm-surface` | Surface·Line 타입 및 diff |
| [termwiz](./termwiz.md) | `termwiz` | 터미널 처리 라이브러리(입력·표면·렌더 프리미티브) |
| [wezterm-char-props](./wezterm-char-props.md) | `wezterm-char-props` | 유니코드/문자 속성(생성 테이블) |
| [bidi](./bidi.md) | `wezterm-bidi` | 유니코드 BiDi 알고리즘(UBA) |

## 3. 렌더링 · 폰트 · 윈도우

| 폴더 | 크레이트 | 역할 |
|---|---|---|
| [wezterm-font](./wezterm-font.md) | `wezterm-font` | 폰트 해석 → 셰이핑(harfbuzz) → 래스터화(freetype) |
| [window](./window.md) | `window` | OS 윈도우 + wgpu GPU 표면 |
| [color-types](./color-types.md) | `wezterm-color-types` | 색상 타입 |

## 4. 멀티플렉서 · 원격(클라이언트/서버)

| 폴더 | 크레이트 | 역할 |
|---|---|---|
| [mux](./mux.md) | `mux` | Pane/Tab/Window/Domain 멀티플렉서 |
| [wezterm-client](./wezterm-client.md) | `wezterm-client` | 원격 mux 클라이언트 |
| [wezterm-mux-server-impl](./wezterm-mux-server-impl.md) | `wezterm-mux-server-impl` | mux 서버 구현(프로토콜 핸들링) |
| [codec](./codec.md) | `codec` | 클라이언트-서버 직렬화 프로토콜 |
| [wezterm-ssh](./wezterm-ssh.md) | `wezterm-ssh` | libssh2 래퍼 |
| [wezterm-uds](./wezterm-uds.md) | `wezterm-uds` | 도메인 소켓 추상화 |
| [async_ossl](./async_ossl.md) | `async_ossl` | 비동기 OpenSSL 어댑터 |
| [ratelim](./ratelim.md) | `ratelim` | 속도 제한 유틸 |

## 5. 설정 · Lua 스크립팅

| 폴더 | 크레이트 | 역할 |
|---|---|---|
| [config](./config.md) | `config` (+`config/derive`) | 전역 설정 스키마·기본값·Lua 바인딩(최대 허브) |
| [wezterm-dynamic](./wezterm-dynamic.md) | `wezterm-dynamic` (+derive) | 동적 값/직렬화 기반 |
| [luahelper](./luahelper.md) | `luahelper` | Lua ↔ Rust 변환 헬퍼 |
| [lua-api-crates](./lua-api-crates.md) | 13개 서브크레이트 | 개별 Lua API 모듈 모음 |
| [wezterm-input-types](./wezterm-input-types.md) | `wezterm-input-types` | 입력 이벤트/키 타입 |

## 6. 지원 · 유틸리티 크레이트

| 폴더 | 크레이트 | 역할 |
|---|---|---|
| [pty](./pty.md) | `portable-pty` | PTY 추상화·자식 프로세스 spawn |
| [promise](./promise.md) | `promise` | 비동기 Promise/Future 프리미티브 |
| [filedescriptor](./filedescriptor.md) | `filedescriptor` | RawHandle/RawFd 래퍼 |
| [procinfo](./procinfo.md) | `procinfo` | 프로세스 정보 조회 |
| [rangeset](./rangeset.md) | `rangeset` | 범위 집합 자료구조 |
| [bintree](./bintree.md) | `bintree` | 이진 트리(분할 레이아웃용) |
| [lfucache](./lfucache.md) | `lfucache` | LFU 캐시 |
| [frecency](./frecency.md) | `frecency` | frecency 스코어링 |
| [tabout](./tabout.md) | `tabout` | CLI 출력 표 정렬 |
| [umask](./umask.md) | `umask` | umask RAII 가드(Windows에선 no-op 스텁) |
| [wezterm-blob-leases](./wezterm-blob-leases.md) | `wezterm-blob-leases` | 이미지 blob 캐싱/리스 |
| [wezterm-open-url](./wezterm-open-url.md) | `wezterm-open-url` | URL 열기 |
| [wezterm-toast-notification](./wezterm-toast-notification.md) | `wezterm-toast-notification` | 토스트 알림 |
| [wezterm-version](./wezterm-version.md) | `wezterm-version` | 버전 문자열 제공 |

## 7. 인프라 · 리소스

| 폴더 | 역할 |
|---|---|
| [deps](./deps.md) | 벤더링된 FFI 바인딩(cairo-sys-rs, fontconfig, freetype, harfbuzz) |
| [assets](./assets.md) | 번들 리소스(아이콘, 셸 통합, 폰트, DLL) |
| [ci](./ci.md) | Windows 인스톨러/패키징 스크립트 |
| [licenses](./licenses.md) | 서드파티 라이선스 텍스트 |

---

> 문서 작성 규약: 한국어 격식체, 코드 인용은 `파일경로:라인`. 생성된 데이터 테이블
> (`wezterm-gui/src/unicode_names.rs`, `wezterm-char-props/src/*` 등)은 내용을 나열하지
> 않고 생성 목적·소비처만 기술한다.
