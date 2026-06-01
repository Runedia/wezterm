# 플랫폼 제거 명세서 — Windows 전용 전환

## 개요

Linux, macOS, FreeBSD, NetBSD 지원을 제거하고 **Windows 전용**으로 전환하는 작업 명세입니다.

> **분기 결정 (2026-06-01):** 이 fork(`Runedia/wezterm`)는 `upstream`(`wezterm/wezterm`)
> 동기화를 포기하고 Windows 전용으로 **영구 분기**합니다. 이 결정으로 upstream 머지는
> 대규모 충돌을 유발하므로 사실상 중단됩니다.

---

## ⚠️ 핵심 정정 — 작업의 실제 효과

확인된 게이팅:
- `window/src/os/x11/mod.rs:1`, `x_and_wayland.rs:1`, `xdg_desktop_portal.rs:1`,
  `xkeysyms.rs:1` → 모두 `#![cfg(all(unix, not(target_os = "macos")))]`
- `macos/` 모듈 → `#[cfg(target_os = "macos")]`
- `core_text.rs:1` → `#![cfg(target_os = "macos")]`
- 플랫폼 의존성 → `[target.'cfg(...)'.dependencies]` 섹션 (Cargo는 타겟 불일치 시 빌드 안 함)

| 기존 "예상 효과" 주장 | 실제 |
|---|---|
| 빌드 시간 향상 | **거짓.** cfg 제외 코드는 애초에 컴파일 안 됨 → Windows 빌드 시간 변화 없음 |
| ~20개 의존성 제거 | Windows 빌드에는 이미 미반영. `Cargo.lock` 해석/저장소 크기만 감소 |
| ~15,200 라인 삭제 | 사실이나 **Windows 바이너리에 기능적 변화 0** |
| 유지보수 단순화 | **유일하게 타당한 효과** (저장소 정리·명료성) |

**결론:** 이 작업은 Windows 빌드 산출물·성능에 영향이 없는 순수 "저장소 정리"입니다.

---

## ✅ 진행 상태 (2026-06-01 기준)

### 완료된 작업 (Phase 1–3, 부분)

**삭제된 파일/디렉토리:**

| 경로 | 상태 |
|------|------|
| `window/src/os/macos/` (7 파일) | ✅ 삭제 |
| `window/src/os/x11/` (8 파일) | ✅ 삭제 |
| `window/src/os/wayland/` (13 파일) | ✅ 삭제 |
| `window/src/os/x_and_wayland.rs` | ✅ 삭제 |
| `window/src/os/xdg_desktop_portal.rs` | ✅ 삭제 |
| `window/src/os/xkeysyms.rs` | ✅ 삭제 (X11 전용, `xcb` 의존 — Windows 유지 불가) |
| `wezterm-font/src/locator/core_text.rs` | ✅ 삭제 |
| `wezterm-font/src/locator/font_config.rs` | ✅ 삭제 (고아화되어 함께 제거) |
| `termwiz/src/terminal/unix.rs` | ✅ 삭제 |
| `filedescriptor/src/unix.rs` | ✅ 삭제 |

**수정된 파일:**

| 파일 | 변경 내용 |
|------|----------|
| `window/src/os/mod.rs` | Windows 전용으로 재작성, 삭제 모듈 선언 6개 제거 |
| `window/src/lib.rs` | `DEFAULT_DPI` macOS 분기 제거 (`96.0` 고정) |
| `window/Cargo.toml` | `cfg(unix,non-macos)`·`cfg(macos)` 의존성 섹션 + `wayland` feature 제거 |
| `wezterm-gui/Cargo.toml` | `wayland` feature 및 `default` 참조 제거 |
| `wezterm-font/src/locator/mod.rs` | `core_text`/`font_config` 모듈 선언·`new_locator` 분기 정리 |
| `termwiz/src/terminal/mod.rs` | `unix` 모듈 선언 제거 |
| `filedescriptor/src/lib.rs` | `unix` 모듈 선언 제거, `windows` 모듈 무조건화 |

**검증 결과 (둘 다 종료 코드 0):**

| 검증 명령 | 결과 |
|---|---|
| `cargo check -p filedescriptor -p termwiz -p wezterm-font -p window` | `Finished` (오류 0) |
| `cargo check -p wezterm-gui -p wezterm` | `Finished` (오류 0) |

기존 lifetime 경고 3건 외 신규 오류·경고 없음. Windows 빌드는 깨끗하게 컴파일됨.

### 추가 완료 작업 (D 인라인 cfg 정리 + C 고아 파일 삭제)

D 단계(인라인 조건부 컴파일 블록)와 그 부수효과인 C 단계(고아 소스 파일)를 완료했습니다.

**원칙:** Windows에서 거짓인 조건(`target_os="macos"`, `all(unix, not(macos))`, `unix`,
`linux/freebsd/netbsd`, `not(windows)`, `feature="wayland"`)으로 게이트된 항목은 삭제,
Windows에서 참인 조건(`windows`, `not(macos)`, `any(windows, …)`)은 속성만 제거하고 코드
유지. `cfg!()` 런타임 매크로는 활성 분기만 남기고 죽은 분기 제거.

**정리된 crate (cfg 블록 제거/단순화):**
`window`(egl.rs, spawn.rs), `env-bootstrap`, `config`(config.rs, lib.rs, daemon.rs,
ssh.rs, version.rs, font.rs, unix.rs, lua.rs, wsl.rs), `wezterm-gui`(commands.rs,
build.rs, main.rs, termwindow/{resize,palette,webgpu}.rs), `wezterm-open-url`,
`wezterm-client`(discovery.rs, client.rs), `wezterm-font`(lib.rs),
`wezterm-input-types`, `wezterm-toast-notification`(lib.rs, build.rs), `procinfo`(lib.rs),
`deps/cairo`(lib.rs), `pty`(lib.rs, cmdbuilder.rs, serial.rs, examples/), `mux`(lib.rs,
domain.rs, localpane.rs, ssh.rs, ssh_agent.rs, tmux_pty.rs), `umask`, `wezterm-uds`,
`async_ossl`, `termwiz`(istty.rs, terminal/mod.rs), `wezterm-ssh`(src/),
`wezterm-mux-server`(main.rs), `wezterm-mux-server-impl`(pki.rs, local.rs, dispatch.rs),
`wezterm`(main.rs, asciicast.rs).

**삭제된 고아 소스 파일:**
`procinfo/src/{macos.rs, linux.rs}`, `wezterm-toast-notification/src/{macos.rs, dbus.rs}`,
`pty/src/unix.rs`, `wezterm-mux-server/src/daemonize.rs`, `wezterm-font/src/fcwrap.rs`.

**삭제된 테스트 스위트:** `wezterm-ssh/tests/` 전체. e2e 하네스가 `/usr/sbin/sshd`를
하드코딩해 `sshd`/`ssh-keygen`/`ssh-agent`(OpenSSH unix 바이너리)를 spawn하므로 Windows에서
실행 원천 불가. (`#[ignore]`로 두는 것보다 삭제가 정확 — "ignore라서"가 아니라 "플랫폼적으로
실행 불가"가 사유.) 라이브러리 `wezterm-ssh/src/`는 유지·정상 빌드.

**검증 결과:**
- 전 소스에서 Windows-거짓 플랫폼 cfg(macos/unix-only/linux/bsd/wayland/not(windows)) **0건** (grep 확인)
- `cargo check --workspace --all-targets`: **exit 0** (전 타겟 — `wezterm-char-props`의 `wcwidth` 벤치 포함. 이 벤치는 `criterion`/`termwiz` dev-dependency 누락 + API 드리프트로 기존부터 깨져 있던 것을 함께 수정함)

**행동 변화 주의 (의도된 제거):**
- `wezterm-ssh` `connect_to_host`의 unix fd-passing(`proxyusefdpass`) 경로 제거 — 원래 unix 전용 기능, Windows엔 없었음
- `wezterm-mux-server`의 unix `daemonize`/pid-file 경로 제거 — Windows는 spawn/detach 방식
- `umask::UmaskSaver`는 Windows에서 no-op (umask 개념 없음)

**보존 확인:** `config/src/unix.rs`의 `UnixDomain`(Windows AF_UNIX/WSL용)은 유지. `mod unix;` 선언 존속.

### 추가 완료 작업 (B crate 의존성 섹션 + A 루트 workspace.dependencies)

**B — crate별 플랫폼 의존성 섹션 제거:** Windows-거짓 `[target.'cfg(...)'.dependencies]`
섹션 삭제. `wezterm-toast-notification`(linux/dbus + macos), `wezterm-font`(fontconfig +
macos), `env-bootstrap`(macos), `config`(unix nix), `termwiz`(unix signal-hook/termios/nix),
`wezterm`(unix termios), `wezterm-ssh`(unix passfd). `async_ossl`은 vendored openssl을
`[dependencies]`로 통합. `wezterm-ssh`의 dev-dependencies는 `tests/` 삭제 후 실제
사용처(examples/ssh.rs, config.rs 단위테스트)만 남기고(`clap`/`env_logger`/`k9`/
`shell-words`/`termwiz`) 정리.

**A — 루트 `workspace.dependencies` 미참조 항목 34개 제거:**
`assert_fs, block2, cgl, cocoa, core-foundation, core-graphics, core-text, fontconfig,
futures-lite, futures-util, mio, objc, objc2, objc2-core-graphics, objc2-foundation,
objc2-user-notifications, passfd, predicates, rstest, signal-hook, smithay-client-toolkit,
termios, wayland-backend, wayland-client, wayland-egl, wayland-protocols,
wayland-protocols-plasma, whoami, x11, xcb, xcb-imdkit, xkbcommon, zbus, zvariant`.
(`ratelim`은 wezterm-client/gui에서 사용 중이라 유지. `phf_codegen`·`human-sort`는
멤버가 직접 버전 선언하는 기존 미사용 항목으로 플랫폼과 무관해 범위 외 → 유지.)

**검증:** `cargo check --workspace --all-targets` **exit 0** (전 타겟, 제외 없음).
미참조 workspace 의존성을 제거하면 멤버의 `.workspace = true`가 끊겨 파싱 오류가 나므로,
exit 0은 제거 대상이 실제로 전부 미참조였음을 증명한다.

---

## 🔲 잔여 작업 — (대부분 완료)

> 평면적 플랫폼 제거(소스 cfg, 고아 파일, 의존성 섹션, 루트 workspace 의존성)는 완료
> 되었습니다. 아래는 선택적 잔여 정리뿐입니다.

### A. 루트 `Cargo.toml`의 `workspace.dependencies` — ✅ 완료

미참조 항목 34개 제거. 위 "추가 완료 작업" 참조.

### B. 다른 crate의 플랫폼 의존성 섹션 — ✅ 완료

Windows-거짓 `[target.'cfg(...)'.dependencies]` 섹션 전부 제거. 위 "추가 완료 작업" 참조.

### C. 잔존 플랫폼 전용 소스 파일 (삭제 후보) — ✅ 완료

`procinfo/src/{macos.rs, linux.rs}`, `wezterm-toast-notification/src/{macos.rs, dbus.rs}`,
`pty/src/unix.rs`, `wezterm-mux-server/src/daemonize.rs`, `wezterm-font/src/fcwrap.rs`
삭제 완료 (선언부 동시 제거). 위 "추가 완료 작업" 참조.

### D. 인라인 조건부 컴파일 블록 (Windows 기준 단순화) — ✅ 완료

Windows-거짓 cfg는 전 소스에서 0건(grep 확인). 원래 집계
`cfg(target_os="macos")` 97건/26파일 + `cfg(unix)` 계열 144건/44파일을
모두 정리. `cfg(unix)`/`cfg(windows)` 쌍은 "Windows 분기만 남기고 unix 분기 제거"로
파일별 개별 처리. 위 "추가 완료 작업" 참조.

### E. `cfg!()` 런타임 매크로 호출 — ✅ 대부분 완료

macОS `cfg!(target_os="macos")` 죽은 분기는 D와 함께 제거. 잔존하는
`cfg!(windows)`/`#[cfg(windows)]`는 Windows에서 항상 참이라 무해하여 일부 유지
(예: `config/src/unix.rs`의 serve_command). 추가 정리는 순수 명료성 목적.

### F. 검토 보류 대상 (삭제하면 안 되는 것)

| 경로 | 이유 |
|------|------|
| `config/src/unix.rs` | `UnixDomain` 정의 — Windows 10+ AF_UNIX·WSL에서 사용 |
| `config/src/wsl.rs` | WSL 도메인 설정 — Windows 환경 필수 |
| `wezterm-uds/` | Unix Domain Socket — Windows에서 `uds_windows`로 동작 |
| `termwiz/src/terminal/windows.rs` 등 | Windows 구현부 |

---

## 작업 순서 (완전 제거 진행 시 권장)

1. ~~**D·E 인라인 cfg 정리**~~ → ✅ 완료
2. ~~**C 소스 파일 삭제**~~ → ✅ 완료
3. ~~**B crate 의존성 섹션 제거**~~ → ✅ 완료
4. ~~**A 루트 workspace.dependencies 제거**~~ → ✅ 완료
5. **전체 `cargo build --release -p wezterm-gui` + 실행 테스트** ← 권장 다음 단계
   (지금까지는 `cargo check`로만 검증 — 실제 릴리스 빌드·기동 확인 필요)
6. 본 문서 최종 갱신

평면적 플랫폼 제거는 모두 완료되었습니다. 남은 것은 선택적 정리(`deps/cairo`의
`xcb`/`xlib` feature, 잔존 `cfg(windows)` 단순화)와 릴리스 빌드 실검증뿐입니다.
검증은 `cargo check --workspace --all-targets`로 매 단계 exit 0 확인했습니다.
