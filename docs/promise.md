# promise 폴더 기능 명세

## 1. 개요 및 책임

`promise` 크레이트는 WezTerm 워크스페이스의 **비동기 실행 추상화 계층(async runtime abstraction)** 이다. 단일 책임은 두 가지다.

- **약속/미래(Promise/Future) 프리미티브 제공**: 한쪽(생산자)에서 값을 채우고 다른 쪽(소비자)에서 `await`로 수신하는 일회성 oneshot 채널을 `std::future::Future`로 노출한다(`promise/src/lib.rs:19`, `promise/src/lib.rs:24`).
- **퓨처 스케줄링 추상화 제공**: "어느 실행기(executor)/스레드에서 퓨처를 폴링할지"를 임베딩 애플리케이션이 런타임에 주입할 수 있게 하는 스케줄러 등록·spawn API를 제공한다(`promise/src/spawn.rs`).

이 크레이트가 별도로 존재하는 이유는 `promise/src/spawn.rs:39`의 주석에 명시되어 있다. GUI 애플리케이션은 통상 "메인 스레드"에서 도는 전용 이벤트 루프를 가지므로 tokio/mio 같은 범용 런타임을 그 컨텍스트에 그대로 끼워 넣을 수 없다. `promise`는 그 플러밍(plumbing)의 구체적 구현을 알지 못한 채 "작업을 스케줄하는 추상화"만 제공한다. 즉, 실제 메인 루프 펌핑은 소비 크레이트(`window`, `wezterm-gui`)가 `set_schedulers`로 콜백을 주입해 담당하고, `promise`는 그 콜백을 호출하는 얇은 디스패치 지점 역할만 한다.

Windows fork에서의 실제 책임은 upstream과 동일하다. 이 크레이트 자체에는 플랫폼 분기 코드가 전혀 없다(`cfg(unix)`/`cfg(windows)` 등 없음). 플랫폼 종속성은 모두 소비 측(`window/src/spawn.rs` 등 Windows 메시지 펌프)에 있으며, `promise`는 순수 플랫폼 독립 추상화로 남아 있다.

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 |
|------|----------|
| 의존(deps) | 없음 (워크스페이스 내부 크레이트 의존 0건) |
| 피의존(usedBy) | config, mux, time-funcs, wezterm, wezterm-client, wezterm-gui, wezterm-mux-server, wezterm-mux-server-impl, window |

`promise`는 워크스페이스 내부 크레이트에 의존하지 않는 **최하위(leaf) 기반 계층**이며, 외부 async 크레이트(`async-executor`, `async-io`, `async-task`, `flume`)만 감싼다. GUI(`wezterm-gui`, `window`)·멀티플렉서(`mux`, `wezterm-client`, `wezterm-mux-server*`)·설정(`config`)·Lua 시간 함수(`time-funcs`)가 모두 이 위에 쌓이는 광범위한 기초 의존 대상이다. 따라서 파이프라인상 "비동기 작업을 메인 루프로 되돌리는 통로"의 가장 아래에 위치한다.

## 3. 공개 API 표면

### 3.1 루트 모듈 (`promise/src/lib.rs`)

| 항목 | 시그니처 요약 | 용도 |
|------|---------------|------|
| `struct BrokenPromise` | `#[derive(Debug, Error)]`, 메시지 "Promise was dropped before completion" | 약속이 완료 전 드롭됐음을 나타내는 에러 타입. 단, 현재 코드에서 직접 반환·생성되는 경로는 없음(아래 9절 참조) |
| `struct Promise<T>` | `core: Arc<Mutex<Core<T>>>` | 값 생산자 핸들 |
| `struct Future<T>` | `core: Arc<Mutex<Core<T>>>`, `impl std::future::Future<Output = Result<T, Error>>` | 값 소비자 핸들. 표준 `Future`로 `await` 가능 |
| `Promise::new()` / `Default` | `fn new() -> Self` | 빈 약속 생성 |
| `Promise::get_future(&mut self)` | `-> Option<Future<T>>` | 약속에 묶인 미래 핸들 발급(항상 `Some` 반환) |
| `Promise::ok(&mut self, value: T)` | `-> bool` | 성공 값으로 약속 이행 |
| `Promise::err(&mut self, err: Error)` | `-> bool` | 에러로 약속 이행 |
| `Promise::result(&mut self, Result<T, Error>)` | `-> bool` | 결과로 약속 이행(`ok`/`err`의 공통 구현). 항상 `true` 반환 |
| `Future::ok(value: T)` | `where T: Send + 'static` | 즉시 준비된 성공 leaf future 생성 |
| `Future::err(err: Error)` | 〃 | 즉시 준비된 에러 leaf future 생성 |
| `Future::result(Result<T, Error>)` | 〃 | 즉시 준비된 결과 leaf future 생성 |

소비 예: `mux/src/connui.rs:47`(`respond: Promise<String>`), `window/src/os/windows/connection.rs:157`(`Promise::new()` 후 `get_future()`로 미래를 얻어 반환), `wezterm-gui/src/frontend.rs:353` 등.

### 3.2 spawn 모듈 (`promise/src/spawn.rs`)

| 항목 | 시그니처 요약 | 용도 |
|------|---------------|------|
| `type SpawnFunc` | `Box<dyn FnOnce() + Send>` | 일회성 작업 클로저 |
| `type ScheduleFunc` | `Box<dyn Fn(Runnable) + Send + Sync + 'static>` | 폴링 가능한 `Runnable`을 메인 루프 큐에 넣는 콜백 |
| `pub use async_task::{Runnable, Task}` | — | 외부 `async_task`의 핵심 타입 재노출. `Task<R>`가 spawn 함수들의 반환 핸들 |
| `set_schedulers(main, low_pri)` | `(ScheduleFunc, ScheduleFunc)` | 임베딩 앱이 정상/저우선 스케줄러 콜백을 등록. `SCHEDULER_CONFIGURED` 플래그를 set |
| `is_scheduler_configured()` | `-> bool` | 스케줄러 등록 여부 조회(`Ordering::Relaxed`) |
| `spawn_into_new_thread<F, T>(f)` | `F: FnOnce() -> Result<T> + Send + 'static` → `Task<Result<T>>` | 새 OS 스레드에서 동기 작업 실행, 결과를 메인 스레드 future로 회수 |
| `spawn_into_main_thread<F, R>(future)` | `F: Future<Output=R> + Send + 'static` → `Task<R>` | 임의 스레드에서 호출, 메인 스레드(정상 우선)에서 폴링 |
| `spawn_into_main_thread_with_low_priority<F, R>(future)` | 〃 → `Task<R>` | 저우선 큐로 폴링 |
| `spawn<F, R>(future)` | `F: Future<Output=R> + 'static`(Send 불요) → `Task<R>` | 메인 스레드에서 호출 시 사용. `async_task::spawn_local` 기반 |
| `spawn_with_low_priority<F, R>(future)` | 〃 → `Task<R>` | 저우선 local spawn |
| `pub use async_io::block_on` | — | 현재 스레드를 future 완료까지 블록(재노출) |
| `struct SimpleExecutor` | `new()`, `tick() -> anyhow::Result<()>` | flume 채널 기반 단순 실행기. 비-GUI 컨텍스트용 |
| `struct ScopedExecutor` | `new()`, `async run<T>(future) -> T`, `Drop` | `async_executor::Executor` 기반 스코프 한정 실행기 |

## 4. 내부 구조

크레이트는 모듈 2개로 구성되며 비대한 모듈은 없다(총 ~371행).

- **루트 모듈(`lib.rs`, 108행)**: Promise/Future oneshot 프리미티브. 데이터 흐름은 단순하다. `Promise`와 `Future`가 동일한 `Arc<Mutex<Core<T>>>`를 공유한다. 생산자가 `result()`로 `Core::result`를 채우고, 대기 중인 `waker`가 있으면 깨운다. 소비자의 `Future::poll`은 `Core::result`를 `take()`하여 있으면 `Ready`, 없으면 자신의 `waker`를 등록하고 `Pending`을 반환한다(`promise/src/lib.rs:96`).

- **spawn 모듈(`spawn.rs`, 263행)**: 스케줄러 등록과 spawn 진입점. 제어 흐름의 핵심은 전역 상태 3종(아래)을 통한 디스패치다.
  - `ON_MAIN_THREAD` / `ON_MAIN_THREAD_LOW_PRI`: 정상/저우선 `ScheduleFunc`를 담는 전역 `Mutex`.
  - `SCOPED_EXECUTOR`: 선택적 스코프 실행기 핸들.
  - spawn 계열 함수는 `async_task`로 `(Runnable, Task)` 쌍을 만들고, `Runnable`의 스케줄 클로저가 `schedule_runnable`을 거쳐 `ON_MAIN_THREAD[_LOW_PRI]`에 등록된 콜백을 호출한다. 단, `SCOPED_EXECUTOR`가 설정돼 있으면 main-thread 계열은 우회하여 스코프 실행기에 직접 spawn한다(`promise/src/spawn.rs:131`, `promise/src/spawn.rs:150`).

  `spawn_into_new_thread`는 내부에 비공개 타입 `WakerHolder`와 `PendingResult<T>`를 정의한다. 새 스레드가 `f()`를 실행해 결과를 `flume::bounded(1)` 채널로 보내고, 폴링 측이 등록해 둔 waker를 깨운다. `PendingResult`는 그 채널을 폴링하는 future이며 결국 `spawn_into_main_thread`로 메인 스레드에 얹힌다(`promise/src/spawn.rs:57`).

## 5. 핵심 데이터 구조·타입

- **`Core<T>`**(`promise/src/lib.rs:13`): `result: Option<anyhow::Result<T>>`, `waker: Option<Waker>`. 불변식: (1) `result`는 한 번만 채워지고, `poll`에서 `take()`되면 다시 비워진다. (2) `result`가 채워지면서 `waker`가 있으면 즉시 깨워야 한다(누락 시 영구 대기). (3) `Mutex`로 보호되므로 생산자/소비자 동시 접근이 직렬화된다.

- **`Promise<T>` / `Future<T>`**: 동일 `Arc<Mutex<Core<T>>>`를 공유하는 한 쌍. 불변식: 둘은 같은 `Core`를 가리켜야 의미가 있다. `Future::result/ok/err`로 만든 leaf future는 `Promise` 없이 독립적으로 즉시 준비 상태가 된다.

- **`Core` waker 갱신 의미론**: `Future::poll`은 매 폴링마다 새 waker를 클론해 `core.waker`에 `replace`한다(`promise/src/lib.rs:97`). 이는 future가 다른 태스크로 이동(재폴링)되어도 올바른 waker를 유지하기 위한 표준 패턴이다.

- **`WakerHolder` / `PendingResult<T>`**(spawn 내부, 비공개): `spawn_into_new_thread`의 스레드↔future 동기화 전용. 불변식: 스레드가 결과 전송 후 waker를 깨우는 시점과 폴링 측이 waker를 등록하는 시점 사이의 경쟁을 `Mutex<Option<Waker>>`로 흡수한다. 채널 `Disconnected`(스레드가 결과 없이 종료)는 `Err(anyhow!("thread terminated without providing a result"))`로 변환된다(`promise/src/spawn.rs:107`).

- **전역 상태**: `ON_MAIN_THREAD`, `ON_MAIN_THREAD_LOW_PRI`(`lazy_static` `Mutex<ScheduleFunc>`), `SCOPED_EXECUTOR`(`Mutex<Option<Arc<Executor>>>`), `SCHEDULER_CONFIGURED`(`AtomicBool`). 불변식: 미설정 시 `no_scheduler_configured`가 `panic!`한다(`promise/src/spawn.rs:13`). 즉, spawn 계열을 호출하기 전에 반드시 `set_schedulers`가 선행되어야 한다.

## 6. 외부 의존성

| 크레이트 | 워크스페이스 버전 | 사용 이유 |
|----------|------------------|-----------|
| `anyhow` | 1.0 | 결과 타입 `Result<T, Error>`, 동적 에러(`anyhow!`) |
| `thiserror` | 1.0 | `BrokenPromise` 에러 파생(`#[derive(Error)]`) |
| `async-task` | 4.7 | 핵심 작업 추상화. `spawn`/`spawn_local`로 `(Runnable, Task)` 생성, `Runnable`/`Task` 재노출 |
| `async-executor` | 1.11 | `ScopedExecutor`의 백엔드(`Executor<'static>`) |
| `async-io` | 2.3 | `block_on` 재노출(현재 스레드 블로킹 폴링) |
| `flume` | 0.11 | MPMC 채널. `spawn_into_new_thread`의 결과 전달(`bounded(1)`), `SimpleExecutor`의 작업 큐(`unbounded`) |
| `lazy_static` | 1.4 | 전역 스케줄러/스코프 실행기 핸들의 지연 초기화 |

이 크레이트는 비동기 런타임의 "조각들"(executor, task, io, 채널)을 직접 조립하여 GUI 메인 루프 친화적 추상화를 만든다. tokio 같은 통합 런타임을 쓰지 않는 것이 의도된 설계다(`promise/src/spawn.rs:39` 주석).

## 7. 설정·기능 플래그

- **feature flag**: 없음. `Cargo.toml`에 `[features]` 섹션이 없다.
- **관련 config 항목**: 없음. `config` 크레이트는 이 크레이트를 사용하지만, `promise`의 동작을 바꾸는 설정 키는 존재하지 않는다.
- **빌드 메타**: `publish = false`, `edition = "2018"`.

## 8. Windows 전용 고려사항

- **플랫폼 분기 코드 없음**: 이 크레이트에는 `cfg(windows)`/`cfg(unix)`/`cfg(target_os=...)` 분기가 전혀 없다. 따라서 Windows fork의 플랫폼 코드 제거 작업의 영향을 받지 않은 순수 추상화다.
- **죽은 코드 없음(플랫폼 측면)**: 비Windows 분기가 본래부터 없으므로 플랫폼성 죽은 경로는 없다.
- **Windows API 직접 사용 없음**: Win32 호출은 소비 측에 있다. 예: `window/src/spawn.rs`의 `SpawnQueue`가 `EventHandle`(Windows 이벤트)를 보유하고 `register_promise_schedulers()`에서 `promise::spawn::set_schedulers`로 콜백을 주입한다(`window/src/spawn.rs:37`). 즉, `promise`는 콜백을 호출만 하고 실제 Windows 메시지 펌프 연동은 `window` 크레이트가 담당한다.

## 9. 리팩토링 주의점

- **전역 가변 상태 결합**: `set_schedulers`는 프로세스 전역 `Mutex`를 갱신한다. spawn 계열 호출 전에 반드시 등록돼야 하며, 미등록 시 `no_scheduler_configured`가 `panic!`한다(`promise/src/spawn.rs:13`). 초기화 순서가 곧 불변식이다. 다중 초기화/재등록 시 마지막 등록이 전역적으로 덮어쓴다.
- **`Promise::result`/`ok`/`err`의 반환값 무의미**: 항상 `true`를 반환하며(`promise/src/lib.rs:64`) 드롭·중복 이행 검사가 없다. `bool` 반환은 사실상 의미 없는 부산물이다. 또한 `result()`는 기존 결과를 `replace`하므로 중복 이행 시 마지막 값으로 덮어쓴다(에러 없이 조용히).
- **`BrokenPromise`가 생성되지 않음**: 에러 타입은 정의돼 있으나, `Promise`가 이행 없이 드롭돼도 `Future::poll`은 영원히 `Pending`을 반환한다. 현재 코드 어디에서도 `BrokenPromise`를 반환·구성하지 않는다. "드롭된 약속이 broken으로 통지된다"는 의미론은 미구현이다. 향후 드롭 감지를 넣으려면 `Promise::Drop`에서 결과를 채우고 waker를 깨우도록 보완해야 한다.
- **`get_future`의 `Option` 반환**: 시그니처는 `Option<Future<T>>`이나 항상 `Some`이다. 호출부는 일관되게 `.unwrap()`한다(`window/src/os/windows/connection.rs:158`, `wezterm-gui/src/frontend.rs:353` 등). 동일 약속에서 여러 future를 발급하는 것을 막지 않으므로, 다중 발급 시 어느 future가 결과를 `take()`할지 비결정적이다(불변식 취약점).
- **저우선 큐의 굶주림(starvation)**: 정상/저우선의 두 큐 분리는 `promise`가 아니라 콜백 구현(`window/src/spawn.rs`의 `pop_func`)에 있다. 그쪽은 정상 큐를 모두 비운 뒤에야 저우선을 처리하므로, 정상 작업이 끊이지 않으면 저우선 작업이 영구 지연될 수 있다. `promise`는 이 정책을 강제하지 않는다.
- **`ScopedExecutor` 전역 우회**: `SCOPED_EXECUTOR`가 설정돼 있으면 `spawn_into_main_thread[_with_low_priority]`가 등록된 메인 스레드 스케줄러를 우회한다(`promise/src/spawn.rs:131`). 따라서 스코프 실행기 활성 구간에서는 우선순위 의미론이 사라진다(스코프 실행기에는 우선순위 구분이 없음). `Drop` 시 전역에서 제거된다.
- **순환 의존**: 워크스페이스 내부 의존이 0이므로 순환 위험 없음. leaf 크레이트로 안전하게 유지해야 한다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|------|-----------|------|
| `promise/Cargo.toml` | 16 | 패키지 메타·외부 의존 7종 선언. 내부 의존·feature 없음 |
| `promise/src/lib.rs` | 108 | Promise/Future oneshot 프리미티브, `Core<T>`, `BrokenPromise` 에러, `spawn` 모듈 선언 |
| `promise/src/spawn.rs` | 263 | 스케줄러 등록(`set_schedulers`), spawn 진입점(메인 스레드/새 스레드/우선순위별), `SimpleExecutor`·`ScopedExecutor`, `block_on` 재노출 |
