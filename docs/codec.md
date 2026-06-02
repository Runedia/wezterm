# codec 폴더 기능 명세

## 1. 개요 및 책임

`codec` 크레이트는 WezTerm의 멀티플렉서(mux) 프로토콜에서 교환되는 **PDU(Protocol Data Unit)의 프레이밍·직렬화·역직렬화 단일 책임**을 담당한다. 클라이언트(`wezterm-client`)와 mux 서버(`wezterm-mux-server-impl`) 사이를 흐르는 모든 요청·응답·알림 메시지를 하나의 `Pdu` 열거형으로 정의하고, 이를 바이트 스트림으로 인코딩하거나 바이트 스트림에서 디코딩하는 코드만 포함한다.

크레이트 문서 주석(`codec/src/lib.rs:1`)이 명시하듯, 이 크레이트는 단순히 serde에만 의존해 enum을 직렬화하지 않는다. 각 enum variant에 **명시적 버전/식별자 태그(ident)**를 부여하고, PDU 길이·serial·ident를 가변 길이 정수(leb128)로 프레이밍한다. 그 목적은 서로 다른 버전으로 빌드된 클라이언트·서버가 알 수 없는 enum variant를 만나더라도 우아하게(graceful) 대응할 수 있게 하는 것이다. 알 수 없는 ident는 `Pdu::Invalid { ident }`로 디코드되어 연결 전체를 깨뜨리지 않는다.

이 크레이트는 네트워크 전송(소켓·TLS) 자체를 담당하지 않는다. 오직 **프레임 단위의 인코딩/디코딩 규약**만 정의하며, 전송 계층은 호출자(`wezterm-client`, `wezterm-mux-server-impl`)가 제공하는 `std::io::Read`/`Write` 또는 `smol`의 `AsyncRead`/`AsyncWrite` 위에서 동작한다.

Windows fork에서의 실제 책임도 동일하다. 이 크레이트 자체에는 플랫폼 분기가 없다(아래 8장 참조). WezTerm의 GUI가 원격 또는 로컬 mux 서버에 붙을 때, 그 사이에서 오가는 메시지의 와이어 포맷을 정의하는 것이 본 크레이트의 유일한 역할이다.

## 2. 워크스페이스 내 위치

### 의존성(deps)

| 크레이트 | 사용 이유 |
| --- | --- |
| `config` | `keyassignment::{PaneDirection, ScrollbackEraseMode, SpawnTabDomain}` 등 설정 enum을 PDU 필드 타입으로 사용 |
| `mux` | `PaneId`, `TabId`, `WindowId`, `ClientId`, `ClientInfo`, `PaneNode`, `SplitRequest`, `RenderableDimensions`, `StableCursorPosition`, `Pattern`, `SearchResult`, `SerdeUrl` 등 mux 도메인 타입을 PDU 필드로 사용 |
| `portable-pty` | `CommandBuilder`(spawn/split 시 실행 명령)를 PDU 필드로 사용 (`serde_support` feature 활성화) |
| `rangeset` | `range_union` 등 범위 연산으로 하이퍼링크 셀 구간을 병합 |
| `termwiz` | `Line`, `SequenceNo`, `Hyperlink`, `ImageData`, `TextureCoordinate`, `KeyEvent` 등 터미널 모델 타입을 PDU 필드로 사용 |
| `wezterm-term` | `ColorPalette`, `Alert`, `ClipboardSelection`, `StableRowIndex`, `TerminalSize`, `MouseEvent` 등 터미널 상태 타입을 PDU 필드로 사용 (`use_serde` feature 활성화) |

추가 외부 의존성(워크스페이스 외부, 6장 참조): `anyhow`, `leb128`, `log`, `metrics`, `serde`, `smol`, `thiserror`, `varbincode`, `zstd`.

### 피의존(usedBy)

| 크레이트 | 사용 방식 |
| --- | --- |
| `wezterm` | 최상위 바이너리. 클라이언트·서버 경로 양쪽에서 codec 타입을 간접 사용 |
| `wezterm-client` | mux 서버로 PDU 요청을 인코딩해 보내고 응답을 디코딩 |
| `wezterm-gui` | GUI에서 원격 pane 상태를 받아 처리하는 경로에서 codec 타입 참조 |
| `wezterm-mux-server-impl` | 서버 측. 들어온 PDU를 디코딩해 처리하고 응답 PDU를 인코딩 |

### 계층상의 위치

`codec`은 mux 클라이언트/서버 계층의 **공통 와이어 프로토콜 계약(contract) 계층**이다. 도메인 타입을 정의하는 하위 크레이트(`mux`, `termwiz`, `wezterm-term`, `config`)에 의존하고, 실제 송수신을 수행하는 상위 크레이트(`wezterm-client`, `wezterm-mux-server-impl`)에 의해 소비된다. 즉 클라이언트와 서버가 동일한 메시지 정의를 공유하도록 강제하는 단일 진실 공급원(single source of truth)이다.

## 3. 공개 API 표면

크레이트는 단일 모듈(`lib.rs`)이며, 별도의 하위 모듈 분할은 없다. 공개 표면은 다음과 같다.

### 핵심 타입

- `pub enum Pdu` — 모든 프로토콜 메시지의 합. `Invalid { ident: u64 }` variant와 65개 명명 variant(`ErrorResponse`, `Ping`, `Pong`, … `AdjustPaneSize`)로 구성. `pdu!` 매크로가 생성한다(`codec/src/lib.rs:333`, `codec/src/lib.rs:452`).
- `pub struct DecodedPdu { pub serial: u64, pub pdu: Pdu }` — 디코드 결과. serial(요청-응답 상관 번호)과 페이로드를 함께 보유(`codec/src/lib.rs:282`).
- `pub struct InputSerial(u64)` — 입력 요청과 출력 이벤트의 순서를 맞추기 위한 시퀀스. 본래 단조 증가 번호였으나 유닉스 epoch 이후 경과 밀리초로 진화(`codec/src/lib.rs:727`).
- `pub struct SerializedLines` — 라인 데이터의 직렬화 표현. 하이퍼링크와 이미지 셀을 별도 보관해 와이어 크기를 줄임(`codec/src/lib.rs:975`).
- `pub struct SerializedImageCell` — 라인에 박힌 이미지 셀 메타데이터(좌표·패딩·z-index·data_hash 등)의 직렬화 표현(`codec/src/lib.rs:950`).
- `pub struct CorruptResponse(String)` — 디코드 시 비정상 serial/길이를 만났을 때 반환하는 오류 타입(`codec/src/lib.rs:39`).
- 그 외 각 PDU variant에 대응하는 `pub struct`(예: `Ping`, `SpawnV2`, `GetLinesResponse`, `GetPaneRenderChangesResponse` 등) — 모두 `Deserialize, Serialize, PartialEq, Debug` 파생.

### 핵심 함수/메서드 (모두 `Pdu`의 연관 함수)

| 시그니처 | 용도 |
| --- | --- |
| `pub fn encode<W: Write>(&self, w: W, serial: u64) -> Result<(), Error>` | PDU를 동기 쓰기 스트림으로 인코딩 |
| `pub async fn encode_async<W: Unpin + AsyncWriteExt>(&self, w: &mut W, serial: u64) -> Result<(), Error>` | 비동기 쓰기 스트림으로 인코딩 |
| `pub fn decode<R: Read>(r: R) -> Result<DecodedPdu, Error>` | 동기 읽기 스트림에서 한 프레임 디코딩 |
| `pub async fn decode_async<R>(r: &mut R, max_serial: Option<u64>) -> Result<DecodedPdu, Error>` | 비동기 읽기 스트림에서 디코딩. `max_serial`로 비정상 serial 방어 |
| `pub fn stream_decode(buffer: &mut Vec<u8>) -> anyhow::Result<Option<DecodedPdu>>` | 버퍼에서 완성된 프레임 하나를 떼어내고 소비분만큼 버퍼 앞을 제거 |
| `pub fn try_read_and_decode<R: Read>(r: &mut R, buffer: &mut Vec<u8>) -> anyhow::Result<Option<DecodedPdu>>` | 버퍼 디코드와 스트림 읽기를 결합한 루프. 논블로킹 시 `WouldBlock`을 `None`으로 변환 |
| `pub fn pdu_name(&self) -> &'static str` | variant 이름 문자열(로깅·메트릭용) |
| `pub fn is_user_input(&self) -> bool` | 사용자 직접 입력성 PDU 여부(백그라운드 트래픽과 구분) |
| `pub fn pane_id(&self) -> Option<PaneId>` | PDU에 연관된 pane id 추출(해당하는 variant만) |

### 상수

- `pub const CODEC_VERSION: usize = 45` — 코덱 전체 버전. 타입·프로토콜에 하위 호환을 깨는 변경이 가해질 때 증가시켜야 한다(`codec/src/lib.rs:443`).

## 4. 내부 구조

크레이트는 단일 파일(`codec/src/lib.rs`, 약 1248행)로 구성되며, 모듈 분할 없이 다음 논리 영역으로 나뉜다.

1. **프레이밍 헬퍼**(`codec/src/lib.rs:43`–`280`) — leb128 가변 정수 입출력(`encoded_length`, `read_u64`, `read_u64_async`), 프레임 인코딩(`encode_raw`, `encode_raw_as_vec`, `encode_raw_async`), 프레임 디코딩(`decode_raw`, `decode_raw_async`). 모두 비공개. 동기/비동기 두 벌이 거의 동일한 로직을 중복 구현한다.
2. **압축·serde 계층**(`codec/src/lib.rs:288`–`331`) — `serialize`/`deserialize`. 페이로드를 `varbincode`로 직렬화하고, 32바이트(`COMPRESS_THRESH`)를 넘으면 zstd 압축을 시도한다. 압축본이 더 작을 때만 압축본을 사용하고, 프레임 길이의 최상위 비트(`COMPRESSED_MASK`)로 압축 여부를 표시한다.
3. **`pdu!` 매크로와 `Pdu` 정의**(`codec/src/lib.rs:333`–`507`) — variant 목록을 받아 `Pdu` enum, `encode`/`encode_async`/`decode`/`decode_async`/`pdu_name`을 일괄 생성한다. ident-디스패치 테이블이 매크로 본문에 박혀 있다.
4. **`Pdu`의 수동 구현**(`codec/src/lib.rs:509`–`604`) — `is_user_input`, `stream_decode`, `try_read_and_decode`, `pane_id`.
5. **PDU 페이로드 struct 정의**(`codec/src/lib.rs:606`–`1145`) — 65개 메시지 struct. 비대한 것은 `GetPaneRenderChangesResponse`(`codec/src/lib.rs:916`)와 라인 직렬화 로직을 품은 `SerializedLines`(`codec/src/lib.rs:975`–`1106`)이다.
6. **테스트 모듈**(`codec/src/lib.rs:1147`–`1248`) — 프레임 왕복, ping/pong 인코딩, 스트림 디코드, 알 수 없는 ident 처리(`test_bogus_pdu`) 검증.

### 제어/데이터 흐름

- **인코딩**: `Pdu::encode(serial)` → `serialize`(varbincode + 선택적 zstd) → `encode_raw`(leb128로 masked_len/serial/ident 기록 후 데이터 첨부). 헤더가 한 패킷에 나가도록 단일 버퍼로 더블 버퍼링한다(nodelay 환경 가정, `codec/src/lib.rs:73`).
- **디코딩**: `decode`/`decode_async` → `decode_raw`(len/serial/ident 읽고 데이터 길이 역산) → ident로 variant 디스패치 → `deserialize`(필요 시 zstd 해제). 알 수 없는 ident는 `Pdu::Invalid`로 변환.
- **스트림 누적**: `try_read_and_decode`가 `stream_decode`를 반복 호출하며 버퍼를 채운다. `stream_decode`는 완성된 프레임 하나를 떼어내고 `unsafe { ptr::copy_nonoverlapping }`으로 버퍼 앞부분을 in-place 압축한다(`codec/src/lib.rs:536`).

## 5. 핵심 데이터 구조·타입

### `Pdu` (enum)

- 불변식: 각 명명 variant는 `pdu!` 매크로에 등록된 **고유한 ident 정수**에 1:1 대응한다(`codec/src/lib.rs:452`–`507`). ident는 재사용·재배치하면 안 되며, variant 제거 시 그 번호는 영구 결번 처리해야 한다(주석이 명시).
- `Invalid { ident }`는 직렬화 불가. `encode`/`encode_async`가 호출되면 `bail!`로 거부한다(`codec/src/lib.rs:349`).
- `#[allow(clippy::large_enum_variant)]` — 외부 크레이트가 각 variant를 값으로 생성·구조 분해하므로 Box화하면 호출부가 깨진다는 이유로 보류됨(`codec/src/lib.rs:335`).

### 프레임 형식 불변식 (`encode_raw`, `codec/src/lib.rs:92`)

와이어 레이아웃은 다음 순서다.
- `tagged_len: leb128` — 데이터 + ident 인코딩 길이 + serial 인코딩 길이의 합. **u64 최상위 비트가 1이면 압축됨**.
- `serial: leb128`
- `ident: leb128`
- `data bytes`

`data_len = len − encoded_length(ident) − encoded_length(serial)`로 역산하며, 음수가 되면(overflowing_sub) `CorruptResponse`를 반환한다(`codec/src/lib.rs:199`, `246`). `COMPRESSED_MASK = 1 << 63`(`codec/src/lib.rs:58`).

### `InputSerial(u64)` (`codec/src/lib.rs:727`)

- `empty()`는 0. `now()`는 현재 시각의 epoch 밀리초. `From<SystemTime>`이 epoch 이전이거나 u64를 넘으면 `panic`한다(`codec/src/lib.rs:748`).
- `PartialOrd`/`Ord` 파생 — 시간 순서 비교 가능.

### `SerializedLines` (`codec/src/lib.rs:975`)

- 불변식: 셀이 `Arc<Hyperlink>`를 참조할 때, 동일 URL은 한 번만 전송하면서도 디코드 후 각 셀의 하이퍼링크 **동일성(identity)**을 복원해야 한다.
- `From<Vec<(StableRowIndex, Line)>>`(`codec/src/lib.rs:1013`)가 직렬화 시 각 라인을 훑어 하이퍼링크를 셀에서 떼어내고 `Arc::ptr_eq`로 연속 구간(streak)을 묶어 `LineHyperlink`로 추출하며, 이미지 셀도 별도 추출 후 셀에서 제거한다.
- `extract_data`(`codec/src/lib.rs:985`)가 역방향으로 URL당 단일 `Arc`를 재구성해 해당 구간 셀에 다시 부여하고, 라인 데이터와 이미지 목록을 반환한다.

### `SerializedImageCell` (`codec/src/lib.rs:950`)

- 이미지 픽셀 데이터 자체를 담지 않고 `data_hash: [u8; 32]`만 보유한다. 실제 픽셀은 `GetImageCell`/`GetImageCellResponse`로 별도 요청해 받는 구조(콘텐츠 해시 기반 지연 전송).

## 6. 외부 의존성

| 크레이트 | 사용 이유 |
| --- | --- |
| `leb128` | PDU 헤더(길이·serial·ident)의 가변 길이 정수 인코딩/디코딩. 작은 값을 적은 바이트로 보냄 |
| `varbincode` | PDU 페이로드 struct의 실제 serde 직렬화 포맷(가변 길이 바이너리 코덱) |
| `zstd` | 32바이트 초과 페이로드의 무손실 압축. `DEFAULT_COMPRESSION_LEVEL` 사용(`codec/src/lib.rs:301`) |
| `serde` | PDU struct 파생 직렬화. `rc` feature로 `Arc` 직렬화, `derive`로 매크로 사용 |
| `smol` | 비동기 I/O(`AsyncRead`/`AsyncWriteExt`)로 `encode_async`/`decode_async` 구현 |
| `metrics` | `pdu.encode.size`, `pdu.decode.size`, `pdu.size`(variant별 라벨) 등 히스토그램 계측 |
| `anyhow` | 디코드/인코드 단계별 `Context`를 붙인 오류 전파 |
| `thiserror` | `CorruptResponse` 오류 타입 파생 |
| `log` | 인코드/직렬화 디버그 로깅 |

`harfbuzz`/`wgpu` 등 셰이핑·GPU 의존은 본 크레이트에 **없다**(상위 GUI 크레이트의 관심사).

## 7. 설정·기능 플래그

- 이 크레이트 자체는 `[features]` 섹션을 정의하지 않는다(`codec/Cargo.toml`에 없음).
- 의존 크레이트의 feature를 활성화한다: `portable-pty`의 `serde_support`, `wezterm-term`의 `use_serde`, `serde`의 `rc`/`derive`. 이들은 PDU 필드 타입이 serde로 직렬화 가능하도록 만들기 위한 것이다.
- 관련 상수(소스 기본값으로 동작 고정):
  - `CODEC_VERSION = 45`(`codec/src/lib.rs:446`) — 프로토콜 호환성 게이트.
  - `COMPRESS_THRESH = 32`(`codec/src/lib.rs:289`) — 압축 시도 임계 바이트.
- 별도의 `config` 항목(`wezterm.lua` 노출 설정)은 이 크레이트에서 직접 읽지 않는다. `config` 의존은 키 어사인 enum 타입 참조용일 뿐이다.

## 8. Windows 전용 고려사항

- 이 크레이트에는 `cfg(unix)`, `cfg(windows)`, `cfg(target_os = ...)` 등 **플랫폼 분기가 전혀 없다**. 코드는 완전히 플랫폼 중립적이다.
- Windows API를 직접 호출하는 지점도 없다. 모든 I/O는 호출자가 넘긴 제네릭 `Read`/`Write` 또는 `smol` 트레이트를 통해 추상화된다.
- 따라서 Windows fork에서 제거 대상이 되는 죽은 플랫폼 경로는 본 크레이트 내부에 없다. 단, 와이어 포맷에 실리는 일부 PDU 필드(예: `SpawnV2.command: CommandBuilder`, `SplitPane.command_dir`)는 Windows에서 실행될 명령·경로를 표현하며, 그 의미는 호출자(서버) 측에서 해석된다.

## 9. 리팩토링 주의점

- **ident 안정성**: `pdu!` 매크로의 번호(`codec/src/lib.rs:452`–`507`)는 와이어 호환의 핵심 불변식이다. 번호를 바꾸거나 재사용하면 서로 다른 버전의 클라이언트·서버가 침묵 속에 오해석한다. 변경 시 반드시 `CODEC_VERSION`을 올려야 한다.
- **`unsafe` 버퍼 압축**: `stream_decode`의 `ptr::copy_nonoverlapping`(`codec/src/lib.rs:536`)은 소비 바이트만큼 버퍼 앞을 이동시킨다. `consumed <= buffer.len()` 불변식이 깨지면 메모리 안전성 위반이다. `Vec::drain`으로 대체 가능하나 성능 의도(핫패스)를 고려해야 한다.
- **동기/비동기 중복**: 프레이밍 로직이 `encode_raw`/`encode_raw_async`, `decode_raw`/`decode_raw_async`, `read_u64`/`read_u64_async`로 이중화되어 있다(`codec/src/lib.rs:98`–`280`). 한쪽 수정 시 다른 쪽 동기화 누락 위험. `encode_raw_as_vec`(`codec/src/lib.rs:60`)는 공유되어 있어 인코딩 측은 부분적으로 통합됨.
- **`large_enum_variant`**: `Pdu`는 가장 큰 variant 크기로 모든 값을 부풀린다(`codec/src/lib.rs:337`). 외부 크레이트가 값 패턴 매칭에 의존하므로 Box화는 호출부 파급이 크다. 분기 전체에서 호출부를 함께 고쳐야 안전하다.
- **`panic` 경로**: `InputSerial::from(SystemTime)`(`codec/src/lib.rs:748`)와 `encoded_length`의 `.unwrap()`(`codec/src/lib.rs:55`)는 패닉한다. 신뢰할 수 없는 입력 경로에서 호출되지 않는다는 가정에 의존한다.
- **결합도**: PDU 필드가 `mux`/`termwiz`/`wezterm-term`/`config`의 구체 타입을 직접 포함한다. 이 타입들의 직렬화 표현이 바뀌면 와이어 포맷이 조용히 바뀌어 호환성이 깨진다(특히 `Line`, `ColorPalette`, `Alert`). 순환 의존은 없다(codec은 이들 위에 단방향으로 얹힘).
- **`#![allow(dead_code)]`**(`codec/src/lib.rs:11`): 일부 비공개 헬퍼·필드가 미사용일 수 있으나 경고가 억제되어 있다. 정리 시 실제 사용처를 grep으로 확인 후 제거할 것.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `codec/Cargo.toml` | 26 | 크레이트 매니페스트. 의존성과 feature 활성화 정의 |
| `codec/src/lib.rs` | 1248 | 크레이트 전체. 프레이밍 헬퍼, 압축/serde 계층, `pdu!` 매크로와 `Pdu` enum, 65개 PDU struct, `SerializedLines` 직렬화 로직, 테스트를 단일 파일에 포함 |

생성(generated) 파일은 없다. 모든 코드는 수기 작성물이다.
