# wezterm-font 폴더 기능 명세

## 1. 개요 및 책임

`wezterm-font` 크레이트는 WezTerm의 **폰트 파이프라인 전체**를 담당한다. 단일 책임은 다음 문장으로 요약된다: "사용자가 지정한 텍스트 스타일을 실제 폰트 파일로 해결(resolve)하고, 입력 문자열을 글리프 시퀀스로 셰이핑(shape)하며, 각 글리프를 픽셀 비트맵으로 래스터화(rasterize)한다."

이 크레이트는 세 단계의 폰트 처리를 통합한다.

1. **로케이션·해결(location/resolution)**: 텍스트 스타일(`TextStyle`)과 폰트 속성(`FontAttributes`)을 받아 시스템·디렉터리·내장 폰트 중 최적 매칭을 찾는다. CSS Fonts Level 3 매칭 알고리즘을 사용한다.
2. **셰이핑(shaping)**: HarfBuzz로 텍스트를 글리프 인덱스와 위치 정보(`GlyphInfo`)로 변환한다. 리거처·결합 그래핌·셀 폭 분배를 처리하며, 폴백 폰트를 재귀적으로 탐색한다.
3. **래스터화(rasterization)**: FreeType 또는 HarfBuzz로 글리프 인덱스를 premultiplied RGBA 비트맵(`RasterizedGlyph`)으로 변환한다. COLRv0/v1, SVG, 비트맵 스트라이크, 그라디언트를 cairo로 처리한다.

Windows fork에서의 실제 책임은 좁혀져 있다. 폰트 로케이터는 사실상 GDI/DirectWrite 단일 경로(`locator/gdi.rs`)만 동작하며, 비Windows 로케이터(FontConfig, CoreText)는 `new_locator`에서 `panic!`으로 막혀 있는 죽은 분기다. 셰이퍼는 HarfBuzz 단일 구현이며, Allsorts 셰이퍼는 제거되어 선택 시 오류를 반환한다.

설정 철학에 부합하게, 이 크레이트는 `config` 크레이트의 값을 읽어 동작을 결정하므로, `wezterm.lua` 대신 `config` 기본값을 수정하는 방식으로 폰트 동작을 고정할 수 있다.

## 2. 워크스페이스 내 위치

| 구분 | 크레이트 | 용도 |
| --- | --- | --- |
| 의존(deps) | `config` | 폰트 속성·스타일·로케이터/셰이퍼/래스터라이저 선택·FreeType 로드 플래그 등 모든 설정 항목 제공 |
| 의존 | `freetype` | 폰트 파싱·메트릭·글리프 래스터화·COLR/팔레트 API (FFI) |
| 의존 | `harfbuzz` | 텍스트 셰이핑·COLRv1 paint API (FFI) |
| 의존 | `cairo-rs`(`cairo`) | COLR/그라디언트/이미지 글리프의 벡터 합성 렌더링 |
| 의존 | `rangeset` | 코드포인트 커버리지 집합 연산 |
| 의존 | `termwiz` | `Presentation`(Text/Emoji), `CellCluster`, 유니코드 셀 폭 계산 |
| 의존 | `wezterm-bidi` | 셰이핑 방향(`Direction` LTR/RTL) |
| 의존 | `wezterm-color-types` | `SrgbaPixel`/`SrgbaTuple`, 선형↔sRGB 감마 변환 |
| 의존 | `wezterm-input-types` | `PixelUnit`(픽셀 길이 타입의 단위 마커) |
| 의존 | `wezterm-term` | `CellAttributes`, `Intensity`, font_rules 매칭용 색상 속성 |
| 의존 | `wezterm-toast-notification` | 누락 글리프 경고를 토스트 알림으로 표시 |
| 의존 | `image`, `memmap2`, `encoding_rs`, `finl_unicode`, `walkdir`, `ordered-float`, `euclid`, `metrics`, `lazy_static` | 보조 (이미지 디코딩, mmap, SFNT 이름 인코딩 디코드, 그래핌 분할, 디렉터리 순회, 메트릭 키, 픽셀 길이, 계측, 정적 초기화) |
| 의존(Windows 전용) | `dwrote`, `winapi` | DirectWrite 폴백·열거, GDI 폰트 데이터 추출 |
| 피의존(usedBy) | `wezterm-gui` | 글리프 캐시·렌더링이 `FontConfiguration`/`LoadedFont`/`RasterizedGlyph`를 소비 |
| 피의존 | `window` | 창 프레임 폰트 등에서 사용 |

이 크레이트는 GUI 렌더링 계층의 **바로 아래, 폰트 백엔드의 바로 위**에 위치한다. 즉 `config`(상위 설정)와 `freetype`/`harfbuzz`/`cairo`(하위 네이티브 백엔드) 사이의 어댑터 계층이며, `wezterm-gui`의 글리프 캐시가 유일하게 의미 있는 소비처다.

## 3. 공개 API 표면

### 진입점 — `lib.rs`

- `FontConfiguration`: 외부에 노출되는 최상위 핸들. 내부적으로 `Rc<FontConfigInner>`를 감싼다. 주요 메서드:
  - `new(config: Option<ConfigHandle>, dpi: usize) -> anyhow::Result<Self>`
  - `resolve_font(&self, style: &TextStyle) -> anyhow::Result<Rc<LoadedFont>>`: 스타일→폰트 해결(캐시됨)
  - `default_font()`, `default_font_metrics()`, `title_font()`, `command_palette_font()`, `pane_select_font()`, `char_select_font()`: 용도별 폰트 획득(각각 캐시)
  - `change_scaling(font_scale, dpi)`, `config_changed(config)`: 스케일/설정 변경 시 캐시 무효화
  - `match_style(config, attrs) -> &TextStyle`: `font_rules`를 셀 속성에 적용해 스타일 선택
  - `list_fonts_in_font_dirs()`, `list_system_fonts()`: 폰트 목록 조회
- `LoadedFont`: 해결된 폰트 1개(폴백 핸들 목록 포함)와 셰이퍼·래스터라이저 캐시를 보유. 주요 메서드:
  - `shape(text, completion, filter_out_synthetic, presentation, direction, range, presentation_width) -> Vec<GlyphInfo>`: 비동기 폴백 해결을 트리거하는 셰이핑
  - `blocking_shape(...)`: 폴백 해결을 동기적으로 완료할 때까지 반복하는 셰이핑
  - `rasterize_glyph(glyph_pos, fallback) -> RasterizedGlyph`
  - `metrics()`, `metrics_for_idx(font_idx)`, `brightness_adjust(font_idx)`, `clone_handles()`, `id()`, `style()`
- `alloc_font_id() -> LoadedFontId`(=`usize`): 전역 원자 카운터로 폰트 ID 발급
- `ClearShapeCache`: 폴백이 재계산될 때 셰이핑을 재시작하라고 알리는 오류 타입
- 재노출: `RasterizedGlyph`, `FallbackIdx`, `FontMetrics`, `GlyphInfo`

### `parser` 모듈

- `ParsedFont`: 파싱된 폰트 1개의 모든 메타데이터(이름·weight·stretch·style·커버리지·합성 플래그 등). 주요 연관 함수:
  - `from_locator(handle)`, `from_face(face, handle)`: FreeType face로부터 생성
  - `best_matching_index(attr, fonts, pixel_size) -> Option<usize>`: CSS Fonts Level 3 매칭(stretch→style→weight→pixel strike 순)
  - `best_match(attr, pixel_size, fonts) -> Option<Self>`, `best_matching_font(source, attr, origin, pixel_size)`
  - `synthesize(attr) -> Self`: 합성 italic/bold/dim 및 emoji presentation 플래그 결정
  - `coverage_intersection(wanted) -> RangeSet<u32>`: 지연 계산되는 코드포인트 커버리지 교집합
  - `matches_name/matches_alias/matches_full_or_ps_name`, `lua_name()`, `lua_fallback(handles)`(Lua 설정 코드 생성)
- `Names`, `FontPaletteInfo`, `MaybeShaped`(public enum), `load_built_in_fonts`(crate-internal), `parse_and_collect_font_info`(crate-internal)

### `locator` 모듈

- `FontLocator` 트레이트: `load_fonts`, `enumerate_all_fonts`(기본 빈 구현), `locate_fallback_for_codepoints`
- `FontDataHandle`(source/index/variation/origin/coverage), `FontDataSource`(`OnDisk`/`BuiltIn`/`Memory`), `FontOrigin` enum
- `new_locator(FontLocatorSelection) -> Arc<dyn FontLocator + Send + Sync>`
- `gdi::GdiFontLocator`(Windows), `gdi::parse_log_font`(public unsafe fn)

### `shaper` 모듈

- `FontShaper` 트레이트: `shape`, `metrics`, `metrics_for_idx`
- `new_shaper(config, handles) -> Box<dyn FontShaper>`
- `GlyphInfo`, `FontMetrics`, `FallbackIdx`(=`usize`), `PresentationWidth<'a>`, `Direction`(재노출)
- `harfbuzz::HarfbuzzShaper`

### `rasterizer` 모듈

- `FontRasterizer` 트레이트: `rasterize_glyph(glyph_pos, size, dpi) -> RasterizedGlyph`
- `new_rasterizer(selection, handle, pixel_geometry) -> Box<dyn FontRasterizer>`
- `RasterizedGlyph`(data/width/height/bearing/has_color/is_scaled)
- `freetype::FreeTypeRasterizer`, `harfbuzz::HarfbuzzRasterizer`, `colr`(PaintOp/DrawOp/그라디언트 함수)

### 저수준 래퍼

- `ftwrap`: `Library`, `Face`, `SelectedFontSize`, `NameRecord`, `PaletteInfo`, `Palette`, `compute_load_flags_from_config`, `IsSvg`/`IsColr1OrLater`
- `hbwrap`(crate-internal `mod`): `Blob`, `Face`, `Font`, `Buffer`, `PaintOp`, `FontFuncs`, `DrawFuncs`
- `units`: `PixelUnit`, `PixelLength`(=`euclid::Length<f64, PixelUnit>`), `IntPixelLength`(=`isize`)

## 4. 내부 구조

모듈 구성과 데이터 흐름은 다음과 같다.

```
config (TextStyle, FontAttributes)
        │
        ▼
FontConfiguration / FontConfigInner   (lib.rs)  ── 캐시·조정 계층
        │ resolve_font_helper_impl
        ▼
┌───────────────┬──────────────────┬───────────────────┐
│ db.rs         │ locator/gdi.rs   │ parser.rs         │
│ FontDatabase  │ GdiFontLocator   │ ParsedFont 매칭   │
│ (font_dirs,   │ (DirectWrite/GDI)│ (CSS L3 매칭)     │
│  built_in)    │                  │                   │
└───────────────┴──────────────────┴───────────────────┘
        │ Vec<ParsedFont> (핸들 목록)
        ▼
LoadedFont  ── shape() ──► shaper/harfbuzz.rs (HarfbuzzShaper)
        │                         │ hbwrap.rs (Buffer/Font/shape)
        │                         │ ftwrap.rs (Face::set_font_size, 메트릭)
        │                         ▼
        │                    Vec<GlyphInfo>
        │
        └─ rasterize_glyph() ──► rasterizer/freetype.rs (FreeTypeRasterizer)
                                      │ COLR/SVG → colr.rs + cairo
                                      │ fallback → rasterizer/harfbuzz.rs
                                      ▼
                                 RasterizedGlyph
```

**제어 흐름의 핵심**: `FontConfigInner`는 모든 폰트를 `HashMap<TextStyle, Rc<LoadedFont>>`로 캐시한다. `resolve_font_helper_impl`은 우선 폰트(non-fallback)와 폴백 폰트를 분리해 처리하며, 각 폰트 출처(font_dirs → locator → built_in)에서 후보를 모아 `best_matching_index`로 최적 1개를 선택한다.

**비동기 폴백 해결**: 셰이핑 중 글리프를 못 찾으면(`no_glyphs`), `schedule_fallback_resolve`가 별도 스레드(`FallbackResolveInfo::process`)로 폴백 폰트 탐색을 디스패치한다. 결과는 `pending_fallback`(`Arc<Mutex<Vec<ParsedFont>>>`)에 쌓이고, 다음 `shape_impl` 호출 시 `insert_fallback_handles`로 핸들 목록에 병합되며 `ClearShapeCache` 오류로 셰이핑이 재시작된다. 이는 UI 블로킹 없이 글리프 커버리지를 점진적으로 확장하는 메커니즘이다.

**비대한 모듈**:
- `ftwrap.rs`(1534행): FreeType 전체 래퍼. Face 메트릭, SFNT 이름 디코딩(다중 인코딩), 커버리지 계산, cap-height 픽셀 측정, COLR/팔레트 API, 커스텀 FreeType 스트림(`FreeTypeStream`, mmap/파일/메모리 백킹)까지 포함해 가장 비대하다.
- `shaper/harfbuzz.rs`(1244행): `do_shape`가 재귀적 폴백·클러스터 해결·셀 폭 분배를 모두 담당하며, 절반 가까이가 스냅샷 테스트(`#[cfg(test)]`).
- `hbwrap.rs`(1194행): HarfBuzz 전체 래퍼 + COLRv1 paint/draw 콜백 디스패치.
- `rasterizer/freetype.rs`(910행): 5종 픽셀 모드 래스터화 + COLRv1 paint 그래프 워커(`Walker`)의 cairo 변환.

## 5. 핵심 데이터 구조·타입

- **`FontConfigInner`**(lib.rs:465): 모든 상태를 `RefCell`로 감싼 단일 스레드 캐시 컨테이너. 불변식: `Rc`로 공유되며 `LoadedFont`는 `Weak<FontConfigInner>`로 역참조하므로 순환 참조가 없다. 설정 변경 시 모든 폰트 캐시·메트릭·용도별 폰트를 무효화해야 일관성이 유지된다.

- **`LoadedFont`**(lib.rs:53): 해결된 폰트 1개. 불변식: `handles`(폴백 시퀀스), `rasterizers`(인덱스별 지연 생성), `shaper`는 모두 `RefCell`이며, `handles`가 변경되면(`insert_fallback_handles`) 반드시 `shaper`를 재생성해야 한다. `tried_glyphs`는 해결 실패한 코드포인트를 기억해 재탐색을 막는다.

- **`ParsedFont`**(parser.rs:24): 폰트 메타데이터. 불변식: `PartialEq`/`Ord`는 `names`·`weight`·`stretch`·`style`(+handle)만 사용하고 `coverage`·합성 플래그는 동등성에서 제외한다. `coverage`는 `Mutex<RangeSet<u32>>`로 지연 계산되며, 비어 있으면 "미계산"을 의미한다. `synthesize`는 원본 폰트가 요청 속성을 갖지 못할 때만 합성 italic/bold/dim 플래그를 켠다.

- **`FontDataHandle`**(locator/mod.rs:139): 폰트 데이터 참조. `source`+`index`+`variation`+`origin`이 정체성을 구성하며 `coverage`는 사전 계산된 캐시(옵션). `Hash`/`Eq`는 `coverage`를 제외한다.

- **`FontDataSource`**(locator/mod.rs:31): `OnDisk`/`BuiltIn`/`Memory` 3변형. 불변식: 수동 `Hash`/`Eq`가 일치해야 하며(`BuiltIn`/`Memory`는 `name`만 비교), 이는 clippy 경고를 유발하던 파생 구현을 의도적으로 교정한 것이다(locator/mod.rs:90-103).

- **`GlyphInfo`**(shaper/mod.rs:12): 셰이핑된 글리프 1개. `num_cells`는 리거처로 결합된 그래핌의 셀 폭을 추적하며, 이를 누락하면 결합 케이스의 폭 계산이 깨진다(이슈 1563). `text` 필드는 debug/test 빌드에서만 보존.

- **`FontMetrics`**(shaper/mod.rs:50): 셀 폭/높이, descender, underline, cap_height, `is_scaled`(스케일러블 여부), `force_y_adjust`(scale override 시 수직 정렬 보정, 이슈 1803).

- **`RasterizedGlyph`**(rasterizer/mod.rs:17): premultiplied RGBA 32bpp 비트맵. `is_scaled`가 false면 비트맵 스트라이크라 GUI가 별도 스케일링해야 한다.

- **`Face`/`FaceSize`**(ftwrap.rs:98): FreeType face 래퍼. `size`는 마지막 설정된 크기를 캐시해 동일 크기 재설정을 회피한다. `palette`는 `&'static mut [FT_Color]`로 face 내부를 가리키는 위험한 참조.

- **`ClusterResolver`**(shaper/harfbuzz.rs:748): HarfBuzz 클러스터를 셀 인덱스 기반으로 재매핑. `presentation_width`가 있으면 셀 경계로 클러스터를 정규화해 surprising clustering(이슈 2572)을 교정한다.

## 6. 외부 의존성

- **`freetype`**: 폰트 파싱·메트릭·글리프 래스터화의 1차 백엔드. SFNT 이름 테이블 직접 파싱(인코딩 한계 회피), 비트맵 스트라이크 선택, cap-height 픽셀 측정, COLRv1 paint 그래프 순회에 사용. 항상 자체 빌드되며 LCD 필터는 특허 이슈로 기본 비활성(ftwrap.rs:1187).
- **`harfbuzz`**: 텍스트 셰이핑(리거처·결합·BiDi)의 유일한 셰이퍼. COLRv1 글리프의 paint/draw 콜백 API도 제공해 별도 COLR 래스터 경로를 구성한다. OT funcs/face는 정합성 문제로 컴파일 타임 비활성(`USE_OT_FUNCS`/`USE_OT_FACE` = false, shaper/harfbuzz.rs:23).
- **`cairo-rs`**(코드명 `cairo`): COLR/그라디언트/이미지 글리프를 벡터로 합성 렌더링. RecordingSurface에 paint op을 기록한 뒤 ink_extents로 잘라 ImageSurface에 래스터화. 선형/방사형/스윕 그라디언트와 27종 합성 연산자를 지원.
- **`memmap2`**: 폰트 파일을 mmap으로 매핑(FreeType·HarfBuzz 양쪽 스트림 백킹). mmap 실패 시 파일 IO로 폴백.
- **`encoding_rs`**: SFNT 이름 레코드의 platform/encoding ID에 따라 Shift-JIS·GBK·Big5·EUC-KR·UTF-16BE·Mac Roman 등으로 디코딩.
- **`finl_unicode`**: 그래핌 클러스터 카운트(누락 글리프 대체 문자열 생성용).
- **`image`**: 비트맵/PNG 임베디드 글리프 디코딩, BGRA 이모지 비트맵의 투명 영역 크롭·R/B 스왑.
- **`rangeset`**: 코드포인트 커버리지를 RangeSet로 표현해 교집합/차집합으로 폴백 폰트를 최소화.
- **`ordered-float`**: 메트릭 캐시 키(`NotNan<f64>`)에 사용.
- **`dwrote`/`winapi`**(Windows): 시스템 폰트 해결·폴백·열거. 아래 8절 참조.

## 7. 설정·기능 플래그

### Cargo feature flags (Cargo.toml:10-14)

내장 폰트 에셋을 바이너리에 임베드할지 결정하는 4개 vendor 플래그다. 모두 `parser.rs::load_built_in_fonts`에서 `#[cfg]`로 조건부 컴파일된다.

- `vendor-jetbrains`: JetBrains Mono 16종(기본 monospace 폴백)
- `vendor-roboto`: Roboto 12종(타이틀·UI 폰트)
- `vendor-noto-emoji`: Noto Color Emoji(이모지 폴백)
- `vendor-nerd-font-symbols`: Symbols Nerd Font Mono(심볼 폴백)

테스트 빌드(`#[cfg(test)]`)에서는 플래그와 무관하게 전부 포함된다.

### 소비하는 config 항목

`config` 크레이트에서 읽는 주요 항목(소스 기본값 수정 시 동작 고정 대상):

- 폰트 선택: `font`, `font_rules`, `font_size`, `font_dirs`, `search_font_dirs_for_fallback`
- 백엔드 선택: `font_locator`(`FontLocatorSelection`), `font_shaper`(`FontShaperSelection`), `font_rasterizer`(`FontRasterizerSelection`), `font_colr_rasterizer`
- FreeType: `freetype_load_flags`, `freetype_load_target`, `freetype_render_target`, `freetype_interpreter_version`, `freetype_pcf_long_family_names`
- 렌더링: `display_pixel_geometry`(RGB/BGR), `harfbuzz_features`, `bold_brightens_ansi_colors`(`BoldBrightening`)
- 폴백·정렬: `sort_fallback_fonts_by_coverage`, `use_cap_height_to_scale_fallback_fonts`, `warn_about_missing_glyphs`, `ignore_svg_fonts`
- UI 폰트: `window_frame.font`/`font_size`, `command_palette_font`/`_size`, `char_select_font`/`_size`, `pane_select_font`/`_size`

## 8. Windows 전용 고려사항

- **로케이터는 Windows 단일 경로**: `new_locator`(locator/mod.rs:229)는 `FontConfig`/`CoreText` 선택 시 `panic!("... not compiled in")`을 호출한다. 실질 경로는 `Gdi`(=`GdiFontLocator`)와 `ConfigDirsOnly`(=`NopSystemSource`) 둘뿐이다. `FontOrigin` enum의 `FontConfig`/`FontConfigMatch`/`CoreText` 변형은 비Windows 잔재로 사실상 죽은 값이다.

- **`locator/gdi.rs`(402행)**: 파일 전체가 `#![cfg(windows)]`. DirectWrite(`dwrote`)와 GDI(`winapi`) 두 경로를 사용한다.
  - `load_fonts`: 먼저 `dwrote::FontCollection::system()`에서 디스크립터로 해결을 시도하고, 실패 시 GDI `CreateFontIndirectW`+`GetFontData`로 폰트 바이트를 메모리에 추출(`extract_raw_font_data`)한다. TTC 컬렉션은 `'ttcf'` 테이블로 전체를 받아 역엔지니어링한다.
  - `locate_fallback_for_codepoints`: DirectWrite `FontFallback::get_system_fallback().map_characters`로 코드포인트별 시스템 폴백 폰트를 질의한다. 이것이 Windows에서 누락 글리프를 해결하는 1차 메커니즘이다.
  - `enumerate_all_fonts`: DirectWrite 패밀리를 순회해 디스크 경로를 수집·파싱한다.
  - `parse_log_font`(public unsafe fn): GDI `LOGFONTW`로부터 `ParsedFont`+포인트 크기를 복원(`window` 크레이트가 창 프레임 폰트 등에 사용).

- **Windows API 직접 사용 지점**: `CreateCompatibleDC`/`SelectObject`/`GetFontData`/`CreateFontIndirectW`/`DeleteObject`/`MulDiv`/`GetDeviceCaps`(gdi.rs). 메모리 추출 후 `FontDataSource::Memory`로 래핑되며, 이 변형은 주석상 "현재 Windows에서만 사용"으로 명시(locator/mod.rs:274).

- **죽은 코드/비Windows 잔재**:
  - `ftwrap.rs`의 SFNT 인코딩 분기에 Macintosh/Apple Unicode/ISO 플랫폼 처리가 남아 있으나, 이는 폰트 파일 내부 데이터 처리이므로 플랫폼과 무관하게 유효하다(죽은 코드 아님).
  - `freetype.rs` 등의 LCD/LCD_V 수직 서브픽셀 경로는 Windows에서 거의 쓰이지 않지만 설정에 따라 도달 가능하다.

- **FreeType 스트림 주석**: `FreeTypeStream`(ftwrap.rs:1265)은 "Windows에서 Path→C 문자열 변환을 보장할 수 없어서" 자체 스트림을 구현한다고 명시. Windows 경로 처리를 위한 의도적 설계다.

## 9. 리팩토링 주의점

- **`too_many_arguments`의 의도적 보류**: `shape`/`shape_impl`/`do_shape`/`FontShaper::shape`는 7~9개 인자를 받으며 `#[allow(clippy::too_many_arguments)]`가 붙어 있다. 주석에 명시되었듯 외부(`wezterm-gui`) 호출부와 트레이트 시그니처를 그대로 중계하므로, 인자 구조체화는 트레이트·호출부 동시 변경을 강제한다. 파급이 크다.

- **비동기 폴백의 동시성 불변식**: `pending_fallback`(`Arc<Mutex>`)와 `ClearShapeCache` 재시작 루프는 정교하게 맞물려 있다. `insert_fallback_handles`가 `handles` 변경 후 반드시 `shaper`를 재생성해야 하며(lib.rs:110), 이를 누락하면 셰이퍼와 핸들이 불일치한다. `blocking_shape`의 `loop`는 `ClearShapeCache`를 무한히 재시도할 수 있으므로 폴백 해결이 수렴함을 보장해야 한다.

- **`Rc`/`RefCell` 단일 스레드 결합**: `FontConfigInner`·`LoadedFont`는 `Rc`+`RefCell` 기반이라 스레드 간 이동 불가. 반면 폴백 해결 스레드는 `Arc<dyn FontLocator + Send + Sync>`·`Arc<FontDatabase>`만 넘겨받는다. 이 경계를 흐리면(예: `LoadedFont`를 스레드로 넘기려는 시도) 컴파일 또는 런타임 패닉.

- **`unsafe` FFI 밀집**: `ftwrap.rs`·`hbwrap.rs`·`rasterizer/freetype.rs`는 FreeType/HarfBuzz/cairo의 raw 포인터를 광범위하게 다룬다. 특히 `Face::palette`(`&'static mut [FT_Color]`, ftwrap.rs:103)는 face 내부를 가리키는 위장된 정적 참조이며, face가 살아 있는 동안만 유효하다. `from_raw_parts`는 null 포인터를 빈 슬라이스로 처리하도록 자체 래핑되어 있다(rust 1.78 null panic 회피).

- **`Hash`/`Eq` 일관성**: `FontDataSource`·`FontDataHandle`·`ParsedFont`는 수동 `Hash`/`Eq`/`Ord`를 구현하며 비교 필드가 서로 다르다(coverage·합성 플래그 제외). 필드 추가 시 세 구현을 동기화하지 않으면 `HashMap`/`HashSet` 정합성이 깨진다.

- **순환 의존 없음, 단방향 계층**: 크레이트 내부는 `lib → {db, locator, parser, shaper, rasterizer} → {ftwrap, hbwrap}` 단방향이다. 단, `rasterizer/freetype.rs`가 `rasterizer/harfbuzz.rs`를 폴백 래스터라이저로 내장(freetype.rs:37,410)하므로 두 래스터라이저는 약하게 결합된다. COLRv1 처리는 ftwrap(FreeType paint 그래프) 또는 hbwrap(HarfBuzz paint 콜백) 어느 쪽이든 `colr.rs`의 동일한 `PaintOp`/`DrawOp`로 수렴한다 — 이 중간 표현이 두 백엔드를 분리하는 경계이므로 보존해야 한다.

- **컴파일 타임 비활성 플래그**: `USE_OT_FUNCS`/`USE_OT_FACE`(shaper/harfbuzz.rs:23)는 "advance 불일치 미해결 버그" 때문에 false 고정된 상수다. true로 켜면 비례 폰트·비트맵 스트라이크에서 잘못된 advance가 발생한다. 활성화 전 근본 원인 규명이 선행되어야 한다.

- **메트릭의 sniff test**: `HarfbuzzShaper::metrics`(shaper/harfbuzz.rs:672)는 이론적 픽셀 높이 대비 편차가 2배를 넘는 폴백 슬롯을 건너뛴다. 잘못된 설정(비트맵 이모지 폰트가 첫 슬롯 등)에서 거대 셀을 방지하는 휴리스틱이며, 임계값 변경은 셀 크기에 직접 영향.

## 10. 파일별 요약

| 파일 | 대략 행수 | 역할 |
| --- | --- | --- |
| `src/lib.rs` | 1131 | 최상위 진입점. `FontConfiguration`/`FontConfigInner`/`LoadedFont`, 스타일→폰트 해결·캐시, 비동기 폴백 해결, font_rules 매칭, 용도별(타이틀/팔레트/선택) 폰트 |
| `src/ftwrap.rs` | 1534 | FreeType 고수준 래퍼. `Library`/`Face`, 메트릭·SFNT 이름 디코딩·커버리지·cap-height·COLR/팔레트 API, 커스텀 `FreeTypeStream`(mmap/파일/메모리) |
| `src/shaper/harfbuzz.rs` | 1244 | `HarfbuzzShaper`. 재귀적 폴백 셰이핑(`do_shape`), `ClusterResolver`로 셀 폭 분배, 메트릭 계산. 약 절반이 스냅샷 테스트 |
| `src/hbwrap.rs` | 1194 | HarfBuzz 고수준 래퍼. `Blob`/`Face`/`Font`/`Buffer`, 셰이핑 호출, COLRv1 paint/draw 콜백 디스패치(`PaintOp`) |
| `src/parser.rs` | 941 | `ParsedFont`/`Names`, CSS Fonts L3 매칭(`best_matching_index`), 합성 플래그(`synthesize`), 커버리지 교집합, 내장 폰트 로드, Lua 코드 생성 |
| `src/rasterizer/freetype.rs` | 910 | `FreeTypeRasterizer`. 5종 픽셀 모드(mono/gray/lcd/lcd_v/bgra) 래스터화, COLRv1 paint 그래프 워커(`Walker`)→cairo 변환 |
| `src/rasterizer/colr.rs` | 634 | COLR 공통 중간 표현(`PaintOp`/`DrawOp`/`ColorLine`)과 cairo 그라디언트(선형/방사형/스윕) 페인팅. BlackRenderer→HarfBuzz 포팅 코드 |
| `src/locator/gdi.rs` | 402 | (Windows 전용) `GdiFontLocator`. DirectWrite/GDI 폰트 해결·시스템 폴백·열거, `LOGFONTW` 파싱 |
| `src/rasterizer/harfbuzz.rs` | 416 | `HarfbuzzRasterizer`. HarfBuzz paint op→cairo 합성, PNG/BGRA 이미지 글리프, premultiply·ARGB/RGBA 변환 |
| `src/locator/mod.rs` | 284 | `FontLocator` 트레이트, `FontDataHandle`/`FontDataSource`/`FontOrigin`, `new_locator`(비Windows는 panic), `NopSystemSource` |
| `src/db.rs` | 158 | `FontDatabase`. 폰트 디렉터리/내장 폰트를 `by_full_name` 맵으로 색인, 후보·해결·폴백 커버리지 조회 |
| `src/shaper/mod.rs` | 154 | `FontShaper` 트레이트, `GlyphInfo`/`FontMetrics`/`PresentationWidth`, `new_shaper`(Allsorts 제거됨) |
| `src/rasterizer/mod.rs` | 122 | `FontRasterizer` 트레이트, `RasterizedGlyph`, `new_rasterizer`, 투명 영역 크롭·R/B 스왑 헬퍼 |
| `src/units.rs` | 3 | `PixelUnit`/`PixelLength`/`IntPixelLength` 타입 별칭 |
