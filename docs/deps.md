# deps 폴더 기능 명세

본 문서는 워크스페이스 루트의 `deps` 폴더에 포함된 네 개 크레이트(`cairo-sys-rs`, `fontconfig`, `freetype`, `harfbuzz`)의 기능 명세다. 네 크레이트는 모두 **C/C++ 라이브러리에 대한 FFI 바인딩**이며, 그중 셋(`cairo-sys-rs`, `freetype`, `harfbuzz`)은 C/C++ 소스를 벤더링하여 `build.rs`에서 정적 라이브러리로 직접 컴파일한다. 본 명세는 코드(`Cargo.toml`, `build.rs`, `src/**/*.rs`)를 근거로 작성했다.

본 저장소는 WezTerm의 Windows 전용 영구 분기다. `build.rs`·소스에 남아 있는 `target.contains("windows")` 비-Windows 분기 및 `cfg(unix)`/`cfg(feature = "xlib")` 등은 대부분 죽은 경로다. 본 명세는 각 절에서 이를 명시한다.

---

## 1. 개요 및 책임

`deps` 폴더는 폰트 렌더링 파이프라인이 의존하는 네이티브 라이브러리를 **워크스페이스 내부에서 자급(vendoring)** 하기 위해 존재한다. 시스템에 설치된 라이브러리에 의존하지 않고, C/C++ 소스를 git submodule로 동봉한 뒤 `cc` 크레이트로 정적 컴파일하여 빌드 재현성과 이식성을 확보하는 것이 단일 책임이다.

크레이트별 책임은 다음과 같다.

- `cairo-sys-rs` (`deps/cairo/`): 2D 벡터 그래픽 라이브러리 cairo와 그 의존 라이브러리 pixman의 FFI 바인딩 및 정적 빌드. cairo를 통해 COLR/벡터 글리프를 래스터화한다.
- `freetype` (`deps/freetype/`): 폰트 래스터라이저 FreeType2와 그 의존(zlib, libpng)의 FFI 바인딩 및 정적 빌드. 글리프 로딩·힌팅·래스터화의 기반이다.
- `harfbuzz` (`deps/harfbuzz/`): 텍스트 셰이핑 엔진 HarfBuzz의 FFI 바인딩 및 정적 빌드. 유니코드 텍스트를 글리프 시퀀스와 위치로 변환한다. FreeType 연동(`HAVE_FREETYPE`)을 켠 상태로 빌드된다.
- `fontconfig` (`deps/fontconfig/`): 폰트 탐색 라이브러리 fontconfig의 FFI 바인딩. **벤더링하지 않고** 시스템 라이브러리를 `pkg-config`로 찾는다. Windows fork에서는 실질적으로 죽은 크레이트다(9절 참조).

세 벤더링 크레이트의 핵심 가치는 "소스 동봉 + `cc` 정적 빌드"이며, FFI 선언 자체는 대부분 `rust-bindgen` 또는 `gtk-rs`에서 가져온 기계 생성물이다. 따라서 이 폴더의 본질적 책임은 *바인딩 작성*이 아니라 *벤더 빌드 오케스트레이션*에 있다.

---

## 2. 워크스페이스 내 위치

| 크레이트 | 경로 | 의존(deps) | 피의존(usedBy) | 워크스페이스 멤버 |
|---|---|---|---|---|
| `cairo-sys-rs` | `deps/cairo/` | `libc`, (빌드) `cc` | `cairo-rs`(외부 크레이트) ← `[patch.crates-io]` 경유 | 예 (`Cargo.toml:4`) |
| `fontconfig` | `deps/fontconfig/` | `libc`, (빌드) `pkg-config` | 없음(미사용) | 아니오 |
| `freetype` | `deps/freetype/` | `fixed`, (빌드) `cc` | `harfbuzz`, `wezterm-font` | 아니오(경로 의존) |
| `harfbuzz` | `deps/harfbuzz/` | `freetype`, (빌드) `cc` | `wezterm-font` | 아니오(경로 의존) |

파이프라인상 위치: 이들은 폰트 스택의 **최하층(네이티브 FFI 계층)** 이다. 상위 크레이트 `wezterm-font`가 `ftwrap.rs`(FreeType)·`hbwrap.rs`(HarfBuzz)에서 안전 래퍼를 씌워 사용한다. `harfbuzz`는 빌드 시 `freetype`이 export한 include/lib 경로를 받아 FreeType 연동 객체를 컴파일하므로, `freetype → harfbuzz` 빌드 순서 의존이 존재한다.

`cairo-sys-rs`의 피의존 관계는 직접적이지 않다. 작업 지시서의 `usedBy: 없음`과 달리, 실제로는 워크스페이스 루트 `Cargo.toml:228-231`의 `[patch.crates-io]`가 crates.io의 `cairo-sys-rs`를 `deps/cairo`로 치환한다. 따라서 `wezterm-font`가 사용하는 안전 크레이트 `cairo-rs`(`cairo-rs.workspace = true`, `wezterm-font/Cargo.toml:18`)는 내부적으로 이 벤더 `cairo-sys-rs`를 거친다. 결과적으로 `deps/cairo`는 cairo 렌더링 경로(`wezterm-font/src/rasterizer/{harfbuzz,freetype,colr}.rs`)를 통해 **간접적으로 사용된다**.

---

## 3. 공개 API 표면

네 크레이트 모두 라이브러리 크레이트이며, 공개 표면은 대부분 `extern "C"` 함수, `#[repr(C)]` 타입, 상수다. 안전 추상화(`struct` 래퍼, RAII)는 제공하지 않는다. 그 책임은 상위 `wezterm-font`(또는 `cairo-rs`)에 있다.

### 3.1 `cairo-sys-rs` (`deps/cairo/src/lib.rs`, 1841행)
- 라이브러리명 재지정: `[lib] name = "cairo_sys"` (`deps/cairo/Cargo.toml:14-15`). 따라서 사용 측에서는 `cairo_sys` 경로로 참조한다.
- 불투명 핸들 타입: `opaque!` 매크로(`lib.rs:88-105`)로 생성한 `cairo_t`, `cairo_surface_t`, `cairo_device_t`, `cairo_pattern_t`, `cairo_region_t`, `cairo_font_face_t`, `cairo_scaled_font_t`, `cairo_font_options_t`. 각각 `_data: [u8; 0]` + `PhantomData<(*mut u8, PhantomPinned)>`로 정의된 영(zero)-크기 불투명 구조체다.
- 값 타입: `cairo_rectangle_t`, `cairo_rectangle_int_t`, `cairo_matrix_t`, `cairo_glyph_t`, `cairo_text_cluster_t`, `cairo_text_extents_t`, `cairo_font_extents_t`, `cairo_path_t`/`cairo_path_data`(union), `cairo_user_data_key_t`, `cairo_bool_t` 등.
- 열거형 별칭: `cairo_status_t`, `cairo_format_t`, `cairo_operator_t`, `cairo_antialias_t`, `cairo_extend_t`, `cairo_filter_t`, `cairo_pattern_type_t` 등 다수를 `c_int`/`c_uint` 별칭으로 노출.
- 함수: `cairo_create`, `cairo_paint`, `cairo_set_source_*`, `cairo_pattern_create_*`(linear/radial/mesh), `cairo_region_*`, `cairo_matrix_*`, `cairo_scaled_font_*`, `cairo_surface_*` 등 수백 개. `cairo_ft_*`(FreeType 연동)는 `feature = "freetype"` 게이트.
- `pub mod gobject`(`deps/cairo/src/gobject.rs`, 37행): cairo GObject 타입 등록 함수(`cairo_gobject_*_get_type`) 선언. `glib::GType`를 반환하므로 `glib` 크레이트가 스코프에 있어야 한다.

### 3.2 `fontconfig` (`deps/fontconfig/src/lib.rs`, 746행)
- 정수형 별칭: `FcChar8`/`FcChar16`/`FcChar32`/`FcBool`.
- 열거형 상수군: `FcType*`, `FcResult*`, `FcMatchKind`, `FcLangResult`, `FcSetName`, `FcEndian` 및 `FC_WEIGHT_*`/`FC_SLANT_*`/`FC_WIDTH_*` 정수 상수.
- 구조체: `FcMatrix`, `FcObjectType`, `FcConstant`, `FcValue`, `FcFontSet`, `FcObjectSet`. `FcPattern`/`FcCharSet`/`FcConfig`/`FcLangSet` 등은 `c_void` 별칭(불투명).
- 함수: `FcInit`, `FcInitLoadConfigAndFonts`, `FcFontMatch`, `FcFontSort`, `FcPatternCreate`/`FcPatternAdd*`/`FcPatternGet*`, `FcConfigSubstitute`, `FcCharSet*`, `FcNameParse`/`FcNameUnparse` 등 전 영역. 일부는 C 가변인자(`...`)를 사용한다(`FcObjectSetBuild`, `FcPatternBuild`).

### 3.3 `freetype` (`deps/freetype/src/lib.rs`, 3011행)
- `rust-bindgen 0.71.1` 생성물(`lib.rs:1`). FreeType2 전 영역의 상수(`FT_LOAD_*`, `FT_FACE_FLAG_*`, `TT_*`), 구조체(`FT_FaceRec_`, `FT_GlyphSlotRec_`, `FT_Bitmap`, `FT_Outline` 등), 포인터 별칭(`FT_Face = *mut FT_FaceRec_`, `FT_Library`), 함수(`FT_Init_FreeType`, `FT_New_Face`, `FT_Load_Glyph`, `FT_Render_Glyph`, …, `lib.rs:1381` 이하 `extern "C"` 블록 다수, 약 103개)를 노출.
- 정수 타입 `FT_Int16`/`FT_UInt16`/`FT_Int32`/`FT_Int64` 등을 직접 정의(`lib.rs:11-16`).
- `pub use fixed_point::*`(`lib.rs:10`): 고정소수점 타입을 별도 모듈로 덮어쓴다(아래 4·5절).
- `__BindgenUnionField<T>`(`lib.rs:18-66`): bindgen이 생성한 union 접근 헬퍼. `as_ref`/`as_mut`는 `unsafe`이며, 한국어 `# Safety` 주석이 부가되어 있다(본 fork의 수정).

### 3.4 `harfbuzz` (`deps/harfbuzz/src/lib.rs`, 4739행)
- `rust-bindgen 0.71.1` 생성물(`lib.rs:1`). 순수 FFI이며 Rust 측 래퍼는 없다(파일 말미까지 `unsafe extern "C"` 블록과 `#[repr]` 타입만 존재).
- 핵심 타입: `hb_codepoint_t`, `hb_tag_t`, `hb_position_t`, `hb_mask_t`, `hb_var_int_t`/`hb_var_num_t`(union), 불투명 핸들 `hb_font_t`/`hb_face_t`/`hb_blob_t`/`hb_buffer_t`/`hb_set_t`/`hb_map_t`/`hb_unicode_funcs_t`/`hb_draw_funcs_t`.
- 열거형: `hb_direction_t`, `hb_script_t`(ISO 15924 4글자 태그를 `u32`로 인코딩한 거대 열거형, `lib.rs:88-265`), `hb_memory_mode_t`, `hb_unicode_general_category_t`, `hb_unicode_combining_class_t`, `hb_ot_math_*`, `hb_ot_metrics_tag_t`, `hb_ot_var_axis_flags_t` 등.
- 함수: blob/face/font 생성·소멸, `hb_shape`, `hb_buffer_*`, `hb_set_*`/`hb_map_*`, draw 콜백, OpenType 보조 API(`hb_ot_*`: math/meta/metrics/var) 전 영역.
- 콜백 타입(`Option<unsafe extern "C" fn ...>`) 다수: `hb_destroy_func_t`, `hb_draw_move_to_func_t`, `hb_unicode_*_func_t`, `hb_reference_table_func_t` 등.

---

## 4. 내부 구조

### 4.1 모듈 분해
- `cairo-sys-rs`: 단일 거대 모듈 `lib.rs`(1841행) + 보조 모듈 `gobject`(37행). 타입 별칭 → `opaque!` 핸들 → `#[repr(C)]` 값 타입 → 콜백 타입 → 단일 `extern "C"` 블록 순으로 선형 배치. `winapi` 모듈은 `win32-surface` feature/docsrs 조건부.
- `fontconfig`: 단일 모듈 `lib.rs`(746행). 상수·타입 → 단일 `extern "C"` 블록.
- `freetype`: `lib.rs`(생성, 3011행) + `mod types`(8행) + `mod fixed_point`(84행, 수작업). `lib.rs`가 `types`를 사용하고 `fixed_point`를 `pub use`로 재노출한다.
- `harfbuzz`: 단일 생성 모듈 `lib.rs`(4739행). 다수의 작은 `unsafe extern "C"` 블록으로 분할되어 있으나 논리적으로는 평면 구조.

### 4.2 빌드 제어/데이터 흐름 (핵심)
빌드 스크립트가 이 폴더의 실질적 로직을 담는다.

- `freetype/build.rs`: `zlib()` → `libpng()` → `freetype()` 순으로 `cc::Build`를 세 번 구성·컴파일한다(`build.rs:240-247`). git submodule(`zlib/`, `libpng/`, `freetype2/`)이 없으면 `git submodule update --init`를 시도한다(`build.rs:234-238`). FreeType 빌드는 `ftoption.h`를 런타임에 문자열 치환하여(`build.rs:134-163`) 에러 문자열·시스템 zlib·PNG·서브픽셀 힌팅(레벨 3)·서브픽셀 렌더링을 활성화한다. 완료 후 `cargo:include=…`/`cargo:lib=…`(`build.rs:226-231`)를 출력하여 하위 `harfbuzz/build.rs`로 경로를 전달한다.
- `harfbuzz/build.rs`: C++(`cpp(true)`, `-std=c++11`)로 `harfbuzz/src/harfbuzz.cc` 단일 통합 소스를 컴파일한다. `HB_NO_MT`(무스레드), `HAVE_FREETYPE` 및 FreeType 변형(variable font) 함수 존재 매크로를 정의한다(`build.rs:38-43`). `DEP_FREETYPE_INCLUDE`/`DEP_FREETYPE_LIB` 환경변수(상위 freetype `links` 메커니즘으로 주입)를 읽어 include 경로와 링크 경로를 설정하고 `freetype`/`png`/`z`를 링크한다(`build.rs:46-56`).
- `cairo/build.rs`: `pixman()` → `cairo()` 순으로 컴파일한다(`build.rs:212-215`). cairo 소스 파일 목록은 meson 빌드에서 추출한 것이며(`build.rs:61-68` 주석), `CAIRO_NO_MUTEX`·`PIXMAN_NO_TLS`를 정의해 스레드 기능을 비활성화한다. 포인터 폭은 `CARGO_CFG_TARGET_POINTER_WIDTH`에서 `SIZE_VOID_P`로 환산한다(`build.rs:199-205`). `cairo-tee-surface.c`(빌드 실패), `strndup.c`(심볼 충돌)는 의도적으로 제외(`build.rs:169`, `build.rs:188-190`).
- `fontconfig/build.rs`: 컴파일하지 않는다. `pkg-config`로 시스템 fontconfig(≥2.10.1)를 찾아 링크 정보를 출력하며, 못 찾아도 패닉하지 않고 조용히 통과한다(`build.rs:21-26`).

---

## 5. 핵심 데이터 구조·타입

대부분의 타입은 C ABI를 그대로 반영한 `#[repr(C)]`·`#[repr(transparent)]`·`#[repr(u32)]` 정의이며, 불변식은 "C 라이브러리의 레이아웃과 정확히 일치"라는 한 가지로 귀결된다. 별도 Rust 측 불변식을 부과하는 것은 다음 둘뿐이다.

### 5.1 `cairo_bool_t` (`deps/cairo/src/lib.rs:248-265`)
`#[repr(transparent)]`로 `c_int`를 감싼다. 불변식: 0이 거짓, 그 외가 참. `as_bool(self) -> bool`과 `From<bool>`로 변환을 제공한다.

### 5.2 freetype 고정소수점 타입 (`deps/freetype/src/fixed_point.rs`)
이 폴더에서 유일하게 의미 있는 수작업 로직이다. FreeType는 분수를 정수 기반 고정소수점(F2Dot14, F26Dot6, F16Dot16)으로 표현하므로, 무가공 정수를 다루다가 스케일을 혼동하기 쉽다. 이를 방지하기 위해 `fixed` 크레이트의 타입으로 치환한다.

- `SelectFixedStorage<T>` 트레이트: `c_long`/`c_short` 같은 플랫폼 의존 정수를 동일 크기의 `fixed::FixedIXX<T>`로 해소한다(`fixed_point.rs:25-40`). 불변식: 스토리지 정수 폭과 `FixedIXX` 폭이 일치해야 한다.
- 별칭: `FT_F2Dot14`(U14), `FT_F26Dot6`(U6), `FT_Fixed`(U16)를 위 트레이트로 결정(`fixed_point.rs:42-44`). 이들이 `lib.rs`의 bindgen 정수 별칭을 `pub use fixed_point::*`로 덮어쓴다.
- `FT_Pos`(`fixed_point.rs:48-72`): `#[repr(transparent)]`로 `FT_Pos`(=`c_long`) 정수를 감싼 벡터 좌표. 불변식: 내부 비트는 "문맥에 따라 폰트 유닛 또는 16.16/26.6 픽셀 좌표"이며, 어떤 해석인지는 호출 문맥이 안다. `font_units()`/`f16d16()`/`f26d6()`로 명시적으로 해석을 선택하게 하여 오용을 줄인다. `From<FT_F26Dot6>`/`From<FT_Fixed>`로 비트 보존 변환을 제공한다.

### 5.3 union 타입
`cairo_path_data`(cairo), `_hb_var_int_t`/`_hb_var_num_t`(harfbuzz), bindgen `__BindgenUnionField`(freetype)는 모두 활성 멤버를 타입 시스템이 추적하지 못한다. 불변식: 읽는 측이 어떤 멤버가 유효한지 보장해야 하며, 위반 시 미정의 동작이다.

---

## 6. 외부 의존성

| 크레이트 | 외부 의존 | 종류 | 사용 이유 |
|---|---|---|---|
| `cairo-sys-rs` | `libc` | 런타임 | C 기본 타입(`c_int`/`c_double` 등) |
| | `cc` | 빌드 | pixman+cairo C 소스 정적 컴파일 |
| | `glib` | (gobject 모듈 한정) | `GType` 참조. 해당 모듈을 쓸 때만 필요 |
| `fontconfig` | `libc` | 런타임 | C 기본 타입 |
| | `pkg-config` | 빌드 | 시스템 fontconfig 탐색 |
| `freetype` | `fixed` | 런타임 | 고정소수점 타입 안전 표현(`fixed_point.rs`) |
| | `cc` | 빌드 | zlib+libpng+freetype2 C 소스 정적 컴파일 |
| `harfbuzz` | `freetype` | 런타임+빌드 | FFI 타입 공유 및 include/lib 경로 전달, FreeType 연동 셰이핑 |
| | `cc` | 빌드 | HarfBuzz C++ 소스 정적 컴파일 |

벤더링된 네이티브 라이브러리(외부 의존이지만 submodule로 동봉): cairo, pixman(cairo), FreeType2·zlib·libpng(freetype), HarfBuzz(harfbuzz). zlib/libpng는 FreeType의 PNG 임베디드 비트맵(컬러 이모지 등) 디코딩을 위해 함께 빌드된다.

---

## 7. 설정·기능 플래그

- `cairo-sys-rs` (`deps/cairo/Cargo.toml:17-29`): `v1_16`/`v1_18`(API 버전 게이트), `png`/`pdf`/`svg`/`ps`/`script`(서피스 백엔드), `win32-surface`, `xlib`/`xcb`(X11 — Windows fork에서 죽은 경로), `freetype`(`cairo_ft_*` 노출), `use_glib`(gobject 연동). 워크스페이스에서는 `cairo-rs`가 선택하는 feature 집합에 따라 활성화되며, 본 패치 빌드는 `default-features=false`로 시작한다(`Cargo.toml:46`).
- `fontconfig`/`freetype`/`harfbuzz`: Cargo feature 없음. `publish = false`. 빌드 동작은 feature가 아니라 `build.rs` 내부의 C 전처리기 매크로(예: `FT_CONFIG_OPTION_*`, `HB_NO_MT`, `HAVE_FREETYPE`)로 고정된다.
- `links` 키: `fontconfig`(`= "fontconfig"`), `freetype`(`= "freetype"`), `harfbuzz`(`= "harfbuzz"`). 동일 네이티브 라이브러리 중복 링크를 Cargo가 방지하고, `cargo:` 메타데이터를 `DEP_<LINKS>_<KEY>` 환경변수로 의존 크레이트에 전달하는 데 쓰인다(freetype→harfbuzz 경로 전달의 토대).
- config 크레이트와의 직접 연동은 없다. 이 계층은 설정값을 모른다.

---

## 8. Windows 전용 고려사항

- `freetype/build.rs:216-220`: Windows에서는 `freetype2/builds/windows/ftdebug.c`를, 그 외에서는 `src/base/ftdebug.c`를 빌드한다. 이 분기는 실제로 사용되는 유일한 플랫폼 분기다.
- `freetype/build.rs:43-45`, `:99-101`: `_LARGEFILE64_SOURCE`는 비-Windows에서만 정의. Windows에서는 비활성(죽은 분기의 반대편).
- `harfbuzz/build.rs:32-35`: `HAVE_UNISTD_H`/`HAVE_SYS_MMAN_H`를 비-Windows에서만 정의. Windows 빌드에서는 정의되지 않는다.
- `cairo-sys-rs/src/lib.rs:14-21`: `cfg(all(windows, feature = "win32-surface"))`에서 `winapi`의 `HDC`를 재노출하는 `winapi` 모듈. cairo의 Win32 서피스 백엔드용이나, 본 빌드는 cairo를 RecordingSurface/ImageSurface 중심으로 사용하므로 실사용 여부는 상위 cairo-rs feature에 달려 있다.
- `cairo-sys-rs`의 `xlib`/`xcb` feature 및 그에 딸린 `x11`/`xcb_*` 타입(`lib.rs:11-12`, `:112-142`)은 X11 백엔드용으로 Windows fork에서 죽은 경로다.
- `fontconfig`는 사실상 죽은 크레이트다. fontconfig는 Linux 폰트 탐색 라이브러리이고 Windows에는 보통 부재하므로 `build.rs`는 조용히 no-op이 된다. 워크스페이스 어디에서도 참조되지 않는다(2절·9절).
- 빌드 환경: 세 벤더 크레이트는 C/C++ 컴파일러(MSVC vcvars)가 필요하다. FreeType의 PNG 경로가 zlib을 요구하므로 OpenSSL과 무관하게 zlib이 동봉된다. 저장소 빌드 전반에 Strawberry Perl + MSVC vcvars가 필요하다는 제약(openssl vendored)은 이 폴더 자체보다 워크스페이스의 다른 의존에서 비롯되나, 이 폴더의 `cc` 빌드도 동일한 MSVC 툴체인을 전제한다.

---

## 9. 리팩토링 주의점

- **`cairo-sys-rs`의 patch 결합**: `deps/cairo`는 워크스페이스 멤버이면서 `[patch.crates-io]`(`Cargo.toml:228-231`)로 crates.io의 `cairo-sys-rs@0.18.0`을 치환한다. 이 때문에 `[lib] name = "cairo_sys"`와 패키지 `version="0.18.0"`은 임의로 변경하면 안 된다. 버전을 올리면 `cairo-rs`가 요구하는 sys 버전과 어긋나 patch가 무효화된다. "사용처 없음"으로 오판하여 제거하면 cairo 렌더링 경로(COLR/벡터 글리프)가 깨진다.
- **freetype→harfbuzz 빌드 순서 의존**: `harfbuzz/build.rs`는 `DEP_FREETYPE_INCLUDE`/`DEP_FREETYPE_LIB`(freetype의 `cargo:include`/`cargo:lib` 출력에서 파생)를 `unwrap()`으로 강하게 가정한다(`build.rs:46`, `:52`). freetype `build.rs`의 출력 키 이름이나 `links="freetype"`을 바꾸면 harfbuzz 빌드가 패닉한다. 두 `build.rs`는 사실상 한 쌍으로 다뤄야 한다.
- **생성 코드 vs 수작업 코드 구분**: `freetype/src/lib.rs`·`harfbuzz/src/lib.rs`·`freetype/src/types.rs`는 `rust-bindgen` 생성물이다. 직접 편집하면 재생성 시 소실된다. 본 fork가 의도적으로 가한 수정은 (a) `__BindgenUnionField`의 한국어 `# Safety` 주석, (b) `freetype/src/fixed_point.rs`와 `lib.rs:10`의 `pub use`로 정수 별칭을 고정소수점 타입으로 덮어쓴 것뿐이다. 헤더 변경으로 재생성할 경우 이 두 가지를 다시 적용해야 한다.
- **`#[repr(C)]` 레이아웃 불변식**: 모든 구조체·열거형은 C ABI와 바이트 단위로 일치해야 한다. 필드 추가/순서 변경/정수 폭 변경은 즉시 미정의 동작을 유발한다. 특히 `cairo_matrix_t`의 필드 순서(`xx, yx, xy, yy, x0, y0` — `lib.rs:226-235`)는 cairo의 비직관적 순서를 따르므로 손대지 말 것.
- **fontconfig 제거 후보**: 워크스페이스 어디에서도 참조되지 않고 Windows에서 빌드 효과도 없다. 미사용 코드 정리 정책(MEMORY 참조)에 따라 제거 후보다. 단, `links="fontconfig"`/`build.rs`/`src/lib.rs` 세 파일을 함께 제거하고 `cairo`의 `freetype`/fontconfig 연동 feature가 이를 끌어들이지 않는지 확인해야 한다.
- **순환 의존 없음**: `freetype → harfbuzz`는 단방향, `cairo-sys-rs`는 독립이다. 순환은 존재하지 않는다.
- **벤더 submodule 무결성**: 세 빌드 스크립트는 submodule 부재 시 `git submodule update --init`를 호출한다(`freetype/build.rs:234-238`, `harfbuzz/build.rs:61-65`, cairo는 submodule을 직접 호출하지 않음). 오프라인/CI 환경에서 submodule이 비면 빌드가 조용히 실패할 수 있으므로, submodule 체크아웃을 빌드 전제로 명문화하는 편이 안전하다.

---

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
|---|---|---|
| `deps/cairo/Cargo.toml` | 36 | cairo-sys-rs 패키지 정의, feature 목록, `[lib] name = "cairo_sys"` |
| `deps/cairo/build.rs` | 215 | pixman + cairo C 소스 정적 컴파일(meson에서 추출한 소스 목록) |
| `deps/cairo/src/lib.rs` | 1841 | cairo FFI 바인딩(타입·핸들·함수). gtk-rs 유래 [생성/수작업 혼합] |
| `deps/cairo/src/gobject.rs` | 37 | cairo GObject 타입 등록 함수 선언(`glib::GType` 반환) |
| `deps/fontconfig/Cargo.toml` | 14 | fontconfig 패키지 정의, `links="fontconfig"` |
| `deps/fontconfig/build.rs` | 27 | pkg-config로 시스템 fontconfig 탐색(실패 시 무시) |
| `deps/fontconfig/src/lib.rs` | 746 | fontconfig FFI 바인딩(Servo 유래). 워크스페이스 미사용 |
| `deps/freetype/Cargo.toml` | 14 | freetype 패키지 정의, `fixed` 의존, `links="freetype"` |
| `deps/freetype/build.rs` | 247 | zlib+libpng+freetype2 정적 컴파일, ftoption.h 치환, include/lib export |
| `deps/freetype/src/lib.rs` | 3011 | FreeType2 FFI 바인딩 [생성: rust-bindgen 0.71.1] |
| `deps/freetype/src/types.rs` | 8 | 고정소수점 스토리지 정수 별칭 [생성: rust-bindgen 0.66.1] |
| `deps/freetype/src/fixed_point.rs` | 84 | 고정소수점 타입 안전 래퍼(`FT_Pos`, `SelectFixedStorage`) [수작업] |
| `deps/harfbuzz/Cargo.toml` | 14 | harfbuzz 패키지 정의, `freetype` 의존, `links="harfbuzz"` |
| `deps/harfbuzz/build.rs` | 72 | HarfBuzz C++ 통합 소스 컴파일, freetype 경로 수입·링크 |
| `deps/harfbuzz/src/lib.rs` | 4739 | HarfBuzz FFI 바인딩(순수 FFI, 래퍼 없음) [생성: rust-bindgen 0.71.1] |

비고: `deps/harfbuzz/harfbuzz/` 하위(`src/rust/`, `src/wasm/`)는 벤더링된 HarfBuzz 업스트림 소스 트리에 포함된 별도 크레이트(`harfbuzz-wasm`, `hello-wasm` 등)로, 본 워크스페이스 빌드 대상이 아니다(`build.rs`는 `harfbuzz/src/harfbuzz.cc`만 컴파일). 본 명세 대상에서 제외한다.
