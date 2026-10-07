# BSEngine vs Unity/Unreal 비교 보고서

> ## ⚠️ 먼저 읽을 것 — 이 문서는 구조적으로 낡는다
>
> 아래 본문은 **날짜가 붙은 스냅샷이 쌓인 것**이다. 병합은 계속되므로 오래된 절의 "없음"은
> 대부분 이미 거짓이다. **실제로 이 문서의 산문을 근거로 항목을 분류했다가 네 번 틀렸다**
> (CSM, 스크립팅 타입 안전성, 플랫폼 폭, 그리고 데칼/RPC/천/IK가 한꺼번에).
>
> **다음 작업을 고를 때는 이 문서가 아니라 `grep`으로 부재를 확인할 것.** 확인 비용은 명령
> 한 줄이고, 틀렸을 때 비용은 이미 있는 기능을 다시 만드는 것이다.

---

## 상용 엔진 표준 기능 대비 격차 (2026-09-29 셀프 리뷰, 전부 grep으로 부재 확인함)

master `e3b5c728` 기준. 아래 "현재 남은 작업" 표는 2026-09-28에 비었지만, **그 표는 작업하다 드러난 격차만 모은
것이라 상용 엔진과의 격차 목록이 아니었다.** 이번에는 Unity·Unreal·Godot이 모두 기본 제공하는 기능 목록을 먼저
세우고 코드에서 하나씩 grep했다. 확인 명령은 전부 `crates/`의 `.rs`·`.wgsl`을 대상으로 하고 대소문자를 무시한다
(`grep -rliE "<패턴>" crates --include=*.rs --include=*.wgsl`).

⚠️ **함정 기록:** 첫 검사는 `grep -E "(?i)..."`로 했다가 모든 패턴이 0을 냈다 — `grep -E`는 `(?i)`를 모르므로 존재하는
천 솔버마저 "없음"으로 나왔다. 존재하는 것 하나를 먼저 같은 방식으로 찾아 **정상성 검사**를 하고 나서 0을 믿을 것.

⚠️ **"예전에 있었다"의 정체:** DOF·모션 블러·컬러 그레이딩·캐릭터 컨트롤러·반사 프로브·입력 맵은 git 이력에 `feat:
<이름> component`로 추가된 적이 있다. 전부 렌더/물리 구현 없이 필드만 있던 **zoo 컴포넌트**였고 `8a623ca7`(#1705)·
`00a86d12`에서 삭제됐다. 이력 검색(`git log -S`)으로 "있었다"고 판단하지 말 것.

### 코드에 없는 것

| 영역 | 항목 | 패턴 → 결과 | 세 엔진 |
|---|---|---|---|
| 게임플레이 | ~~**캐릭터 컨트롤러**~~ → **키네마틱** 캐릭터 컨트롤러, 닫힘 | ⚠️ 이 행의 원래 판정(`character_?controller\|move_and_slide` → 0)은 틀렸다: 동적 강체용 `CharacterBody`(item 27, 회전 잠금 + 착지 레이)가 이미 있었고 이름이 달라 grep이 놓쳤다. 없던 것은 세 엔진의 주력인 키네마틱 스윕·슬라이드 방식 → 2026-09-29 `CharacterController`(Rapier KCC: 벽 슬라이드·경사 한계·계단 오르기·바닥 붙기, `Bsengine.moveCharacter`·`isCharacterGrounded`)로 닫힘. 확인: `grep -n "pub struct CharacterController" crates/bsengine-physics/src/components.rs` | Unity `CharacterController`, Unreal `CharacterMovementComponent`, Godot `CharacterBody3D.move_and_slide` |
| 게임플레이 | ~~**입력 액션 맵·리바인딩**~~ 닫힘 | `InputAction\|action_?map\|rebind` → 0(판정 맞음; `catalog --concept` input/action/binding/key/axis로도 재확인) → 2026-09-29 `project.toml`의 `[input.actions]` + `InputActions`/`ActionState`(bsengine-input) + `isActionPressed/Down/Up`·`getActionStrength`·`getAxis`·`getVector`·`get/setActionBindings`로 닫힘. 키·마우스·게임패드 버튼·스틱 방향·트리거 바인딩, 데드존 재정규화(Godot식), 잘못된 바인딩은 시작·패키징에서 실패. 매핑 컨텍스트·홀드/탭 인터랙션은 없음. 같은 작업에서 스크립트가 키 11개만 읽던 결함과 `onKeyDown`이 #1829 이후 한 번도 안 불리던 결함을 찾아 #1916으로 고침. 확인: `grep -n "pub struct InputActions" crates/bsengine-input/src/actions.rs` | Unity Input System, Unreal Enhanced Input, Godot `InputMap` |
| 애니메이션 | ~~루트 모션~~ 닫힘 | `root_?motion` → 0 → 2026-09-30 `RootMotion`(bone 이름, 빈 값=스킨 첫 조인트): 루트 본의 **수평 이동**을 모델 공간에서(아마추어 회전·스케일 통과) 재서 포즈에서 빼고 엔티티 `Transform`에 적용, 수직은 포즈에 남김(Unity "Bake Y into pose" 기본), 루프 랩·역재생·첫 프레임 포함. `apply_to_transform=false`면 `last_delta`만 보고(스크립트가 `moveCharacter`로). 2026-09-30 **요(yaw) 추출**도: 모델 +Y 둘레 트위스트(스윙-트위스트, 기울기·롤은 포즈에 남김), 포즈에서 휴지 요로 되돌리고 엔티티를 `rotation * delta`로 회전(Unity `deltaRotation`식), 매 구간의 이동은 **그 구간 시작의 루트 방향 기준**으로 재서 호를 걷는 클립이 두 번 돌지 않음(이동만 모드와 같은 위치에 도착), 루프 랩도 방향을 이어받음. `apply_rotation=false`는 Unity "Root Transform Rotation: Bake Into Pose". `last_rotation_delta` 보고. 확인: `grep -n "fn apply_root_motion" crates/bsengine-gltf/src/skinned_mesh.rs` | 셋 다 |
| 애니메이션 | ~~애니메이션 이벤트~~ 닫힘 | `AnimationEvent\|anim_?event` → 0 → 2026-09-30 `AnimationEvents`(엔티티에 {clip, time, name} 목록; glTF엔 이벤트가 없어 클립이 아닌 엔티티에) — 재생 헤드가 지날 때 한 번(루프 랩·여러 랩·역재생 포함), `AnimationEventFired` ECS 이벤트 + 스크립트 `Bsengine.onAnimationEvent(entity, (name, clip) => …)`. 같은 작업에서 역방향 루프가 0 아래로 영영 내려가던 결함을 고침. 확인: `grep -n "pub struct AnimationEvents" crates/bsengine-core/src/animation_events.rs` | Unity Animation Event, Unreal Anim Notify, Godot 메서드 트랙 |
| 애니메이션 | ~~모프 타깃(블렌드셰이프)~~ 닫힘 | `morph_?target\|blend_?shape` → 0 → 2026-09-30 glTF 타깃(위치·노멀 델타, `extras.targetNames`, 기본 가중치, 임포트 스케일) → `MorphWeights`(core) → 스키닝 컴퓨트 안에서 **스킨 전에** 가중합(스킨 없는 메시도 항등 조인트 하나로 같은 경로). ⚠️ downlevel 한도(스테이지당 스토리지 버퍼 4개)를 스키닝이 이미 다 써서 델타는 `rest` 버퍼 뒤에, 가중치는 유니폼에(최대 64 타깃). 스크립트 `setMorphWeight/getMorphWeight/getMorphTargetNames`. 2026-09-30 클립의 `weights` 트랙도 재생: `KeyframeValues::Weights` → 메시를 그리는 노드(`MeshData.node`)로 `animate_morph_weights`가 샘플(스켈레톤 없는 모프 메시도 클립 라이브러리+플레이어를 받음). 새 셋 `AnimationPoseSystems`(루트 모션·가중치)가 `AnimationSystems` 뒤, 스크립트는 그 뒤 -- Unity `LateUpdate`처럼 스크립트가 설정한 가중치가 클립을 이김. CubicSpline은 값 블록만(탄젠트 무시, 이동 샘플러와 같은 단순화). 첫 메시만(스키닝과 같은 제약). 확인: `grep -n "pub struct MorphWeights" crates/bsengine-core/src/morph_weights.rs` | 셋 다(glTF도 morph target을 담음) |
| 렌더링 | ~~**반사 프로브**~~ 닫힘 | `reflection_?probe` → 0(`catalog --concept` reflection/cubemap/specular도 0 확인) → 2026-09-30 `ReflectionProbe`(bsengine-core): 프로브 위치에서 큐브맵 1회 캡처(라이트 프로브 캡처 셰이더 재사용, 하드웨어 큐브 규약으로 Y 플립) → 하늘과 같은 `prefilter_from_cubemap` → 큐브 배열(최대 4) 슬롯, 박스 안에서 스카이 스페큘러를 **대체**, 박스 투영 선택(기본 꺼짐, Unity·Godot와 같음), 겹치면 가장 작은 박스 우선. 블렌딩·실시간 갱신·SSR과의 합성 규칙은 없음. 확인: `grep -n "pub struct ReflectionProbe" crates/bsengine-core/src/reflection_probe.rs` | Unity Reflection Probe, Unreal Sphere/Box Reflection Capture, Godot `ReflectionProbe` |
| 렌더링 | ~~**DOF**~~ 닫힘 | `depth_?of_?field\|bokeh_scale` → 1(`scripting/src/ops.rs` 삭제된 스냅샷의 주석) → 2026-09-30 `DepthOfField`(카메라): Godot·Unity Gaussian식 근·원 거리 밴드(거리+전이), 톤매핑 전 HDR에서 32탭 골든앵글 디스크 수집, 스캐터-애즈-개더 가중(선명한 피사체는 헤일로 없음, 흐린 전경은 초점 맞은 배경 위로 번짐, 뒤쪽 흐린 배경은 선명한 피사체를 넘지 않음). 물리 렌즈 모델(초점거리·조리개)·보케 모양은 없음. 확인: `grep -n "fn fs_dof" crates/bsengine-rhi-wgpu/src/post_process.rs` | 셋 다 |
| 렌더링 | ~~모션 블러~~·~~오브젝트별 모션 벡터~~ 닫힘 | `motion_?blur\|shutter_angle` → 1(같은 주석) → 2026-09-30 `MotionBlur`(core): 깊이+이전 프레임 무지터 view-proj로 재투영한 화면 속도를 따라 HDR 이미지에서 N탭 평균(피사계 심도 뒤, 블룸 앞 -- Unreal 순서). 파라미터는 Unity·Unreal 공통(강도 0.5, 화면 5% 클램프). 컷 뒤 첫 프레임은 선명(`invalidate_history`). 에디터 오빗 카메라에선 끔. 2026-10-01 **오브젝트별 모션 벡터**: 불투명 패스가 `Rg16Float` 속도 버퍼(`VELOCITY_FORMAT`)에 메시마다 자기 움직임+카메라 움직임을 uv 단위로 씀(무지터 view-proj와 이전 프레임 view-proj, 모델 유니폼의 `prev_model` -- 256B 스트라이드 안에 들어가 버퍼 증가 없음). 이전 행렬은 렌더 플러그인이 엔티티별로 기억(`Local<HashMap<Entity, Mat4>>`, 매 프레임 재구성). TAA는 속도로 히스토리를 찾고 모션 블러는 속도를 따라 번짐 -- 정지 카메라 앞에서 움직이는 물체도 번짐(Unreal·HDRP식; URP는 카메라만). 속도를 안 쓰는 표면(하늘·터레인·커스텀 셰이더·반투명)은 클리어 값 "없음"(65504) → 깊이+카메라 재투영으로 폴백; 그래서 터레인·커스텀 셰이더를 불투명 메시보다 먼저 그림(뒤에 가려진 움직이는 메시의 속도가 남지 않게). MSAA에선 노멀처럼 리졸브. **스키닝 변형은 미포함** -- 뼈 움직임은 속도에 안 들어가고 엔티티 변환만(Unreal·Unity는 이전 프레임 스키닝 보관). 확인: `grep -n "VELOCITY_FORMAT" crates/bsengine-rhi-wgpu/src/post_process.rs` | Unreal·Unity(오브젝트별은 Unreal·HDRP); Godot 없음 |
| 렌더링 | ~~**컬러 그레이딩 LUT**~~ 닫힘 | `color_?grading\|colou?r_lut` → 0 → 2026-09-30 `ColorGrading`(카메라 컴포넌트): 대비·채도·컬러 필터를 톤매핑 **뒤** sRGB 디스플레이 공간에서(Godot·URP LDR 방식; Unreal·HDRP는 톤매핑 전 HDR — 갈림, LUT와 같은 공간을 택함), Unity 순서(대비→필터→채도). LUT는 `ColorGrading::lut`(스트립 N²×N, Unreal 레이아웃, 트라이리니어, `lut_contribution`)로 조정 뒤에 적용 — 스트립이 아니거나 BC 압축이면 거부하고 언바인드. 확인: `grep -n "fn apply_grading" crates/bsengine-rhi-wgpu/src/post_process.rs` | 셋 다 |
| 렌더링 | ~~TAA 외 AA~~ 닫힘 -- ~~FXAA~~·~~MSAA~~·~~SMAA~~ | `msaa\|fxaa\|smaa` → 0 → 2026-09-30 `Fxaa`(core): FXAA 3.11 품질 알고리즘(루마 대비 → 에지 방향 → 양쪽 끝까지 걷기 → 가까운 끝 기준 블렌드 + 서브픽셀 항), 톤매핑된 LDR에, TAA 리졸브 **앞**(TAA가 FXAA 결과를 누적). 세 임계값은 FXAA 3.11 기본값(0.166/0.0833/0.75). sRGB 타깃이면 루마를 sqrt로 지각 공간에. 셋 다 있는 유일한 비-TAA AA라 먼저. 2026-10-01 **MSAA**: `[render] msaa = 4`(프로젝트 설정 — Unity URP 에셋·Godot `msaa_3d`처럼 카메라별 아님) → `MsaaSettings` → 지오메트리 패스 4개(불투명·스카이·투명·파티클)가 4x 멀티샘플 HDR·노멀·깊이에 그리고 패스마다 리졸브(조건부 스킵해도 안전), WebGPU엔 깊이 리졸브가 없어 전용 풀스크린 패스(샘플 0 → `frag_depth`, URP CopyDepth식)로 `depth_view`를 채워 SSAO·포그·DOF·모션 블러·TAA·SSR이 그대로 읽음. 파이프라인 6개(메시·투명·터레인·스카이·커스텀·파티클)는 같은 디스크립터로 1x/4x 두 벌을 만들어 프레임마다 고름. 4x만(WebGPU 보장), 어댑터가 못 하면 꺼짐(경고 1회). 2026-10-01 **SMAA**: `Smaa`(core, 카메라) — 레퍼런스 `SMAA.hlsl`(Jimenez, MIT) SMAA 1x 포팅: 루마 에지(국소 대비 적응) → 블렌드 가중치(양끝 탐색·교차 에지·`AreaTex`, High/Ultra는 대각·코너) → 이웃 블렌드. `AreaTex`/`SearchTex`는 레퍼런스 헤더 바이트 그대로(`crates/bsengine-rhi-wgpu/src/smaa/*.bin`). 프리셋 Low/Medium/High/Ultra(레퍼런스 값, 기본 High — URP처럼). FXAA와 같은 자리(톤매핑 뒤 LDR, TAA 리졸브 앞), 둘 다 켜면 SMAA만(URP·Godot 모두 한 선택지). 테이블·타깃은 처음 켤 때 생성. 확인: `grep -n "fn fs_fxaa" crates/bsengine-rhi-wgpu/src/post_process.rs`, `grep -n "fn fs_weights" crates/bsengine-rhi-wgpu/src/smaa.rs` | FXAA는 셋 다; SMAA는 Unity URP·Godot(Unreal 없음); MSAA는 셋 다 |
| 렌더링 | 라이트맵 베이크 | `light_?map` → 0 | 셋 다(Unreal Lightmass, Unity Progressive, Godot LightmapGI) |
| 렌더링 | 동적 GI | `ddgi\|voxel_?gi\|sdfgi\|global_illumination` → 0 | Unreal Lumen, Godot SDFGI/VoxelGI, Unity APV(확산만) |
| 렌더링 | ~~에어리어 라이트~~ 닫힘(사각형, 그림자 없음) | `area_?light\|rect_?light` → 0 → 2026-10-01 `RectLight`(core): 엔티티 중심, 로컬 x로 `width`·y로 `height`, 로컬 -z로 비춤(스포트와 같음), 단면, `range`에서 Unreal/Frostbite 창 함수로 사라짐. LTC(Heitz 2016): 사각형을 GGX 로브가 코사인이 되는 공간으로 옮겨 지평선에서 **정확히 클리핑**(논문의 `ClipQuadToHorizon`, 꼭짓점 0/3/4/5) 후 변 적분. 변 적분·프레넬 분리·테이블 배치는 three.js(MIT)를 따르되 three.js의 구 근사(Hill 2016)는 안 씀 -- 정면으로 마주한 패널에서만 정확하고, 벽에 반쯤 묻힌 패널에서 빛을 1/3 더 줬음. LTC 테이블은 저자들 것(BSD), half float(`crates/bsengine-rhi-wgpu/src/ltc/*.bin` -- 32비트 float 텍스처는 WebGPU에서 필터 불가). 강도는 패널 휘도라 큰 패널이 더 밝음(Unreal·HDRP nits 옵션). 메시·터레인 셰이더, 최대 4개(`MAX_RECT_LIGHTS`). 정확도: 평행 사각형의 해석적 폼 팩터와 3% 이내(측정 1% 안팎), 수직 패널은 수치 적분과 5% 이내. **그림자 없음**(Unreal·HDRP는 있음), 원판·튜브 모양 없음(HDRP), 프로브 베이크에 안 들어감. 확인: `grep -n "fn rect_light_radiance" crates/bsengine-rhi-wgpu/src/area_light.rs` | Unreal Rect Light, Unity HDRP Area Light; Godot 없음 |
| 렌더링 | HDR 디스플레이 출력 | `hdr_output\|bt2020\|hdr10` → 0 | Unity·Unreal; Godot 4.x 진행 중 |
| 콘텐츠 | ~~로컬라이제이션~~ 닫힘 | `locali[sz]ation\|string_?table` → 0 → 2026-09-30 `bsengine_core::Localization`: Godot식 CSV 문자열 테이블(`keys,en,ko`, `_` 열은 주석, 빈 칸은 미번역), `[localization] tables/default_locale/locale="auto"`(시스템 로캘 → 테이블에서 정확히/같은 언어로 매칭, `sys-locale`), 조회 체인 로캘 → 언어 → 기본 → 키 자체(Godot처럼 화면에 드러남), `{name}` 플레이스홀더. 스크립트 `tr/setLocale/getLocale/getLocales`(스냅샷이 즉시 바뀜). 테이블은 `--package`와 `cook_project` 둘 다 루트로 포함, pak에서 읽음. **남음**: 복수형(ICU plural), UI 위젯 텍스트 자동 번역(Godot auto-translate), 에디터 편집 UI, 로캘별 폰트/에셋 대체. 확인: `grep -n "pub fn tr" crates/bsengine-core/src/localization.rs` | 셋 다 |
| AI | ~~비헤이비어 트리~~ 닫힘(런타임·스크립트·디버거) | `behaviou?r_?tree\|blackboard` → 2(둘 다 "블랙보드"라는 비유 주석) → 2026-09-30 사용자 결정 "에셋 + Rust 실행기": `bsengine_core::behavior_tree`(`.bt.ron` 트리 에셋, `Blackboard`/`BehaviorTree` 컴포넌트, **순수 실행기** — `BtContext` 트레이트로만 세상을 봄), Unreal 표준 노드(Sequence/Selector/Parallel, 옵저버 abort Self_/LowerPriority/Both 있는 블랙보드 Condition, Cooldown/TimeLimit/Repeat/Inverter/Force*, Wait/MoveTo(`NavMeshAgent`, 엔티티 추적·acceptance)/SetValue/ClearValue). `bsengine-app::BehaviorTreePlugin`이 에셋 캐시 + `BehaviorTreeSystems`(내비 `NavAgentSystems` 전, 일시정지 중 정지). **2단계(같은 날): `Script(task)` 노드 → `Bsengine.bt.task(name, fn(self, bb, first), onAbort)`(Unreal 블루프린트 태스크식, 한 프레임 지연 — `BtScriptQueue` 요청/응답 리소스, 스크립팅 체인이 `BehaviorTreeSystems` 뒤), JS 블랙보드 `Bsengine.bt.get/set/clear/entity`, 에디터 편집 중엔 틱 안 함. 3단계: 디버거 — `BehaviorTree.active_path`(실행 중 분기, 노드마다 키·태스크·시간이 붙은 라벨)를 Inspector가 반영으로 보여주고, MCP `get_entity`가 `components`(전용 필드 없는 반영 컴포넌트 전부, 타입 경로 → RON)로 트리와 블랙보드를 읽음 — `set_reflected_component`의 읽기 짝. **남음**: 서비스 노드, 서브트리, 핫 리로드. 확인: `grep -n "pub struct BtRuntime" crates/bsengine-core/src/behavior_tree.rs` | Unreal 기본, Unity·Godot은 에셋/플러그인 |
| 운영 | ~~크래시 핸들러·리포트~~ 닫힘(패닉·네이티브) | `panic::set_hook\|minidump\|crash_?report` → 0 → 2026-09-30 `bsengine_core::crash`: 사용자별 디렉터리(Windows `%LOCALAPPDATA%\<프로젝트>`, macOS `~/Library/Application Support`, Linux XDG; `BSENGINE_USER_DIR`로 덮어씀)에 `logs/game.log`(+`game-prev.log`, Unity Player-prev식) 파일 로그, 패닉 훅이 `crashes/crash-<UTC>-<pid>.txt`(메시지·위치·스레드·강제 백트레이스·로그 끝 200줄, 최신 20개 유지). 러너 `build_windowed_app`이 매니페스트 직후·`new_app()` 전에 설치. 그 전엔 **로그 파일 자체가 없었음**(stderr만). 2026-10-07 **네이티브 크래시**: `bsengine_core::native_crash` -- UnityCrashHandler·CrashReportClient·Crashpad처럼 게임이 시작할 때 자기 실행 파일을 `--crash-monitor`로 하나 더 띄우고(minidumper IPC), `crash-handler`의 시그널/SEH 핸들러는 예외 코드만 보내고 덤프를 요청. 감시 프로세스가 `crashes/crash-<UTC>-<게임 pid>.dmp`(미니덤프)와 같은 이름의 `.txt`(예외 이름 `EXCEPTION_ACCESS_VIOLATION`/`SIGSEGV`/`EXC_BAD_ACCESS`, 덤프 이름, 로그 끝 200줄)를 쓰고, 게임이 끝나면(정상 종료 포함) 같이 종료. 덤프를 밖에서 쓰는 이유: 죽은 프로세스의 힙은 못 믿고, Linux는 자기 스레드를 ptrace 못 함(`set_ptracer`로 감시 프로세스 허용). 감시 프로세스를 못 띄우면 경고만, 게임은 패닉 리포트만으로 계속. `--force-crash`(Unity `Utils.ForceCrash`·Unreal `debug crash` 격)로 빌드가 덤프를 남기는지 확인. `.dmp`도 최신 20개만 유지. 세이브 파일은 여전히 cwd(별도 과제). 확인: `grep -n "pub fn init_for_project" crates/bsengine-core/src/crash.rs`, `grep -n "pub fn attach" crates/bsengine-core/src/native_crash.rs` | 셋 다 |
| 플랫폼 | ~~웹~~ 닫힘 · 모바일 남음 | `wasm32\|target_os = "android"\|target_os = "ios"` → 0 → 2026-10-02 사용자 결정: **웹부터, 스크립팅 백엔드 분리**(V8/deno_core는 wasm 타깃 없음 → 네이티브는 V8, 브라우저는 페이지의 JS 엔진). 1단계: 스크립팅·런타임을 뺀 엔진 전체가 `wasm32-unknown-unknown`으로 컴파일 -- getrandom 0.2/0.3·uuid를 브라우저 난수로(`bsengine-core`의 wasm 의존성 + `.cargo/config.toml` cfg), 파일 워처(notify)·에디터·MCP는 네이티브 전용, kira는 바이트에서 디코드, wgpu는 `naga-ir`+naga `wgsl-in`(커스텀 셰이더 검증). CI가 우분투에서 크레이트별 `cargo check --target wasm32-unknown-unknown`. 2단계: **스크립팅 백엔드 분리** -- `#[script_op]`(새 `bsengine-scripting-macros`)가 op 하나를 네이티브에선 `#[deno_core::op2]`로, wasm에선 일반 함수 + 같은 이름 모듈의 `register(ops)`로 바꿈(브라우저 JS 함수 `(...a) => f(a)`가 deno처럼 변환: ToNumber·ToUint32·ToBoolean, 문자열 아니면 `""`, `#[serde]`는 serde-wasm-bindgen에 None→`null`). op 목록은 `with_every_op!` 하나에서 deno 확장과 `register_ops`를 모두 만듦. 브라우저 `ScriptRuntime`은 전역 간접 eval(`var`가 전역에)·`String()` 변환·예외는 `toString()`, `Deno.core.ops`를 등록 객체로. CI가 Node에서 wasm-bindgen-test로 실제 실행(`tests/browser_runtime.rs`). 3단계(2026-10-02): **게임이 브라우저에서 돈다** -- `scripts/build_web.sh <project>`가 Godot·Unity 웹 익스포트와 같은 세 부분을 만듦: `index.html`(페이지, `--package --mode web`이 씀), `game.pak`(매니페스트 포함), `pkg/`(런타임 wasm + wasm-bindgen 글루). 런타임 wasm 진입점(`bsengine-runtime/src/web.rs`)이 `?pak=`(기본 `game.pak`)을 fetch → pak 소스 설치 → 데스크톱과 같은 `build_player_app` → winit `spawn_app`으로 캔버스에. 웹에서 막혔던 것: 디바이스가 future로 와서 `FirstFrameGate`(`bsengine-window`)가 첫 프레임을 디바이스 도착까지 미룸(늦게 오면 `Added<…>` 시스템이 기회를 놓침), `Instant`은 `bsengine_core::clock`(web-time), WebGPU 캔버스엔 sRGB 포맷이 없어 sRGB 뷰로 그림(`srgb_view_of`), Tint가 비균일 흐름의 `textureSampleCompare`를 거부해 `…Level`로, wgpu 22가 보내는 폐지된 한도 `maxInterStageShaderComponents`를 JS에서 제거, 오디오는 첫 입력에 AudioContext 재개(Unity·Godot 웹과 같음). CI(우분투)가 cube-evader를 빌드해 헤드리스 Chrome + SwiftShader WebGPU로 `scripts/web_smoke.mjs` 실행 -- 프레임·드로 콜과 콘솔/WebGPU 경고(Chrome은 셰이더 거부를 경고로 냄)로 판정. 남음: 모바일; pak 빌드의 에셋 identity 인덱스(아카이브에 `.meta`가 없어 데스크톱 pak 빌드와 같은 경고). 확인: `grep -n "wasm32\|web_smoke" .github/workflows/ci.yml` | 셋 다(콘솔은 셋 다 별도 SDK) |

**있는 것으로 확인된 것**(존재만, 품질 비교는 아님): 메시 LOD(`LodLevels`), 오클루전·프러스텀 컬링, SSAO, 블룸, 톤맵·노출,
TAA, SSR, 데칼, 볼류메트릭 포그, 라이트 프로브, IBL, 래그돌, 차량, IK, 리타기팅, 타임라인, 에디터 언두, 세이브, 게임패드, 오디오
리버브 버스·오클루전, 천, 내비메시, 네트워킹 RPC, 터레인, 비주얼 스크립팅, 셰이더 그래프.

### 구조 문제

- **ECS가 단일 스레드.** `Cargo.toml`의 `bevy_ecs`·`bevy_app`이 `default-features = false`에 `multi_threaded`가 없어 모든
  스케줄이 단일 스레드 실행기로 돈다(`grep -c multi_threaded Cargo.toml` → 0). 렌더도 별도 스레드 없이 같은 스레드. Unity
  (Job System·DOTS)·Unreal(태스크 그래프·렌더 스레드)과 규모 차이의 뿌리.
- **에디터 플러그인 한 파일이 94,553줄**(`crates/bsengine-editor/src/plugin.rs`; 제품 코드 약 3만 줄 + 30,272행부터 테스트).
  다음으로 큰 것은 `scripting/src/ops.rs` 11,939줄, `rhi-wgpu/src/surface.rs` 8,490줄.
- **삭제된 zoo 컴포넌트의 주석 잔재.** `scripting/src/ops.rs`에 `// <이름>: <필드들>` 형태 220줄(1888~2110행), DOF·모션 블러·컬러
  그레이딩 스냅샷 자리에는 static 없이 주석만 남음.
- **실제 GPU 검증 없음.** CI는 lavapipe(Ubuntu)·Metal(macOS)·Windows 러너라 BC 압축·성능 수치가 실제 데스크톱 GPU에서
  확인된 적 없다(아래 "플랫폼" 절과 같은 사실).

### 내 최근 작업의 결함 — 압축 텍스처가 패키지에 인코딩된 채 실리지 않는다

세 엔진 모두 **빌드 때** 압축된 블록을 게임에 싣는다(Unity Library → 빌드, Unreal 쿠킹, Godot `.ctex`). #1904는 **런타임 첫
업로드 때** 인코드하고 결과를 `<프로젝트>/.bsengine_cache/mips`에 캐시한다. 패키저(`bsengine-asset/src/cook.rs`)는 밉
캐시를 다루지 않으므로(`grep -ciE "compress|mips" crates/bsengine-asset/src/cook.rs` → 0):
- 플레이어마다 첫 실행에 2048² 한 장당 약 0.5초(릴리스)를 프레임 스레드에서 치른다.
- 캐시 디렉터리에 쓸 수 없는 설치 위치(읽기 전용)에서는 **매 실행** 인코드한다(쓰기 실패 시 체인이 RAM에만 남음).
- 기본값이 `None`이라 E2E·패키징 테스트는 이 경로를 한 번도 지나지 않는다 — 그래서 CI가 못 잡았다.
지금 이 경로를 타는 게임은 없다(모든 `.meta`가 기본값). 고칠 방향: `cook`이 압축 텍스처의 블록 체인을 인코드해 pak에
넣고, 런타임은 pak의 블록을 먼저 읽는다.

**→ 닫힘(2026-09-29, 4단계 첫 PR).** `--package`가 사이드카에 압축이 켜진 텍스처마다 밉 캐시 파일을 미리 만든다
(`bsengine_asset::cook::package_with_precook` + `bsengine_rhi_wgpu::precook_mip_cache`; 에셋 크레이트는 GPU 크레이트에
의존하면 안 되므로 함수로 넘긴다). 파일 이름은 디코드된 픽셀·크기·인코딩의 해시라 런타임이 찾는 이름과 같고, 바이트까지
같음을 테스트가 단언한다. 두는 곳은 `.bsengine_shipped/mips`: loose 패키지는 그 디렉터리, pak·단일 실행 파일은 같은
접두사의 아카이브 항목. **쓰기 가능한 캐시(`.bsengine_cache/mips`)와 일부러 분리** — 그쪽은 시작할 때 2주 미사용 파일을
청소하므로, 거기 두면 한 달 만에 켠 플레이어가 동봉 파일을 잃는다. 레지스트리는 인코드 전에 동봉 파일을 먼저 찾고
(`set_shipped_mip_cache`), 창·헤드리스 런타임은 pak이 있으면 아카이브 조회를 설치한다. 확인:
`grep -n "precook_mip_cache\|SHIPPED_MIP_DIR" crates/bsengine-rhi-wgpu/src/texture.rs`.

### 순서 (2026-09-29, 사용자 결정)

**기록 → 삭제·정리 → 구조 수정 → 나머지 기능 격차.**
1. **기록** — 이 절.
2. **삭제·정리** — zoo 주석 잔재 등 죽은 코드. 구조 작업 전에 해서 옮길 코드를 줄인다.
3. **구조** — 에디터 플러그인 파일 분할(기계적, 위험 낮음, 이후 모든 작업의 충돌을 줄임)을 먼저, 그다음 ECS 멀티스레딩.
   기능을 넣기 전에 해서 새 기능이 단일 스레드를 전제로 짜이지 않게 한다.
4. **기능 격차** — 맨 앞은 위 압축 패키징 결함, 그다음 게임을 만들 때 먼저 부딪히는 것(캐릭터 컨트롤러, 입력 액션 맵),
   그다음 기본 후처리(반사 프로브, DOF, 컬러 그레이딩).

## 현재 남은 작업 (2026-09-28, 전부 grep으로 부재 확인함)

master `bf2c649b` 기준. 열린 PR·이슈 0개, 소스 TODO/FIXME 0개, 워크스페이스 2,176 +
에디터 949 테스트 통과, Windows·Linux·macOS 3플랫폼 CI 초록. 2026-09-22 표의 항목은 전부
닫혔고(#1888·#1893·#1899), 아래 세 줄은 #1895~#1899를 하면서 드러난 다음 격차다.

### 코드에 없는 것 (확인 명령 포함)

| 항목 | 확인 | 메모 |
|---|---|---|
| **macOS FSEvents rename 페어링** | 백엔드 측정 테스트 2개만 macOS `#[ignore]` | #1888: 엔진이 더는 백엔드의 짝맞춤에 의존하지 않음 — 파일 identity(inode)로 잃어버린 반쪽을 재구성. 런타임 복구 테스트 2개 + 실백엔드 rename 테스트 1개를 macOS에서 다시 켬. 아래 절 참조 |
| ~~텍스처 스트리밍 2단계(거리 기반 목표 밉·메모리 예산·퇴거)~~ | `grep -rn "wanted_base\|budget_bytes" crates/` | 2026-09-26 구현(아래 "텍스처 스트리밍 2단계" 절): 카메라 거리·화면 크기 기반 목표 밉, `[render]` 예산·밉 바이어스, 예산 초과 시 퇴거, 프로파일러 스탯. 디스크에서 스트리밍(체인을 RAM에 둠)은 여전히 없음 |
| ~~단일 실행 파일~~ | `grep -rn "BSEMBED1\|PackageMode::Single" crates/` | 2026-09-28 구현(아래 "단일 실행 파일" 절): `--mode single`이 아카이브(매니페스트 포함)를 exe 꼬리에 임베드, macOS는 옆에 |
| ~~**스카이박스·터레인이 `TextureCache`를 안 거침 → `release_pixels` 기본 off**~~ | `grep -n "&tex.data" crates/bsengine-render/src/plugin.rs crates/bsengine-app/src/terrain.rs` → 0 | 2026-09-28 닫힘, 두 PR: #1901이 둘을 `TextureCache::upload`로 돌리고(스카이박스는 GPU→GPU 복사 + 세대 카운터), 그다음 PR이 기본값을 `true`로(Unity의 Read/Write Enabled 반대). 아래 "`release_pixels` 기본값 on"·"스카이박스·터레인 레이어가 `TextureCache`를 거친다" 절. **2026-09-28 사용자 결정: 아래 세 항목을 이 순서로 하나씩** |
| ~~**디스크 밉 읽기가 동기**~~ | `grep -n "levels_from(base)" crates/bsengine-rhi-wgpu/src/texture.rs` → 0(`PendingRead`·`poll_reads`) | 2026-09-28 닫힘(아래 "디스크 밉 읽기 비동기" 절): 올리기는 새 레벨 하나만 워커 스레드에서 읽어 도착한 프레임에 GPU 복사+쓰기로 재구성, 내리기는 디스크·CPU 없이 GPU 복사. 릴리스 측정치는 그 절에 |
| ~~**텍스처 압축/포맷 변환 없음**~~ | `grep -rn "TextureFormat::Bc1\|TEXTURE_COMPRESSION_BC" crates/bsengine-rhi-wgpu/src/` → `texture.rs`·`surface.rs`·`profiler.rs` | 2026-09-28 닫힘(아래 "텍스처 압축" 절): 임포트 세팅 `compression: None|Bc1|Bc3`(기본 None), texpresso(순수 Rust) 클러스터 핏으로 업로드 때 인코드, 압축 체인은 밉 캐시(`BSMIPS02`, 인코딩 태그)에 저장해 한 번만 인코드, `TEXTURE_COMPRESSION_BC` 없는 디바이스·4의 배수 아닌 이미지는 RGBA8 폴백(경고). ASTC/ETC2(모바일)·BC7은 여전히 없음 |

**순서(2026-09-28, 사용자 "하나씩 순차"):** 공유 GPU 사본 → 디스크 읽기 비동기 → 텍스처 압축. 앞의 둘이 작고
스트리밍 경로를 정리하며, 압축은 `TextureAsset.data`가 담는 것과 밉 캐시가 저장하는 것을 바꾸므로 정리된 기반 위에.

### 플랫폼 — 무엇이 검증됐고 무엇이 안 됐나

**검증됨(매 PR)**: Windows·Linux(Ubuntu)·macOS 3종에서 빌드, 전체 테스트, E2E 리플레이,
패키징, 그리고 **실제 창이 열리고 프레임이 그려지는지**(#1869의 `window_smoke.rs`).

**미검증**: ⚠️ Ubuntu는 Xvfb + 소프트웨어 Vulkan(lavapipe)이라 **실제 GPU·오디오 장치·
Wayland**는 아무 플랫폼에서도 검증된 적 없다. 모바일·콘솔 없음.

### 이미 닫혔지만 아래 옛 절들이 "없음"이라 적고 있는 것

`grep`으로 존재 확인함 — **아래 날짜 절의 서술을 믿지 말 것**:
네트워킹 RPC(11파일) · 풀바디 IK(4) · 데칼(11) · 천/소프트바디(3) · SSR(8) ·
캐스케이드 섀도우(18) · 씬 스트리밍(6) · **GPU 스키닝(#1884, `rhi-wgpu/src/skinning.rs`)** ·
**텍스처 밉 스트리밍 1단계(#1885, `TextureImportSettings::streaming`)**.

**에셋 의존성 그래프 — 데이터·질의(2026-09-25, #1886).** 세 엔진 조사: Unity는 `AssetDatabase.GetDependencies`
(의존만, 역참조 뷰어 없음), Unreal은 Reference Viewer(참조자 ← 에셋 → 의존 그래프), Godot 4는 Dependency
Editor + View Owners + Orphan Resource Explorer. 수렴점은 **에셋별 의존/참조자 + 아무도 안 쓰는 에셋 목록**이라
그것을 먼저 넣었다. 새 워커를 짜지 않고 **패키저의 정적 걷기(`bsengine_asset::cook`)가 이미 따라가던 참조를
`(참조자, 에셋)` 간선으로 남기게** 했다(첫 방문에만 기록하면 "누가 쓰나"가 큐 순서에 따라 달라짐). 같은 걷기라
인스펙터의 "Not reached from the entry scene"은 정확히 "패키지 빌드에 빠진다"와 같은 뜻이다. `cook_project`가
`project.toml`의 `entry_scene`·`extra_assets`를 읽어 에디터·MCP가 런타임의 매니페스트 타입 없이 같은 걷기를
쓴다.

**비주얼 스크립팅 1단계 — 모델과 컴파일러(2026-09-25, #1889).** 사용자가 "만들자"를 택했고(비교 문서 아래 5절의
"AI-native와 어긋난다"는 판단은 그대로 남긴다), 셰이더 그래프 선례를 그대로 따랐다: `.scriptgraph.ron` 노드 그래프를
**런타임이 이미 읽는 `.js`로 컴파일**한다. 런타임·핫리로드·패키저·`api.d.ts`·MCP 도구는 한 줄도 안 바뀌고, 생성된
텍스트가 1급이라 에이전트가 읽고 고친다. 세 엔진 조사: Unreal Blueprint·Unity Visual Scripting·Godot 3 VisualScript
(4에서 제거)가 모두 **흐름 포트 + 데이터 포트, 이벤트 노드, Branch/Sequence, 변수, 엔진 API 호출**로 수렴한다 — 그것이
노드 집합이다(`OnStart`/`OnUpdate`/`OnKeyPressed`/`OnCollision`, `Branch`, `Sequence`, `Literal`, `SelfEntity`,
`GetVar`/`SetVar`, `Call(op)` 44개, 산술·비교·논리, `Vec3Make`/`Vec3Split`). 갈리는 건 실행 방식(Unreal 컴파일, Unity
인터프리트)이고 우리는 컴파일. 런타임이 엔티티당 `onUpdate(self)` 하나만 부르므로 모든 이벤트를 거기서 분기한다
(`OnStart`=첫 프레임 가드, `OnKeyPressed`=`isKeyPressed` 검사, `OnCollision`=첫 프레임에 콜백 등록). 컴파일 출력은
사람이 쓴 것처럼 읽히도록 **통째로 핀 고정**했고, 생성된 JS가 실제 V8에서 엔티티를 움직이는 것을 `bsengine-scripting`
테스트가 확인한다.

**비주얼 스크립팅 2단계 — Script Graph 패널(2026-09-25, #1890).** 셰이더 그래프 패널과 같은 뼈대(툴바·Painter 캔버스·
포트 히트테스트·컴파일러의 포트 표로 연결 허용/거부)에 스크립트 그래프에만 있는 둘을 더했다: **출력 포트가 여럿인 노드**
(`Branch`의 흐름 둘, `Vec3Split`의 값 셋)라 출력도 입력처럼 행으로 배치하고, **인라인 파라미터**(리터럴 값·키 이름·변수
이름·비교 연산자)를 선택한 노드의 편집 행과 변수 목록에서 고친다. 흐름 출력은 Blueprint처럼 한 곳으로만 이어지며 두 번째
연결은 첫 번째를 대체한다(컴파일러의 `AmbiguousFlow`와 같은 규칙). Compile은 `bob.scriptgraph.ron` 옆에 `bob.js`를 쓴다.
`egui`가 스크롤 영역 밖 위젯을 그리지 않아 44개 호출 메뉴는 데이터로 단언했다. 이로써 **비교 문서 표에 단일 실행 파일만
남았다.**

**비주얼 스크립팅 3단계 — 루프·타이머·스코프 검사(2026-09-26).** 1단계가 남긴 노드 격차를 Blueprint의 이름 그대로
닫았다: `ForLoop`(first..last, `body`/`completed`/`index`), `WhileLoop`(Blueprint의 폭주 루프 가드처럼 한 프레임
`LOOP_LIMIT`=100,000회에서 로그를 남기고 끊는다 — 한 프레임은 다른 모든 엔티티의 스크립트가 기다리는 V8 틱이라
Blueprint의 백만보다 낮게), `Delay`(프레임 수, `Bsengine.setTimeout`으로 다음 프레임에 이어짐), `OnInterval(초)`
(모듈 레벨 누산기에 `getDeltaTime`을 더해 주기마다 실행), HUD 문자열용 `ToText`/`Concat`. `Call` 표는 44→68개
(방향 벡터·속도·질량·타이머·카메라 FOV·재질 등, 각각 프렐류드의 시그니처를 확인해 넣음). **`OnCollision`의 `other`와
`ForLoop`의 `index`는 그 흐름 안에서만 존재하는 값**이라, 밖에서 읽으면 생성된 JS가 첫 프레임에 ReferenceError를
내던 것을 컴파일 시 `GraphError::OutOfScope`로 막았다(컴파일러가 콜백·루프 본문을 쓰는 동안 스코프 스택을 유지하고,
흐름 노드 인자와 식 피연산자 두 경로 모두에서 검사 — 1단계에서 한쪽만 검사한 뮤테이션이 살아남았던 그 두 경로).
`bsengine-scripting`의 E2E는 고정 0.25s 클럭에서 for 본문이 프레임당 3번, 2프레임 딜레이가 정확히 두 번째 프레임에,
0.5s 인터벌이 두 프레임에 한 번(세 번째 프레임엔 안) 도는 것을 잰다.

**비주얼 스크립팅 4단계 — 캔버스 팬/줌과 MCP 도구(2026-09-26).** 패널에 `View{pan, zoom}`을 두어 노드 `position`
(그래프 좌표, 파일에 저장)과 화면 좌표를 분리했다 — 스크롤했다고 파일이 바뀌면 안 된다. 세 엔진 조사: 휠 줌은
Blueprint·Unity VS가 맨 휠, Godot은 Ctrl+휠; 팬은 Blueprint 우클릭 드래그, Unity·Godot 가운데 버튼 드래그. **수렴점은
"휠은 포인터를 중심으로 줌, 주 버튼이 아닌 드래그는 팬"**이라 맨 휠(+Ctrl/핀치)로 줌하고 보조·가운데 버튼 모두
팬한다. 노드 드래그는 화면 거리가 아니라 그래프 거리로 움직이고(200%에서 두 배 멀리 가면 안 됨), 포트 히트 반경은
줌에 비례, **Fit**(Blueprint Home / Unity F)은 전체 노드를 화면에 넣되 1:1 이상으로 키우지 않는다. 팬 뒤에 추가한
노드는 원점이 아니라 보고 있는 곳에 놓인다. MCP `script_graph_compile {game, path}`는 패널의 Compile과 같은 일을
에이전트에게 준다: `.scriptgraph.ron`을 컴파일해 옆에 `.js`를 쓰고 JS를 돌려주며, 그래프 오류면 아무것도 안 쓰고
노드·포트를 지목한다. 도구 설명이 RON 형식·노드 종류·포트와 **`OPS` 표에서 생성한 호출 68개의 시그니처**를 담아
에이전트가 텍스트 없이 그래프를 저작할 수 있고, 표에 op를 더하면 설명도 따라온다(테스트가 대조).

**macOS 워처 — rename 재구성(2026-09-25, #1888).** #1871/#1872가 남긴 문제는 FSEvents가 같은 디렉터리 rename의
옛 경로 이벤트를 실행마다 다르게 떨어뜨리는 것이었고, 기록기는 디바운서가 짝지은 `[from, to]`만 봤다. 추측으로
CI를 한 번 더 돌리는 대신 **백엔드와 무관한 메커니즘**을 넣었다: 워처가 감시 대상 파일을 `file_id::FileId`(inode/
파일 인덱스 — rename에도 변하지 않는 유일한 것)로 기억하고, 경로 하나짜리 이벤트의 identity가 이미 다른(이제 없는)
경로에 있었다면 그것이 옛 반쪽이 사라진 rename이다. 지워진 경로는 잊어서 재활용된 inode가 옛 파일로 오인되지 않게
했다. **모든 플랫폼에서 재현되는 테스트**가 디스크에서 rename한 뒤 드레인에 `Create(new)` 하나만 손으로 먹여 사이드카
이동과 `former_paths`를 단언한다. 백엔드의 짝맞춤을 *측정*하는 테스트 2개는 FSEvents의 사실이므로 macOS에서 그대로
ignore; 엔진의 성질을 보는 3개는 다시 켰다 — macOS CI가 그 답이다.

**에셋 의존성 그래프 — 시각화(2026-09-25, #1887).** Unreal Reference Viewer 배치 그대로: 선택한 에셋을
가운데, 참조자를 왼쪽 열, 의존을 오른쪽 열에 놓고 선으로 잇고, 노드를 클릭하면 그 에셋으로 재중심(선택이
`select_asset`을 지나므로 인스펙터도 따라온다). Unreal 기본값처럼 **한 단계**만 — 프로젝트 전체 그래프는
털뭉치이고 한 에셋에 대한 질문은 이웃으로 답이 난다. 아래에 Godot Orphan Resource Explorer 격인 "Unreferenced"와
"Missing"(참조자 → 없는 파일) 목록. 그래프는 패널이 처음 열릴 때·Refresh·**씬 저장 후**에만 다시 걷는다
(`asset_graph_refresh` 플래그 — 스냅샷을 버리는 대신 플래그라 새 그래프가 올 때까지 옛 것을 그린다).
`render_frame`이 Bevy 파라미터 천장에 있어 그래프도 `InspectorState` 안에 산다([[project_editor_panel_registration]]
제약과 같은 이유). 이로써 **비교 문서 표에서 macOS FSEvents·비주얼 스크립팅·단일 실행 파일만 남았다.**

**텍스처 스트리밍 1단계(2026-09-24, #1885).** 사이드카의 `streaming: true`(Unity의 텍스처별
"Streaming Mipmaps", Unreal "Never Stream"의 반대; Godot 4엔 없음)가 켜진 텍스처는 로드 시
**최대 변이 64px 이하인 밉만 GPU에 올리고**, 큰 레벨은 이후 프레임당 하나씩 들어온다
(`stream_textures`, 텍스처 간 라운드로빈). wgpu 텍스처는 선언한 모든 밉을 할당하므로 "부분
상주"는 플래그가 아니라 **상주 레벨만큼의 크기로 만든 텍스처 오브젝트**이고, 레벨이 들어오면
오브젝트·뷰·바인드 그룹을 다시 만든다(Unreal과 같은 재할당 방식). 256² 텍스처가 341KiB 대신
21KiB로 시작하는 것을 프로파일러 크기로 단언했고, 픽셀 테스트는 **바인드 그룹을 갱신하지 않는
뮤테이션**(풋프린트 테스트는 전부 통과하지만 화면은 영원히 흐릿함)을 잡는다. ⚠️ 에셋 코퍼스의
이미지는 전부 합쳐 1KiB 미만이라 **필요가 측정된 게 아니라 사용자가 "구현"을 고른 것**이다.
2단계(카메라 거리 기반 목표 밉·메모리 예산·퇴거·`project.toml` 설정)는 위 표에 남겨 뒀다.

**텍스처 스트리밍 3단계 — 디스크에서(2026-09-27).** 1·2단계는 밉 체인 전체를 RAM에 뒀다(이미지의 1.33배, 그것도
`Assets<TextureAsset>`의 원본 픽셀과 별도로). 세 엔진의 수렴점: Unity는 Library의 임포트 사본, Unreal은 쿡된 `.ubulk`,
Godot은 `.godot/imported/`의 `.ctex` — **밉이 전부 들어 있는 쿡 파일을 디스크에 두고 레벨을 필요할 때 읽으며, 디코드는
임포트 때 한 번**. 구현: 스트리밍 텍스처를 처음 올릴 때 계산한 체인을 `<프로젝트>/.bsengine_cache/mips/<blake3(픽셀+크기)>.mips`
(헤더 + 레벨별 (w,h,offset,len) 표 + 원시 RGBA)로 쓰고, 레지스트리는 **체인을 RAM에 전혀 두지 않는다** — 레벨을 올리고
내릴 때마다 필요한 레벨을 파일에서 읽는다(내리는 레벨의 합은 새 레벨의 1/3이라 읽기도 그만큼). 파일 이름이 내용 해시라
같은 이미지는 파일 하나를 공유하고 재임포트는 새 파일을 얻는다(옛 파일은 썸네일 캐시처럼 남는다). 캐시 디렉터리는
`WgpuRHIPlugin`이 `ProjectDir`(없으면 cwd) 아래로 정하고, 못 쓰는 곳(읽기 전용 설치)이면 한 번 경고하고 RAM으로
되돌아간다; 디렉터리를 안 정한 레지스트리(테스트·터레인·glTF 하네스)는 이전처럼 RAM. 파일이 도중에 사라지면 raise는
거부되고 상주는 그대로다. 관측: `chain_ram_bytes`(캐시면 0, 아니면 체인 전체 — "절약"을 믿지 않고 잰다),
`mip_cache_file`. 테스트는 파일 하나·RAM 0·GPU 풋프린트 동일, 같은 픽셀 재사용(mtime 불변)·다른 픽셀 새 파일, 파일의
128 레벨을 덮어쓴 뒤 raise가 그 바이트를 읽는 것, 파일 삭제 후 raise 거부, 디렉터리 불가 시 RAM 폴백, 잘못된 파일 재작성.
`Assets<TextureAsset>`의 원본 픽셀은 핫리로드 핸들이 붙들고 있어 그대로 — 다음 단계가 있다면 그것.

**`release_pixels` 기본값 on(2026-09-28).** 스카이박스·터레인이 캐시를 거치게 된 뒤(아래 문단) 남은 마지막 단계. Unity는
Read/Write Enabled가 기본 off — 업로드 뒤 CPU 사본을 버린다 — 이고 저자가 켜야 남는다. 여기서도 `TextureImportSettings::
release_pixels`의 기본을 `true`로: 사이드카가 없는 파일, 필드가 생기기 전에 쓴 사이드카(`#[serde(default = …)]`가 필드 기본을
줌 — "옛 동작"이 아니라 Unity의 선택) 모두 업로드 뒤 해제. `false`로 쓰면 유지(CPU에서 읽는 이미지용 — 터레인 스플랫맵이
동시에 머티리얼 텍스처인 경우). `raw()`(메모리 바이트 업로드)는 해제할 에셋이 없으니 그대로 off. 핀된 RON 문자열 두 곳·옛
사이드카 테스트·캐시 테스트의 전제("기본은 유지" → "기본은 해제, false면 유지")를 바꿈. 이로써 위 표의 첫 줄이 닫힌다.

**스카이박스·터레인 레이어가 `TextureCache`를 거친다(2026-09-28).** #1897이 `release_pixels`를 옵트인으로 둔 이유 하나를
지운다. 스카이박스는 자기 `AssetSlot`으로 이미지를 요청해 `tex.data`를 `set_skybox_from_rgba`로 올렸고, 터레인은 레이어 4장을
자기 슬롯으로 받아 `load_with`로 올렸다 — 머티리얼이 같은 이미지를 먼저 올려 픽셀을 해제하면 둘은 빈 버퍼를 만났다. 이제 둘 다
`TextureCache::upload(path)`(공개로 승격)로 **캐시가 소유한 하나의 GPU 사본**의 id를 받는다. 스카이박스는 그 레지스트리 객체를
`set_skybox_from_texture`로 **GPU→GPU 복사**해 자기 텍스처(sRGB·자체 샘플러: 가로 Repeat·극 Clamp)를 만든다 — 레지스트리
뷰 위에 바인드 그룹을 만들면 핫리로드·스트리밍으로 객체가 바뀔 때 옛 객체를 붙들고 계속 그리기 때문. 그 변경을 알기 위해
`GpuTextureRegistry`에 **텍스처별 세대 번호**(`generation(id)`: 리로드·레벨 in/out마다 새 번호, 다른 텍스처엔 무영향)를 두고
`sync_skybox`가 매 프레임 비교해 다시 복사한다; 에셋 이벤트를 듣던 `rebuild_modified_skybox`는 사라졌다. 레지스트리 텍스처는
`COPY_SRC`를 얻었다. 터레인 스플랫맵은 CPU에서 블렌드 가중치로 읽는 **데이터**라 슬롯을 유지하고 아무도 업로드·해제하지
않는다(같은 파일이 머티리얼 텍스처이기도 하면 경고 후 포기). ⚠️ 옛 스카이박스 테스트 5개는 `PendingSkybox` 내부(슬롯·핸들)를
봤는데 그 기계가 없어져, **공유 디바이스 위 실제 오프스크린 서피스**(`WgpuSurface::offscreen_for_testing`, 디바이스 예산 0)에서
`has_skybox`·`loaded_skybox_path`·업로드 횟수·세대를 관측하도록 다시 썼다 — "중간 전환 시 옛 요청 포기"는 캐시 설계에서
구조적으로 무의미해졌고(둘 다 캐시에 남지만 화면엔 `SkyboxPath`가 원하는 것만), 대신 "전환 뒤 되돌리면 재업로드 없이 복사"를
잰다. 기본값 뒤집기(Unity의 Read/Write 반대 = 해제 on)는 다음 PR.

**디스크 밉 읽기 비동기(2026-09-28).** 남은 작업 ②. 3단계(#1895)의 `set_residency`는 프레임 스레드에서 캐시 파일을 열어
상주 레벨 전부를 읽었고(올리기), 내리기도 작은 레벨들을 다시 읽어 재업로드했다. Unreal은 밉당 비동기 IO 요청을 내고 도착한
프레임에 업로드, Unity는 백그라운드에서 로드해 적용 — 그대로 따랐다. `raise_residency`는 캐시 파일이면 **새 레벨 하나만**
워커 스레드(`PendingRead`: 스레드 하나, `read_exact` 하나; 결과는 `Arc<Mutex<Option<…>>>` 슬롯 — 레지스트리가 리소스라
`Receiver`는 `!Sync`)로 읽기 시작하고 즉시 돌아온다. 상주는 그대로이고 읽기 중인 텍스처는 `step_streaming`·raise·lower가
전부 건드리지 않는다(읽은 레벨이 요청 당시 상주에 착륙해야 하므로). `poll_reads`가 프레임마다(`stream_textures`가 `set_wants`
전에) 도착한 바이트로 객체를 재구성한다. **재구성(`rebuild_object`)은 새 레벨 하나만 쓰고 옛 객체가 이미 가진 레벨은
GPU→GPU 복사**(#1901의 `COPY_SRC`) — 그래서 **내리기는 디스크도 RAM도 전혀 안 읽고**, RAM 체인의 올리기도 전체 재업로드가
아니라 레벨 하나 쓰기가 됐다. 실패한 읽기(파일 없음)는 경고 후 상주 유지·재요청 가능. 관측: `disk_reads() -> (횟수, 바이트)`
(착륙 시 카운트), `has_pending_read`, 테스트 전용 `read_level0_for_testing`(GPU 레벨 0을 읽어 파일에서 변조한 바이트가 실제로
GPU에 올라갔는지 확인). 테스트: 요청 직후 상주 불변·중복 요청 거부·착륙 후 (1, 128²·4) 읽기·GPU 바이트 = 파일·내리기 0 읽기·
복사된 64 레벨 = 원본·파일 삭제 시 실패 처리; 렌더 플러그인은 자기 프레임만으로 캐시 파일 텍스처를 완전 상주까지(읽기 정확히
2회). 뮤테이션 A1~A7. **릴리스 측정(2048², 레벨0 16MiB, NVMe, `measure_raise_cost_from_a_cache_file`)**: 옛 경로가 체인 전체
22.4MB를 프레임 스레드에서 읽는 데 **6.6ms**(base 0으로 올릴 때 그렇게 읽었다; 그 위에 전 레벨 재업로드). 새 경로는 요청이
프레임 스레드에서 **46~97µs**, 레벨은 워커에서 1.3~5.4ms 뒤 도착, 착륙(레벨 하나 `write_texture` + GPU 복사)이 프레임
스레드에서 **0.2~1.7ms**(가장 큰 레벨 1.70ms). NVMe에서도 프레임당 ~5ms를 덜고, 느린 디스크에선 프레임이 읽기를 기다리는
일 자체가 없어진다. ⚠️ 테스트 디바이스의 2D 한계가 2048이라 4096²는 만들 수 없다(첫 측정이 그것으로 실패).

**텍스처 압축 — BC1/BC3(2026-09-28).** 남은 작업 ③, 셋 중 마지막. 세 엔진 모두 임포트 때 압축하고 런타임은 블록을 그대로
올린다(Unity: 플랫폼별 DXT/BC7/ASTC, Unreal: DXT1/DXT5/BC7, Godot: `.ctex`에 S3TC/ETC2). 그대로 따르되 데스크톱 wgpu가 주는
`TEXTURE_COMPRESSION_BC`만: 임포트 세팅에 `compression: None | Bc1 | Bc3`(`.meta`·Inspector 콤보·MCP `asset_import_settings`),
**기본은 None** — 픽셀 테스트·E2E가 정확한 텍셀을 단언하고, Unity도 압축은 플랫폼 기본이지 파일 기본이 아니다. 인코더는
**texpresso**(libsquish의 순수 Rust 포팅; ISPC(intel_tex_2)도 C++(basis-universal)도 빌드에 안 들어옴) 클러스터 핏 + rayon —
품질 쪽을 고른 건 결과가 디스크에 캐시돼 이미지당 한 번만 치르기 때문. **밉 체인은 RGBA8로 만든 뒤 레벨마다 인코드**(압축된
이미지의 밉은 블록 아티팩트의 블러). 압축 체인은 **스트리밍 여부와 무관하게 항상 밉 캐시에 저장**(`BSMIPS02`: 헤더에 인코딩
태그, 파일명 해시에도 태그 — 같은 이미지의 RGBA8 체인과 BC1 체인은 다른 파일, 4×4 이미지의 BC1과 BC3는 길이가 같아 태그가
가른다). 두 번째 업로드부터는 인코드 0회(`encodes()` 카운터). 스트리밍 올리기는 캐시 파일에서 **압축 블록을 읽어** 그대로
`write_texture`. 지원 없는 디바이스는 RGBA8 폴백(한 번 경고), **4의 배수가 아닌 이미지도 RGBA8 폴백**(이미지마다 경고; Unity가
같은 경고로 거부한다. wgpu는 BC 텍스처 레벨 0이 블록 배수가 아니면 `create_texture`를 거부). 그래서 **스트리밍 바닥**
(`Streamed::floor`): 압축 체인의 상주는 레벨 0부터 이어지는 4의 배수 레벨 중 마지막(64²면 4×4, 96²면 12×12)까지만 내려간다 —
2×2·1×1은 어떤 객체의 *밉*으로는 있어도 객체의 레벨 0은 못 된다. `set_wants`도 예산 퇴거도 그 바닥에서 멈춘다. 복사·쓰기
extent는 블록 단위로 올림(`copy_extent_for`; 2×2 밉을 2×2로 쓰면 `Copy width is not a multiple of block width`). 객체의 포맷은
업로드 때 한 번 정해 `GpuTexture::format`에 두고 재구성은 그걸 쓴다(세팅에서 매번 다시 정하면 두 검사를 같은 답으로 반복해야
함). 프로파일러 `level_bytes`가 블록 단위로 세므로 예산·풋프린트가 실제 크기(64² BC1 전체 체인 = 343블록×8B = 2,744B, RGBA8은
21,844B). 스카이박스 GPU→GPU 복사는 원본 포맷 그대로라 압축 스카이박스도 된다. 테스트: 레지스트리 단위 3개(포맷·블록 풋프린트·
폴백·인코드 횟수 / 캐시 파일 한 번 인코드·RGBA8 체인과 분리·스트리밍 올리기가 압축 블록 읽음 / 바닥·want 클램프·예산 0에서
None·66×40 폴백), 픽셀 2개(`pixels_compression.rs`: 4×4 블록 정렬 흑백 체커는 BC1이 무손실이라 **RGBA8 렌더와 채널 차 ≤ 1**을
전체 프레임에서 단언 + 메모리 1/8·1/4; 전제로 양쪽 색이 화면에 있음을 센다 — 첫 판은 카메라 z=4에서 전부 어두워 전제가 잡았다).
뮤테이션 T1~T9(T5 인코딩 태그는 처음 살아남음 — 오늘의 세 인코딩은 레벨 길이만으로 갈리므로; 태그 전용 테스트 추가).
**첫 업로드 인코드 비용(릴리스, 24스레드, texpresso 단독 측정 `scratchpad/encode_bench`)**: 클러스터 핏 512² 41ms · 1024² 138ms ·
**2048² 541ms**(BC3 482ms); 레인지 핏은 2048² 17ms. 즉 압축 텍스처 하나당 첫 실행에 최대 0.5초(+밉 체인 1/3)를 프레임 스레드에서
치르고 그 뒤는 캐시. `.bsengine_cache`는 gitignore라 클론 직후 첫 실행이 그 비용을 전부 낸다 — 텍스처 수십 장이면 수십 초.
품질을 고른 근거는 위와 같지만, 첫 실행이 문제가 되면 `encode_level`의 `Params`를 `RangeFit`으로 바꾸는 한 줄(30배 빠름,
밴딩 증가)이거나 패키징(`cook`) 때 캐시를 미리 채우는 것이 다음 단계. ASTC/ETC2(모바일 타깃 없음)·BC7(texpresso에 없음)은
남는다.

**단일 실행 파일 — `--mode single`(2026-09-28).** 로드맵 item 55에서 의도적으로 범위 밖에 뒀던 마지막 항목. 선례는 갈린다:
Unity(`exe + Data/`)·Unreal(`exe + Content/Paks/`)은 단일 파일을 만들지 않고, **Godot만 "Embed PCK"로 `.pck`를 내보내기
템플릿 바이너리 뒤에 붙이고 끝에 오프셋+매직을 써서 실행 중인 바이너리가 자기 꼬리를 읽는다.** 사용자가 "가장 좋은 쪽"을
골라 Godot을 따랐다. 포맷(`bsengine-asset/src/embed.rs`): `[exe][game.pak][u64 offset][u64 len]["BSEMBED1"]` — 트레일러가
맨 끝이라 exe 길이를 몰라도 seek 한 번으로 찾고, 매니페스트는 pak 안에 `project.toml` 엔트리로 실어 `exe + project.toml +
game.pak` 세 파일이 **한 파일**이 된다. 매직은 있는데 오프셋이 안 맞으면 "없음"이 아니라 **오류**(깨진 빌드가 옆에 굴러다니는
파일로 다른 게임을 시작하면 안 됨). 런타임은 `current_exe()` 꼬리에서 먼저 찾고 없으면 지금처럼 옆의 `game.pak`; 매니페스트는
아카이브 엔트리 우선. 단일 빌드에서 `--package`를 돌리면 옛 아카이브를 벗겨내고 새것을 붙인다(중첩 방지). **macOS는 Godot처럼
임베드하지 않는다** — Mach-O 로드 커맨드 뒤에 바이트가 붙으면 코드 서명 검증이 깨지고 Apple silicon은 서명 불가 바이너리를
안 돌림; 옆에 `game.pak`(매니페스트 포함)으로 두 파일. Unix는 `fs::write`가 실행 비트를 떨어뜨려 원본 런타임의 권한을 복사.
테스트: embed 왕복·평범한 exe는 None·깨진 트레일러는 Err·재첨부 치환, cook 단일 모드(디렉터리에 exe 하나뿐·매니페스트가
아카이브 안·Unix 실행 비트·단일 빌드에서 재패키징), **E2E는 mini-arena를 `single`로 패키징해 그 exe로 리플레이**(옆에
`assets/`·`project.toml`·`game.pak`이 없어야 exe에서 읽었다는 증거). CI의 "모든 프로젝트 패키징" 루프에 `single` 추가.

**`.bsengine_cache` 정리 — 안 쓴 파일 스윕(2026-09-28).** 썸네일(`<blake3(경로)>-<mtime>.png`)과 밉 캐시(`<blake3(픽셀)>.mips`)는
둘 다 내용으로 키를 매겨서, 이미지를 고칠 때마다 옛 파일이 아무도 안 여는 고아로 남았다(두 캐시의 테스트가 "고아는 정리
안 함"이라고 자백하고 있었음). 세 엔진 모두 캐시가 내용 키라 같은 문제가 있고, 정책을 명시한 건 Unreal의 DDC 하나:
파일시스템 백엔드의 `DeleteUnused`+`UnusedFileAge`(일 단위, 배포 설정은 10~34일, 5.4의 삭제 전용 레거시 캐시는 8일)로
**시작 시 스윕하고, 쓰는 파일은 touch로 살린다**(Unity는 Library를 지우면 재임포트, Accelerator는 크기 기준 퇴거; Godot은
`.godot/imported/`가 그냥 자란다). 구현: `cache_sweep::sweep_unused_files(root, 14일)`이 디렉터리 바로 아래 파일 중 mtime이
한도보다 오래된 것을 지운다(하위 디렉터리 진입 없음, 없는 디렉터리는 만들지 않음). 밉 캐시는 `WgpuRHIPlugin`이 레지스트리에
루트를 주기 *전에*, 썸네일은 에셋 브라우저의 첫 `ui()`가 타일을 그리기 *전에* 한 번 스윕하므로 스윕과 읽기가 같은 파일을
두고 경합하지 않는다. 캐시 히트(`cache_chain`이 파일을 찾을 때, `read_disk_cache`가 디코드에 성공할 때)는 mtime을 갱신한다
— atime은 Windows(NTFS 기본 off)·Linux(`relatime`) 어느 쪽도 못 믿어서 Unreal처럼 mtime. ⚠️ 옛 테스트가 "같은 픽셀
재사용 = mtime 불변"으로 재사용을 관측했는데 touch가 그 관측자를 깨뜨려, **파일 끝 바이트를 바꿔 두고 살아남는지**로
바꿨다(재작성이면 원복됨). 테스트: 스윕 단위(오래된 것만·하위 디렉터리 무시·없는 루트 no-op·바이트 수), touch가 다음
스윕에서 살림, 플러그인 시작 스윕(같은 오프스크린 테스트에 합침 — 디바이스 예산), 에셋 브라우저 첫 `ui()` 스윕, 두 캐시의
히트 touch.

**에셋 브라우저 더블클릭이 그래프 편집기를 연다(2026-09-27).** Unity(Project 패널)·Unreal(Content Browser)·Godot(FileSystem
독) 모두 더블클릭이 에셋의 편집기를 여는 데서 수렴. 지금까지는 `.scriptgraph.ron`/`.shadergraph.ron`이 `.ron` 규칙 때문에
**Scene으로 분류돼 더블클릭이 씬 로드를 시도**했다. 두 종류(`AssetKind::ScriptGraph`/`ShaderGraph`, 두 부분 접미사를 `.ron`
규칙보다 먼저 검사)를 더하고, 더블클릭이 `InspectorState::request_open_asset`을 남기면 독 호스트가 `DockArea`를 그리기 전에
그 패널 탭을 띄우거나(없으면 포커스된 리프에 push) 앞으로 가져오고, 패널이 자기 접미사의 요청만 `take_open_asset_request`로
가져가 `open()`한다(스크립트 그래프는 뷰도 원점으로). 요청이 `InspectorState`를 지나는 이유: 열 패널이 아직 독에 없을 수
있고 그것을 띄울 수 있는 건 호스트뿐이라서. 테스트: 분류(접미사 대소문자·prefabs 아래에서도), 실제 `ui()`를 통한 타일
더블클릭(요청은 남고 씬 로드는 안 남, 한 번 클릭은 아무것도 안 함), 독 호스트(없으면 push·뒤에 있으면 앞으로·두 번 안 push·
주인 없는 에셋은 무시), 두 패널의 요청 소비(남의 종류는 남겨 둠, 없는 파일은 상태에 보고하고 기존 그래프 유지).

**텍스처 스트리밍 2단계 — 목표 밉과 예산(2026-09-26).** 두 엔진(Godot 4엔 텍스처 스트리밍 자체가
없음)이 수렴하는 것: **텍스처마다 화면에 그려지는 크기에서 "원하는 밉"을 구하고, 전체 메모리 예산이
그것을 덮어쓰며, 예산이 넘치면 가장 덜 필요한 텍스처의 큰 레벨부터 뺀다**(Unity mipmap streaming의
`streamingMipmapsMemoryBudget`, Unreal의 `r.Streaming.PoolSize`). 구현: `stream_textures`가 매 프레임
스트리밍 텍스처를 쓰는 엔티티마다 바운딩 구의 투영 픽셀 높이(`radius·screen_h / ((dist−radius)·tan(fov/2))`,
UV가 오브젝트에 한 번 깔린다는 Unity의 가정)를 재서 텍스처별 최대를 `set_wants`로 기록하고, 레지스트리의
`step_streaming`이 한 프레임에 한 레벨만 움직인다 — 예산 초과면 잉여가 가장 큰(=가장 먼) 텍스처의 큰 레벨을
빼고(잉여가 없으면 가장 큰 레벨), 아니면 결핍이 가장 큰 텍스처에 다음 레벨을 넣되 예산에 안 맞으면 대신 잉여를
빼고, 그것도 아니면 **한 레벨 여유**(경계에서 흔들리는 want가 매 프레임 재구성하지 않게)를 넘는 잉여만 뺀다.
카메라·화면 크기가 없거나 메시가 안 쓰는 텍스처(UI·파티클)는 1단계처럼 통째로. `project.toml [render]`에
`texture_streaming_budget_mb`(기본 512, Unity 기본값; 0=무제한)·`texture_mip_bias`(기본 0), 프로파일러
패널·`get_frame_stats`에 상주 바이트·예산·미달 개수. 통합 테스트는 33유닛 거리의 단위 구가 720p에서 39px라
**64 레벨에 머무는 것**(1단계 구현은 통째로 올려 실패)을, 가까이 오면 프레임당 한 레벨씩 통째로, 멀어지면
여유 아래까지 내려가고, 1바이트 예산은 카메라와 무관하게 1×1까지, 바이어스 2는 두 레벨 아래에 멈추는 것을 잰다.

**텍스처 스트리밍 — 텍셀 밀도(2026-09-27).** 2단계는 "텍스처가 오브젝트에 한 번 깔린다"고 가정했다. Unity는 렌더러의 UV
밀도, Unreal은 메시 빌드 때의 `TexelFactor`로 그 가정을 측정값으로 바꾼다. 구현: `GpuMeshRegistry`가 등록 때 UV 범위의
너비·높이 중 큰 쪽(`compute_uv_extent`: 한 번 깔리면 1, 네 번 타일이면 4, 아틀라스 1/4이면 0.25; 정점이 없거나 UV가 전부
같으면 1)을 기억하고, `stream_textures`가 투영 픽셀을 그 값으로 나눈다 — 네 번 타일된 텍스처는 반복마다 1/4 크기로 보여
두 레벨 덜 원하고, 아틀라스 1/4은 네 배 크게 보여 두 레벨 더 원한다. 통합 테스트는 같은 팔면체(단위 구와 같은 바운딩)를
같은 33유닛에 두고 UV만 1/4/0.25로 바꿔 want가 2/4/0이 되는 것을 잰다 — 밀도를 무시하면 셋 다 2.

**텍스처 원본 픽셀 해제 — `release_pixels`(2026-09-28).** 스트리밍 3단계 뒤에도 `Assets<TextureAsset>`이 디코드된 픽셀을
텍스처마다 이미지 한 장만큼 RAM에 들고 있었다(핫리로드 핸들이 붙듦). Unity는 "Read/Write Enabled"가 꺼져 있으면(기본) 업로드
뒤 CPU 사본을 버린다. 여기서는 **사이드카 옵트인**(`release_pixels`, 기본 off)으로 했다: 재질·UI 이미지의 업로드(`TextureCache`)가
끝나면 픽셀을 비우고 `pixels_released`를 세우며, 크기·설정·핸들은 남겨 파일이 바뀌면 새 픽셀로 한 번 더 올리고 다시 비운다.
기본이 off인 이유: 스카이박스와 터레인 레이어는 재질 경로의 GPU 사본을 쓰지 않고 **픽셀을 직접 읽어 따로 올리므로**, 같은
이미지를 재질과 함께 쓰면 그 둘이 빈 버퍼를 만난다 — 그 경우 조용히 아무것도 안 올리는(wgpu 검증 패닉) 대신 한 번 경고하고
건너뛴다(스카이박스는 경로를 기록해 재시도를 멈추고, 터레인은 실패한 로드처럼 `PendingTerrain`을 거둔다). 해제 자체가
`Modified` 이벤트를 내므로 재업로드 시스템은 `pixels_released`인 에셋을 건너뛴다(빈 버퍼 업로드 방지). 근본 해법은 스카이박스·
터레인도 `TextureCache`의 GPU 사본을 공유하는 것 — 그때 기본값을 Unity처럼 뒤집을 수 있다.

**GPU 스키닝(2026-09-24, #1883 측정 → #1884 구현).** 격리 측정(릴리스, `skinning_cost_table`)에서
CPU 스키닝은 애니메이션 캐릭터당 프레임 0.04~0.05ms — **100마리 4.2ms, 300마리 15.4ms**, 그중
~90%가 정점 블렌드+업로드였다(옛 "100마리 2.8ms"는 전체 씬 델타). Unity GPU(Batched)·Unreal
GPU Skin Cache·Godot 4 스켈레톤 패스가 수렴하는 대로 **컴퓨트 패스가 메시의 정점 버퍼에 직접
써서** 메인·CSM·포인트 섀도우 파이프라인은 무변경, 조인트 합성만 CPU에 남겼다. ⚠️ 첫 버전은
이미터당 `queue.submit`이라 마리당 0.028ms로 35%밖에 못 줄였고, 프레임당 submit 하나로 묶자
**마리당 0.008ms — 100마리 0.81ms, 300마리 2.6ms**(83% 감소)가 됐다. 비용은 블렌드가 아니라
제출이었다.

---

**작성일:** 2026-07-31 (원본) → 2026-08-26 전면 갱신 → 2026-09-09 재검수 → **2026-09-11 재검수**

> **2026-09-11 재검수 요약.** 2026-09-09판이 남은 격차로 지목한 두 축이 모두 움직였고,
> 그 판의 **수치 근거 하나가 틀렸던 것으로 밝혀졌다.**
>
> * **오디오 — 격차 해소.** 856줄에서 **3,141줄**이 됐다(3.7배). 믹서 버스(#1837), DSP
>   이펙트 체인(#1838), 오클루전(#1839), 런타임 파라미터(#1840). 2026-09-09판이 "없음"으로
>   적은 세 가지가 전부 있고, 세 레퍼런스 엔진이 모두 갖고 있으나 이 문서가 요구하지도
>   않았던 **런타임 파라미터 제어**까지 있다.
> * **UI — 해상도 독립성 확보.** `ui_state.rs` 254 → **551줄**. 정규화 앵커(#1841)로
>   Unity/Unreal/Godot과 같은 배치 모델이 됐다. 레이아웃 컨테이너는 여전히 없다.
> * **규모 — 개선됐으나, ⚠️ 아래 판의 수치가 디버그 빌드였다.** 섀도우 패스 인스턴싱(#1836)으로
>   draw가 8,481 → 2,116으로 줄었다. 그러나 **2026-09-09판의 모든 프레임 시간은 `cargo test`
>   (dev 프로필) 값이었다.** 릴리스로 재보니 같은 씬이 **2.87ms(≈350 FPS)**다. 상대적 개선은
>   유효하지만 "엔진이 CPU 바운드"라는 서술은 **무효**다. 자세한 것은 아래 1-c.
>
> **2026-09-09 재검수 요약.** 2026-08-26판이 "의도적으로 보류 중인 큰 백로그"로 적었던
> **15개 항목이 전부 출하됐다**(로드맵 item 43~57). 번호 항목 1~57이 모두 완료됐고, 남은
> 미체크 박스는 item 55의 '단일 실행 파일' 하나뿐이며 의도적으로 범위 밖이다. 각 항목은
> 로드맵 산문이 아니라 **소스 경로 존재 확인**으로 검증했다.
>
> 그 결과 **판정표의 거의 모든 행이 바뀌었고**, 축 두 개가 새로 식별됐다 — **규모**(손으로
> 저작된 최대 씬 24 엔티티)와 **UI 프레임워크**(254줄). 그리고 **오디오는 856줄 그대로**로,
> 다른 모든 축이 전진하는 동안 유일하게 손대지 않은 축이 됐다. 자세한 것은 맨 아래
> "남은 진짜 격차".
>
> ⚠️ 아래 본문의 크레이트 라인 수 표는 **2026-08-26 시점 값**이다. 2026-09-09 실측으로
> `bsengine-physics` 1,688→5,553, `bsengine-network` 467→1,689, `bsengine-asset`
> 9,583→11,564, `bsengine-render` 2,319→3,478로 늘었다.
>
> 2026-09-11 실측: **`bsengine-audio` 856→3,141**, `bsengine-core` →7,242,
> `bsengine-scripting` →18,118, `bsengine-rhi-wgpu` →29,019.
> 컴포넌트/op 카탈로그는 **72 / 278**(2026-09-09 71 / 273).
**근거:** 각 크레이트 소스 코드 직접 조사(라인 수, 실제 구현 내용), `ENGINE_ROADMAP.md`(1~42번
항목 전체), 이후 추가로 병합된 PR들(엔티티 계층, 프리팹 시스템 전체, 픽셀/스크린샷 검증,
에디터 UI 리디자인, CI 성능 개선).

이 문서는 `docs/superpowers/specs/2026-07-28-bsengine-vs-unity-unreal-comparison-design.md`에서
계획된 산출물이다. 2026-07-31 최초 작성 이후 약 4주간 로드맵 item 23~42가 전부 완료됐고,
로드맵에 없던 대형 작업(엔티티 계층/씬 부모-자식 관계, 프리팹 시스템 전체 — 생성/중첩/순환
탐지/필드별 override tracking/pull-sync, 헤드리스 픽셀·스크린샷 검증, 에디터 UI 리디자인
phase 3+4, CI E2E 리플레이 33배 가속)까지 병합되면서 원본의 "매우 큼"/"큼" 판정 다수가
무효화됐다. 이번 갱신은 **판정을 다시 매기는 것**이지, 사소한 오타 수정이 아니다.

---

## 방법론

문서/스펙을 읽고 추측하는 대신, 각 축마다 다음 근거를 직접 확인했다:
- 관련 크레이트의 소스 코드(파일 목록 + 라인 수 + 실제 구현 내용)를 직접 grep/read
- `ENGINE_ROADMAP.md`에 기록된 각 기능의 완료 조건과 구현 방식 설명
- 새 기능이 실제로 존재하는지 파일 경로/함수 시그니처 수준에서 확인(예: `prefab.rs`의
  `instantiate_prefab`, `validate_prefab_descriptor` 실존 확인)

각 크레이트의 소스 라인 수(공백 포함, `crates/*/src/**/*.rs` 합계, 2026-08-26 기준):

| 크레이트 | 라인 수 | 2026-07-31 대비 | 비고 |
|---|---:|---:|---|
| bsengine-editor | 100,084 | +7,414 | MCP 툴 다수, 프리팹/기즈모 UI 추가 |
| bsengine-asset | 9,583 | **+9,365** | "경로→바이트 캐시"에서 실제 에셋 파이프라인으로 |
| bsengine-scripting | 16,440 | +1,953 | |
| bsengine-rhi-wgpu | 12,477 | +1,563 | 렌더 백엔드 + 에디터 패널 |
| bsengine-scene | 3,819 | **+2,925** | 프리팹 시스템 전체 |
| bsengine-gltf | 3,108 | +1,897 | 스켈레탈 스키닝 등 |
| bsengine-core | 5,878 | +800 | |
| bsengine-render | 2,319 | +1,718 | 파티클 등 |
| bsengine-runtime | 2,606 | +1,212 | |
| bsengine-mcp | 2,359 | +477 | |
| bsengine-physics | 1,688 | +451 | 캐릭터 컨트롤러 |
| bsengine-app | 2,124 | -530 | |
| bsengine-audio | 856 | +605 | 3D 포지셔널 오디오 |
| bsengine-catalog | 1,200 | (신규) | 컴포넌트/op 카탈로그 |
| bsengine-input | 893 | 0 | |
| bsengine-network | 467 | 0 | |
| bsengine-window | 383 | 0 | |
| bsengine-plugin | 290 | 0 | |
| bsengine-ecs | 62 | (신규) | bevy_ecs 얇은 래퍼 |
| bsengine-demo | 79 | (신규) | 예제 바이너리 |

(`bsengine-rhi`는 2026-08-26 삭제됨 — 채택된 적 없는 죽은 RHI 트레이트 추상화였음, PR #1799.)

전체 워크스페이스 약 **166,700줄**(2026-07-31 대비 +31,000줄, +23%). 가장 극적인 변화는
**에셋 파이프라인**(218줄 → 9,583줄, 44배) — 원본 문서가 "가장 원시적인 구현"이라 판정한
축이 이제 가장 크게 성장한 축이다.

---

## 축별 비교

### 1. 렌더링/그래픽스

**Unity/Unreal:** GI(라이트매스/Lumen), 스크린스페이스+리얼타임 리플렉션, IBL, 다양한
안티에일리어싱(TAA/FXAA/DLSS), 볼류메트릭 포그, 파티클 시스템, 터레인 시스템, LOD,
오클루전 컬링, 노드 기반 머티리얼/셰이더 그래프 에디터.

**BSEngine 현재:**
- wgpu 기반(Vulkan/Metal/DX12 이식 가능), Cook-Torrance PBR
- 방향광 PCF 섀도우, 포인트라이트 섀도우(선형거리 큐브 배열)
- Bloom, ACES 톤매핑, SSAO
- **파티클 시스템**(item 28) — `bsengine_core::ParticleEmitter`, burst/rate 방출, 수명
  관리, 전용 렌더 패스(`particles.rs`). 헤드리스 테스트에서도 실제 방출 검증됨.
- 프러스텀 컬링(바운딩 스피어)
- CPU 스켈레탈 스키닝 — glTF skin/joint 파싱, 매 프레임 CPU LBS 블렌딩.
  ⚠️ **2026-09-09 정정:** 이 줄은 원래 "의도적으로 캐릭터 1~2개 규모용"이라고 적혀 있었고,
  첫 작성 이래 한 번도 측정되지 않은 추정이었다. 실측하니 **애니메이션이 도는 여우 100마리가
  2.8ms**(마리당 0.03ms)였다 — 1~2개가 아니라 수십~수백 규모다. 아래 "1-b. 페이즈 2" 참조.
  GPU 스키닝이 없다는 사실은 그대로지만, 그것이 캐릭터 수를 한 자릿수로 묶는다는 함의는
  사실이 아니었다.
- 커스텀 WGSL 셰이더 + 셰이더 자체 time 유니폼
- **CI 헤드리스 렌더 fast-path**(2026-08-26) — 그림자/SSAO/블룸을 clear-only로 건너뛰는
  모드가 CI 리플레이에만 적용됨. 실제 게임 렌더 품질에는 영향 없음(엔지니어링 인프라 개선).

**2026-09-09 추가 (item 43~50) — 위 "여전히 없음"이 전부 뒤집혔다:**
- **프레임 프로파일러**(43) — 텍스처 메모리 추적, 드로우콜/삼각형 카운터, GPU 패스 타이밍
- **터레인**(44) — 청크 메시 + 하이트필드 콜라이더, 4레이어 스플랫, 에디터 브러시 툴
- **LOD**(45) — 히스테리시스 상태 머신. 터레인 청크도 같은 메커니즘 재사용
- **오클루전 컬링**(46) — CPU 소프트웨어 래스터화. 드로우콜 71→11 실측
- **안티에일리어싱**(47) — TAA(Halton 지터 + 히스토리 재투영)
- **GI/IBL**(48) — 큐브맵 IBL + BRDF LUT, 라이트 프로브 볼륨(L2 SH)
- **볼류메트릭 포그**(49) — froxel 컴퓨트 패스 + 섀도우 샘플링 라이트 샤프트
- **셰이더 그래프**(50) — 그래프→WGSL 컴파일러 + 노드 에디터 패널. WGSL 텍스트 경로는
  1급으로 유지(테스트로 고정)

**여전히 없음(2026-09-17 갱신):** ~~GPU 스키닝(CPU LBS만)~~(#1884 컴퓨트 스키닝, 맨 위 절 참조),
데칼. (캐스케이드 섀도우 맵은 #1843으로 해소.)

**⚠️ 2026-09-11 정정 — CSM을 "품질 티어"로 분류한 것이 틀렸다.**
이 문서는 CSM을 계속 "콘텐츠가 요구하기 전엔 시작할 근거가 없는 항목"으로 묶어왔다.
실제로 코드를 열어보니 `compute_light_view_proj`가 `Vec3::ZERO`를 무조건 바라보며
**60×60 박스를 월드 원점에 고정**하고 있었다. `games/scale-level`은 x = 0..152에 저작돼
있으므로 **출하된 데모의 대부분이 어떤 해상도에서도 방향광 그림자를 받지 못했다.**
이건 품질 등급이 아니라 **지목 가능한 결함**이었고, 콘텐츠는 이미 요구하고 있었다.
#1842로 카메라 추종(Unity의 "stable fit" — 바운딩 스피어)으로 교체했다.

→ **CSM은 #1843으로 출하됐다.** 캐스케이드 4개, `[render]`에서 저작 가능
(`shadow_distance` / `shadow_cascades` / `shadow_cascade_blend`), 경계 크로스페이드 포함.

분할은 **Unreal의 `CascadeDistributionExponent`** 방식(`거리 * (i/n)^지수`)을 택했다 —
Unity·Godot의 손으로 적는 퍼센트 표는 캐스케이드 개수마다 표가 따로 필요한데 지수식은
임의의 n에 일반화된다. 기본 지수 2.0이 Unity의 4캐스케이드 기본값과 몇 포인트 안에서
일치하며, **주석으로 주장하지 않고 테스트로 단언**했다.

**메모리 총량은 의도적으로 불변**: 캐스케이드당 1024²이므로 4장이 예전 단일 2048² 한 장과
정확히 같다. Unity의 섀도우 아틀라스와 Godot의 `directional_shadow_size`가 모두 고정
총량을 캐스케이드로 나누는 방식이고, Unreal만 캐스케이드마다 풀 해상도를 잡는다. 2:1로
갈리는 지점에서 다수를 따랐고, **컴파일 타임 단언**으로 고정했다 — 기본 VRAM 예산이 4배가
되는 변경은 테스트도 경고도 내지 않고 그냥 모든 플레이어에게 4배를 물리기 때문.

경계 페이드는 **기본 on**(Unreal 선례). Unity와 Godot은 기본 off이고
`shadow_cascade_blend = 0`이 그 동작을 정확히 재현한다.

⚠️ **로드맵/비교 문서의 산문을 근거로 항목을 분류하지 말 것.** "CSM 없음"이라는 옛 표현을
그대로 옮겨 적었을 뿐인데, 코드가 실제로 무엇을 하는지 확인하자 분류가 뒤집혔다.

**판정:** **격차 큼 → 작음.** 원본이 "콘텐츠를 채우는 도구가 전무"라 지적한 상태는 해소됐다.
~~남은 것(CSM/GPU 스키닝)은 기능 부족이라기보다 **규모 격차의 렌더링 쪽 얼굴**이다.~~ 둘 다
해소됐다(CSM #1843, GPU 스키닝 #1884).

---

### 2. 게임플레이 시스템 깊이

**물리 (`bsengine-physics`, 1,688줄)**
- 있음: rigidbody, collider, raycast, sensor 트리거, MCP 툴 부착/해제
- **캐릭터 컨트롤러**(item 27, 신규) — 원본 문서가 "없음"으로 판정했던 항목. 넉백도 이제
  스크립트가 위치를 흉내내는 방식이 아니라 **실제 Rapier 임펄스**로 동작(원본 문서가
  "실증된 한계"로 지적했던 Kinematic-바디-무시-임펄스 문제가 해결됨).
- **조인트/제약**(item 51) — Fixed/Revolute/Spherical, 씬에서 이름으로 저작
- **래그돌**(item 52) — 본별 캡슐 바디 + 조인트, ASM 상태에서 전환/복귀 블렌딩
- **비히클 물리**(item 53) — Rapier `DynamicRayCastVehicleController`, 휠 레이캐스트+서스펜션,
  보이는 휠 엔티티. `bsengine-physics` 1,688→5,553줄
- 여전히 없음: 소프트바디/천

**애니메이션**
- 있음: `AnimationStateMachine` + 크로스페이드, **1D 블렌드 스페이스**(item 29, 신규 —
  원본 문서가 "블렌드 트리 없음"으로 지적했던 항목이 부분적으로 해소됨: 속도 같은 단일
  연속 파라미터로 블렌드 가능. 다차원 블렌드 트리는 아직 없음)
- **two-bone IK + 리타게팅**(item 54) — `bsengine-gltf/src/ik.rs`
- 여전히 없음: 다차원 블렌드 트리(1D만), 풀바디 IK

**내비게이션**
- **실제 폴리곤 navmesh**(item 26, 신규) — 원본 문서가 "이름과 실제 구현의 괴리"로 강하게
  지적했던 균일 그리드 A*가 실제 레벨 지오메트리 기반 navmesh로 교체됨. "NavMesh"라는
  이름과 구현이 이제 일치한다.

**오디오 (`bsengine-audio`, 2026-09-11 기준 3,141줄)**
- **3D 포지셔널 오디오**(item 25) — 원본 문서가 "매우 큼" 격차로 지적했던 항목.
  엔티티 위치 기반 거리 감쇠 구현됨.
- **믹서 버스·DSP 이펙트·오클루전·런타임 파라미터** — 2026-09-11에 전부 추가됨
  (#1837~#1840). 아래 "남은 진짜 격차 → 2. 오디오" 참조.
- 여전히 없음: 에디터 믹서 패널, send/return 버스, 스냅샷/더킹

**네트워킹 (`bsengine-network`, 467→1,689줄)**
- 있음: 서버/클라 역할 구분, Transform 동기화
- **클라이언트 예측 + 서버 재조정**(item 56) — `MSG_CLIENT_INPUT`(시퀀스 번호), 서버의 마지막
  처리 시퀀스 에코, 미확인 입력 재생
- **스냅샷 보간**(item 56) — `interpolation_delay_ticks`만큼 과거를 렌더. 최신 스냅샷을 지나면
  외삽하지 않고 **정지**한다(네트워크 문제가 네트워크 문제처럼 보이도록)
- **관심 영역(AOI)**(item 56) — `aoi_radius`. 업데이트를 끊을 뿐 엔티티를 숨기지는 않는다
- ~~여전히 없음: RPC 프레임워크~~ **#1854/#1855로 해소.** 신뢰성 채널(시퀀스+32비트 ack 비트필드
  +재전송+재정렬 버퍼) 위에 Unreal의 `Server`/`Owner`/`NetMulticast` 라우팅. 소유권은 서버가 검사

**판정 (2026-09-09):** 물리는 item 51~53으로 **작음**(소프트바디/천만 남음), 애니메이션은
item 54로 **작음**(다차원 블렌드 트리·풀바디 IK 남음), 네트워킹은 item 56으로 **중간→작음**
(RPC만 남음).

**추가 (2026-09-21):** 씬 스트리밍이 #1866(추가 로드/언로드 + 스크립트 API)과
#1867(거리 기반 자동 스트리밍)로 들어왔다. 거리 기반은 **세 엔진 중 Unreal만** 갖고 있던 것이다
(Unity는 추가 로딩만 주고 트리거는 스크립트에 맡기며, Godot은 둘 다 없다).

⚠️ **여기서 내 수치 보고가 틀렸다.** "스트리밍 히치 16.2ms"라고 적었던 건 로드 *전*
프레임(1,231 엔티티)과 로드 프레임(2,462)을 비교한 값이다. 로드 *후* 정상 상태와 비교하면
**-1.59ms** — 로드 프레임이 오히려 빠르다. 비용은 일회성이 아니라 "씬이 두 배가 됐다"는
영구 비용이었고, 그 오독 위에 세운 설계 둘(비동기 로딩, 스폰 분할)이 무효가 됐다.
⚠️ 히치를 다시 잴 때는 **로드 *후* 프레임과 비교할 것.** 로드 *전*과 비교해 "히치 16.2ms"로 오독한 적이 있고, 그 위에 설계 두 개(비동기 로딩·스폰 분할)를 세웠다가 전부 무효가 됐다. 올바르게 재면 -1.59ms로, 히치가 없다.

실제 일회성 비용은 1,231 엔티티에 약 5.9ms(엔티티당 4.8µs), 릴리스 기준.

**정정 (2026-09-19):** 천이 #1861~#1864로 네 조각 전부 닫혔다(시트+고정 정점 → 씬 충돌 →
굽힘 강성 → 라이브 재생성). 위 2026-09-09 판정의 "소프트바디/천만 남음"이 소진됐으므로
**물리 축에 남은 명명된 격차는 없다.** 자기 충돌까지 #1865로 닫혀, 천은 Unity/Unreal이
가진 항목을 전부 갖췄다(강성·굽힘·감쇠·반복·중력·고정점·씬 충돌·자기 충돌). Godot은
굽힘과 자기 충돌이 없으므로 이 축에서는 우리가 더 많다.

⚠️ **#1862가 #1861의 실제 버그를 찾았다** — 천의 세계 변환을 `Transform`에서 읽고 있었는데,
케이프는 캐릭터에 부모로 붙으므로 그건 어깨로부터의 오프셋일 뿐이다. 충돌이 붙기 전에는
로컬/세계 구분이 관측 불가능해서 드러나지 않았다. **다음 기능이 이전 기능의 결함을 드러내는
경우**이므로, 축을 한 조각씩 닫을 때 앞 조각을 다시 의심할 것.

**정정 (2026-09-18):** 위 판정의 세 항목이 전부 닫혔다. 물리는 천(#1861)으로,
애니메이션은 풀바디 IK(#1860)로, 네트워킹은 RPC(#1854/#1855)로. 렌더링에서도 SSR(#1859)과
데칼(#1856~#1858)이 닫혔다. 판정을 지우지 않고 남겨 두는 이유는 위의 2026-09-11 정정과 같다 --
그 판정이 실제로 다음 작업을 결정한 근거였기 때문이다.

⚠️ 남은 것을 이 문서의 산문에서 고르기 전에 grep으로 확인할 것. 이 문서는 이미 세 번 낡았고,
그때마다 "미착수"라고 적힌 항목이 이미 구현돼 있었다.

**정정 (2026-09-11):** 위 판정은 "오디오만 856줄 그대로이며 가장 뒤처진 축"이라고 적었다.
그 뒤 #1837~#1840으로 3,141줄이 되면서 그 서술은 더 이상 성립하지 않는다. 남겨 두는
이유는 그 판정이 실제로 다음 작업을 결정한 근거였기 때문이다.

---

### 3. 콘텐츠 파이프라인 (에셋)

**Unity/Unreal:** GUID 기반 에셋 데이터베이스, 임포트 세팅, 파일 변경 감지 핫리로드,
의존성 그래프, 에셋 번들/쿠킹.

**BSEngine 현재 (`bsengine-asset`, 9,583줄 — 원본 대비 44배 성장):**
- **`bevy_asset` 통합**(item 23) — 원시 경로→바이트 캐시에서 실제 에셋 서버로
- **파일 변경 감지 핫리로드 워처**(item 24)
- **안정적 GUID 에셋 identity**(item 30) — `.meta` 사이드카 기반, 파일 이동/이름변경에도
  참조가 안 깨짐(스캔 기반 identity 검증 테스트로 확인됨)

- **쿠킹/패키징**(item 55, 2026-09-09) — 진입 씬에서 도달 가능한 에셋만 수집하는 도달성
  워크. `loose`(파일 그대로)와 `pak`(자체 포맷 무압축 아카이브) 두 모드. 미해결 참조는
  빌드를 실패시키고 그걸 이름 지은 파일과 함께 보고한다.
- **임포트 세팅**(2026-09-23, #1878 텍스처 · #1879 모델) — `.meta` 사이드카의 `import:`
  필드, Unity `.meta`/Godot `.import`와 같은 자리. 텍스처는 sRGB·밉맵·필터·랩(기본값은
  세 엔진 공통: sRGB 켬·밉맵 켬·Linear·Repeat), 모델은 균일 스케일과 애니메이션 임포트
  여부(기본 1.0·켬). 스케일은 Unity/Unreal/Godot(`apply_root_scale`)처럼 **베이크** —
  정점·노드 이동·역바인드 행렬 이동열·이동 키 네 곳 — 하며, 스킨 캐릭터가 정확히 s배
  포즈로 변형되는지를 실제 스키닝 경로로 검증한다. `.meta` 편집은 옆 에셋을 핫리로드한다.
  MCP `asset_import_settings`(#1880)로 에이전트가 읽고 고칠 수 있다 — 부분 편집은 기록된
  값 위에 병합되고, 모르는 필드는 무시가 아니라 오류이며, 기존 identity(guid)는 보존된다.

**여전히 없음:** ~~임포트 세팅~~(→ 위 항목. 텍스처 압축/포맷 변환은 여전히 없음), ~~의존성
그래프 시각화~~(#1886/#1887), ~~**단일 실행 파일**~~
(2026-09-28 `--mode single`, 맨 위 절 참조 — 아카이브를 exe 꼬리에 임베드, macOS는 옆에).

**판정:** 원본의 "가장 원시적인 구현" 판정은 무효화된 지 오래고, item 55로 마지막 구멍인
쿠킹/패키징까지 메워졌다. **격차 매우 큼 → 작음.**

---

### 4. 에디터 워크플로우/UX

**Unity/Unreal:** 프리팹 시스템, 타임라인/시퀀서, 터레인/파티클 에디터, 비주얼
스크립팅(Blueprint), 프레임 프로파일러/GPU 디버거, 빌드/패키징 파이프라인.

**BSEngine 현재:**
- egui_dock 기반 도킹 패널: Hierarchy / Inspector / Viewport / Asset Browser
- `bevy_reflect` 기반 범용 컴포넌트 편집 — 타입별 하드코딩 없이 임의 컴포넌트 필드 편집
- **트랜스폼 기즈모 이동+회전+스케일**(Scale gizmo 신규, 2026-08-25) — 원본 문서가
  "명시적 보류"로 적었던 항목이 해소됨. 축별 드래그는 가산, 균일 핸들 드래그는 승산(비균일
  축 비율 보존).
- **프리팹 시스템 전체**(신규, 원본 문서가 "없음"으로 명시했던 항목) —
  - 씬/스크립트/에디터 3개 경로에서 프리팹 인스턴스화, 중첩 프리팹, 순환 참조 탐지
  - **필드별 override tracking** — 물리/라이트/카메라/에셋 참조 필드별로 개별 구현,
    인스턴스가 프리팹 소스와 다르게 수정된 필드를 추적
  - **Pull-sync** — 프리팹 소스를 다시 저장하면 override 안 된 필드는 모든 인스턴스에
    전파됨(신규 필드 추가 시 기존 인스턴스로 리싱크되는 케이스까지 커버)
  - 에디터 Hierarchy 패널의 "Apply to Prefab" 버튼으로 인스턴스 변경을 소스에 역전파 가능
- Undo/Redo, 멀티셀렉트, 키보드 단축키
- **AI 에이전트 전용 조작 경로:** MCP 툴 다수 + `set_reflected_component` 범용 리플렉트
  부착 툴 — Unity/Unreal에 대응 개념 자체가 없음
- **헤드리스 픽셀/스크린샷 검증**(신규) — `get_pixel`/`screenshot` MCP 툴로 AI 에이전트가
  실제 렌더링된 프레임을 이미지로 받아볼 수 있음(MCP 이미지 콘텐츠 블록). 렌더링/머티리얼/
  파티클처럼 트랜스폼만으로는 검증 불가능했던 영역까지 AI가 직접 확인 가능해짐.

**2026-09-09 추가 — 패널이 4개에서 7개로:**
- **프로파일러 패널**(item 43) — 프레임 타이밍, 패스별 GPU 비용, 드로우콜/삼각형/텍스처 메모리
- **셰이더 그래프 패널**(item 50) — 노드 에디터. `.shadergraph.ron`을 직접 열고 저장
- **타임라인 패널**(item 57) — 트랙 뷰, 스크러빙, 컷신 미리보기, 키프레임 편집/저장
- **터레인 브러시 툴**(item 44) — 뷰포트에서 높이 조각 + 텍스처 페인팅
- 메시 3D 썸네일/에셋 디스크 캐시/에셋 브라우저 워처도 완료(post-release 감사 항목)

**2026-09-23 추가 — 파티클 패널(#1882):** 씬의 모든 이미터를 살아있는 파티클 수와 함께
나열하고, 이미터별 Burst/Restart와 프리뷰 재생/일시정지/속도(0.1~4x)/Restart All을 제공한다.
Unity의 Scene 뷰 Particle Effect 오버레이·Niagara 프리뷰 툴바·Godot의 에디터 시 방출이
수렴하는 네 가지(재생/일시정지, 재생 속도, 재시작, 살아있는 수)가 그대로다. 파라미터는
전처럼 반사 인스펙터에서 편집(행 클릭 = 엔티티 선택). 프리뷰 제어는 **에디터 모드 + 정지
상태에서만** 적용되어 실행 중인 게임의 이펙트 속도에 새지 않는다.

**여전히 없음:** 비주얼 스크립팅(Blueprint 대응물). ~~파티클 전용 에디터 UI~~(위 #1882).

**판정 (2026-09-09):** 원본이 "없음"이라 적었던 항목이 전부 해소됐다 — Scale gizmo, 프리팹
시스템, 타임라인/시퀀서, 프로파일러, 터레인 에디터, 패키징. **격차 큼 → 작음.** 남은 것은
비주얼 스크립팅 하나인데, 이건 AI-native 방향과 정면으로 어긋나는 항목이라 "격차"로 셀지
자체가 판단 대상이다(셰이더 그래프는 같은 이유로 텍스트 경로를 1급으로 유지한 채 넣었다).

---

### 5. 스크립팅/프로그래밍 API

**Unity:** C# — 정적 타입, 전체 .NET 생태계, IDE 디버거 완전 지원.
**Unreal:** C++ (엔진 소스 접근) + Blueprint 비주얼 스크립팅.

**BSEngine 현재:** Deno Core(V8) 기반 JavaScript, `bsengine-scripting` 16,440줄. 동적
타입이라 정적 타입 체크가 없고, 비주얼 스크립팅도 없다 — 이 축은 JS라는 언어 선택 자체에서
오는 근본적 특성이라 원본 문서 이후 구조적으로 변한 것이 없다.

> ⚠️ **위 문단은 오진이다.** 아래 「정정 — "스크립팅 타입 안전성 = JS 근본 특성"은
> 오진이었다」를 볼 것. 언어 탓이 아니라 엔진이 자기 API의 타입을 배포하지 않던 것이었고,
> #1848이 `scripts/api.d.ts`를 전체 표면으로 생성해 해소했다. 비주얼 스크립팅이 없다는
> 부분만 여전히 맞다. **이 포인터가 없어서 같은 문단을 근거로 두 번 잘못 분류했다.**

**판정:** 원본 문서와 동일. **Unity C# 대비 격차 있음.** 다만 텍스트 코드라 AI 에이전트가
직접 읽고 쓰기엔 유리한 형태라는 판단도 유지.

---

### 6. BSEngine만의 강점 — Unity/Unreal에 없는 축

**AI-Native 에디터 조작:** MCP 툴로 AI 에이전트가 에디터를 완전히 조작 가능. 프리팹
생성/적용까지 MCP 경로로 가능해져 이 축이 원본 문서 이후 더 넓어졌다.

**헤드리스 E2E 테스트 + 리플레이:** `bsengine-runtime --test`의 프로토콜로 실제 게임
플레이스루를 기록·재생 검증. 원본 문서 작성 이후 여기에 **픽셀/스크린샷 검증**이
추가되어, 트랜스폼/상태값만으로는 잡을 수 없던 렌더링/머티리얼 버그까지 AI가 직접
"보고" 검증할 수 있게 됐다 — Unity/Unreal에도 준하는 것이 없는 검증 방식이 한 단계 더
깊어짐.

**엔지니어링 인프라의 자기 개선 사례:** CI의 E2E 리플레이 스텝이 소프트웨어 렌더링 환경에서
불필요하게 전체 렌더 파이프라인을 돌리고 있던 것을 발견해 33배 가속(2h50m → 5분).
Unity/Unreal 비교와는 직접 관련 없지만, "AI가 스스로 엔진 개발 워크플로우의 병목을 찾아
고친다"는 점에서 이 강점 축의 연장선.

**판정:** 원본 문서의 판정 유지 — 이 축들은 "따라잡아야 할 격차"가 아니라 BSEngine이
Unity/Unreal과 다른 방향으로 앞서 있는 지점이며, 이번 갱신 기간 동안 오히려 더 벌어졌다.

---

## 종합 판정 표

| 축 | 2026-07-31 | 2026-08-26 | 2026-09-09 | 2026-09-11 | 변화 |
|---|---|---|---|---|---|
| 에셋 파이프라인 | 매우 큼 | 중간 | 작음 | 작음 | 변화 없음 |
| 오디오 | 매우 큼 | 작음 | 작음 (손대지 않음) | **작음 (해소)** | 856→3,141줄. 버스·이펙트·오클루전·런타임 파라미터 전부(#1837~#1840) |
| 내비게이션 | 큼 | 작음 | 작음 | 작음 | 변화 없음 |
| 렌더링(콘텐츠 도구) | 큼 | 큼 (완화) | 작음 | 작음 | 변화 없음 |
| 프리팹/대규모 콘텐츠 지원 | 큼 | 중간 | 중간 (미검증) | **작음** | `games/scale-level` 1,238 엔티티가 릴리스에서 2.87ms |
| 물리(캐릭터/래그돌/천) | 중간 | 작음 | 작음 | **작음 (축 해소)** | 천 #1861~#1865 — PBD 시트·고정 정점·씬 충돌·굽힘 강성·라이브 재생성·자기 충돌. Unity/Unreal이 가진 것을 전부 갖췄고 Godot보다 많다 |
| 애니메이션(블렌드) | 중간 | 중간 (완화) | 작음 | **작음 (해소)** | ~~다차원 블렌드 트리~~ #1850. ~~풀바디 IK~~ #1860 — 후진 패스에서 공유 관절 평균, 전진 패스가 유일한 길이 복원 |
| 스크립팅 타입 안전성 | 중간 | 중간 | 중간 | **작음 (⚠️ "JS 근본 특성"은 오진이었음)** | `scripts/api.d.ts`가 2개 함수 → 전체 표면 생성(#1848). 파라미터 596개 중 459개가 구체 타입 |
| 네트워킹(예측/보간) | (미평가) | 중간 | 작음 | **작음 (해소)** | ~~RPC 프레임워크~~ #1854/#1855. 축 전체가 닫힘 |
| 에디터 조작성 | 작음 | 작음 | 작음 | 작음 | 패널 8개. ~~오디오 믹서 패널은 없음~~ #1853으로 해소 |
| 렌더링(기본 PBR/섀도우) | 작음 | 작음 | 작음 | **작음 (결함 1건 + CSM 해소)** | 절두체 원점 고정 버그 수정(#1842) + 캐스케이드 섀도우 맵(#1843). ~~SSR~~ #1859. ~~GPU 스키닝만 없고~~ #1884 컴퓨트 스키닝 — 격리 측정 릴리스 100마리 4.2ms → 0.81ms(맨 위 절) |
| **규모(씬 크기/스트리밍)** | (미평가) | (미평가) | 중간 (실측·개선됨) | **작음 (스트리밍 해소)** | 인스턴싱(#1836). ~~스트리밍 없음~~ #1866 추가 로드/언로드 + #1867 거리 기반 자동. ⚠️ **히치는 없었다** — 내가 보고한 16.2ms는 로드 *전* 프레임과 비교한 값 |
| **UI 프레임워크** | (미평가) | (미평가) | 중간 (신규 식별) | **작음** | 앵커(#1841) + 행/열(#1844) + Grid(#1846) + 이미지 실제 렌더링(#1845) + ~~스크롤~~(#1851/#1852, `{scroll: true}` 컨테이너 + 스크롤 op) |
| AI-native 조작/테스트 | 역격차(강점) | 역격차(강점) | 역격차(강점) | **역격차(강점)** | 변화 없음 — 여전히 대응 개념 없음 |

---

## ⚠️ 정정 — "스크립팅 타입 안전성 = JS 근본 특성"은 오진이었다 (2026-09-15)

이 표는 스크립팅 타입 안전성을 네 판 연속 **중간**으로 두고 이유를 "JS 근본 특성"이라고
적어왔다. **틀렸다.** 코드를 열어보니 `scripts/api.d.ts`는 **10줄에 함수 2개**(`log`,
`version`)였고, 네임스페이스 이름조차 실제 전역(`Bsengine`)과 달랐다. 실제 표면은 프렐류드
함수 약 300개 + op 282개다. **커버리지 1% 미만, 루트 이름도 틀림.**

즉 격차는 "JS에 타입이 없어서"가 아니라 **엔진이 자기 API의 1%도 설명하지 못하는 타입
정의를 배포하고 있어서**였다. 이건 언어의 성질이 아니라 고칠 수 있는 결함이다.

#1848로 생성 방식으로 바꿨다: 프렐류드를 평가해 살아있는 `Bsengine` 객체를 순회해 이름을
얻고, 각 래퍼가 전달하는 op의 **Rust 시그니처**에서 타입을 얻는다(#1847). 파라미터 596개 중
**459개가 구체 타입**, 나머지 137개는 추측 대신 `unknown`.

⚠️ **손으로 유지하던 것이 10줄로 썩은 원인이므로, 손으로 다시 쓰는 건 썩는 과정을 재시작할
뿐이다.** `catalog --check` 선례대로 드리프트를 테스트가 잡고, 그 테스트 자체를 뮤테이션으로
검증했다(생성 파일의 반환 타입 하나를 손으로 고치면 실패).

⚠️ **자신 있게 틀린 타입은 정직한 `unknown`보다 나쁘다** — 저자가 믿기 때문. 출력물을 눈으로
읽다가 두 건을 잡았다: (1) 반환값이 있는 op 282개 중 123개를 전부 `void`로 선언(TypeScript가
대입을 거부하므로 엔진이 지원하는 코드를 타입 정의가 거부하게 됨), (2) 파라미터를 **인덱스**로
op에 매칭 — `setContainer`는 인자 7개인데 op는 10개라 `opts`가 `columns: u32`를 받아
`number`가 됐고, `direction`은 실제로 `'horizontal'|'vertical'|'grid'` 문자열인데 `number`가
됐다. 지금은 **실제 전달 관계**를 따라간다(도달하는 인자 슬롯의 타입; 도달하지 않거나 변환되면
`unknown`).

⚠️ **비교 문서의 산문을 근거로 항목을 분류하지 말 것.** CSM에 이어 두 번째다
(1-c 절 참조). 둘 다 "코드를 열어 확인"이 분류를 뒤집었다.

## 남은 진짜 격차 (2026-09-11 기준)

**2026-08-26판이 "의도적으로 보류 중인 큰 백로그"로 적었던 15개 항목은 전부 출하됐다.**
GI/IBL, 안티에일리어싱, 볼류메트릭 포그, 터레인, LOD, 오클루전 컬링, 셰이더 그래프,
프레임 프로파일러, 빌드/패키징, 타임라인/시퀀서, 네트워킹 예측/보간/AOI, 애니메이션
IK/리타게팅, 래그돌, 조인트/제약, 비히클 물리 — `ENGINE_ROADMAP.md` item 43~57.
번호 항목 1~57이 모두 완료됐고, 미체크 박스는 item 55의 '단일 실행 파일' 하나뿐이며
그건 의도적으로 범위 밖에 둔 것이다.

이 갱신에서 각 항목은 로드맵 산문이 아니라 **실제 소스 경로 존재 확인**으로 검증했다.

### 1-c. ⚠️ 정정 — 아래 규모 수치는 전부 디버그 빌드였다 (2026-09-11)

**아래 1, 1-a, 1-b의 모든 프레임 시간은 `cargo test`(dev 프로필)로 잰 값이다.** wgpu의
검증 레이어는 최적화되지 않은 빌드에서 대단히 비싸다. 같은 씬·같은 draw 수·같은 삼각형
수로 `--release`에서 재측정한 결과:

| `games/scale-level`, 8 point + 8 spot | 디버그 | 릴리스 |
|---|---:|---:|
| 인스턴싱 전 (8,481 draws) | 40.64ms | **4.26ms** |
| 인스턴싱 후 (2,116 draws) | 19.80ms | **2.87ms** |

**유효한 것:** 상대적 개선. 인스턴싱은 릴리스에서도 1.48배다(#1836).

**무효인 것:** "엔진이 CPU 바운드"라는 서술. 출하 빌드는 1,238 엔티티 + 그림자를 드리우는
포인트 라이트 8개를 **약 2.9ms(≈350 FPS)**에 그린다. 예산 안쪽으로 한참 들어와 있다.
아래에 남은 µs/draw 수치들도 전부 디버그 값이다.

이 절을 지우지 않고 정정으로 남기는 이유는, 이 수치들이 #1833~#1836 네 개 PR의 근거였기
때문이다. 근거가 틀렸다는 사실 자체가 기록될 값어치가 있다.

⚠️ **성능 결론 전에는 `--release`로 잴 것, 그리고 어느 프로필인지 밝힐 것.** CI의 E2E
리플레이도 디버그로 도므로 그쪽 벽시계 시간 역시 출하 속도를 말해주지 않는다.

### 1. 규모 — 천장은 제거됐고, 이제 draw당 CPU 비용이 남았다

**2026-09-09 갱신 (#1834).** 아래 실측이 찾은 1024 천장은 제거됐고, 같은 측정이 프레임의
44%를 차지하던 호출 패턴도 드러내 함께 고쳤다. 셰이더 계약은 안 건드렸다.

|      N | draw_calls | cpu_ms (전) | cpu_ms (후) |
|-------:|-----------:|------------:|------------:|
|    500 |      1,005 |         7.4 |         5.0 |
|  2,000 |      4,005 |        21.6 |        14.2 |
|  5,000 |     10,005 |        70.3 |    **33.1** |

두 가지가 사실이었다:

- **`MAX_OBJECTS = 1024`는 하드웨어 한계가 아니라 임의의 상수였다.** 16 KiB 유니폼 바인딩
  제한은 draw마다 바인딩되는 *범위*(112바이트)에 걸리지 버퍼 전체에 걸리지 않고,
  `max_buffer_size`는 256 MiB다. 16384으로 올려도 검증 오류가 없다.
- **프레임의 97%가 CPU였고, 그중 44%가 호출 하나였다.** `write_buffer`를 오브젝트마다
  불렀고(호출당 6.2µs, 5,000개면 31ms), 패딩된 배열을 한 번에 쓰면 전부 회수된다. 벌크
  버전이 "쓰기를 아예 끈" 버전과 같은 값을 냈으므로 비용은 데이터 이동이 아니라 호출
  오버헤드였다.

**잘릴 때 경고가 붙었다**(프로세스당 1회). 침묵이 높이보다 나빴다는 게 애초 발견의 핵심이다.

⚠️ **단일 측정값은 15% 정도 편차가 있다.** 위 숫자는 유효숫자 두 자리로 읽을 것.

**남은 것:** 5,000개 33ms = 약 30 FPS. 여전히 draw call당 ~3.3µs의 CPU 비용이고, GPU는
2ms(6%)로 놀고 있다. 다음 후보는 **`set_bind_group` 제거(스토리지 버퍼)**와 **인스턴싱**인데,
전자의 실제 이득은 **아직 격리 측정되지 않았다** — 동적 오프셋을 상수로 고정했을 때 4ms가
절약됐지만 그때도 호출은 하고 있었다. 추정은 측정이 아니므로 후보로 남긴다.

---

### 1-b. 페이즈 2 — 저작된 레벨에서 재보니, 문서의 두 주장이 반대 방향으로 틀렸다

페이즈 1은 색칠한 프리미티브만 봤다. 페이즈 2는 `games/scale-level`(160×160 터레인,
프롭 1,200개, 애니메이션 도는 여우, 라이트, 캐릭터 컨트롤러, 스크립트)로 **전체 스택**을
잰다. 프롭 1,200개 고정, 한 번에 하나씩만 바꿔 실측:

| 구성 | cpu_ms | draws |
|---|---:|---:|
| 여우 0, 라이트 0 | 9.9 | 1,559 |
| **여우 100**, 라이트 0 | **12.7** | 1,759 |
| 여우 20, **포인트 1** | 26.7 | 6,333 |
| 여우 20, 포인트 2 + 스팟 4 | 45.0 | 11,067 |
| 여우 20, **포인트 8 + 스팟 8** | **165.5** | 39,471 |

**① "CPU 스키닝은 의도적으로 캐릭터 1~2개 규모용"은 틀렸다 — 심하게 보수적이었다.**
이 문장은 이 문서 첫 버전(2026-07-31)부터 축 1에 있었고 한 번도 측정되지 않았다.
**애니메이션이 도는 스킨드 여우 100마리가 2.8ms**, 마리당 0.03ms다. 몇 달간 엔진을 실제보다
훨씬 약하게 묘사해온 셈이다. (해당 문구는 위 축 1에서 수정했다.)

**② 아무도 주장하지 않은 라이트가 전부였다.** 섀도우 캐스팅 **포인트 라이트 하나가 ~18ms**,
draw call을 4,734개 추가한다 — 포인트 섀도우가 **6면 큐브 렌더**라 엔티티 1,231개가 draw
6,333개가 된다. `MAX_POINT_LIGHTS`(8)를 채우면 **165ms, 약 6 FPS**. 스팟은 큐브가 없어
포인트 옆에서 거의 공짜다.

**③ 처음으로 GPU가 실제로 일하는 지점을 찾았다.** 페이즈 1에서 GPU는 프레임의 3%로 놀았다.
8+8 구성에서는 `point_shadow` 패스만 **26.9ms**로 메인 패스(1.57ms)의 **17배**다. 규모의
병목은 오브젝트 수가 아니라 **그림자를 던지는 광원 수**다.

**다음 후보 (실측이 가리키는 것, 결정 아님):**
1. **포인트 섀도우의 광원 범위 컬링.** 포인트 라이트 하나가 6면 × 약 790개를 그린다 —
   `range: 45`인 광원이 160×160 월드의 대부분을 각 면에 그리고 있다는 뜻이다. 범위 밖 오브젝트를
   빼는 것이 규모 축에서 가장 값싼 다음 한 걸음으로 보인다.
2. 섀도우 캐스팅 광원 수 제한 또는 섀도우 아틀라스/캐싱.
3. 인스턴싱 — 페이즈 1이 남긴 draw당 CPU 비용.

⚠️ **이 표가 말하지 않는 것:** 메시는 여전히 `fox.glb` 하나뿐이고(저장소에 그것뿐), 프롭은
프리미티브다. 텍스처는 터레인 스플랫이 실제로 샘플링되지만 프롭별 텍스처는 없다. 단일 측정값
편차는 15% 정도다.

---

### 1-a. (기록) 천장을 찾은 최초 실측 — 1024에서 조용히 잘렸다

2026-09-09에 실제로 재봤다(`bsengine-runtime`의 `scale_sweep_table`, 이 머신, `fast_render`
off — CI의 clear-only 고속 경로를 켜면 그림자/SSAO/블룸이 안 돌아 측정 자체가 무의미해진다):

|      N | draw_calls | triangles | occluded |  cpu_ms |
|-------:|-----------:|----------:|---------:|--------:|
|    100 |        205 |    79,925 |        0 |   3.180 |
|    500 |      1,005 |   399,605 |        0 |   7.013 |
|  2,000 |      2,053 |   818,993 |        0 |  14.887 |
|  5,000 |      2,053 |   818,993 |        0 |  13.210 |

**2,000과 5,000이 바이트 단위로 같다.** 원인은 추론이 아니라 소스에서 확인했다 —
`MAX_OBJECTS`가 **1024**이고(`bsengine-rhi-wgpu/src/surface.rs:1163`, `MODEL_STRIDE` 256짜리
모델 유니폼 버퍼 1024칸), `render_frame`은 `.min(MAX_OBJECTS)`로 자르며 터레인 청크는
`break`한다. 2,053 = 2 × 1024 + 5 — 그려진 엔티티당 2패스(메인+섀도우) 더하기 상수.

**그리고 아무 경고도 없다.** 2,000개짜리 레벨을 저작한 사람은 천 개 남짓만 그려지는 것을 보고
어떤 진단도 받지 못한다. 격차의 크기보다 이 **침묵**이 더 나쁜 성질이다.

천장은 테스트로 박아뒀다(`entities_past_the_renderers_object_cap_are_silently_dropped`).
숫자가 아니라 **침묵을 단언**하므로, `MAX_OBJECTS`를 올리거나 경고를 추가하면 실패하면서 이
문서를 갱신하라고 알려준다.

> **그리고 실제로 그렇게 됐다(2026-09-21 대조).** #1834가 천장을 16384로 올리고 경고
> (`surface.rs`의 `warn_if_truncated`, 프로세스당 1회)를 추가하면서 그 테스트는 실패했고,
> 지금은 **반대를 단언**한다 — 엔티티 수가 잘리지 않고 draw 목록에 도달하는지(2,000 →
> 4,005 draw, 5,000 → 10,005). 위 문단은 그 시점의 기록으로 읽을 것.

**부수 관찰:**
- `occluded`가 모든 N에서 0. 오클루더는 저작 대상이고 이 그리드엔 없으니 정상이지만, 동시에
  **오클루전 컬링은 저작된 오클루더가 있을 때만 일한다**는 뜻이다. 규모를 자동으로 구해주지
  않는다.
- 1024 이하에서 cpu_ms는 N에 거의 선형이다(100→3.2ms, 500→7.0ms). **성능 병목이 오기 전에
  천장이 먼저 온다.**
- item 44~46(터레인/LOD/오클루전)은 전부 규모를 위한 기술이고 각각 정확히 검증됐지만, 그
  도구들이 필요해지는 크기에 닿기 전에 렌더러가 먼저 막는다.
- ~~**레벨 스트리밍이 없다** — 씬은 통째로 로드된다.~~ **#1866/#1867로 해소.** `loadSceneAdditive`/`unloadScene` + `StreamedScene` 앵커(경로·반경·히스테리시스 밴드).

⚠️ **이 표가 말하지 않는 것:** 색칠한 프리미티브뿐이고 텍스처도 스키닝도 없으며 그림자는
디렉셔널 라이트 하나뿐이다. 실제 씬에 대한 일반 주장으로 읽으면 안 된다 — 그건 페이즈 2
(저작된 레벨)의 몫이다.

### 실측이 가리키는 후보 (결정 아님)

1. **`MAX_OBJECTS` 자체.** 1024는 개인 규모 게임에도 낮다. 유니폼 버퍼를 키우거나 인스턴싱/
   스토리지 버퍼로 옮기는 것이 규모 축에서 가장 값싼 한 걸음이다. 그게 크다면 **최소한 잘릴 때
   경고라도** 내야 한다 — 지금은 침묵이 가장 큰 문제다.
2. **CSM / GPU 스키닝.** 천장을 올린 뒤에야 실제로 필요한지 알 수 있다.
3. **레벨 스트리밍.** 위 둘 다음의 문제.

**저작 쪽 별개 발견 — 해소됨 (2026-09-22):** `SpawnParams`는 `name`/`primitive`/트랜스폼/
`color`/`emissive`/`script`만 받아 **glTF 메시나 텍스처를 만들 수 없었고**, 게임 11개 중 어느
것도 호출하지 않았으며 실행하는 테스트도 없었다. 지금 `Bsengine.spawn`은 씬 파일의
`EntityDescriptor`를 JS 객체 철자로 그대로 받아 `spawn_scene_entities`(씬·프리팹과 같은 로더)를
탄다 — 씬이 저작할 수 있는 것(glTF·텍스처·물리·라이트·리플렉트 컴포넌트)은 스크립트도 만든다.
Unity `Instantiate` / Unreal `SpawnActor` / Godot `instantiate()`가 수렴하는 "인자 = 완전한 서술"
모델이다. `parent:`/`joint:`는 같은 호출 안의 엔티티끼리만 해석되므로(씬 파일과 같은 규칙) 기존
엔티티에 붙이려면 다음 틱에 `setParent`를 쓴다.

### 2. 오디오 — 해소됨 (2026-09-11)

2026-09-09판은 이 축을 "유일하게 손대지 않은 축, 856줄 그대로"로 적었다. **지금은
3,141줄이고 그때 '없음'으로 적은 세 가지가 전부 있다.**

| 2026-09-09판이 없다고 적은 것 | 지금 |
|---|---|
| 믹서 버스 | `assets/audio/buses.ron` — Master 루트 트리, 이름으로 선택, `setBusVolume` (#1837) |
| DSP 이펙트 | 버스별 체인 7종(reverb/filter/compressor/delay/distortion/EQ/panning) (#1838) |
| 오디오 오클루전 | 레이캐스트 + 로우패스 + 감쇠, Unreal의 `OcclusionInterpolationTime` 의미론 (#1839) |

여기에 **이 문서가 요구하지도 않았던 런타임 파라미터 제어**(#1840)가 더해졌다 — Unity의
exposed parameters에 해당하며, 세 레퍼런스 엔진이 모두 갖고 있던 것이다.

설계는 전부 세 엔진의 수렴점을 따랐다: 저작된 프로젝트 단위 에셋, Master로 합쳐지는 트리,
이름 기반 선택, 버스별 이펙트 체인. 갈리는 지점(미지의 버스 이름)에서는 Unreal의 Master
폴백을 택했다 — 오디오에서 가장 나쁜 실패 양상은 침묵이기 때문.

**남은 것:** 에디터 믹서 패널(Unity/Godot에는 있음), send/return 버스, 스냅샷/더킹.
전부 "있으면 좋은 것"이지 격차로 세울 만한 것은 아니다.

### 3. 렌더링 — item 43~50이 다루지 않은 것

소스 검색으로 부재를 확인한 항목:

- ~~**캐스케이드 섀도우 맵(CSM) 없음**~~ — **#1842/#1843으로 해소.** 절두체가 월드 원점에
  고정돼 있던 결함을 먼저 고치고(#1842) 캐스케이드 4개 + `[render]` 저작 + 경계
  크로스페이드를 넣었다(#1843).
- ~~**스크린스페이스 리플렉션 없음**~~ — **#1859로 해소**(뎁스 버퍼 레이 마칭 + 불투명
  패스가 쓰는 노멀/거칠기 버퍼, 기본 꺼짐). 원문은 item 48이 IBL과 라이트 프로브 GI를 넣었지만 SSR은
  별개다. **여전히 유효한 격차.**
- **GPU 스키닝 없음** — CPU LBS를 매 프레임 돌린다. 원본 문서의 "캐릭터 1~2개 규모용"이라는
  단서가 그대로 유효하며, 이것도 규모 격차의 일부다.
- ~~**데칼 없음**~~ — **#1856으로 해소.** Unity URP의 DBuffer(뎁스 프리패스 + 조명 전
  albedo 접기). 터레인 수신자와 노멀/거침기 데칼은 의도적 범위 밖.

### 4. 그 밖에

- **UI 프레임워크**: 254 → **551줄**. 앵커(#1841)로 해상도 독립성은 확보했다 —
  Unity `RectTransform`, Unreal UMG, Godot `Control`이 모두 쓰는 정규화 0~1 앵커와 같은
  모델이다. **남은 것은 레이아웃 컨테이너**(HBox/VBox/Grid): `UiState.widgets`가 아직
  평면 `Vec`이라 부모-자식 구조가 먼저 필요하고, 그래서 앵커보다 큰 작업이다.
  작은 구멍 하나 더 — `UiWidget::Image`는 Rust에 있으나 `setImage` op이 없어 게임에서
  도달할 수 없다.
  **위 두 줄 모두 해소됨**: 레이아웃 컨테이너는 #1844(행/열) + #1846(Grid) + #1851/#1852
  (스크롤), 이미지는 #1845.
- **네트워킹 RPC 프레임워크 없음** — item 56이 예측/보간/AOI를 넣어 "기초 수준"은
  벗어났지만 RPC는 여전히 없다. **착수 중**(신뢰 채널 + 전송 계층이 1번 PR, 스크립트
  표면이 2번 PR).
- ~~**다차원 블렌드 트리 없음**~~ — **#1850으로 해소.** 삼각분할 + 바리센트릭 가중치
  (Unreal·Godot 다수결).
- ~~**플랫폼 폭**: Windows/현재 개발 플랫폼만~~ — **Windows·Linux(Ubuntu)·macOS 셋 다 매 PR CI에서 검증된다**(#1871) — 빌드, 전체 테스트, #1869의 실제 창 스모크(winit이 창을 열고 5프레임을 그려 `draw_calls > 0`). **없음**: 모바일, 콘솔. ⚠️ **macOS는 부분 검증** — CI가 파일워처의 실제 버그 2개를 찾았다. **버그 1(경로 심볼릭 링크 불일치)은 #1872로 확정 해소**(측정 테스트가 CI에서 통과해 직접 증명됨). **버그 2(FSEvents가 같은 디렉터리 rename의 두 반쪽을 비신뢰적으로 보고)는 미해결** — 우리 코드 제어 밖으로 보이고, 로컬 macOS 없이 3번째 CI 왕복은 비용 대비 근거가 약해 정직하게 보류함. 관련 테스트 4개는 여전히 macOS에서만 `#[ignore]`, 근거는 `crates/bsengine-asset/src/watcher.rs`의 `resolve_watch_prefix` 문서 주석과 그 테스트들의 `ignore` 사유. ⚠️ Ubuntu 검증은 Xvfb + 소프트웨어 Vulkan(lavapipe)이라 실제 GPU·오디오 장치·Wayland는 여전히 미검증.

### 판정 (2026-09-11)

원본 문서와 그 갱신들이 지적한 격차는 **전부 닫혔다.** 오디오까지 닫히면서, 이 문서가
"격차"로 이름 붙일 수 있는 축은 사실상 남지 않았다.

**2026-09-09판의 권고가 실행됐고, 결과를 채점할 수 있다.** 그 판은 "개별 기능 목록보다
규모로 한 번 밀어보기가 정보량이 크다"고 적었다. 실제로 그렇게 했고(#1833~#1836), 절반은
맞고 절반은 틀렸다.

- **맞은 것:** 실험이 스스로 다음 작업을 만들어냈다. 1024 draw 천장(#1834), 섀도우
  캐스팅 포인트 라이트가 프레임의 95%(#1835), 섀도우 패스 인스턴싱(#1836) — 어느 것도
  기능 목록에서는 나오지 않았을 항목이다.
- **틀린 것:** 그 판이 근거로 삼은 수치가 **디버그 빌드**였다(위 1-c). "CPU 바운드라
  CSM·GPU 스키닝·스트리밍이 차례로 필요해질 것"이라는 예측은 릴리스에서 성립하지 않는다.
  1,238 엔티티 + 그림자 포인트 라이트 8개가 2.87ms다.

**그래서 다음 권고는 다르다.** 규모는 더 밀 이유가 약하다. 남은 것들은 성격이 둘로
갈린다:

1. **실제로 쓰이면 곧 불편해질 것** — UI 레이아웃 컨테이너(앵커만으로는 목록·메뉴를
   손으로 좌표 계산해야 함), 네트워킹 RPC. 둘 다 "있으면 좋은 것"이 아니라 그것 없이
   만들다 보면 매번 우회로를 짜게 되는 종류다.
2. **증명되지 않은 필요** — CSM, GPU 스키닝, 스트리밍, 다차원 블렌드 트리, 모바일/콘솔.
   릴리스 실측이 나온 지금, 이것들은 **필요해지는 콘텐츠를 먼저 만들어 보기 전에는
   착수할 근거가 없다.**

⚠️ 이 문서를 근거로 쓸 때의 규칙 하나가 이번 판에서 추가됐다: **성능을 말하는 줄은
어느 프로필에서 잰 것인지 밝혀야 한다.** 그러지 않은 줄은 2026-09-09판처럼 반대 방향의
결론을 만들어낼 수 있다.

---

## 2026-09-16 정정 — 위 목록에서 이미 닫힌 항목들

이 문서를 근거로 다음 작업을 고르다가, **"남은 격차"로 적힌 것 중 셋이 이미 닫혀
있었다**는 것을 소스 검색으로 확인했다. 2026-09-11판 이후 병합된 것들이다.

| 문서가 "없음"이라 적은 것 | 실제 |
|---|---|
| 캐스케이드 섀도우 맵 | #1842(절두체 고정 버그) + #1843(캐스케이드 4개)로 해소 |
| 다차원 블렌드 트리 | #1850(삼각분할 + 바리센트릭)으로 해소 |
| UI 레이아웃 컨테이너 / `setImage` | #1844·#1846·#1851·#1852 / #1845로 해소 |
| 오디오 믹서 패널 | #1853으로 해소 |
| 애니메이션 IK | **문서가 틀린 게 아니라 내가 틀렸다** — `bsengine-gltf/src/ik.rs`의
  `IkChain`/`IkChains`와 `bsengine-physics`의 `FootIkGround`가 이미 있다. 남은 건
  *풀바디* IK뿐 |

⚠️ **이 문서의 산문을 근거로 항목을 분류하지 말 것.** CSM(1-c 절)과 스크립팅 타입
안전성에 이어 **세 번째**다. 세 번 모두 "코드를 열어 확인"이 분류를 뒤집었고, 세 번 모두
문서 쪽이 뒤처져 있었다. 다음 작업을 고를 땐 이 표가 아니라 `grep`이 근거여야 한다.

### 남은 진짜 격차 (2026-09-16 기준, 전부 소스 검색으로 부재 확인)

- ~~**스크린스페이스 리플렉션 없음**~~ — #1859로 해소
- **데칼 없음**
- **GPU 스키닝 없음** — 단, '⚠️ 스케일 수치는 전부 디버그 빌드였다' 정정 이후 성능
  근거는 약하다. 릴리스에서 여우 100마리 CPU 스키닝이 문제가 되지 않는다
- **네트워킹 RPC 없음** — 착수 중
- **소프트바디/천 없음**
- **풀바디 IK 없음** (두 뼈 IK와 발 IK는 있음)
- ~~**에셋 스트리밍 없음**~~ — 씬 스트리밍은 #1866/#1867로 있음. 텍스처 밉 스트리밍 1단계는 #1885, 거리 기반 목표 밉·예산은 2단계(2026-09-26, 맨 위 절 참조)
- ~~**플랫폼 폭**: Windows만~~ — **Windows와 Linux(Ubuntu)는 매 PR CI에서 검증된다** — 빌드, 전체 테스트, 그리고 #1869의 실제 창 스모크(winit이 창을 열고 5프레임을 그려 `draw_calls > 0`). **없음: macOS**(CI 없음, 검증된 적 없음), 모바일, 콘솔. ⚠️ Ubuntu 검증은 Xvfb + 소프트웨어 Vulkan(lavapipe)이라 실제 GPU·오디오 장치·Wayland는 미검증.

---

## 2026-09-17 갱슴 — 네트워킹 RPC와 데칼

| 항목 | PR | 결과 |
|---|---|---|
| 오디오 및서 패널 | #1853 | 해소. 패널 8개 |
| 네트워킹 RPC | #1854 + #1855 | 해소. **신뢰 채널이 없던 게 진짜 원인이었다** — 세 엔진 모두 RPC 기본값이 reliable |
| 데칼 | #1856 | 해소. DBuffer(덩스 프리패스 + 조명 전 albedo 접기) |

### 남은 결과 (전부 부재를 grep으로 확인)

- 스토리밍
- SSR
- GPU 스키닝 (단, 릴리스 수치 이후 성능 근거는 약함)
- 소프트바디/천
- 풀바디 IK (두 변 IK와 발 IK는 이미 있음)
- ~~플랫폼 폭: Windows만~~ — Windows와 Linux는 CI 검증됨(#1869), macOS·모바일·콘솔은 없음
- 터레인 데칼 수신, 노멀/거침기 데칼 (#1856의 명시된 후속)

---

## 2026-09-17 (2) — 렌더링 축이 닫혔다

| 항목 | PR |
|---|---|
| 데칼 | #1856 + #1857(터레인 수신자) + #1858(노멀) |
| 스크린스페이스 리플렉션 | #1859 |

**이 문서가 "렌더링 격차"로 이름 붙였던 항목은 이제 없다.** CSM · SSR · 데칼 전부 해소.

### 남은 것 (전부 부재를 grep으로 확인)

- **GPU 스키닝** — 단 '⚠️ 스케일 수치는 전부 디버그 빌드였다'(디버그 오측정) 이후 성능 근거는 약하다 — 릴리스에서 CPU 스키닝 여우 100마리가 2.8ms로 문제되지 않는다. 규모 논거로만 유효.
  릴리스에서 CPU 스키닝 여우 100마리가 문제되지 않는다. 규모 논거로만 유효
- **소프트바디/천**
- **풀바디 IK** (두 뼈 IK와 발 IK는 이미 있음)
- **에셋 스트리밍**
- ~~**플랫폼 폭**: Windows만~~ — **Windows와 Linux(Ubuntu)는 매 PR CI에서 검증된다** — 빌드, 전체 테스트, 그리고 #1869의 실제 창 스모크(winit이 창을 열고 5프레임을 그려 `draw_calls > 0`). **없음: macOS**(CI 없음, 검증된 적 없음), 모바일, 콘솔. ⚠️ Ubuntu 검증은 Xvfb + 소프트웨어 Vulkan(lavapipe)이라 실제 GPU·오디오 장치·Wayland는 미검증.

### SSR의 명시된 한계 (나중에 발견되지 않도록)

- **커스텀 셰이더로 그린 표면은 노멀을 쓰지 않으므로 반사에 참여하지 않는다.** 저자의
  WGSL은 출력이 하나뿐이다
- **투명 표면은 노멀 버퍼에서 의도적으로 제외** — 유리의 노멀에서 추적한 반사는 유리
  뒤에 있는 것을 덮어쓴다
- 시간적/계층적 정제 없음. 픽셀당 레이 하나, 실패하면 페이드
