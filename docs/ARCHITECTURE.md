# WezTerm (Windows 전용 Fork) — 아키텍처 개요

> 이 문서는 워크스페이스 전체 구조와 크레이트 간 관계를 다룬다. 개별 폴더의 상세
> 기능 명세는 [`docs/README.md`](./README.md)의 인덱스에서 각 문서로 연결된다.

---

## 1. 프로젝트 성격

- **Windows 전용 영구 분기.** upstream(wez/wezterm) 동기화를 포기하고 Windows
  플랫폼에만 집중하는 fork다. X11·Wayland·macOS 등 비(非)Windows 플랫폼 코드는
  소스에서 제거되었다. 따라서 본 워크스페이스의 `cfg(unix)` 계열 분기는 대부분
  죽은 경로이거나 컴파일 대상에서 빠진다.
- **설정 철학: 소스 기본값 수정.** 사용자(`wezterm.lua`) 설정 대신 `config`
  크레이트의 기본값을 직접 수정해 동작을 고정하는 방식을 우선한다. 즉 Lua 설정
  계층은 유지되지만, 본 fork의 "기본 동작"은 소스 코드의 기본값이 정의한다.
- **빌드 툴체인.** 소스 빌드에는 **Strawberry Perl + MSVC `vcvars`** 환경이
  필요하다(openssl을 vendored로 빌드하기 때문). 자세한 절차는 메모리
  `wezterm-windows-build-toolchain` 참조.

---

## 2. 워크스페이스 구성

워크스페이스는 **63개 크레이트**로 이루어진다(파생 `*-derive`, 벤더 바인딩,
`lua-api-crates/*` 서브크레이트 포함). 디렉터리 기준으로는 약 47개 최상위 폴더가
있으며, 본 문서 집합은 폴더 단위로 명세를 분할한다.

### 2.1 실행 바이너리 (3종)

| 바이너리 | 크레이트 | 폴더 | 역할 |
|---|---|---|---|
| `wezterm` | `wezterm` | `wezterm/` | CLI 프런트엔드(서브커맨드: cli, ssh, serial, connect 등) |
| `wezterm-gui` | `wezterm-gui` | `wezterm-gui/` | GPU 기반 GUI 터미널 본체 |
| `wezterm-mux-server` | `wezterm-mux-server` | `wezterm-mux-server/` | 헤드리스 멀티플렉서 서버(데몬) |

### 2.2 규모(코드 라인) 상위 폴더

> **주의:** 일부 폴더의 라인 수는 **생성된 데이터 테이블**이 지배한다. 리팩토링
> 시 이 파일들은 수기 코드가 아니므로 별도 취급해야 한다.

| 폴더 | 총 .rs 행 | 비고(생성 파일) |
|---|---:|---|
| `wezterm-gui` | ~177,000 | 이 중 `src/unicode_names.rs` **140,379행**은 생성된 유니코드 이름 테이블. 실제 수기 코드는 ~37,000행 |
| `wezterm-char-props` | ~79,000 | `emoji_variation.rs`(66,406), `nerdfonts_data.rs`(10,757), `widechar_width.rs`(1,689)가 생성 데이터. 수기 코드는 수백 행 |
| `mux` | ~11,500 | 멀티플렉서 핵심 |
| `wezterm-escape-parser` | ~10,800 | VT/escape 시퀀스 파서 |
| `term` (`wezterm-term`) | ~9,900 | 가상 터미널 모델 |
| `termwiz` | ~9,800 | 터미널 처리 라이브러리 |
| `config` | ~9,600 | 설정 스키마·Lua 바인딩 |
| `wezterm-font` | ~9,100 | 폰트 해석·셰이핑·래스터화 |
| `window` | ~6,500 | OS 윈도우 + GPU 표면 |
| `wezterm-ssh` | ~6,100 | SSH 클라이언트 래퍼 |

---

## 3. 의존성 계층 (Layering)

크레이트의 계층은 "리프(외부 크레이트만 의존)까지의 최장 경로"로 계산했다.
L0가 기반, L14가 최상위 바이너리다. 화살표는 위 계층이 아래 계층을 의존함을 뜻한다.

```
L14  wezterm-gui · wezterm-mux-server          ← 최상위 바이너리/GUI
L13  wezterm · wezterm-mux-server-impl
L12  env-bootstrap · wezterm-client
L11  codec · mux-lua · window-funcs
L10  mux · window
L9   wezterm-font · 다수 lua-api-crates · lfucache · ratelim · wezterm-gui-subcommands
L8   config                                    ← 최대 허브(28개가 의존)
L7   tabout · wezterm-ssh
L6   termwiz · wezterm-term                     ← 터미널 처리/모델 핵심
L5   wezterm-surface
L4   wezterm-cell
L3   procinfo · wezterm-escape-parser
L2   luahelper · wezterm-bidi · wezterm-color-types · wezterm-input-types
L1   harfbuzz · portable-pty · wezterm-char-props · wezterm-dynamic · wezterm-toast-notification
L0   async_ossl · bintree · filedescriptor · frecency · promise · rangeset · umask ·
     vtparse · wezterm-blob-leases · wezterm-uds · wezterm-version · *-derive · 벤더 바인딩
```

### 3.1 허브 크레이트(피의존도 = fan-in)

리팩토링 시 **변경 파급이 가장 큰** 크레이트들이다. 공개 API 변경은 아래 숫자만큼의
크레이트를 직접 깨뜨릴 수 있다.

| 크레이트 | 직접 의존하는 크레이트 수 | 성격 |
|---|---:|---|
| `config` | 28 | 전역 설정·Lua 스키마. 사실상 모든 상위 크레이트가 의존 |
| `wezterm-dynamic` | 21 | 동적 값/직렬화 기반 타입 |
| `termwiz` | 14 | 터미널 처리 라이브러리 |
| `luahelper` | 12 | Lua ↔ Rust 변환 헬퍼 |
| `wezterm-term` | 11 | 가상 터미널 모델 |
| `portable-pty` | 10 | PTY 추상화 |
| `promise` | 9 | 비동기 프리미티브 |
| `wezterm-bidi` · `mux` | 7 | BiDi 알고리즘 / 멀티플렉서 |

### 3.2 거대 의존자(의존도 = fan-out)

| 크레이트 | 직접 의존 수 | 비고 |
|---|---:|---|
| `wezterm-gui` | 29 | 사실상 워크스페이스의 "조립 지점". 결합도가 가장 높음 |
| `env-bootstrap` | 17 | 모든 Lua API 크레이트를 등록하는 부트스트랩 허브 |
| `wezterm` · `wezterm-client` | 14 | CLI / 원격 mux 클라이언트 |
| `mux` | 13 | 멀티플렉서 |

---

## 4. 핵심 데이터 플로우 (터미널 파이프라인)

```
[자식 프로세스]
     │  bytes (stdout/stderr)
     ▼
portable-pty (pty/)              ── 자식 프로세스 spawn + PTY I/O
     │
     ▼
vtparse → wezterm-escape-parser  ── 바이트 → VT/escape 액션 파싱
     │
     ▼
wezterm-term (term/)             ── 액션을 화면 모델에 적용
     │   └ wezterm-cell, wezterm-surface : Cell·Line·Surface 데이터 모델
     ▼
mux (mux/)                       ── Pane/Tab/Window/Domain 추상화, 로컬·원격
     │
     ├──(원격)── codec ↔ wezterm-client ↔ wezterm-mux-server(-impl)  : 클라이언트-서버 프로토콜
     │
     ▼
wezterm-gui (wezterm-gui/)       ── 렌더 루프
     │   ├ wezterm-font : 폰트 해석 → harfbuzz 셰이핑 → freetype 래스터화
     │   ├ window       : OS 윈도우 + wgpu GPU 표면
     │   ├ glyphcache/renderstate : 글리프 아틀라스 → GPU 정점/텍스처
     │   └ lua-api-crates + luahelper + config : Lua 설정/스크립팅 UI
     ▼
[화면 출력 + 입력 이벤트 역방향 전파]
```

설정/스크립팅 계층은 파이프라인과 직교한다: `config`가 스키마와 기본값을 정의하고,
`wezterm-dynamic`이 직렬화 기반을, `lua-api-crates/*`가 개별 Lua API 모듈을,
`env-bootstrap`이 이들의 등록을 담당한다.

---

## 5. 리팩토링 시 횡단 관심사(Cross-cutting)

1. **생성 데이터 테이블 분리.** `wezterm-gui/src/unicode_names.rs`(140K행),
   `wezterm-char-props/src/{emoji_variation,nerdfonts_data,widechar_width}.rs`는
   생성물이다. 빌드 스크립트/생성기와 결과 테이블을 명확히 구분하고, 코드 메트릭·
   리뷰 대상에서 제외해야 한다.
2. **`wezterm-gui` 결합도.** fan-out 29로 워크스페이스에서 가장 강하게 결합된
   크레이트다. `termwindow/` 모듈(특히 `mod.rs` 3,617행)이 비대하다. 책임 분리
   리팩토링의 1순위 후보다.
3. **`config` 허브성.** 28개 크레이트가 의존한다. 설정 스키마 변경은 광범위한
   재컴파일·API 파급을 일으킨다. 변경은 하위 호환을 우선 고려해야 한다.
4. **`wezterm-char-props ↔ termwiz` 순환.** `wezterm-char-props`가 `termwiz`를
   참조하는 것은 **벤치마크용 dev-dependency**(`wcwidth` 벤치)로 보인다. 런타임
   순환이 아니므로 일반 의존성으로 격상하지 않도록 주의한다.
5. **Windows 전용 가정.** 남아 있는 `cfg(unix)`/`cfg(target_os=...)` 분기는
   대부분 죽은 경로다. 정리 시 Windows 경로만 살아 있음을 전제로 검증한다.

---

## 6. 문서 인덱스

폴더별 상세 명세는 [`docs/README.md`](./README.md)를 참조한다.
