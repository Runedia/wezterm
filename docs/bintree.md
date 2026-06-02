# bintree 폴더 기능 명세

## 1. 개요 및 책임

`bintree`는 **Zipper 기반 커서(Cursor)를 갖춘 이진 트리** 자료구조를 제공하는 단일 책임 크레이트다. 외부 의존성이 전혀 없는 순수 알고리즘 라이브러리이며, 트리 구조 자체와 그 위를 이동·변형하는 커서 연산만을 정의한다(`bintree/src/lib.rs:1`).

이 크레이트가 모델링하는 트리는 "(거의) 정상(proper) 이진 트리"다. 즉 각 비단말(Node) 노드는 항상 0개 또는 2개의 자식을 가지며, 단말(Leaf)은 자식이 없다. 단 하나의 예외로, 트리 전체가 단일 리프 하나로 루팅될 수 있다(`bintree/src/lib.rs:11`). 리프는 필수 데이터 타입 `L`을, 비단말 노드는 선택적(`Option<N>`) 데이터 타입 `N`을 가진다(기본값 `()`).

Windows 포크에서의 실제 책임은 **터미널 패널 분할 레이아웃의 트리 표현**이다. `mux` 크레이트가 이 트리를 `Tree<Arc<dyn Pane>, SplitDirectionAndSize>`로 인스턴스화하여, 리프=패널, 비단말 노드=분할(split) 경계로 사용한다(`mux/src/tab.rs:17`). 즉 사용자가 화면을 가로/세로로 분할할 때마다 리프가 노드로 쪼개지고, 패널을 닫을 때마다 노드가 다시 리프로 합쳐진다. `bintree` 자체에는 패널이나 터미널 개념이 없으며, 순수하게 트리 구조를 일반화(generic)된 형태로만 다룬다.

이 크레이트에는 플랫폼 의존 코드가 전혀 없다. 표준 라이브러리(`std::cmp::PartialEq`, `std::fmt::Debug`)만 사용하므로 Windows 포크 분기와 무관하게 동작한다.

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 |
|------|----------|
| 의존(deps) | 없음 |
| 피의존(usedBy) | `mux` |

이 크레이트는 의존성 그래프의 **최하단 리프(leaf) 크레이트**다. 외부 크레이트나 워크스페이스 내 다른 크레이트를 일절 참조하지 않으며, 오직 `mux`가 패널 레이아웃 트리를 구성하기 위해 소비한다. 따라서 `bintree`의 변경은 `mux`(나아가 `mux`에 의존하는 GUI 계층)에만 파급된다.

## 3. 공개 API 표면

공개 항목은 두 개의 타입(`Tree`, `Cursor`), 하나의 보조 반복자(`ParentIterator`), 하나의 보조 열거형(`PathBranch`)으로 구성된다. `Path` 열거형은 비공개(`enum Path`, `bintree/src/lib.rs:73`)이며 커서 내부 상태로만 쓰인다.

### 3.1 `Tree<L, N = ()>` (`bintree/src/lib.rs:16`)

소유권 기반 이진 트리 enum. 공개 메서드:

- `Tree::new() -> Self` — 빈 트리(`Empty`) 생성(`:176`).
- `Tree::is_empty(&self) -> bool` — `Empty` 여부(`:181`).
- `Tree::cursor(self) -> Cursor<L, N>` — 트리를 소비하여 커서(Zipper) 표현으로 변환(`:186`).
- `Tree::num_leaves(&self) -> usize` — 재귀적으로 리프 개수 집계(`:193`).
- `impl PartialEq` / `impl Debug` — `L: PartialEq, N: PartialEq` / `L: Debug, N: Debug` 제약 하에 구조적 비교·디버그 출력(`:26`, `:52`).

### 3.2 `Cursor<L, N>` (`bintree/src/lib.rs:127`)

트리 내 현재 위치를 나타내는 Zipper 커서. 트리에 대한 참조가 아니라 **트리의 대체 표현**이며, 생성 시 트리의 소유권을 가져간다. 모든 변형/이동 메서드는 `self`를 소비하고 새 커서를 반환한다(소유권 이동 기반 무복사 변형). 실패 시 변경되지 않은 커서를 `Err`로 되돌려준다.

조회 메서드(`&self`):
- `new() -> Self` — 빈 트리를 가리키는 커서(`:205`).
- `subtree(&self) -> &Tree<L, N>` — 현재 위치 서브트리 참조(`:213`).
- `is_leaf` / `is_left` / `is_right` / `is_top -> bool` — 현재 위치 판별(`:218`, `:223`, `:228`, `:232`).
- `path_to_root(&self) -> ParentIterator<'_, L, N>` — 현재 위치에서 루트까지 부모 체인을 순회하는 반복자(`:274`).

가변 참조 메서드(`&mut self`):
- `leaf_mut(&mut self) -> Option<&mut L>` — 리프일 때 리프 데이터의 가변 참조(`:254`).
- `node_mut(&mut self) -> Result<&mut Option<N>, ()>` — 노드일 때 노드 데이터 컨테이너의 가변 참조(`:264`).

소비형 변형/이동 메서드(`self -> Result<Self, Self>` 또는 변형):
- `assign_top(self, leaf: L)` — 빈 트리 루트에 첫 리프 부여(`:242`).
- `assign_node(self, value: Option<N>)` — 현재 노드의 데이터 설정(`:284`).
- `unsplit_leaf(self) -> Result<(Self, L, Option<N>), Self>` — 비루트 리프 제거 후 부모를 반대편 가지로 대체(unsplit), 제거된 리프 값과 부모 노드 값을 함께 반환(`:301`).
- `split_node_and_insert_left/right(self, to_insert: L)` — 현재 노드를 새 노드로 감싸 한쪽에 새 리프 삽입(`:324`, `:338`).
- `split_leaf_and_insert_left/right(self, value: L)` — 현재 리프를 두 리프를 가진 노드로 분할(`:382`, `:360`).
- `go_left/go_right(self)` — 자식으로 하강(`:402`, `:422`).
- `go_up(self)` — 부모로 상승하며 노드 재구성(`:442`).
- `preorder_next(self)` / `postorder_next(self)` — 전위/후위 순회 다음 위치(`:472`, `:506`).
- `go_to_nth_leaf(self, n: usize)` — 전위 순서 기준 n번째 리프로 이동(`:533`).
- `tree(self) -> Tree<L, N>` — 루트까지 상승하며 커서를 트리로 환원(`:547`).
- `impl Debug` — `L: Debug, N: Debug` 제약 하에 디버그 출력(`:132`).

### 3.3 `ParentIterator<'a, L, N>` (`bintree/src/lib.rs:145`)

`path_to_root`가 반환하는 반복자. `Item = (PathBranch, &'a Option<N>)`로, 현재 위치에서 루트 방향으로 한 단계씩 부모 노드의 분기 방향과 노드 데이터를 산출한다(`:155`).

### 3.4 `PathBranch` (`bintree/src/lib.rs:149`)

`IsLeft` / `IsRight` 두 변형의 열거형. `Debug, Clone, Copy, PartialEq, Eq` 파생. 자식이 부모의 어느 쪽에 위치하는지를 나타낸다.

## 4. 내부 구조

크레이트는 단일 파일(`bintree/src/lib.rs`, 약 698행)로 구성되며, 모듈 분해 없이 평면적이다. 비대한 모듈은 없다. 논리적으로 네 부분으로 나뉜다.

1. **트리 정의부** — `Tree` enum과 그 `PartialEq`/`Debug` 구현, 기본 조회/변환 메서드.
2. **경로(Path) 정의부** — 비공개 `Path` enum과 `Debug` 구현. Zipper의 핵심으로, 현재 위치에서 **루트를 향하는 역방향 경로**를 표현한다.
3. **커서 정의부** — `Cursor` 구조체와 변형/이동 메서드 전체. 이 부분이 크레이트 코드량의 대부분을 차지한다.
4. **테스트** — `#[cfg(test)] mod tests`. `split_and_split_and_iterate`(`:561`), `populate`(`:596`) 두 테스트가 분할·노드 데이터 부여·전위 순회를 검증한다.

**제어/데이터 흐름의 핵심**은 Zipper 패턴이다. `Cursor`는 두 부분을 보유한다: `it`(현재 초점이 맞춰진 서브트리)과 `path`(루트로 돌아가는 길에 있는 부분 구성된 부모 노드들의 스택). 하강(`go_left`/`go_right`) 시 현재 노드를 분해하여 한쪽 자식을 `it`에 넣고, 반대편 자식·노드 데이터·기존 경로를 `Path::Left`/`Path::Right`로 포장해 `path` 위에 쌓는다. 상승(`go_up`) 시 역연산으로 노드를 재조립한다. 이로써 트리 내 임의 위치에서의 변형이 참조나 인덱스 없이 **상수 시간 소유권 이동**으로 이뤄진다.

순회 메서드(`preorder_next`/`postorder_next`)는 "정상 이진 트리(좌/우가 항상 짝을 이룸)"라는 불변식을 활용해 한쪽 자식만 검사하는 식으로 구현된다. 두 메서드 모두 순회 종료 시 `Err`를 반환하지만, 이때 반환되는 커서는 위치가 이동된 상태이므로 `Err` 수신 후에는 반드시 순회를 중단해야 한다(주석 `:467`, `:501`에 명시).

## 5. 핵심 데이터 구조·타입

### `Tree<L, N>` (`bintree/src/lib.rs:16`)
- `Empty` — 비어 있음.
- `Leaf(L)` — 단말. 필수 리프 데이터 보유.
- `Node { left: Box<Self>, right: Box<Self>, data: Option<N> }` — 비단말. 항상 두 자식 보유.

**불변식**: 정상 이진 트리. `Node`는 항상 좌·우 자식을 모두 가진다(좌만 있거나 우만 있는 경우 없음). 트리 전체가 단일 `Leaf`로 루팅되는 것이 유일한 예외다. 이 불변식은 `split_*`/`unsplit_leaf` 연산이 항상 쌍 단위로 노드를 생성·소멸시키기 때문에 유지된다. `preorder_next`/`postorder_next`의 `unreachable!`·단순화된 분기는 이 불변식에 의존한다(`unsplit_leaf`의 `:318`~`:320` 참조).

### `Path<L, N>` (비공개, `bintree/src/lib.rs:73`)
- `Top` — 현재 위치가 루트.
- `Left { right, data, up }` — 현재 위치가 부모의 좌측 자식. `right`/`data`는 부모 `Node`의 부분 구성 상태.
- `Right { left, data, up }` — 현재 위치가 부모의 우측 자식.

**불변식**: `up`을 따라 거슬러 올라가면 반드시 `Top`에 도달한다(경로는 유한·비순환). `Path::Left`는 부모의 우측 가지를, `Path::Right`는 좌측 가지를 보관하므로, `go_up` 시 `it`과 결합하면 원래 `Node`가 정확히 복원된다.

### `Cursor<L, N>` (`bintree/src/lib.rs:127`)
- `it: Box<Tree<L, N>>` — 현재 초점 서브트리.
- `path: Box<Path<L, N>>` — 루트로의 역경로 스택.

**불변식**: `(it, path)`는 항상 하나의 완전한 트리를 분해 표현하며, `go_up`을 반복하면 손실 없이 원 트리로 환원된다(`tree()` 메서드가 이를 수행).

### `PathBranch` (`bintree/src/lib.rs:149`)
`IsLeft`/`IsRight`. 외부 소비자가 부모 분기 방향을 식별하는 데 사용된다(`mux`에서 분할 방향 계산에 활용).

## 6. 외부 의존성

**없음.** `Cargo.toml`의 `[dependencies]` 섹션이 비어 있다(`bintree/src/lib.rs` 대응 `bintree/Cargo.toml:10`). 표준 라이브러리의 `std::cmp::PartialEq`, `std::fmt::Debug`만 사용한다. 이는 의도된 설계로, 트리 알고리즘을 완전히 독립적인 순수 라이브러리로 격리한 것이다.

## 7. 설정·기능 플래그

feature flag 없음. `config` 크레이트와의 연동 없음. 컴파일 타임 분기나 런타임 설정 항목이 존재하지 않는다. 동작은 전적으로 제네릭 타입 매개변수(`L`, `N`)와 호출자의 메서드 호출 순서로 결정된다.

## 8. Windows 전용 고려사항

플랫폼 의존 코드, `cfg(windows)`/`cfg(unix)` 분기, Windows API 호출이 **전무하다**. 순수 `std` 기반 자료구조이므로 Windows 포크의 플랫폼 코드 제거 작업과 무관하다. 죽은 경로도 없다. 이 크레이트는 어떤 OS에서도 동일하게 컴파일·동작한다.

## 9. 리팩토링 주의점

- **소유권 이동 API의 전염성**: 대부분의 커서 메서드가 `self`를 소비하고 `Result<Self, Self>`를 반환한다. 시그니처를 `&mut self` 기반으로 바꾸려면 `mux/src/tab.rs`의 모든 호출부(`go_to_nth_leaf`/`preorder_next`/`split_*`/`unsplit_leaf` 등 다수, 섹션 2 참조)를 동시 수정해야 한다. 파급 범위는 `mux`에 한정되나 호출 지점이 많다.
- **순회 종료의 함정**: `preorder_next`/`postorder_next`는 `Err` 반환 후에도 위치가 이동되어 있고, 재호출 시 이미 방문한 노드를 다시 산출할 수 있다(주석 `:467`). 이 계약을 깨면 `mux`의 패널 순회가 무한 루프·중복 처리로 이어진다. `go_to_nth_leaf`(`:533`)는 이 위험을 내부적으로 흡수하지 않고 `preorder_next`의 `Err`를 그대로 전파하므로, n이 리프 수를 초과하면 `Err`를 반환한다.
- **정상 이진 트리 불변식 의존**: 순회 로직과 `unsplit_leaf`의 `unreachable!`(`:318`~`:320`)는 "노드가 항상 자식 2개"라는 불변식을 전제한다. 외부에서 `Tree::Node`를 직접 구성하여 한쪽만 채우는 변형을 추가하면 이 가정이 깨지고 패닉이 발생할 수 있다. `mux/src/tab.rs:299`, `:2128`에서 직접 `Tree::Node`를 구성하므로(직렬화 복원 경로) 이 경로가 불변식을 위반하지 않도록 유지해야 한다.
- **`node_mut`의 `Result<_, ()>`**: 단위 타입 에러를 반환하는 비관용적 시그니처다(`#[allow(clippy::result_unit_err)]`로 경고 억제, `:263`). `Option`으로 단순화 가능하나 호출부 변경이 필요하다.
- **재귀 `num_leaves`**: `Tree::num_leaves`(`:193`)는 재귀 구현이다. 패널 트리 깊이는 실사용상 얕아 스택 위험은 낮으나, 매우 깊은 트리에서는 고려 대상이다.
- **순환 의존 없음**: 외부 의존이 0이므로 순환 위험이 원천적으로 없다.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|------|-----------|------|
| `bintree/Cargo.toml` | 11 | 패키지 메타데이터. 의존성 없음, `publish = false`. |
| `bintree/src/lib.rs` | 698 | 크레이트 전체. `Tree`/`Cursor`/`Path`/`ParentIterator`/`PathBranch` 정의, 모든 변형·이동·순회 메서드, 단위 테스트 2건 포함. |

생성 파일 없음.
