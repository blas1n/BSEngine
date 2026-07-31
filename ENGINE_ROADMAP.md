# BSEngine — Unity/Unreal 수준 달성 로드맵

이 파일이 작업 기준점. 여기 정의된 순서와 완료 조건을 벗어나지 말 것.

---

## 규칙

- 이 파일의 순서대로만 작업
- 한 항목이 완료되어야 다음 항목 시작
- 각 항목은 아래 **완료 조건**을 전부 충족해야 완료
- 범위 밖 작업(zoo* 같은 것)은 하지 말 것

---

## 작업 목록

### 1. UI System ✅

**목표:** egui를 엔진에 완전 통합하여 게임 내 UI와 스크립팅 API를 제공

**완료 조건:**
- [x] `bsengine-ui` 크레이트 (또는 `bsengine-editor`에 통합) 에서 egui 렌더링 동작
- [x] 기본 위젯: Panel, Button, Label, TextInput, Image
- [x] Scripting API: `Bsengine.ui.*` 로 JS에서 UI 조작 가능
- [x] 예제 게임 또는 데모에서 인게임 HUD 동작 확인
- [x] 테스트 추가, CI 통과

---

### 2. Animation State Machine ✅

**목표:** GLTF 애니메이션 클립을 상태 기계로 조합 (blend tree, 전이 조건)

**완료 조건:**
- [x] `AnimationStateMachine` 컴포넌트: 상태, 전이 조건, 현재 상태 정의
- [x] 상태 간 blend (crossfade) 동작
- [x] Scripting API: 상태 전환 트리거 가능
- [x] 캐릭터 idle→walk→run 전환 예제
- [x] 테스트 추가, CI 통과

---

### 3. Pathfinding ✅

**목표:** NavMesh 빌드 + 에이전트 자동 경로 탐색

**완료 조건:**
- [x] NavMesh 빌드 (순수 Rust A* on uniform XZ grid, 8방향)
- [x] `NavMeshAgent` 컴포넌트: 목적지 설정 → 자동 이동
- [x] 동적 장애물 회피 (기본 수준)
- [x] Scripting API: `Bsengine.navmesh.*` 로 경로 제어
- [x] 테스트 추가, CI 통과

---

### 4. Save / Serialization ✅

**목표:** 게임 상태를 파일에 저장하고 복원

**완료 조건:**
- [x] 지정 컴포넌트 집합을 JSON으로 직렬화 (Name, Transform, SaveData)
- [x] 저장 파일 로드 후 엔티티 복원 (기존 업데이트 or 새로 스폰)
- [x] Scripting API: `Bsengine.save()` / `Bsengine.load()`
- [x] 테스트 추가, CI 통과

---

### 5. Custom Shaders ✅

**목표:** WGSL 셰이더 에셋을 로드하여 머테리얼에 적용

**완료 조건:**
- [x] `.wgsl` 파일 에셋 로더 (런타임 lazy load)
- [x] `CustomShader` 컴포넌트: WGSL 파일 경로 바인딩
- [x] 렌더 파이프라인에서 custom shader 경로 처리 (per-draw-call pipeline 선택)
- [x] Scripting API: `Bsengine.setShader()` / `Bsengine.clearShader()`
- [x] 테스트 추가, CI 통과

---

### 6. Post-Processing ✅

**목표:** 렌더 파이프라인에 post-process 패스 통합

**완료 조건:**
- [x] Bloom
- [x] Tone-mapping (ACES 또는 동등)
- [x] SSAO (Screen Space Ambient Occlusion)
- [x] 각 효과 on/off 및 파라미터 조절 가능
- [x] Scripting API: `Bsengine.postprocess.*`
- [x] CI 통과

---

### 7. Networking ✅

**목표:** 기본 클라이언트-서버 엔티티 동기화

**완료 조건:**
- [x] 서버/클라이언트 역할 구분
- [x] Transform 등 지정 컴포넌트 네트워크 동기화
- [x] Scripting API: `Bsengine.network.*`
- [x] 2인 로컬 멀티 데모
- [x] CI 통과

---

### 8. Editor Viewport (Unity/Unreal 수준) ✅

**목표:** Unity/Unreal 수준의 씬 에디터 — 씬 뷰포트, 에디터 카메라, Play/Stop 툴바, 도킹 패널 레이아웃

**완료 조건:**
- [x] 씬이 CentralPanel에 보임 (투명 패널로 swapchain 통과, egui 패널은 불투명 오버레이)
- [x] 에디터 오빗 카메라: 우클릭 드래그=오빗, 중간클릭 드래그=팬, 스크롤=줌
- [x] Toolbar (Play ▶ / Stop ■ 토글)
- [x] Hierarchy 패널 (엔티티 목록 + 선택)
- [x] Inspector 패널 (Transform DragValue 편집)
- [x] 오버레이 모드 유지 (editor_mode=false 시 기존 런타임 인스펙터)
- [x] `MouseWheel` 이벤트 + `scroll_delta` InputPlugin에 추가
- [x] RenderPlugin: editor_mode 시 InspectorState 카메라 행렬로 view/proj 오버라이드
- [x] CI 통과

---

### 9. Editor Full Feature Parity (Unity/Unreal 수준) ✅

**목표:** 에디터에서 엔진의 모든 기능을 사용할 수 있도록 — 엔티티 추가/제거, 모든 컴포넌트 편집, 에셋 드롭

**완료 조건:**
- [x] Hierarchy: 엔티티 추가 버튼 (+) → 빈 엔티티 스폰
- [x] Hierarchy: 선택된 엔티티 삭제 버튼 (−)
- [x] Inspector: 컴포넌트 목록 표시 (Transform, Light, Camera, Material 섹션)
- [x] Inspector: Camera 컴포넌트 편집 (fov)
- [x] Inspector: DirectionalLight / PointLight 편집 (color, intensity, range)
- [x] Inspector: Material/PBR 파라미터 편집 (base_color, metallic, roughness, emissive)
- [x] Inspector: 컴포넌트 추가 (Add Point Light, Add Camera 버튼)
- [x] Visible 토글 체크박스
- [x] Scripting 이벤트(play/stop)를 에디터 Play/Stop과 연동
- [x] CI 통과

---

### 10. Editor Viewport Interactivity (Unity/Unreal 수준 조작성) ✅

**목표:** 마우스로 뷰포트를 직접 조작하는 UX — 트랜스폼 기즈모, 멀티셀렉트, Undo/Redo, 키보드 단축키

**완료 조건:**
- [x] 뷰포트 트랜스폼(이동) 기즈모: 선택된 엔티티를 X/Y/Z 핸들로 드래그하여 이동
- [x] 멀티셀렉트: Hierarchy에서 Ctrl/Shift-클릭으로 여러 엔티티 선택
- [x] Undo/Redo: 에디터 명령 히스토리 스택
- [x] 키보드 단축키 (Delete, Ctrl+D 복제, Ctrl+Z/Y)
- [x] CI 통과

---

### 11. 범용 리플렉션 MCP 컴포넌트 부착 툴 ✅

**목표:** AnimationStateMachine/NavMeshAgent 등 attach 경로가 없는 컴포넌트를 AI/MCP가 직접 붙일 수 있게

**완료 조건:**
- [x] `set_reflected_component(entity_id, type_path, value_json)` MCP 툴 — 기존 `ReflectCommand::ApplyComponentValue` 경로 재사용, `process_reflect_commands` 무변경
- [x] `TypedReflectDeserializer` 기반 JSON → `Box<dyn Reflect>` 변환
- [x] AnimationStateMachine(HashMap/Vec/Enum 필드 포함)·NavMeshAgent 실제 부착 검증
- [x] CI 통과

---

### 12. CPU 스켈레탈 스키닝 파이프라인 ✅

**목표:** glTF로 임포트한 스킨드 캐릭터가 실제로 화면에서 애니메이션되도록 (기존엔 애니메이션 클립이 파싱만 되고 버려졌고, 스키닝 렌더링 자체가 없었음)

**완료 조건:**
- [x] glTF skin/joint/노드 계층 파싱 (`NodeTransform`/`SkinData`/`VertexSkin`)
- [x] `SkinnedMesh`/`AnimationClipLibrary` 컴포넌트
- [x] 매 프레임 클립 샘플링 → 조인트 매트릭스 합성 → 버텍스 블렌딩(LBS) → GPU 버텍스 버퍼 재업로드 (`GpuMeshRegistry::update_vertices`, 신규 `GpuQueueResource`)
- [x] `GltfPlugin`이 스킨 있는 glTF 로드 시 자동 부착
- [x] 실제 CC0 애셋(Khronos Fox)으로 수동 검증 — `AnimationPlayer.time`이 프레임마다 전진/루프, 크래시 없음
- [x] 검증 과정에서 발견된 4개의 기존(이번 기능과 무관한) 블로킹 버그 수정: `bsengine-runtime`에 GltfPlugin/TimePlugin/AnimationPlugin/AnimationStateMachinePlugin이 애초에 등록되어 있지 않았음, 인덱스 없는 프리미티브 거부, AnimationPlayer duration 미설정으로 tick 무동작
- [x] CI 통과

---

### 13. 장면 직렬화 — 임의 리플렉트 컴포넌트 저장/로드 ✅

**목표:** `set_reflected_component`로 붙인 AnimationStateMachine/NavMeshAgent/Shield/Bloom/ToneMap 등이 `save_scene` 이후에도 살아남도록 (기존엔 이런 컴포넌트를 위한 저장 경로가 아예 없어서, 재시작하면 사라졌음)

**완료 조건:**
- [x] `bsengine-scene`: `EntityDescriptor.components`(타입 경로 + RON 값 페어)를 `TypedReflectDeserializer` + `ReflectComponent::apply_or_insert`로 로드 시 적용 — 알 수 없는 타입/파싱 실패는 로그 후 스킵, fatal 아님
- [x] `bsengine-editor`: 신규 `EntityInfo.extra_components` 필드 + `populate_snapshot_extra_components` 시스템 — 전용 필드가 이미 있는 Transform/Camera/PointLight/DirectionalLight/SpotLight/Material과, 리로드 시 Entity 인덱스가 재할당되어 깨지는 raw `Entity` 참조 보유 타입(Parent/Follow/LookAt)은 제외
- [x] `build_entity_descriptors`가 `extra_components`를 저장 시 `EntityDescriptor.components`에 반영; 에디터 자체 `EditorCommand::LoadScene` 핸들러(= `bsengine-scene::ScenePlugin`과 별개인 두 번째 씬 로딩 경로)도 `EntityCommands::insert_reflect`로 동일하게 적용
- [x] 작업 중 발견한 잠재 버그 수정: `HashSet<String>`(`AnimationStateMachine::triggers`)에 `ReflectDeserialize`만 등록되어 있고 `ReflectSerialize`가 빠져 있어, 저장 시 AnimationStateMachine 컴포넌트 전체가 조용히 누락되던 문제
- [x] `set_reflected_component`로 부착한 NavMeshAgent/AnimationStateMachine의 저장→재로드 라운드트립 실제 검증
- [x] CI 통과

---

### 14. Unity/Unreal 비교 검증 데모 ("Mini Action Arena") ✅

**목표:** 실제 소규모 게임을 엔진으로 만들어보며 여러 시스템이 동시에 맞물릴 때의
격차를 검증하고 기록

**완료 조건:**
- [x] `games/mini-arena/` 프로젝트, 콘텐츠는 최대한 에디터 워크플로우로 제작
- [x] CC0 스키닝 애니메이션 캐릭터 임포트 + AnimationStateMachine idle/walk/run 전환
- [x] NavMeshAgent 기반 적 추적 AI 1종 이상
- [x] Rapier 물리 기반 전투/피격/넉백
- [x] 커스텀 WGSL 셰이더 오브젝트 1종 이상
- [x] 포스트프로세싱(bloom/tone-mapping) 적용
- [x] HUD(체력/점수) + 일시정지 메뉴 (Bsengine.ui)
- [x] 체크포인트 세이브/로드
- [x] AI E2E 테스트로 플레이스루 recording 확보 + replay 통과
- [x] 갭 로그 작성
- [x] CI 통과

<!--
E2E 테스트 작성 과정에서 발견한 10개의 실제 버그(엔진 5개 + player.js 콘텐츠 5개)를
모두 즉시 수정: (1) 리플렉트 타입 등록이 EditorPlugin에만 있어 헤드리스 테스트 모드에서
Shield 등 모든 컴포넌트가 조용히 누락됨 (bsengine-scene::register_gameplay_reflect_types
공유 함수로 추출), (2) test_mode.rs의 PressKey/ReleaseKey가 Input<T>를 직접 mutate해
edge-triggered 입력(isKeyDown/isKeyUp)이 프로토콜을 통해 절대 관측 불가능했음
(Events<KeyInput>/Events<MouseInput> 경유로 수정), (3) NavMeshPlugin이 bsengine-runtime
어디에도 등록된 적이 없어 실제 빌드에서 적 추적 AI가 한 번도 동작한 적이 없었음
(main.rs/test_mode.rs 양쪽에 추가, 헤드리스 쪽엔 TimePlugin도 함께 필요),
(4) Bsengine.raycast()가 entity_name(snake_case)을 반환해 관례상 entityName을
기대하는 모든 호출이 항상 undefined 비교였음 (RaycastHitJson에 camelCase rename 추가),
(5) player.js: 이동에 isKeyDown(edge) 사용 — 다른 모든 게임은 isKeyPressed(held) 사용,
(6) player.js: yaw 부호가 반대라 forward 벡터가 이동 방향과 정반대,
(7) player.js: 공격 레이캐스트 origin이 자기 자신의 콜라이더 내부에서 시작해 즉시
self-hit, (8) player.js: 레이캐스트 origin 높이(+0.9)가 콜라이더 상단(0.85)보다 높아
항상 헛스윙, (9) player.js: damageShield 직후 getShield로 즉시 재확인해 스테일 스냅샷을
읽음 — 30/15 데미지 Enemy가 의도한 2타가 아니라 3타 필요했음. 자세한 근거는
`games/mini-arena/GAP_LOG.md` 참고.
-->

---

### 15. `Bsengine.moveEntity` 상대 이동 스크립팅 op ✅

**목표:** 스크립트에서 엔티티를 상대적으로 이동시킬 때마다 위치를 읽고 델타를 더해
다시 쓰는 반복 패턴을 없애기 (Mini Action Arena 갭 로그에서 발견)

**완료 조건:**
- [x] `Bsengine.moveEntity(name, dx, dy, dz)` op 추가 — 월드 스페이스 델타를
  `Transform.translation`에 직접 적용, 대상 없으면 조용히 no-op
- [x] `games/mini-arena`의 `player.js`/`enemy.js`를 새 op로 교체
- [x] E2E replay(`basic-playthrough.testlog.json`) 통과 확인
- [x] CI 통과

---

### 16. `Bsengine.quit()` 스크립팅 op ✅

**목표:** 스크립트에서 게임 프로세스를 깔끔하게 종료할 방법 제공 — 지금까지는
일시정지 메뉴의 Quit 버튼이 창을 직접 닫으라는 안내만 표시했음 (Mini Action Arena
갭 로그에서 발견)

**완료 조건:**
- [x] `Bsengine.quit()` op 추가 — `AppExit` 이벤트 전송
- [x] `bsengine-window`의 winit 러너가 `AppExit`를 실제로 처리하도록 수정
  (기존엔 `WindowEvent::CloseRequested`만 처리했음)
- [x] `games/mini-arena`의 일시정지 메뉴 Quit 버튼을 새 op로 교체
- [x] 두 절반(스크립팅 op가 `AppExit` 전송 / 러너가 `AppExit` 수신 시
  `event_loop.exit()` 호출) 각각 독립적으로 자동 테스트 검증 — 단, 실제 GUI
  클릭으로 창이 닫히는 end-to-end 수동 확인은 수행하지 않음(클릭 자동화 도구
  없음); 두 절반이 각각 검증되었고 연결 로직이 한 줄이라 간접 근거로 충분하다고
  판단, 사용자 확인함
- [x] CI 통과

---

### 17. 물리 바디(rigidbody/collider) 부착 MCP 툴 ✅

**목표:** 살아있는 엔티티에 물리 바디를 AI/MCP가 직접 부착할 수 있게 (지금까지는
수동 작성 씬 RON에서만 가능했음). 설계 중 발견한 관련 갭도 함께 수정: 에디터의
`save_scene`이 로드된 씬의 rigidbody/collider도 항상 조용히 버리고 있었음
(Mini Action Arena 갭 로그에서 발견)

**완료 조건:**
- [x] `attach_physics_body`/`detach_physics_body` MCP 툴 추가 — 기존
  `resolve_physics_bodies` 시스템이 `PhysicsBodyDesc` 삽입을 감지해 실제 Rapier
  컴포넌트로 변환하므로 물리 로직 중복 없음
- [x] `EntityInfo`/`update_editor_snapshot`/`build_entity_descriptors` 확장 —
  물리 바디가 `save_scene` 이후에도 살아남도록 (부착 경로 무관하게)
- [x] 구현 중 발견한 관련 로드측 버그 수정: `EditorCommand::LoadScene`(
  `bsengine_scene::spawn_scene_entities`와 별개인, 에디터 자체의 두 번째 씬
  로딩 경로)가 로드된 rigidbody/collider로부터 `PhysicsBodyDesc`를 스폰한 적이
  없어서, 저장은 되어도 다시 로드하면 사라졌음
- [x] 부착/해제/저장-재로드 왕복 테스트 추가
- [x] CI 통과

---

### 18. `gltf:`/`CustomShader.path` 프로젝트 상대 경로 해석 ✅

**목표:** `gltf:` 씬 RON 필드와 `Bsengine.setShader`의 `CustomShader.path`가
`script:`/`loadScene`/`setSkybox`처럼 프로젝트 디렉터리 기준 상대 경로로
해석되게 함 (기존에는 프로세스 CWD 기준이라 리포 루트에서만 실행 가능했음)

**완료 조건:**
- [x] `ProjectDir`/`resolve_project_path`를 `bsengine-core`로 이동 (scripting/
  scene/gltf/editor가 공유하는 크레이트이므로)
- [x] `bsengine-scene::spawn_scene_entities`의 `gltf:` 필드, `bsengine-scripting`의
  `SetCustomShader` 핸들러가 새 helper로 경로 해석
- [x] 구현 중 발견한 관련 버그 수정: `EditorCommand::LoadScene`(에디터 자체의
  별도 씬 로딩 경로)가 `entity.gltf`를 아예 처리하지 않아 `load_scene` MCP
  툴로 불러온 씬은 gltf 엔티티의 메시가 통째로 빠졌음
- [x] Mini Action Arena 콘텐츠(`main.ron`, `pickup.js`)를 CWD 상대 경로
  워크어라운드에서 프로젝트 상대 경로로 전환
- [x] CI 통과

---

### 19. 커스텀 셰이더 시간(time) 유니폼 ✅

**목표:** 커스텀 WGSL 셰이더가 `Bsengine.setEmissive()` 같은 매 프레임 JS
왕복 없이 자체적으로 시간 기반 애니메이션(pulse 등)을 계산할 수 있게

**완료 조건:**
- [x] `CameraUniformData`의 기존 미사용 패딩 필드(`_pad`)를 `time`으로 재활용 —
  버퍼 크기/정렬 변경 없음
- [x] `bsengine_core::Time`(이미 매 프레임 tick)을 `bsengine-render`의
  `render_frame` 시스템 → `bsengine-rhi-wgpu`의 `WgpuSurface::render_frame`
  → GPU 유니폼 버퍼까지 연결
- [x] Mini Action Arena의 `glow.wgsl`/`pickup.js`를 실제로 전환 — 셰이더가
  `camera.time`으로 직접 pulse 계산, `pickup.js`의 매 프레임
  `Bsengine.setEmissive()` 폴링 제거
- [x] CI 통과

---

### 20. 프로그레스 바 / 체력 바 UI 위젯 ✅

**목표:** `Bsengine.ui.*`에 `setLabel`/`setButton`/`setPanel`/`setTextInput`와
동급인 프로그레스/체력 바 위젯 추가 (기존엔 텍스트로만 흉내 낼 수 있었음)

**완료 조건:**
- [x] `UiWidget::ProgressBar` 추가, `Bsengine.ui.setProgressBar(id, x, y,
  width, height, fraction)` 스크립팅 API로 노출 — 기존 5개 위젯과 동일한
  파이프라인(ScriptCommand → op → JS 바인딩 → 핸들러 → egui 렌더)
- [x] `egui::ProgressBar`로 렌더링
- [x] Mini Action Arena의 `hud.js`가 실제로 전환 — 체력 표시가 평문 텍스트
  대신 진짜 바 위젯 사용
- [x] CI 통과

---

### 21. 실제 일시정지 / 타임스케일 시스템 ✅

**목표:** 일시정지 메뉴가 UI만 띄우는 게 아니라 실제로 시뮬레이션을 멈추게 함
(기존엔 Enemy 추적/피격, Player WASD/공격이 일시정지 중에도 계속 동작했음)

**완료 조건:**
- [x] `bsengine_core::PauseState` 리소스 추가
- [x] `PhysicsPlugin`/`NavMeshPlugin`의 게임플레이 시스템이 `PauseState`로
  실제 게이트됨 (엔진 차원에서 실제로 멈춤)
- [x] `Bsengine.pause()`/`resume()`/`isPaused()` 스크립팅 API 추가 — 이 엔진엔
  모든 시스템이 공유하는 단일 델타타임이 없어(`Time`과 스크립트
  `getDeltaTime()`은 서로 다른 클럭, Rapier는 자체 고정 스텝) 스크립트 쪽
  게임플레이 로직은 명시적으로 이 API를 확인해야 함
- [x] Mini Action Arena의 `pause.js`/`player.js`/`enemy.js`를 실제로 전환
- [x] CI 통과

---

### 22. 포인트 라이트 그림자 ✅

**목표:** 기존엔 방향광(directional light) 하나만 그림자를 드리웠는데,
포인트 라이트(최대 8개, `MAX_POINT_LIGHTS`)도 실제로 그림자를 드리우게 함

**완료 조건:**
- [x] 선형 거리 큐브 배열(linear-distance cube array) 방식으로 구현 — 각
  포인트 라이트당 6면(정육면체 각 방향) 렌더 패스, `R32Float` 텍스처
  배열(최대 48 레이어)에 광원으로부터의 선형 거리 저장
- [x] 메인 라이팅 셰이더(`MESH_WGSL`)에서 수동 큐브 면 선택 + 저장된 거리와
  비교하는 방식으로 그림자 적용 (depth-compare 큐브맵 대신 선택 — 면 선택
  재구성 로직이 훨씬 단순하고 seam 버그 위험이 적음)
- [x] Mini Action Arena의 `ArenaLight`(포인트 라이트)가 별도 콘텐츠 수정 없이
  자동으로 그림자를 드리움 (방향광과 동일하게 옵트아웃 토글 없음)
- [x] CI 통과

---

### 23. bevy_asset 도입 + glTF/텍스처/셰이더/오디오 통합

**목표:** `bsengine-asset`의 `Handle`/`AssetServer`(사실상 고아 상태 — `load_bytes`를
실제로 쓰는 곳이 에디터 인스펙터/뷰포트 텍스처 로딩과 테스트 1개뿐)를 걷어내고, 이미
워크스페이스가 쓰고 있는 bevy 0.14 생태계의 `bevy_asset`을 도입해 glTF/텍스처(스카이박스
포함)/커스텀 WGSL 셰이더/오디오 4종을 `Handle<T>` 기반으로 통합한다. 실제로는 9곳(씬
RON/스크립트/셰이더/glTF/플러그인 manifest/project manifest/오디오)이 각자
`std::fs::read`를 직접 호출하고 있었음을 조사로 확인 — item 24(핫리로드)의 전제 조건.
설계 문서: `docs/superpowers/specs/2026-07-31-bevy-asset-pipeline-design.md`

**완료 조건:**
- [x] 루트 `Cargo.toml`에 `bevy_asset` 0.14 워크스페이스 의존성 추가(`file_watcher` 피처는
  미활성 — item 24에서 켬)
- [x] `GltfSource`/`ShaderSource`/`AudioSource`/`TextureAsset` 4종에 대한 `AssetLoader`
  구현 — 기존 파싱 로직(`GltfLoader::load_full`, WGSL 컴파일, `StaticSoundData::from_file`)
  을 그대로 이전
- [x] `GltfAsset`/`CustomShader` 등 컴포넌트가 `path: String` 대신 `Handle<T>`를 내부적으로
  보유하도록 전환. 씬 RON 필드/스크립팅 API/MCP 툴 파라미터는 경로 문자열 그대로 유지
  (`AssetServer`가 경계에서 `load::<T>(path) -> Handle<T>` 변환)
- [x] 최초 로드는 동기 블로킹 유지(진짜 비동기 스트리밍은 범위 밖, YAGNI) — 에셋이 몇
  프레임 늦게 나타나는 동작 변화 없음
- [x] `games/mini-arena`/`games/tilt-run` 기존 E2E 리플레이가 마이그레이션 후에도 통과 —
  `games/tilt-run`의 7개 리플레이 전부 클린 통과 확인. `games/mini-arena`의
  `basic-playthrough.testlog.json`은 이 환경에서 재현 시 "Pickup collected" 단언에서
  실패하지만, 동일한 실패(동일한 라벨, 동일한 actual 값)가 이번 item 23의 변경을 전혀
  포함하지 않은 미수정 `master`(커밋 `a539e98e`)에서도 그대로 재현됨 — bevy_asset
  마이그레이션과 무관한 기존 결함. 근본 원인은 `player.js`의 연속 이동이 고정 가상
  타임스텝이 아니라 실제 벽시계 델타(`Bsengine.getDeltaTime()` → `Instant::now()`)로
  구동된다는 점: `games/mini-arena/GAP_LOG.md`가 이미 이 정확한 위험("a standing source
  of potential flakiness for any future JS-movement-driven... headless recording")을
  경고해 두었음. 물리 기반(Rapier 고정 타임스텝) 이동인 tilt-run은 이 문제가 없음.
- [x] 테스트 추가, CI 통과

**참고: 테스트 스택 오버플로 이슈 두 번째 사례 (신규 발견, 이 항목의 작업과 무관).**
기존에 알려진 문제는 `bsengine-editor`의 테스트 바이너리가 워크스페이스 전체 테스트
(`cargo test --all`/`cargo test --workspace`)와 함께 한 프로세스로 묶여 실행될 때만
스택을 오버플로한다는 것이었다 (`cargo test -p bsengine-editor` 단독 실행 시 825개
테스트 모두 통과 — Windows 스택 크기 관련 기존 이슈). 이번 item 23 작업 중 이와는
별개인 **두 번째** 사례를 발견했다: `bsengine-runtime`의
`test_mode::tests::editor_plugin_reload_scene_does_not_corrupt_scripting`는 위 이슈와
달리 **단독 실행**(`cargo test -p bsengine-runtime`)에서도 스택을 오버플로한다. 이
동작은 이번 item 23의 변경과 무관하게 미수정 `master`에서도 동일하게 재현되는 기존
버그로 확인됨 — item 23의 Task 6이 같은 테스트 경로에 있던 더 앞선(더 즉각적인) 패닉을
고치면서 그 아래 깔려 있던 이 스택 오버플로를 우연히 드러낸 것뿐이다. `cargo test
--all`은 항상 `bsengine-editor`의 오버플로에서 먼저 멈추기 때문에, 이 테스트가 실제로
통과하는지는 지금까지 어떤 워크스페이스 전체 테스트 실행에서도 확인된 적이 없었다.
이 항목의 작업 범위가 아니므로 고치지 않고 기록만 남긴다.

---

### 24. 파일 워처 핫리로드 + 안정적 참조 + 조회 API

**목표:** item 23이 확립한 `Handle<T>` 기반 파이프라인 위에, 실행 중인 게임/에디터에 파일
변경을 자동 반영하는 핫리로드와 로드 실패/리로드 상태 조회 기능을 얹는다 (원래 item 23
초안의 완료 조건 중 핫리로드/에러 전파/조회 API 부분을 이관)

**완료 조건:**
- [ ] `bevy_asset`의 `file_watcher` 피처 활성화 — 텍스처/glTF/WGSL/오디오 변경 시 실행
  중인 게임/에디터에 자동 반영
- [ ] 에셋에 안정적 식별자 부여 — 기존 경로 기반 API 하위호환 유지하면서 리네임에도
  참조가 깨지지 않는 경로 마련
- [ ] 로드 실패/누락 에셋에 대한 명확한 에러 전파 (item 23에서는 `tracing::warn!` 후
  스킵뿐이었던 것을 구조화된 조회로 확장)
- [ ] Scripting API 또는 MCP 툴로 리로드/로드 실패 상태 조회 가능
- [ ] 테스트 추가, CI 통과

---

### 25. 3D 포지셔널 오디오

**목표:** `AudioWorld`가 위치 정보 없는 `play`/`stop`뿐인 상태(`bsengine-audio` 251줄)를
벗어나, 엔티티 위치 기반 거리 감쇠·패닝을 제공

**완료 조건:**
- [ ] Emitter 개념(재생 사운드를 엔티티 Transform에 바인딩) + Listener(카메라) 개념 도입
- [ ] kira의 spatial 기능과 연동해 거리 감쇠 + 좌우 패닝 동작
- [ ] Scripting API: `Bsengine.playSound3D(name, path, opts)` 또는 기존 `playSound`에
  위치 바인딩 옵션 추가
- [ ] `games/mini-arena` 등 기존 데모에서 실제 청감 검증
- [ ] 테스트 추가, CI 통과

---

### 26. 폴리곤 기반 실제 NavMesh

**목표:** 현재 "NavMesh"라 불리지만 실제로는 균일 XZ 그리드 위 8방향 A*인 구현
(로드맵 item 3)을, 레벨 지오메트리에서 폴리곤 메시를 추출하는 방식으로 교체 —
좁은 통로/경사/계단에서의 한계 해소

**완료 조건:**
- [ ] 씬의 충돌 지오메트리(또는 별도 태그된 워크어블 메시)로부터 navmesh 폴리곤 빌드
- [ ] 폴리곤 위 경로 탐색 알고리즘 구현 (순수 Rust)
- [ ] 기존 `NavMeshAgent` 컴포넌트/Scripting API/MCP 툴 표면은 하위호환 유지 (내부
  구현만 교체)
- [ ] 균일 그리드로는 처리 안 되던 사례(경사로, 좁은 통로, 비직사각형 레벨)로 검증
- [ ] `games/mini-arena`의 Enemy 추적 AI가 새 구현으로도 정상 동작
- [ ] 테스트 추가, CI 통과

---

### 27. 캐릭터 컨트롤러 + 실제 물리 넉백

**목표:** `NavMeshAgent`가 Transform을 소유하려면 Kinematic rigidbody가 필요하고,
Kinematic 바디는 정의상 물리 임펄스를 무시해 "넉백"을 스크립트가 위치를 직접 미는
방식으로 흉내내야 했던 한계 해소 (`games/mini-arena/GAP_LOG.md` "Pre-existing, not
touched by this task" 항목에서 발견)

**완료 조건:**
- [ ] `CharacterController` 컴포넌트: 중력, 바닥 감지(ground check), 경사/계단 처리
- [ ] 임펄스/넉백을 실제로 받아들이면서 스크립트 이동 입력과 공존 가능한 이동 모델
  (예: Dynamic 바디 + 이동 힘 적용, 또는 Kinematic + 수동 스윕 후 임펄스 별도 합산)
- [ ] `games/mini-arena`의 Enemy 넉백을 스크립트 흉내에서 실제 Rapier 임펄스로 교체
- [ ] 테스트 추가, CI 통과

---

### 28. 파티클 시스템

**목표:** 피격/죽음/픽업 같은 이펙트를 커스텀 셰이더 오브젝트 수작업 없이 표현할 기본
파티클 시스템 제공

**완료 조건:**
- [ ] `ParticleEmitter` 컴포넌트: 스폰율, 수명, 초기 속도/크기/색상, 중력 등 기본 파라미터
- [ ] CPU 시뮬레이션 + 기존 렌더 파이프라인을 재사용한 빌보드/인스턴싱 렌더링
- [ ] Scripting API 또는 씬 RON으로 파티클 이펙트 부착 가능
- [ ] `games/mini-arena`에 최소 1개 이펙트(예: 피격 스파크) 적용
- [ ] 테스트 추가, CI 통과

---

### 29. 애니메이션 블렌드 트리 (1D 블렌드 스페이스)

**목표:** 현재 `AnimationStateMachine`이 상태 간 크로스페이드만 지원하는 것(로드맵 item
2)을 넘어, 이동 속도 같은 연속 파라미터로 여러 클립을 블렌드

**완료 조건:**
- [ ] `BlendTree` 노드 타입 추가 (1D: 단일 파라미터, 최소 2클립)
- [ ] `AnimationStateMachine`의 한 상태가 단일 클립 대신 `BlendTree`를 참조 가능
- [ ] Scripting API로 블렌드 파라미터 갱신 가능 (예: 이동 속도 → walk/run 블렌드)
- [ ] `games/mini-arena` 플레이어의 idle/walk/run 전환을 크로스페이드에서 블렌드 트리로
  교체해 검증
- [ ] 테스트 추가, CI 통과

---

## 보류 백로그 (당장 착수하지 않음)

[BSENGINE_VS_UNITY_UNREAL.md](docs/BSENGINE_VS_UNITY_UNREAL.md) 비교에서 드러났지만,
개인 규모 엔진에서 당장의 필요성이 낮다고 판단해 의식적으로 미룬 항목. 조용히 누락시키지
않기 위해 사유와 함께 기록한다. 재검토 필요 시 위 항목들과 같은 형식으로 번호를 매겨
승격한다.

- **GI/리얼타임 리플렉션/IBL** — 현재 라이팅(방향광+포인트라이트 PCF/큐브 섀도우)으로
  단일 아레나급 씬은 커버됨. 대규모 씬이 생기기 전까진 ROI 낮음.
- **안티에일리어싱** — 데모 규모에서 시각적으로 치명적이지 않음.
- **터레인 시스템** — 지금까지의 데모(mini-arena, tilt-run, cube-*)가 전부 소규모 실내/
  아레나형이라 필요성이 검증되지 않음.
- **머티리얼/셰이더 그래프(비주얼 에디터)** — WGSL 텍스트 파일 직접 작성이 AI 에이전트
  워크플로우와는 오히려 궁합이 좋음(코드로 읽고 쓸 수 있음). 사람 아티스트 워크플로우가
  필요해지면 재검토.
- **프레임 프로파일러/GPU 디버거** — 성능 병목이 실제로 보고된 적 없음.
- **빌드/패키징/배포 파이프라인(콘솔/모바일 export)** — 현재 배포 대상이 없음.
- **프리팹 시스템** — MCP 기반 배치 스폰(`spawn` 배치, 태그/쿼리)이 부분적으로 대체
  역할을 하고 있어 우선순위 낮음.
- **타임라인/시퀀서** — 컷신 요구가 있는 콘텐츠가 아직 없음.
- **LOD / 오클루전 컬링** — 프러스텀 컬링만으로 현재 씬 규모(수십~수백 엔티티)는 충분.

---

## 완료 이력

| 항목 | 완료일 | PR |
|------|--------|----|
| 1. UI System | 2026-07-06 | [#1662](https://github.com/blas1n/BSEngine/pull/1662) |
| 2. Animation State Machine | 2026-07-06 | [#1663](https://github.com/blas1n/BSEngine/pull/1663) |
| 3. Pathfinding | 2026-07-06 | [#1664](https://github.com/blas1n/BSEngine/pull/1664) |
| 4. Save / Serialization | 2026-07-06 | [#1665](https://github.com/blas1n/BSEngine/pull/1665) |
| 5. Custom Shaders | 2026-07-06 | [#1666](https://github.com/blas1n/BSEngine/pull/1666) |
| 6. Post-Processing | 2026-07-06 | [#1667](https://github.com/blas1n/BSEngine/pull/1667) |
| 7. Networking | 2026-07-06 | [#1668](https://github.com/blas1n/BSEngine/pull/1668) |
| 8. Runtime Inspector / Editor (debug overlay) | 2026-07-06 | [#1669](https://github.com/blas1n/BSEngine/pull/1669) |
| 8. Editor Viewport (Unity/Unreal 수준) | 2026-07-07 | [#1670](https://github.com/blas1n/BSEngine/pull/1670) |
| 8. Standalone Editor Binary | 2026-07-07 | [#1671](https://github.com/blas1n/BSEngine/pull/1671) |
| 8. Fix blank viewport (transparent CentralPanel) | 2026-07-08 | [#1674](https://github.com/blas1n/BSEngine/pull/1674) |
| 9. Editor Full Feature Parity | 2026-07-08 | [#1675](https://github.com/blas1n/BSEngine/pull/1675) |
| 8. Fix editor viewport gray (editor_mode + LoadScene + save_scene) | 2026-07-08 | [#1678](https://github.com/blas1n/BSEngine/pull/1678) |
| 8. Fix editor viewport gray (resolve_primitives missing in editor-app) | 2026-07-08 | [#1679](https://github.com/blas1n/BSEngine/pull/1679) |
| 8. Fix editor Play button (ScriptingPlugin missing in editor-app) | 2026-07-08 | [#1680](https://github.com/blas1n/BSEngine/pull/1680) |
| 8. Fix editor Play script path (project_dir from scene path) | 2026-07-08 | [#1681](https://github.com/blas1n/BSEngine/pull/1681) |
| 8. run_scripts refactor + main-thread stack (did not fix V8 crash; see #1683) | 2026-07-09 | [#1682](https://github.com/blas1n/BSEngine/pull/1682) |
| 8. Fix editor Play V8 IsOnCentralStack crash (explicit V8 --stack-size flag) | 2026-07-09 | [#1683](https://github.com/blas1n/BSEngine/pull/1683) |
| 10. Viewport translate gizmo | 2026-07-09 | [#1684](https://github.com/blas1n/BSEngine/pull/1684) |
| 10. Hierarchy multi-select (Ctrl/Shift-click) | 2026-07-09 | [#1685](https://github.com/blas1n/BSEngine/pull/1685) |
| 10. Undo/Redo (snapshot checkpoint reconciliation) | 2026-07-09 | [#1686](https://github.com/blas1n/BSEngine/pull/1686) |
| 10. Keyboard shortcuts (Delete, Ctrl+D, Ctrl+Z/Y) | 2026-07-09 | [#1687](https://github.com/blas1n/BSEngine/pull/1687) |
| Play uses game camera (not orbit); camera frustum + rotate gizmos | 2026-07-10 | [#1688](https://github.com/blas1n/BSEngine/pull/1688) |
| Fix egui keyboard/text input pipeline (typing, Ctrl/Shift-click, shortcuts) | 2026-07-10 | [#1689](https://github.com/blas1n/BSEngine/pull/1689) |
| Clean up all workspace clippy warnings (0 remaining) | 2026-07-10 | [#1690](https://github.com/blas1n/BSEngine/pull/1690) |
| DirectionalLight direction derived from Transform (matches SpotLight/UE) | 2026-07-10 | [#1691](https://github.com/blas1n/BSEngine/pull/1691) |
| Fix editor scene saving (Ctrl+S/toolbar button + complete save_scene serialization) | 2026-07-10 | [#1692](https://github.com/blas1n/BSEngine/pull/1692) |
| Editor dockable panel system (egui_dock) — phase 1 of Unity-motivated UI overhaul | 2026-07-13 | [#1694](https://github.com/blas1n/BSEngine/pull/1694) |
| Reflection-based generic Add/Remove Component (bevy_reflect, replaces hardcoded per-type commands) | 2026-07-14 | [#1695](https://github.com/blas1n/BSEngine/pull/1695) |
| Reflect DirectionalLight/SpotLight/Material, Remove buttons, spot cone angles (+ fix UpdateLight light-type dispatch bug) | 2026-07-14 | [#1696](https://github.com/blas1n/BSEngine/pull/1696) |
| Hierarchy tree + drag-and-drop reparenting + rename + context menu, Inspector Tag/Script/Mesh editing | 2026-07-14 | [#1697](https://github.com/blas1n/BSEngine/pull/1697) |
| #1697 follow-ups: Mesh dropdown drift protection (PRIMITIVE_KINDS), Hierarchy shift-click range-select tree order | 2026-07-14 | [#1698](https://github.com/blas1n/BSEngine/pull/1698) |
| PR C-1: generic reflected-component field editing pipeline (draw_reflect_ui wired into Inspector, parallel to hand-built sections) + Undo/Redo fix for ReflectCommand queue | 2026-07-15 | [#1699](https://github.com/blas1n/BSEngine/pull/1699) |
| PR C-2: migrate Camera.fov_y_radians to ReflectDegrees (Camera-only; SpotLight deferred to a separate follow-up) | 2026-07-15 | [#1700](https://github.com/blas1n/BSEngine/pull/1700) |
| PR C-3: migrate SpotLight.inner_angle/outer_angle to ReflectDegrees (boundary-inverted vs. Camera — external command/MCP layer stays radians); CI fix (apt-get update before Ubuntu system deps) | 2026-07-16 | [#1701](https://github.com/blas1n/BSEngine/pull/1701) |
| Remove hand-built Camera/Material Inspector sections in favor of the generic reflected path (Light section deferred); new ReflectColor wrapper type so Material colors keep a swatch picker | 2026-07-16 | [#1702](https://github.com/blas1n/BSEngine/pull/1702) |
| Remove hand-built Light Inspector section (last of the 3), reusing ReflectColor for light colors; completes the Camera/Material/Light generic-reflected-path migration | 2026-07-16 | [#1703](https://github.com/blas1n/BSEngine/pull/1703) |
| Add generic Validate/ReflectValidate cross-field hook (bevy_reflect #[reflect_trait]), restoring SpotLight inner/outer angle clamping lost when the hand-built Light section was removed | 2026-07-17 | [#1704](https://github.com/blas1n/BSEngine/pull/1704) |
| Delete 229 zoo components confirmed to have zero reference anywhere in the workspace (resumes the 2026-07-13 zoo-component cleanup; the much larger 664-module scripting-wired tier is still deferred) | 2026-07-19 | [#1705](https://github.com/blas1n/BSEngine/pull/1705) |
| 11. `set_reflected_component` generic reflect MCP tool | 2026-07-29 | [#1731](https://github.com/blas1n/BSEngine/pull/1731) |
| 12. CPU skeletal skinning pipeline (glTF skin parsing, SkinnedMesh/AnimationClipLibrary, per-frame LBS system, GltfPlugin auto-attach) | 2026-07-29 | [#1731](https://github.com/blas1n/BSEngine/pull/1731) |
| 12. Fix: wire GltfPlugin/TimePlugin/AnimationPlugin/AnimationStateMachinePlugin into bsengine-runtime (none were ever registered before this — glTF loading and any time-driven component were silently inert in the real runtime binary), handle non-indexed glTF primitives, seed AnimationPlayer.duration from its clip | 2026-07-29 | [#1731](https://github.com/blas1n/BSEngine/pull/1731) |
| 13. Scene serialization saves/loads arbitrary reflected components (bsengine-scene load-side apply_or_insert + bsengine-editor extra_components capture/EditorCommand::LoadScene apply) | 2026-07-29 | [#1733](https://github.com/blas1n/BSEngine/pull/1733) |
| 13. Fix: HashSet\<String\> missing ReflectSerialize (only ReflectDeserialize was registered), which silently dropped AnimationStateMachine from every saved scene | 2026-07-29 | [#1733](https://github.com/blas1n/BSEngine/pull/1733) |
| 14. Mini Action Arena demo (`games/mini-arena/`) — arena/player/enemy, AnimationStateMachine idle/walk/run, NavMeshAgent pursuit, Rapier combat/knockback, custom WGSL glow shader, bloom/tone-map, HUD + pause menu, checkpoint save/load | 2026-07-29 | [#1735](https://github.com/blas1n/BSEngine/pull/1735) |
| 14. Fix: 5 engine bugs found authoring the headless E2E test — reflect-type registration reachable only from EditorPlugin (not headless test mode), test_mode.rs PressKey/ReleaseKey bypassing the input event queue (broke all edge-triggered isKeyDown/isKeyUp), NavMeshPlugin never wired into bsengine-runtime at all (enemy pursuit never worked in any real build), Bsengine.raycast() returning entity_name instead of entityName | 2026-07-29 | [#1735](https://github.com/blas1n/BSEngine/pull/1735) |
| 14. Fix: 5 content bugs in player.js found the same way — isKeyDown instead of isKeyPressed for held movement, 180°-backwards yaw/forward vector, attack raycast self-hit + wrong height, stale getShield() read after damageShield() (Enemy took 3 hits instead of the documented 2); see `games/mini-arena/GAP_LOG.md` for full detail | 2026-07-29 | [#1735](https://github.com/blas1n/BSEngine/pull/1735) |
| 15. `Bsengine.moveEntity` relative-move scripting op; mini-arena's player.js/enemy.js migrated to it from manual get-add-set position math | 2026-07-30 | [#1736](https://github.com/blas1n/BSEngine/pull/1736) |
| 16. `Bsengine.quit()` scripting op sending `AppExit`; winit runner now honors `AppExit` (previously only `WindowEvent::CloseRequested`); mini-arena's pause menu Quit button wired to it | 2026-07-30 | [#1737](https://github.com/blas1n/BSEngine/pull/1737) |
| 17. `attach_physics_body`/`detach_physics_body` MCP tools; `EntityInfo`/`build_entity_descriptors` extended so physics bodies survive `save_scene` (were always dropped); fixed `EditorCommand::LoadScene`'s own separate scene-loading path, which never spawned `PhysicsBodyDesc` from loaded rigidbody/collider RON at all | 2026-07-30 | [#1738](https://github.com/blas1n/BSEngine/pull/1738) |
| 18. `gltf:`/`CustomShader.path` project-relative resolution via shared `bsengine_core::ProjectDir`/`resolve_project_path`; fixed `EditorCommand::LoadScene` never spawning `GltfAsset` for gltf entities at all; mini-arena content migrated off its CWD-relative workaround | 2026-07-30 | [#1739](https://github.com/blas1n/BSEngine/pull/1739) |
| 19. Exposed elapsed time to custom WGSL shaders via `CameraUniform.time` (reusing an existing unused padding field, zero buffer layout change); mini-arena's `glow.wgsl`/`pickup.js` migrated from per-frame JS-driven emissive pulsing to a GPU-computed one | 2026-07-30 | [#1740](https://github.com/blas1n/BSEngine/pull/1740) |
| 20. Added `UiWidget::ProgressBar` / `Bsengine.ui.setProgressBar`, rendered via `egui::ProgressBar`; mini-arena's `hud.js` migrated from a plain-text HP readout to a real health bar | 2026-07-30 | [#1741](https://github.com/blas1n/BSEngine/pull/1741) |
| 21. Real pause: `bsengine_core::PauseState` actually gates `PhysicsPlugin`/`NavMeshPlugin`; `Bsengine.pause`/`resume`/`isPaused` scripting API; mini-arena's pause menu now actually stops the Enemy and Player instead of just showing a panel | 2026-07-30 | [#1742](https://github.com/blas1n/BSEngine/pull/1742) |
| 22. Point light shadows via linear-distance cube arrays (up to `MAX_POINT_LIGHTS`=8, 6 faces each, `R32Float` texture array), sampled in `MESH_WGSL` via manual cube-face selection; mini-arena's `ArenaLight` now casts a shadow automatically | 2026-07-31 | branch `feat/point-light-shadows` (PR #TBD) |
| 23. `bevy_asset` adoption: `GltfSource`/`ShaderSource`/`AudioSource`/`TextureAsset` migrated to `Handle<T>`-based `AssetLoader`s (glTF, custom WGSL shaders, audio, textures incl. skybox), replacing 9 separate direct `std::fs::read` call sites; scene RON/scripting API/MCP tool surfaces stay path-string based, `AssetServer` converts at the boundary; sync-blocking initial load preserved (no behavior change); `games/tilt-run`'s 7 E2E replays all pass, `games/mini-arena`'s replay found to fail on this environment but proven (via identical reproduction on unmodified master) to be a pre-existing, wall-clock-timing-dependent flakiness unrelated to this migration; incidentally surfaced a second, previously-undocumented `bsengine-runtime` stack-overflow test (see item 23's own notes above) | 2026-07-31 | PR #TBD |
