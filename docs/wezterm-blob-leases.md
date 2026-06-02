# wezterm-blob-leases 폴더 기능 명세

## 1. 개요 및 책임

`wezterm-blob-leases`는 이미지 같은 대용량 이진 데이터(blob)를 프로세스 내 메모리 대신 디스크 기반 캐시 저장소에 보관하고, 그 데이터에 대한 참조 카운트형 핸들(lease)을 발급·관리하는 크레이트다. 단일 책임은 다음과 같다.

- **콘텐츠 중복 제거(content-addressed dedup)**: 동일한 바이트열은 SHA-256 해시(`ContentId`)로 식별되어 저장소에 한 번만 보관된다.
- **수명 관리(lease)**: 저장된 데이터에 대한 핸들 `BlobLease`를 발급한다. 핸들이 살아 있는 동안 데이터는 보장되며, 마지막 핸들이 드롭되면 저장소에서 제거될 수 있다.
- **저장 백엔드 추상화**: 실제 저장 메커니즘을 `BlobStorage` 트레이트로 추상화하고, 전역 싱글턴으로 한 개의 백엔드를 등록받아 사용한다.

이 크레이트가 존재하는 이유는, 터미널 셀이 참조하는 이미지(`iTerm2`/`Sixel`/`Kitty` 이미지 프로토콜의 디코딩 전 인코딩 바이트)를 셀·서피스 자료구조 안에 직접 들고 다니면 메모리 사용량이 폭증하고, 멀티플렉서를 통한 직렬화 시 같은 이미지를 반복 전송하게 되기 때문이다. 이를 디스크 캐시로 외부화하고 가벼운 핸들만 전파하기 위함이다.

Windows fork에서의 실제 책임은 동일하다. `wezterm-gui`와 `wezterm-mux-server` 두 실행 바이너리가 기동 시 디스크 임시 디렉터리 백엔드(`SimpleTempDir`)를 `config::CACHE_DIR` 아래에 등록하고, `wezterm-cell`의 이미지 데이터(`ImageDataType::EncodedLease`)가 이 lease를 통해 인코딩 바이트를 보관한다. 저장소가 등록되지 않은 환경에서는 `wezterm-cell`이 lease 대신 데이터를 인메모리(`EncodedFile`)로 폴백한다(`wezterm-cell/src/image.rs:366-368`).

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 | 비고 |
| --- | --- | --- |
| deps (이 크레이트가 의존) | 없음(워크스페이스 내부 크레이트 의존 없음) | 외부 크레이트만 사용: `sha2`, `uuid`, `thiserror`, `serde`(opt), `tempfile`(opt) |
| usedBy (이 크레이트를 의존) | `termwiz` | `image` feature 경유 |
| usedBy | `wezterm-cell` | `use_image` feature, `ImageDataType` 보관 |
| usedBy | `wezterm-escape-parser` | `image` feature 경유 |
| usedBy | `wezterm-surface` | `use_serde` 경유(현재 주석 처리된 전이 설정 존재) |
| usedBy | `wezterm-gui` | 실행 바이너리, 저장소 등록 |
| usedBy | `wezterm-mux-server` | 실행 바이너리, 저장소 등록 |

이 크레이트는 의존성 그래프의 **최하단 리프(leaf) 유틸리티**다. 워크스페이스 내부 크레이트에 전혀 의존하지 않으므로 순환 의존이 없으며, 셀·이스케이프 파서·서피스 계층이 이미지 바이트를 외부화할 때 사용하는 공통 저수준 인프라로 자리한다. 실제 저장소 등록은 최상위 실행 크레이트(`wezterm-gui`, `wezterm-mux-server`)가 담당한다.

## 3. 공개 API 표면

`lib.rs`는 모든 내부 모듈을 `pub use ...::*`로 재노출하고, `simple_tempdir`만 `pub mod`로 노출한다(`wezterm-blob-leases/src/lib.rs:10-15`).

- **`BlobManager`** (`manager.rs`): 빈 단위 구조체. 진입점 정적 메서드 제공.
  - `BlobManager::store(data: &[u8]) -> Result<BlobLease, Error>` — 바이트를 저장(중복 제거)하고 lease 반환.
  - `BlobManager::get_by_content_id(content_id: ContentId) -> Result<BlobLease, Error>` — 이미 저장된 콘텐츠에 대해 새 lease 발급. 없으면 `ContentNotFound`.

- **`BlobLease`** (`lease.rs`): 저장 데이터에 대한 참조 카운트 핸들(`Arc<LeaseInner>` 내부 보유).
  - `get_data(&self) -> Result<Vec<u8>, Error>` — 데이터 사본 반환.
  - `get_reader(&self) -> Result<BoxedReader, Error>` — 시킹 가능한 스트리밍 핸들 반환.
  - `content_id(&self) -> ContentId`.
  - `Clone`, `Debug`, `PartialEq`, `Eq` 파생.

- **`ContentId`** (`content_id.rs`): SHA-256 기반 콘텐츠 식별자. `[u8; 32]` 래퍼.
  - `for_bytes(bytes: &[u8]) -> Self`, `as_hash_bytes(&self) -> [u8; 32]`.
  - `Display`은 `sha256-<hex>` 형식, `Debug`은 `ContentId(...)`. `Copy`, `Hash`.

- **`LeaseId`** (`lease_id.rs`): 개별 lease 식별자. `Uuid`(v4) + 프로세스 `pid` 조합.
  - `new()`, `pid() -> u32`, `Default`, `Display`(`lease:pid=<pid>,<uuid>`).

- **`BlobStorage` 트레이트** (`storage.rs`): 저장 백엔드 구현 계약. `store`, `lease_by_content`, `get_data`, `get_reader`, `advise_lease_dropped`, `advise_of_pid`, `advise_pid_terminated`.

- **`BufSeekRead` 트레이트 / `BoxedReader` 타입** (`storage.rs`): `BufRead + Seek` 결합 트레이트와 그 박싱 타입 `Box<dyn BufSeekRead + Send + Sync>`.

- **전역 저장소 함수** (`storage.rs`):
  - `register_storage(storage: Arc<dyn BlobStorage + Send + Sync + 'static>) -> Result<(), Error>`.
  - `get_storage() -> Result<Arc<...>, Error>`(미등록 시 `StorageNotInit`).
  - `clear_storage()`.

- **`Error` enum** (`error.rs`): 크레이트 공통 오류 타입.

- **`simple_tempdir::SimpleTempDir`** (`simple_tempdir.rs`, `simple_tempdir` feature 게이트): 디스크 임시 디렉터리 기반 `BlobStorage` 기본 구현체. `new()`, `new_in(path)`.

- **serde 어댑터 모듈**(`serde` feature 게이트, `lease.rs`):
  - `lease_bytes` — `#[serde(with = ...)]` 어댑터. lease를 그 데이터 바이트로 직렬화하고, 역직렬화 시 바이트를 저장소에 다시 저장해 lease 복원.
  - `lease_content_id` — lease를 `ContentId`로 직렬화하고, 역직렬화 시 로컬 저장소에 이미 존재해야 복원(없으면 실패).

## 4. 내부 구조

모듈 분해는 다음과 같다. 비대한 모듈은 없으며 전 파일이 작다.

- `manager.rs`: 사용자 진입점. `store`/`get_by_content_id`가 `get_storage()`로 전역 백엔드를 얻고, `LeaseId::new()`로 lease id를 생성한 뒤 백엔드 호출 후 `BlobLease::make_lease`로 핸들을 조립한다.
- `lease.rs`: 핸들 본체와 수명 추적의 핵심. `BlobLease`는 `Arc<LeaseInner>`를 감싸 클론 시 참조를 공유한다. `LeaseInner::drop`이 트리거되면(마지막 `Arc` 소멸 시) 전역 저장소의 `advise_lease_dropped`를 호출한다. 데이터 접근(`get_data`/`get_reader`)은 매번 `get_storage()`로 백엔드를 재획득해 위임한다.
- `storage.rs`: 백엔드 계약(`BlobStorage`)과 전역 싱글턴(`static STORAGE: Mutex<Option<Arc<...>>>`). 전역 상태는 `Mutex`로 보호되며 단일 백엔드만 보유한다.
- `content_id.rs`, `lease_id.rs`: 값 타입 정의.
- `error.rs`: `thiserror` 기반 오류.
- `simple_tempdir.rs`: 디스크 기반 기본 백엔드. 자체 참조 카운트 맵(`refs: Mutex<HashMap<ContentId, usize>>`)으로 콘텐츠별 lease 수를 추적하고, 카운트가 0이 되면 파일을 삭제한다.

**제어/데이터 흐름**(저장→소비→해제):
1. 호출자가 `BlobManager::store(bytes)` 호출 → `ContentId::for_bytes`로 해시 계산 → `LeaseId::new()` → 백엔드 `store(content_id, data, lease_id)` → `BlobLease` 반환.
2. `SimpleTempDir::store`는 임시 파일에 기록 후 `content_id` 이름의 최종 경로로 `persist`(원자적 치환)하고 ref 카운트를 1 증가시킨다.
3. 소비자는 `lease.get_data()`/`get_reader()`로 데이터를 읽는다. `get_reader`가 반환하는 `Reader`는 자체 `Drop`에서 다시 `advise_lease_dropped`를 호출한다.
4. `BlobLease`의 마지막 클론이 드롭되면 `LeaseInner::drop` → `advise_lease_dropped` → `SimpleTempDir::del_ref` → 카운트가 1→0이면 파일 삭제.

주의: `BlobLease`가 직접 보유하는 lease 수명과 `get_reader`가 발급하는 `Reader`의 수명은 **둘 다** ref 카운트를 증가/감소시킨다. `get_reader` 경로에서 `add_ref`가 호출되고 `Reader::drop`에서 `del_ref`가 호출되므로, reader는 원래 lease와 독립적으로 콘텐츠 수명을 한 단위 더 연장한다(`simple_tempdir.rs:111-158`).

## 5. 핵심 데이터 구조·타입

- **`ContentId([u8; 32])`** (`content_id.rs:9`): SHA-256 다이제스트. 불변식 — 동일 바이트열은 항상 동일 `ContentId`로 매핑(결정론적). `Copy`이므로 자유 복제 가능. 충돌 가능성은 SHA-256 강도에 의존하며, 충돌 시 서로 다른 콘텐츠가 같은 저장 경로를 공유하는 위험이 이론적으로 존재한다.

- **`LeaseId { uuid: Uuid, pid: u32 }`** (`lease_id.rs:5`): 개별 lease 식별. 불변식 — `uuid`는 v4 난수로 전역 유일에 준하며, `pid`는 lease를 생성한 프로세스 ID. `pid`는 프로세스 종료 기반 일괄 무효화(`advise_pid_terminated`)를 위한 설계 흔적이다.

- **`BlobLease { inner: Arc<LeaseInner> }`** / **`LeaseInner { content_id, lease_id }`** (`lease.rs:11-19`): 불변식 — `inner` 필드는 생성 후 변경되지 않으며, `Arc` 참조 카운트가 핸들 수명을 결정한다. `LeaseInner::drop`은 마지막 클론 소멸 시 한 번만 실행되어 `advise_lease_dropped`를 정확히 한 번 호출한다. 클론된 핸들은 ref를 추가로 늘리지 않는다(저장소 ref 카운트는 백엔드가 lease id 발급 시점에만 관리). `PartialEq`/`Eq`는 `content_id`+`lease_id` 동등성 기준.

- **`SimpleTempDir { root: TempDir, refs: Mutex<HashMap<ContentId, usize>> }`** (`simple_tempdir.rs:11-14`): 불변식 — `refs[content_id]`는 해당 콘텐츠에 대한 활성 참조 수다. `store`와 `lease_by_content`/`get_reader`가 카운트를 증가시키고 `advise_lease_dropped`가 감소시킨다. 카운트가 1에서 다시 감소하면 파일을 삭제하고 값을 0으로 둔다(엔트리 자체는 제거하지 않음). `root`가 드롭되면 `TempDir`이 디렉터리 전체를 제거한다.

- **`static STORAGE: Mutex<Option<Arc<dyn BlobStorage + Send + Sync>>>`** (`storage.rs:5`): 불변식 — 전역에 최대 한 개의 백엔드만 존재. `register_storage`는 기존 값을 **무조건 교체**(`replace`)한다.

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
| --- | --- |
| `sha2` | 콘텐츠 주소화용 SHA-256 다이제스트 계산(`ContentId::for_bytes`). |
| `uuid` (`v4`, `rng`) | `LeaseId`의 전역 유일 식별자 생성. `serde` feature 시 `uuid/serde` 활성. |
| `thiserror` | `Error` enum의 보일러플레이트 없는 오류 정의. |
| `serde` (옵션) | lease/콘텐츠 식별자 직렬화 어댑터(`lease_bytes`, `lease_content_id`). |
| `tempfile` (옵션) | `SimpleTempDir` 백엔드의 임시 디렉터리·원자적 파일 persist. `simple_tempdir` feature에서만. |

## 7. 설정·기능 플래그

`Cargo.toml` 기준 feature는 3종이다(`wezterm-blob-leases/Cargo.toml:18-21`).

- `default = []` — 기본은 코어만. serde/tempdir 백엔드 없음.
- `serde = ["dep:serde", "uuid/serde"]` — serde 파생 및 lease 직렬화 어댑터 모듈 활성화. 상위 크레이트의 `use_serde` 체인을 통해 켜진다(`termwiz`/`wezterm-surface`의 `use_serde`가 `wezterm-blob-leases/serde`를 전이).
- `simple_tempdir = ["dep:tempfile"]` — `SimpleTempDir` 디스크 백엔드 활성화. `wezterm-gui`와 `wezterm-mux-server`가 이 feature로 의존한다(`wezterm-gui/Cargo.toml:92`, `wezterm-mux-server/Cargo.toml:25`).

관련 config 항목: 백엔드 저장 위치는 호출자가 결정한다. 두 바이너리 모두 `config::CACHE_DIR`를 루트로 `SimpleTempDir::new_in`을 호출한다(`wezterm-gui/src/main.rs:415-417`, `wezterm-mux-server/src/main.rs:151-153`). 크레이트 자체에는 `config` 의존이 없다.

## 8. Windows 전용 고려사항

- 이 크레이트의 소스에는 `cfg(unix)`/`cfg(windows)` 분기나 직접적 Windows API 호출이 **전혀 없다**. 파일 I/O는 모두 `std::fs`/`tempfile`의 크로스플랫폼 경로를 사용하므로 Windows에서 그대로 동작한다.
- 플랫폼 결합은 **소비처**에 있다. `wezterm-gui`의 저장소 등록 직전 코드가 `libc::getpid()`(`wezterm-gui/src/main.rs:413`)를 호출하는데, 이는 Windows fork에서 죽은 경로로 남아 있을 수 있는 비Windows 잔재다. 단, 저장소 등록 호출(`register_storage`/`new_in`) 자체는 플랫폼 무관하다.
- `LeaseId`의 `pid`는 `std::process::id()`로 얻으며 Windows에서 정상 동작한다. `advise_of_pid`/`advise_pid_terminated`는 `SimpleTempDir`에서 no-op이므로, 프로세스 종료 기반 일괄 lease 무효화는 현재 구현되어 있지 않다(설계상의 미사용 훅).
- `tempfile`의 원자적 `persist`는 Windows에서 동일 볼륨 내 rename 의미론을 따르므로, `CACHE_DIR`와 임시 파일이 같은 볼륨에 있어야 원자성이 보장된다.

## 9. 리팩토링 주의점

- **전역 가변 싱글턴**: `static STORAGE`(`storage.rs:5`)는 프로세스 전역 상태다. `BlobManager`와 `BlobLease`의 모든 동작이 이 싱글턴에 암묵 결합되어 있어 테스트 격리·다중 백엔드가 불가능하다. `register_storage`가 기존 백엔드를 경고 없이 교체(`replace`)하므로 중복 등록 시 이전 백엔드의 lease는 이후 데이터 접근에서 비결정적으로 실패할 수 있다.
- **lease와 백엔드 ref 카운트의 이중성**: `BlobLease`의 `Arc` 카운트(핸들 수명)와 `SimpleTempDir::refs`(콘텐츠 수명)는 서로 다른 메커니즘이다. `get_reader`가 추가 ref를 잡고 `Reader::drop`이 푸는 구조라, lease 핸들을 드롭하지 않은 채 reader만 반복 생성/소멸시키면 카운트 정합이 백엔드 구현에 의존한다. 백엔드 교체 시 이 호출 규약(store/lease_by_content/get_reader가 ref+1, advise_lease_dropped가 ref-1)을 정확히 보존해야 한다.
- **드롭 시 오류 무시**: `LeaseInner::drop`과 `Reader::drop`은 `advise_lease_dropped(...).ok()`로 오류를 삼킨다(`lease.rs:49-57`, `simple_tempdir.rs:141-147`). 저장소가 이미 `clear_storage`된 뒤 lease가 드롭되면 정리가 누락되어 임시 파일이 잔존할 수 있다. 단, `SimpleTempDir`의 `root: TempDir`가 드롭되면 디렉터리 전체가 제거되므로 통상 누수는 회수된다.
- **serde 어댑터의 부작용**: `lease_bytes::deserialize`는 역직렬화 과정에서 `BlobManager::store`를 호출해 **저장소에 부작용을 발생**시킨다(`lease.rs:81-88`). 따라서 역직렬화는 전역 저장소가 등록된 상태를 전제한다. `lease_content_id::deserialize`는 로컬에 콘텐츠가 이미 있어야만 성공한다. 직렬화 형식을 바꾸면 멀티플렉서 프로토콜 호환성에 직접 영향이 간다.
- **`del_ref`의 엔트리 잔존**: 카운트가 0이 되어도 `HashMap` 엔트리는 제거되지 않고 값만 0으로 남는다(`simple_tempdir.rs:55-62`). 동일 콘텐츠가 자주 재등록되는 워크로드에서는 무한 누적은 아니나 엔트리가 영구 잔존한다.
- **`ContentId` 표시 형식 의존**: 파일명이 `format!("{content_id}")` 즉 `sha256-<hex>`에 의존한다(`simple_tempdir.rs:42`). `Display` 구현을 바꾸면 기존 캐시 경로와 불일치한다.
- **순환 의존 없음**: 내부 크레이트 의존이 없으므로 이 크레이트 변경이 워크스페이스에 순환을 유발하지 않는다. 다만 공개 API(특히 `Error` 변형, lease 직렬화 형식)는 6개 상위 크레이트가 직접 소비하므로 시그니처 변경의 파급이 크다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `Cargo.toml` | 22 | 패키지·의존성·feature(`serde`, `simple_tempdir`) 정의. |
| `src/lib.rs` | 16 | 모듈 선언 및 전 모듈 재노출(`pub use`). |
| `src/manager.rs` | 28 | `BlobManager` 진입점. `store`/`get_by_content_id`. |
| `src/lease.rs` | 122 | `BlobLease` 핸들·`Drop` 기반 수명 추적, serde 어댑터(`lease_bytes`, `lease_content_id`). |
| `src/storage.rs` | 72 | `BlobStorage` 트레이트, `BufSeekRead`/`BoxedReader`, 전역 싱글턴 등록/조회/해제. |
| `src/content_id.rs` | 38 | `ContentId`(SHA-256) 정의, `Display`/`Debug`. |
| `src/lease_id.rs` | 33 | `LeaseId`(uuid+pid) 정의. |
| `src/error.rs` | 25 | `thiserror` 기반 `Error` enum. |
| `src/simple_tempdir.rs` | 173 | `simple_tempdir` feature: 디스크 임시 디렉터리 백엔드 + 콘텐츠 ref 카운트. |
| `LICENSE.md` | — | MIT 라이선스 본문. |
