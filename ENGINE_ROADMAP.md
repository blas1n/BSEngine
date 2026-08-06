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

**하위호환은 검증했지 주장하지 않았다.** 기존 `NavMesh` 테스트 10개가 **하나도 수정 없이** 통과한다 —
내부는 그리드 A*에서 폴리곤 A*+퍼널로 완전히 바뀌었는데 `NavMesh::new`/`set_walkable`/`find_path`와
그 위의 `Bsengine.navmesh.init`/`setWalkable`은 그대로다. E2E 8개도 전부 통과하며, 그중 mini-arena가
이 저장소에서 navmesh를 쓰는 유일한 게임이다.

**끝점 판정은 여전히 그리드가 한다.** 폴리곤 로케이터는 메시 밖 점을 가장 가까운 조각으로 스냅하는데,
임펄스에 밀려난 에이전트에게는 옳지만 "이 칸이 막혔나"의 답으로는 틀리다. 그래서 벽 안에서 `find_path`가
`None`을 돌려주는 성질이 유지된다.

**변이 검증이 두 번 값을 했다.** 직선 경로 테스트는 처음에 퍼널을 전혀 검증하지 않았다 — 빈 방은
폴리곤이 하나라 조기 반환되기 때문이고, 퍼널을 제거해도 통과하는 걸 보고서야 알았다. 경로 밖에
장애물을 놓아 실제로 포털을 건너게 만들자 곧바로 **진짜 버그**가 나왔다: 마지막 코너가 최종 포털일 때
퍼널이 목표를 두 번 넣고 있었다. 바닥 슬랩 사이 구멍을 막는 코드도 같은 방식으로 확인했다 — 빼면
에이전트가 허공을 가로질러 경로를 낸다.

**경사로는 범위 밖이다.** 다층 navmesh가 필요하고, 그걸 쓸 레벨이 아직 없다. 필요해지면 별도 항목이다.

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
- [x] `LoadedGltf`/`ShaderSource`/`AudioSourceAsset`/`TextureAsset` 4종에 대한 `AssetLoader`
  구현 — 기존 파싱 로직(`GltfLoader::load_full`, WGSL 컴파일, `StaticSoundData::from_file`)
  을 그대로 이전 (`AudioSource`는 실제 구현 중 `bsengine-audio`의 기존 ECS 컴포넌트와
  이름이 충돌해 `AudioSourceAsset`으로 리네임됨)
- [x] `GltfAsset`/`CustomShader` 등 컴포넌트가 `path: String` 대신 `Handle<T>`를 내부적으로
  보유하도록 전환. 씬 RON 필드/스크립팅 API/MCP 툴 파라미터는 경로 문자열 그대로 유지
  (`AssetServer`가 경계에서 `load::<T>(path) -> Handle<T>` 변환)
- [x] 최초 로드는 동기 블로킹 유지(진짜 비동기 스트리밍은 범위 밖, YAGNI) — 에셋이 몇
  프레임 늦게 나타나는 동작 변화 없음
- [x] `games/mini-arena`/`games/tilt-run` 기존 E2E 리플레이가 마이그레이션 후에도 통과 —
  `games/tilt-run`의 7개 리플레이 전부 클린 통과 확인. `games/mini-arena`의
  `basic-playthrough.testlog.json`은 item 23 검증 당시 실패했고, 이번 item 23의 변경을
  전혀 포함하지 않은 미수정 `master`(커밋 `a539e98e`)에서도 동일하게 재현되어
  bevy_asset 마이그레이션과 무관한 기존 결함으로 확인됨. 근본 원인은 `player.js`의
  연속 이동이 고정 가상 타임스텝이 아니라 실제 벽시계 델타(`Bsengine.getDeltaTime()`
  → `Instant::now()`)로 구동된다는 점: `games/mini-arena/GAP_LOG.md`가 이미 이 정확한
  위험("a standing source of potential flakiness for any future JS-movement-driven...
  headless recording")을 경고해 두었음. 물리 기반(Rapier 고정 타임스텝) 이동인
  tilt-run은 이 문제가 없음.

  **정정(2026-08-02, item 24 Phase 1에서 측정):** 위 서술이 "이 환경에서는 항상 실패"로
  읽히도록 쓰였으나(당시 `master` 10회·HEAD 14회 모두 실패 관측), 실제로는 **머신 부하에
  의존**한다. 유휴 상태에서는 같은 파일이 5회 연속 통과하고, 리플레이 4개를 동시에 돌려
  부하를 주면 4회 모두 실패한다. 부하가 높을수록 프레임당 벽시계 델타가 커져 고정 900
  프레임 동안 플레이어가 더 멀리 이동하고, Pickup 수집 반경(경계 `x > 2.6`)을 벗어나기
  때문이다. item 23 검증이 다수의 서브에이전트가 병렬 빌드/테스트를 돌리던 고부하
  상황에서 이뤄져 100% 실패로 관측된 것. 이 부하 의존성 자체가 결함이며, item 24
  Phase 1에서 고정 프레임을 `wait_until` 웨이포인트 대기로 교체해 해소했다(같은 동시
  4회 부하에서 4/4 통과).
- [x] 테스트 추가, CI 통과

**참고: 로컬 테스트 스택 오버플로 — 이 항목의 작업과 무관한 별개 이슈이며, 이후
근본 원인 규명 후 별도 브랜치(`fix/test-thread-stack-size`)에서 수정 완료.**

item 23 작업 중 로컬 `cargo test --workspace`가 스택 오버플로로 죽는 현상을 겪었다
(`bsengine-editor`의 테스트 바이너리, 그리고 `bsengine-runtime`의
`test_mode::tests::editor_plugin_reload_scene_does_not_corrupt_scripting`). 이 문서에는
처음에 서로 다른 두 개의 원인 불명 기존 버그로 기록했으나, 이후 조사에서 **둘 다 동일한
하나의 원인**이고 애초에 **로컬 전용 설정 누락**이었음이 확인됐다:

`.cargo/config.toml`의 `/STACK:67108864` 링커 플래그는 **메인 스레드만** 키운다. 반면
libtest는 모든 `#[test]`를 spawn된 스레드에서 실행하고, Rust는 그 스레드에 PE 헤더 값이
아니라 자체 기본값 **2 MiB**를 넘긴다 — 그래서 테스트는 그 64MB를 한 번도 본 적이 없다.
2 MiB는 V8(deno_core, 스크립팅 테스트면 무조건 경유)이나 `bsengine-editor`의 리플렉션
테스트에는 부족하다. (`--test-threads=1`로도 해결되지 않는 것으로 spawn 스레드 문제임을
확인.)

**초기 기록의 오류 정정:** 이 문서에는 "이 테스트가 실제로 통과하는지 어떤 워크스페이스
전체 실행에서도 확인된 적이 없었다"고 적었으나 이는 **사실이 아니다**. CI는
`.github/workflows/ci.yml`에서 두 테스트 스텝 모두에 `RUST_MIN_STACK: 8388608`을
설정하고 있고, `bsengine-runtime`은 CI에서 제외 대상이 아니므로 해당 테스트는 **CI에서
매번 통과해 왔다**. 실패하는 것은 이 설정이 없는 로컬 실행뿐이었다.

**수정:** 동일한 8 MiB 값을 `.cargo/config.toml`의 `[env]`로 옮겨 로컬과 CI를 일치시켰다
(환경변수 없이 `cargo test --workspace` exit 0 확인). 2026-07-13 플랜 문서들이 모든 테스트
명령에 `RUST_MIN_STACK=8388608`을 수동으로 붙이던 관행도 이걸로 대체된다. CI의
`--exclude bsengine-editor` 분리 스텝은 CI 특유의 이유(V8이 큰 VA 영역을 예약해 editor
테스트 스레드 스택 확장을 막음)가 있어 그대로 둔다.

---

### 24. 파일 워처 핫리로드 + 로드 상태 조회 API

**목표:** item 23이 확립한 `Handle<T>` 기반 파이프라인 위에, 실행 중인 게임/에디터에 파일
변경을 자동 반영하는 핫리로드와 로드 실패/리로드 상태 조회 기능을 얹는다 (원래 item 23
초안의 완료 조건 중 핫리로드/에러 전파/조회 API 부분을 이관)

설계 문서: `docs/superpowers/specs/2026-08-02-asset-hot-reload-design.md`. 그 문서가 이
항목을 5단계로 나눈다 — (1) E2E `wait_until`, (2) 호출부 Async 전환, (3) 워처 + 소비자
재구축, (4) 에러 전파/상태 조회, (5) 안정적 식별자. 아래 완료 조건과 대응한다.

**완료 조건:**
- [x] **(Phase 1)** E2E 프로토콜에 `wait_until` 추가 — 고정 프레임 수 대신 술어 충족까지
  진행. Async 전환(Phase 2)이 모든 기존 녹화를 불안정하게 만들기 때문에 선행 조건이며,
  이미 부하에 따라 실패하던 `games/mini-arena` 녹화도 이걸로 복구했다(동시 4회 부하에서
  옛 녹화 4/4 실패 → 새 녹화 4/4 통과). MCP `test_wait_until`로도 노출해 신규 녹화가
  고정 프레임으로 되돌아가지 않게 함
- [x] **(Phase 2)** glTF/셰이더/텍스처/오디오 호출부를 `LoadMode::Async`로 전환 — 4개
  소비자 모두 "요청은 한 번, 핸들을 보관, 보관한 핸들을 폴링" 형태로 통일. 각각
  `PendingGltf`(엔티티별 컴포넌트) / `PendingShaders`(경로 키 맵) / `PendingSkybox`(단일
  슬롯) / `SoundLoads`(경로 키 맵) + `PendingSounds`(재생별 큐)로, 자료구조만 소비자
  수에 맞게 다르다 — 엔티티당 glTF 하나, 셰이더 하나에 엔티티 여럿, 스카이박스는 정확히
  하나, 사운드는 같은 경로로 동시에 여러 번.

  실패 후 재시도 정책은 소비자마다 다르고, 이건 의도된 것이다: glTF는 마커 컴포넌트를
  지워 두 번 다시 시도하지 않고, 셰이더/오디오는 `GaveUp`을 영구 보관해 같은 경로를 다시
  요청하지 않으며, 스카이박스만 `SkyboxPath`가 다른 값으로 바뀌었다 돌아오면 재요청한다
  (사용자가 경로를 되돌린 건 명시적 재시도 의사로 본다).

  **핵심 함정 — 실패한 에셋을 다시 요청하면 실패가 지워진다.** `AssetServer::load`는
  `HandleLoadingMode::Request`를 쓰는데, `bevy_asset-0.14.2`(`server/info.rs:212-221`)는
  이미 `Failed` 상태인 에셋에 이게 호출되면 **상태를 `Loading`으로 되돌리고 로드를 다시
  띄운다**. 그리고 `LoadState::Failed`는 `PreUpdate`에서 설정되는데 소비자 시스템은
  `Update`/`PostUpdate`에서 돈다. 그래서 "매 프레임 경로로 재요청한 뒤 `load_state`로
  실패를 확인" 하는 자연스러워 보이는 형태는 **실패 분기에 영원히 도달하지 못하고,
  프레임당 파일시스템 태스크를 하나씩 흘린다.** 실측: 없는 셰이더 경로 하나에 대해
  200프레임 동안 재요청 방식은 `Path not found` 200회, 현재 방식은 1회.

  부수적으로, 실패 분기가 헤드리스 테스트에서 도달 가능하려면 GPU 리소스
  검사(`GpuMeshRegistry`/`WgpuSurfaceResource`)를 요청·폴링·실패감지 **아래로** 내려야
  했다. 진짜 서피스는 진짜 winit 윈도우를 요구하므로, 위에 두면 이 워크스페이스가 쓸 수
  있는 어떤 테스트로도 give-up 경로를 검증할 수 없다.

  **동반 회귀 수정 — 비동기가 만든 "억제 명령이 반전되는" 버그.** `playSound`가
  비동기가 되면서 id가 `SoundHandles`에 몇 프레임 늦게 도착한다. 그 사이 도착한
  `stopSound`/`pauseSound`는 아무것도 못 찾고 no-op이 되고, 이후 로드가 끝나면 **정지·일시정지를
  요청한 사운드가 재생되는** 반전이 생긴다. 둘 다 대기 큐에 닿도록 고쳤다(`StopSound`는
  항목을 제거, `PauseSound`는 항목에 `paused` 플래그를 세워 재생 직후 `pause()` 호출).
  나머지 4개(`setSoundVolume`/`setSoundPanning`/`setSoundPlaybackRate`/`seekSound`)는
  대기 중인 항목에 닿지 않는 채로 두었다 — 이들은 사운드를 **억제**하는 게 아니라
  **조율**하므로, 놓쳐도 "원치 않은 소리가 나는" 게 아니라 "소리가 조금 다르게 나는" 것에
  그치고, 재생 중이 아닌 id에 조용히 무시되는 건 원래부터의 동작이기 때문이다.

  같은 이유로 `getSoundState`가 대기 중인 사운드에 `""`를 돌려주던 것도 `"loading"`으로
  바꿨다 — `""`는 "한 번도 재생된 적 없는 id"와 구분되지 않아서, 상태를 폴링하는 스크립트가
  사운드가 시작되기도 전에 "끝났다" 분기를 타게 만들었다.

  **후속으로 남긴 것(둘 다 현재 도달 불가이거나 경로 수로 제한됨):** glTF만 로드 중
  `GltfAsset.path`가 바뀌는 걸 감지하지 않는다(스카이박스는 감지하고 테스트도 있다) —
  현재 이 필드를 제자리에서 바꾸는 코드가 없어 도달 불가. 그리고 로드 중인 엔티티에
  `MeshRenderer`가 붙거나 마지막 참조 엔티티가 despawn되면 pending 항목이 고아로 남아
  핸들을 계속 붙잡는다 — 서로 다른 경로 수만큼으로 제한되므로 무한 증가는 아니다.
- [x] **(Phase 3a)** 에셋 데이터가 교체되면 각 소비자가 GPU 상태를 **제자리에서** 재구축 —
  재시작 없이, 엔티티를 하나도 건드리지 않고. 이 단계는 `AssetServer::reload`를 직접 호출해
  구동하며, 그걸 호출해 줄 파일 워처는 Phase 3b다.

  **측정된 전제 — `Modified`는 강한 핸들이 살아있는 동안에만 발생한다.** 마지막 핸들이
  풀리면 `Assets::track_assets`(PreUpdate)가 에셋을 해제하고, 그 경로에 대한
  `AssetServer::reload`는 **조용한 no-op**이 된다. 설계 문서는 Phase 2 이전에 쓰여 이걸
  알 수 없었지만, Phase 2 이후 핸들을 계속 쥐고 있던 소비자는 오디오뿐이었다 — glTF·셰이더·
  스카이박스는 로드가 끝나면 핸들을 버렸으므로, 워처를 아무리 잘 만들어도 이벤트 자체가
  오지 않았을 것이다. 그래서 "로드 후 핸들 보관"은 최적화가 아니라 **선행 조건**이다.
  추론이 아니라 실행으로 확인했고(`reload_emits_modified_only_while_a_handle_is_retained`,
  `bsengine-gltf`), 핸들을 쥔 채로는 `LoadedWithDependencies` + `Modified`가 나오고
  버린 뒤에는 이벤트가 **0개**였다.

  **설계 문서보다 나은 방법을 택했다.** 문서는 "핸들을 가진 모든 엔티티에서
  `MeshRenderer.mesh_id`를 교체"하라고 했지만, `GpuMeshRegistry::register`는 호출마다 새
  id를 할당하고 해제 API가 없어서 리로드마다 버퍼 두 개를 영구 누수시킨다(`update_vertices`는
  정점 수가 같아야 하고 인덱스·바운드를 갱신하지 않아 메시가 실제로 바뀐 경우엔 못 쓴다).
  대신 registry에 **같은 id 아래 내용만 교체하는** `replace`를 추가했다. `MeshRenderer.mesh_id`와
  `Material.texture_id`가 그대로 유효하므로 엔티티를 찾을 필요가 없고, 멀티메시 glTF가
  추가로 스폰한 엔티티까지 공짜로 갱신된다.

  소비자별로: glTF는 `GltfLoaded`(핸들 + 만들어 낸 mesh/texture id)를 엔티티에 남기고
  `rebuild_modified_gltf`가 그 id 아래를 교체한다. 셰이더는 `PendingShader::Ready(handle)`을
  보관하고 `compile_and_store_shader`가 경로 키로 덮어쓰므로 별도 무효화가 필요 없다.
  스카이박스는 `PendingSkyboxState::Ready(handle)`을 보관하되, 경로 비교 단축 분기가 그
  슬롯을 지우지 않도록 고쳤다. **오디오는 프로덕션 변경이 전혀 없다** — 이미 핸들을 쥐고
  있고 `bevy_asset`이 그 아래 데이터를 갈아끼우므로, 다음 `playSound`가 새 데이터를 쓴다
  (가정으로 두지 않고 테스트로 고정했다).

  **한계(의도적):** 리로드된 glTF의 메시/이미지 **개수**가 달라지면 겹치는 부분만 재구축하고
  경고한다 — 나머지 엔티티는 로드 시점에 스폰된 것이라 이 방식으로 표현할 수 없다. 재시작이
  필요하다. 스킨이 새로 생기거나 사라지는 경우도 같은 부류라 경고 후 건너뛴다.
  `AnimationPlayer.clip`은 갱신하지 않는다 — 어떤 클립을 재생할지는 `AnimationStateMachine`이
  소유할 수 있는 결정이기 때문이며, duration만 현재 재생 중인 클립에서 다시 읽는다.
  컴파일에 실패한 셰이더는 `CompileFailed`로 남아 프레임마다 재시도하지 않지만 핸들은 계속
  쥐고 있다 — 고친 파일이 도착할 `Modified`가 바로 그 핸들에 실린다.

  **최종 리뷰가 잡은 Critical — 스킨드 메시가 리로드를 매 프레임 덮어썼다.**
  `update_skinned_meshes`(PostUpdate)는 로드 시점에 복제해 둔 `SkinnedMesh.rest_vertices`로
  변형 정점을 만들어 같은 `mesh_id` 버퍼에 쓴다. 재구축은 Update에서 도니, **같은 프레임 안에서**
  새 지오메트리가 옛 정점 기반 데이터로 덮어써졌다 — 화면상 아무 일도 일어나지 않는 조용한
  no-op. 게다가 정점 수가 줄면 `update_vertices`에 크기 가드가 없어 wgpu `BufferOverrun` →
  **프로세스 사망**이었다(실측: `Copy of 0..76032 would end up overrunning the bounds of the
  Destination buffer of size 132`). 이건 가설이 아니라 `games/mini-arena`가 두 번 로드하는
  `fox.glb`가 정확히 스킨드 에셋(skins:1, 애니메이션 3종)이라 실제로 걸리는 경로였다.
  재구축이 `rest_vertices`/`skin`/`skin_data`/`nodes`/클립 라이브러리를 함께 갱신하도록 고쳤고,
  `update_vertices`에는 방어선으로 길이 가드를 넣었다.

  **왜 못 잡았나:** 헤드리스 테스트 헬퍼가 registry 두 개만 넣고 `GpuQueueResource`를 넣지
  않아 `update_skinned_meshes`가 조기 반환했고, 단언이 `get_bounds`(재구축은 갱신하지만
  덮어쓰기는 건드리지 않는 값) 대상이라 어느 쪽이든 초록이었다. 이 브랜치가 내내 경계해 온
  "깨진 구현에서도 통과하는 테스트"가 가장 중요한 테스트에서 일어난 셈이다.

  **런타임 셰이더 컴파일이 프로세스를 죽이던 것도 함께 고쳤다.** `create_shader_module`은 에러
  스코프 없이 호출돼 WGSL 오류가 wgpu 기본 핸들러의 패닉으로 이어졌다. 시작 시점이면 "깨진
  셰이더를 배포했다"로 끝이지만, 핫리로드는 **중간에 깨진 상태를 거치며 반복하는 것 자체가
  목적**이라 용납할 수 없다. 이제 naga의 두 단계(파서 + `Validator`)로 미리 검증하고, 실패하면
  경고 후 **이전 파이프라인을 그대로 둔다** — 편집 중인 오브젝트가 새까매지지 않아야 한다는
  설계 문서의 요구사항 그대로다.

  **테스트 규율:** 각 소비자의 보관 테스트는 열거형 상태만 보면 안 된다는 걸 실측했다 —
  `clone_weak` 핸들은 `matches!(Ready(_))`를 만족시키면서도 `track_assets`가 에셋을 해제하게
  둔다. 그래서 모든 테스트가 에셋 생존과 `Modified` 발생까지 단언하며, 각각 `clone_weak`
  변이로 실패를 확인했다.
- [x] **(Phase 3b)** `<ProjectDir>/assets`로 스코프를 좁힌 파일 워처(`AssetWatcherPlugin`,
  `notify-debouncer-full`)가 변경을 감지해 `AssetServer::reload`를 호출 — 창을 띄우는 두 호스트
  (런타임과 에디터 앱) 모두에 자동 반영. 실측 확인: 게임을 띄운 채 `glow.wgsl`을 편집하면
  `asset hot reload: games/mini-arena/assets/shaders/glow.wgsl changed on disk, reloading`이
  찍힌다. `bevy_asset`의 `file_watcher` 피처는 쓰지 않는다 — 에셋 루트가 리포 전체(`target/`,
  `.git/` 포함)가 되기 때문이다.

  **경로 철자가 이 단계의 전부였다.** `AssetServer::reload`는 경로 **문자열**로 매칭하고,
  어긋나면 경고도 이벤트도 없이 조용한 no-op이 된다. 측정 결과 Windows에서는 `/`와 `\`가 둘 다
  구분자라 어느 철자든 매칭되지만, canonicalize된 철자(`..` 해소 + Windows `\\?\` 접두)는 양쪽
  플랫폼 모두에서 매칭되지 않는다. (구분자 관련 단언은 `#[cfg(windows)]`로 한정한다 — Unix에서
  `\`는 유효한 파일명 문자라 구분자를 바꾸면 같은 파일의 다른 철자가 아니라 존재하지 않는 다른
  파일이 된다. 이걸 보편적 사실인 양 단언했다가 CI의 ubuntu 러너에 잡혔다.) 그리고 `notify`는
  상대 경로를 `watch()`에 줘도 **절대 경로**를 돌려주되 정규화는 전혀 하지 않는다 — 보고되는
  경로는 정확히 `current_dir().join(watch_root)` + 나머지다. 그래서 재구성은
  `strip_prefix(current_dir()?.join(watch_root))` 한 번이면 되고, 이를 엔진 형태 루트에 다시
  붙인다. 두 사실 모두 추론이 아니라 테스트로 고정돼 있다.

  디바운스는 200ms. 에디터는 파일을 여러 단계로 쓰므로(truncate 후 write) 한 번의 저장이
  여러 이벤트를 내고, 반쯤 쓰인 파일을 읽을 위험이 있다. 실측: 한 번의 `fs::write`는 1개,
  연속 5회 쓰기도 1개로 합쳐진다.

  **`--test` 모드에서는 켜지 않는다.** 리플레이 중에는 파일을 편집하는 사람이 없어 순수 비용인
  데다, 어렵게 확보한 결정성에 배경 스레드를 하나 더 얹을 이유가 없다.

  **커버하지 않는 것:** `assets/` 아래에 있어도 `scenes/`(RON)와 `scripts/`(JS)는 핫리로드되지
  않는다. 전자는 `std::fs::read_to_string`, 후자는 스크립팅 플러그인이 직접 읽어서 `bevy_asset`을
  거치지 않기 때문이다. 워처는 이들 확장자를 아예 거른다 — `reload`를 불러 봐야 조용한 no-op이라
  로그만 오해를 부른다.

  **조용한 실패 경로를 남기지 않는 것이 이 기능의 절반이다.** 최종 리뷰가 네 군데를 더 찾아냈고
  전부 막았다: 경로 재구성의 `strip_prefix` 실패(오늘 이 플랫폼들에서는 불가능하지만, 실패하면
  증상이 정확히 "아무 로그 없이 아무 일도 안 일어남"), 워처 스레드가 죽었을 때 채널
  `Disconnected`를 유휴와 구분 못 하던 것(경고 후 `AssetWatcher` 리소스를 제거해 스팸 없이
  멈춘다), 뮤텍스 오염, 그리고 **한 번도 로드된 적 없는 파일에 대해 "reloading"이라고 찍던 것**.
  마지막 건 `AssetServer::get_path_ids`로 정확히 판별하도록 바꿨다 — 확장자 필터는 "이 *종류*를
  서빙할 수 있다"까지만 증명하지 "이 *경로*가 로드됐다"는 증명하지 못한다. 덤으로 손으로 관리하던
  확장자 목록의 부담도 줄었다: 누락은 여전히 리로드를 잃지만, 과다 포함은 이제 무해하다.

  **함께 고친 선행 버그 — 에셋이 애초에 로드되지 않고 있었다.** `AssetPlugin`이 `file_path: ""`를
  주면서 "경로를 파일시스템 상대로 다룬다"고 주석에 적어 뒀지만 **사실이 아니었다.** `file_path`는
  루트가 아니라 bevy가 스스로 고른 루트 **아래에 join되는 경로**이고
  (`AssetPlugin::build` → `init_default_source` → `FileAssetReader::new`의
  `get_base_path().join(path)`), 그 루트는 `BEVY_ASSET_ROOT` → `CARGO_MANIFEST_DIR` →
  실행 파일 디렉터리 순으로 결정된다. `cargo run` 아래에서는 실행되는 바이너리의 **패키지**
  디렉터리, 즉 `crates/bsengine-runtime`이다. 그래서 `games/mini-arena/assets/models/fox.glb`가
  `<repo>/crates/bsengine-runtime/games/...`에서 찾아지고, 로드 실패는 `WARN`일 뿐이라
  **mini-arena는 fox 메시도 glow 셰이더도 없이 돌고 있었다.** E2E 리플레이는 스크립트·물리
  기반 결과를 단언하므로 이걸 잡지 못했다. 절대 CWD를 `file_path`로 넘겨 고쳤다(절대 경로는
  `join`에서 베이스를 버린다). 부작용으로 `BEVY_ASSET_ROOT`는 더 이상 효과가 없으며, 이는
  의도적이고 문서화돼 있다 — 이 엔진은 경로를 스스로 해석한다.
- [x] **(Phase 4)** 로드 실패/누락 에셋에 대한 에러 전파를 `tracing::warn!` 후 스킵에서
  **질의 가능한 상태**로 확장. `AssetStatuses`(경로 → `Unknown`/`Loading`/`Loaded`/`Failed(사유)`)를
  스크립팅 `Bsengine.getAssetStatus(path)`와 MCP `test_get_asset_status` 두 표면으로 노출한다.

  **왜 필요했는지는 이 item 자체가 증명한다.** 같은 작업에서 "`WARN`으로만 남는 실패"에 세 번
  당했다: item 23의 에셋 루트 오류로 `games/mini-arena`가 **메시도 셰이더도 없이** 두 단계 내내
  돌았고(우연히 발견), 없는 glTF가 조용히 무한 재시도했으며(Phase 2에서 give-up 경고 추가),
  Phase 3b의 워처는 경로 철자가 어긋났다면 아무 출력 없이 아무것도 리로드하지 않았을 것이다.
  이제 "아무 일도 안 일어남"과 "실패했고 이유는 이것"이 프로그램적으로 구분된다.

  **누적이 필요한 건 실패뿐이었다.** `UntypedAssetLoadFailedEvent`는 타입과 무관하게 모든 실패에
  대해 경로와 에러를 실어 배치 전송되므로(`server/mod.rs:1211-1231`), 시스템 하나가 glTF·셰이더·
  텍스처·오디오를 전부 커버한다 — `bsengine-asset`이 그 타입들의 존재를 알 필요가 없다. 나머지
  상태는 `AssetServer`에 즉석에서 물으면 된다.

  **다만 성공은 공짜가 아니다.** `bevy_asset`에는 "알고 있는 경로 나열" API가 없어(`AssetInfos`의
  맵이 비공개), 무언가 실패하기 전까지는 그 경로의 존재조차 알 수 없다. 그래서 **요청 시점에 경로를
  기록**한다. `bsengine_asset::load`가 소비자 셋을 담당하고, 직접 `asset_server.load`를 부르던
  스카이박스는 기록하는 async 전용 헬퍼로 태웠다(동기 텍스처 로더가 없으니 스텁을 만들지 않았다).

  `NotLoaded`의 취급이 미묘했다. `bevy_asset-0.14.2`의 `load_state` 대입을 전부 확인한 결과
  (`server/info.rs:47`/`140`/`217`/`478`/`594`), `Failed`가 `NotLoaded`로 덮이는 경로는 없다 —
  기록된 경로가 `NotLoaded`로 읽히면 마지막 핸들이 풀려 `AssetInfo`가 축출된 것이지 새로운
  미지 상태가 아니다. 그래서 규칙은 단순하다: **`NotLoaded`는 아무 정보도 없으므로 아무것도
  덮어쓰지 않고, `Loading`/`Loaded`/`Failed`는 각각 덮어쓴다.** 한계: 실패한 경로의 핸들이 풀린 뒤
  다시 요청되지 않으면 계속 `Failed`로 보고된다.

  MCP 툴이 `test_` 접두를 갖는 건 **MCP 서버가 엔진과 별개 프로세스**이기 때문이다 — 리소스를
  in-process로 읽을 수 없어 기존 `--test` 자식 질의 프로토콜을 타며, 그래서 `session_id`를 받는
  세션 스코프 툴이다(`test_query_state`/`test_wait_until`과 같은 부류). 두 표면은 같은
  `render_asset_status`를 공유해 같은 어휘(`loaded`/`loading`/`failed: <사유>`/`unknown`)로 답한다.

  **플러그인은 게임을 실행하는 세 호스트 모두에 등록했다** — 런타임(창), 에디터 앱, 그리고 `--test`.
  워처와 달리 스레드를 띄우지도 게임 상태를 바꾸지도 않는다. 이 등록을 빠뜨리면 기능 전체가
  무력해진다는 걸 Phase 3b에서 이미 한 번 겪었다.

  **다만 `--test` 앱에는 렌더러도 glTF 임포터도 없다.** 그래서 헤드리스 리플레이에서 요청되는 건
  사운드뿐이고, 메시·셰이더·텍스처는 어떻게 철자를 써도 `unknown`으로 읽힌다. **이 단계의 동기로
  적어 둔 "메시도 셰이더도 없이 돌던" 사고는 정작 MCP 표면으로는 아직 못 잡는다** — 창을 띄우는
  런타임과 에디터에서는 전체 스택이 있으므로 스크립트의 `getAssetStatus`가 제대로 답한다.

  **최종 리뷰가 잡은 Critical, 그리고 리뷰가 놓쳤다가 수정 중에 드러난 쌍둥이.** 두 표면 모두
  경로 철자를 문서대로 쓰면 동작하지 않았고, 실패 방식이 하필 이 단계가 없애려던 바로 그
  `"unknown"`이었다. MCP 쪽은 `.mcp.json` → `server.rs`(`root.join("games")`) →
  `session.rs`(`games_root.join(game)`을 `--test`에 그대로 전달)를 거쳐 실제 키가
  `f:\...\games\mini-arena/assets/...`가 되는데, 툴 설명은 `games/mini-arena/...`를 예시로
  들고 있었다. 테스트가 못 잡은 이유도 분명하다 — 테스트의 `games_root`는 상대 경로이고 배포되는
  서버의 것은 절대 경로다. **파생 로직은 옳았고, 그걸 검증한 구성이 실제 구성이 아니었다.**
  JS 쪽은 더 나빴다: 스크립트가 접두사를 알 방법이 아예 없어(`getProjectDir` 같은 op이 없다)
  **모든 호출이 `unknown`**이었다. 양쪽 다 "정확한 키를 먼저 찾고, 빗나가면
  `resolve_project_path`로 해소해 다시 찾는" 같은 형태로 고쳤고, 프로젝트 상대 철자 —
  스크립트가 `playSound`에 넘기는 바로 그 문자열 — 가 이제 통한다. 절대 경로 games-root로
  구성한 회귀 테스트도 함께 넣었다.

  **또 하나:** 요청 경로를 나르는 프로세스 전역 채널이 `retain`으로 **파괴적으로** 비워지고 있어서,
  한 프로세스의 두 App이 같은 경로를 요청하면 먼저 도는 쪽이 가져가고 다른 쪽 맵에는 키가 영영
  들어가지 않았다(실패는 이벤트가 구제하므로 **성공 방향만 조용히** 사라진다 — 하필 요청 기록이
  존재하는 이유인 그 방향이다). 이미 가져간 것만 건너뛰도록 바꿨다.
- ~~**(Phase 5)** 에셋에 안정적 식별자 부여~~ → **item 30으로 이관.** 설계 단계에서 이 단계의
  전제가 틀렸음이 드러났다: 저장소 씬 파일의 에셋 참조 32개 중 **30개가 `script:` 경로**이고,
  스크립트는 `std::fs::read_to_string`으로 읽혀 `bevy_asset`을 거치지 않는다(Phase 3b 워처가
  거르는 그 이유다). 즉 `bevy_asset` 에셋만 대상으로 한 GUID 사이드카는 **참조의 6%만** 덮는다.
  셰이더·사운드 경로는 씬이 아니라 JS 안 문자열 리터럴이라 인덱스가 닿지도 않는다. 제대로
  고치려면 스크립트를 에셋으로 승격시켜야 하고, 그건 에셋 파이프라인에 항목을 더하는 게 아니라
  스크립팅 로드 경로를 바꾸는 일이다 — 이 스펙이 미리 적어 둔 "구현 중 커지면 별도 로드맵
  항목이 된다"에 해당한다.
- [x] 테스트 추가, CI 통과

---

### 25. 3D 포지셔널 오디오 ✅

**목표:** `AudioWorld`가 위치 정보 없는 `play`/`stop`뿐인 상태(`bsengine-audio` 251줄)를
벗어나, 엔티티 위치 기반 거리 감쇠·패닝을 제공

**설계 전에 잰 것.**

- kira 0.12는 `add_listener` → `add_spatial_sub_track(listener, position, builder)` →
  **그 트랙에서 재생하면 거리 감쇠와 패닝이 자동**이다. 우리가 감쇠 곡선을 쓸 일이 없다.
- **API가 glam이 아니라 `mint` 타입을 받는다.** kira는 glam 0.33을, 이 워크스페이스는 0.29를
  쓴다(`Cargo.lock`에 glam이 여섯 버전 있다). 그대로 넘겼으면 타입이 맞지 않았을 텐데,
  `add_listener`/`add_spatial_sub_track`이 `mint::Vector3<f32>`를 받고 `mint 0.5.9`가 이미
  잠겨 있어 성분별 변환으로 충돌이 없다. `reflect_glam.rs`가 기록한 bevy_reflect glam 0.27
  문제와 같은 부류이지만 이쪽은 상류가 이미 해결해 뒀다.
- `ListenerHandle::set_position`/`set_orientation`과 `SpatialTrackHandle::set_position`이
  있어 엔티티가 움직여도 따라간다.

**카탈로그가 이름 충돌을 미리 잡았다.** `sound` 질의가 `bsengine_get_sound_position`을 물어왔고,
확인해 보니 그건 3D 위치가 아니라 **재생 시점(초)**이다. 즉 오디오 API에서 "position"은 이미
시간을 뜻하므로 3D용으로 그 단어를 재사용하지 않는다 — 위치는 엔티티가 소유하고, 소리 API는
엔티티 이름만 받는다. 실수가 나기 전에 카탈로그가 막은 첫 사례다.
(`listener`/`emitter`/`spatial`/`attenuation`/`pan`은 전부 비어 있어 새 어휘로 안전하다.)

**완료 조건:**
- [x] `AudioListener` 컴포넌트 — 이 엔티티의 `Transform`이 kira 리스너를 구동한다(카메라에 붙인다)
- [x] `AudioEmitter` 컴포넌트 — 엔티티마다 kira spatial 트랙을 소유하고 매 프레임 위치를 동기화
- [x] `Bsengine.playSound3D(entityName, path, opts)` — 기존 `playSound`는 비위치 재생으로 유지
- [x] `games/mini-arena`에서 위치 기반 소리 사용
- [x] 테스트 추가, CI 통과

**구현이 드러낸 것: WAV과 OGG는 지원된 적이 없었다.** 이 워크스페이스는 kira의 `wav`/`ogg`
기능만 켜고 `pcm`/`vorbis`를 안 켰다. 그건 symphonia의 **컨테이너** 리더일 뿐이고 안에 든 것을
푸는 **코덱**이 아니라서, 평범한 PCM WAV가 파싱된 뒤 "unsupported codec"으로 실패했다. 워처는
`wav`/`ogg`를 로드 가능 확장자로 광고하고 로더도 받아들이는데 실제로는 디코딩되지 않았다.

**그리고 이 진단은 이미 적혀 있었다.** `audio_source.rs`의 테스트 헬퍼 독 주석이 정확히 이 문제를
설명하고 있었다 — "실제 .wav(PCM)나 .ogg(Vorbis) 파일을 넣어도 오늘은 디코딩에 실패한다"까지.
그런데 수정으로 이어지지 않았다. **결함을 적어 두는 것은 고치는 것이 아니고**, 테스트 헬퍼의 독
주석은 아무도 찾아보지 않는 곳이다. 이번에 실제로 `.wav`을 재생하려다 부딪혔고, 코덱 두 개를 켜서
고쳤으며 `a_pcm_wav_decodes_via_kira`가 지킨다.

**부수적으로: 어떤 게임도 소리를 내고 있지 않았다.** 사운드 에셋이 저장소에 하나도 없었고 어떤
스크립트도 `playSound`를 부르지 않았다. item 33이 "죽은 ECS 오디오 경로"를 지웠는데, 살아 있는
경로조차 아무도 쓰지 않고 있었다. mini-arena의 Enemy에 합성 험(220Hz 사인, 루프 지점이 튀지
않도록 양끝 페이드)을 생성해 붙였고, 이것이 이 엔진에서 실제로 재생되는 첫 소리다.

**테스트가 `AudioManager`를 만들지 않는다.** 오디오 장치 없는 Windows에서 kira가 WASAPI/COM을
백그라운드 스레드에서 초기화하다 프로세스를 죽인다(`STATUS_ACCESS_VIOLATION`) — 이 크레이트의
기존 테스트 두 개가 그래서 `#[ignore]`돼 있다. 처음엔 새 테스트도 같은 크래시를 밟았다. `AudioPlugin`이
이미 있는 `AudioWorld`를 덮어쓰지 않게 하고 테스트가 `AudioWorld::silent()`를 넣도록 바꿔서,
새 테스트는 `#[ignore]` 없이 모든 플랫폼에서 실제로 돈다.

**청감 검증은 이 작업 안에서 못 한다.** 원래 완료 조건에 있던 "실제 청감 검증"은 소리를 들을 수
있어야 하는 항목이라 자동화된 작업이 만족시킬 수 없다. 대신 **kira에 넘어가는 값**을 검증한다 —
리스너와 이미터의 위치가 각자의 `Transform`을 따라가는지, 이미터가 리스너에서 멀어질 때 그 사실이
kira에 반영되는지. **이것이 "제대로 들린다"를 증명하지는 않으며**, 청감 확인은 사람이 해야 한다.

---

### 26. 폴리곤 기반 실제 NavMesh ✅

**목표:** 현재 "NavMesh"라 불리지만 실제로는 균일 XZ 그리드 위 8방향 A*인 구현
(로드맵 item 3)을, 레벨 지오메트리에서 폴리곤 메시를 추출하는 방식으로 교체 —
좁은 통로/경사/계단에서의 한계 해소

**설계 전에 잰 것 — 범위가 처음 보이는 것보다 훨씬 작다.**

| | |
|---|---|
| 그리드 모양 공개 API의 외부 사용 | `world_to_cell`/`cell_center`/`is_walkable` **0곳**(내부 전용) |
| `find_path(from, to) -> Option<Vec<Vec3>>` | 1곳. **이미 형태에 무관한 추상화**라 에이전트 시스템은 손댈 필요가 없다 |
| 그리드 모양 스크립팅 op | `navmesh.init`과 `navmesh.setWalkable` 둘뿐. **`setWalkable`은 어떤 게임도 안 쓴다** |
| navmesh를 쓰는 게임 | `games/mini-arena` 하나 |
| 게임의 콜라이더 | Box·Sphere·Capsule 프리미티브뿐. **임의 삼각형 메시가 없다** |
| 경사로가 있는 레벨 | **하나도 없다** |

**따라서 평면 폴리곤 navmesh로 한정한다.** Recast식 복셀라이저는 "평평한 아레나 + 장애물 서넛"에
과하고, 경사로는 다층 navmesh가 필요한데 그걸 쓸 게임이 지금 없다. 경사로는 명시적으로 범위 밖이며,
필요해지면 별도 항목으로 승격한다.

**표현과 알고리즘.** XZ 평면의 볼록 사각형 집합. 워커블 사각형에서 장애물 발자국을 빼고 스윕라인으로
분해한다. 경로는 폴리곤 인접 그래프 위 A* 뒤 퍼널(string-pulling)로 직선화하므로, 그리드가 만들던
8방향 계단 경로와 해상도 인공물이 사라진다.

**하위호환은 표현을 바꾸는 것으로 얻는다.** `find_path`는 그대로다. `NavMesh::new`/`set_walkable`은
버리지 않고 **폴리곤을 저작하는 한 가지 방법**으로 남긴다 — 셀이 장애물 사각형으로 번역되므로
`Bsengine.navmesh.init`/`setWalkable`이 지금과 똑같이 동작한다. 콜라이더에서 빌드하는 것은 두 번째
저작 경로다.

**완료 조건:**
- [x] 볼록 사각형 분해 + 인접 그래프 (순수 Rust)
- [x] 폴리곤 A* + 퍼널 직선화 — 그리드 경로보다 짧고 계단이 없음을 테스트로 고정
- [x] `NavMesh::new`/`set_walkable`이 폴리곤 저작 경로로 계속 동작 (스크립팅 표면 무변경)
- [x] 씬의 충돌 지오메트리로부터 빌드
- [x] 그리드로는 못 하던 사례로 검증 — 좁은 통로, 비직사각형 레벨. **경사로는 범위 밖**
- [x] `games/mini-arena`의 Enemy 추적이 새 구현으로도 동작 (E2E)
- [x] 테스트 추가, CI 통과

---

### 27. 캐릭터 컨트롤러 + 실제 물리 넉백 ✅

**목표:** `NavMeshAgent`가 Transform을 소유하려면 Kinematic rigidbody가 필요하고,
Kinematic 바디는 정의상 물리 임펄스를 무시해 "넉백"을 스크립트가 위치를 직접 미는
방식으로 흉내내야 했던 한계 해소 (`games/mini-arena/GAP_LOG.md` "Pre-existing, not
touched by this task" 항목에서 발견)

**두 후보를 실제로 실험한 뒤 Dynamic으로 정했다.** 추정으로 고르지 않았다.

**Dynamic으로 바꾸면 무슨 일이 나는가 — 측정함.** Enemy를 `rigidbody: Some(Dynamic)`으로 한 줄
바꾸고 E2E를 돌리면 `REPLAY FAILED: Enemy destroyed after two raycast melee hits — actual: 5.0`.
원인은 코드가 직접 설명한다: `sync_transform_from_physics`가 **Dynamic 바디의 `Transform`을 매
프레임 물리 값으로 덮어쓴다.** 즉 `NavMeshAgent`가 쓴 위치가 같은 프레임에 버려진다. 튜닝 문제가
아니라 경로 추적이 아예 동작하지 않으며, 따라서 **`NavMeshAgent`를 속도 구동으로 바꾸는 것이
선택이 아니라 전제다.**

**Kinematic + `KinematicCharacterController`를 안 고른 이유.** rapier3d 0.33의 `control` 모듈은
경사·계단·바닥스냅을 처리하고 `grounded`를 반환하며, 필요한 `QueryPipeline` 생성 패턴도 이미
`world.rs:437`에 있다. 그럼에도 안 쓴 이유는 두 가지다. ① 넉백이 임펄스 솔버를 거치지 않는다 —
우리가 감쇠시킨 값을 `desired_translation`에 넣는 방식이라 흉내보다는 진짜지만(벽에 막힌다) 이
item 제목이 말하는 것은 아니다. ② Rapier는 Kinematic 바디의 속도를 적분하지 않으므로 캐릭터가
자기 속도를 다시 갖게 되는데, 이는 item 33이 정리한 소유 구도를 되돌리고 실제로 그 회귀 가드가
실패한다.

**힘이 아니라 임펄스를 쓴다.** `PhysicsWorld::step`은 힘을 리셋하지 않고 `apply_force`는 Rapier의
`add_force`라 스텝을 넘어 누적된다. 힘 기반 에이전트는 매 프레임 힘이 쌓여 적이
날아간다. 매 프레임 `가속도 × 질량 × dt` 임펄스를 주면 연속 힘과 물리적으로 동등하면서 누적되지
않으므로, 보류된 엔진 수정을 이 항목이 끌어올 이유가 없다.

> 이 문단의 누적 서술은 **item 34에서 고쳐졌다.** 임펄스 선택 자체는 그대로 옳지만
> (한 프레임의 밀기를 표현하는 데는 임펄스가 정확하다), "힘을 쓰면 쌓인다"는 더는
> 사실이 아니다.

**완료 조건:**
- [x] `CharacterBody` 컴포넌트 — 삽입 시 `lock_rotations(e, true, false, true)`(이미 있는 API,
      씬 `RigidBodyDesc`가 노출하지 않는다), 스텝 뒤 캡슐 밑 레이캐스트로 `grounded`와 경사 판정
- [x] `navigate_agents`를 임펄스 구동으로 — `Transform`을 쓰지 않는다. **`NavMeshAgent.acceleration`이
      처음으로 의미를 갖는다**(지금은 선언만 되고 아무도 읽지 않으며, mini-arena 씬이 값을 저작하는데도
      그렇다). `bsengine-app → bsengine-physics` 간선이 새로 필요하고 순환은 아니다
- [x] `games/mini-arena`의 Enemy를 Dynamic + `CharacterBody`로, 넉백은 기존 `Bsengine.addImpulse`로.
      스크립트에서 넉백이라는 개념 자체가 사라진다
- [x] 테스트 추가, CI 통과 — 특히 **넉백이 에이전트 이동과 공존하는지**(에이전트가 속도를 덮어쓰면
      실패한다)와 `acceleration`이 실제로 도달 시간을 바꾸는지

**카탈로그 확인:** `grounded`/`character`/`controller`/`jump`/`slope`/`step` 여섯 개념 모두
`nothing owns this yet`. 새 어휘 도입이 맞고 중복이 아니다.

**구현 중 드러난 갭 — 씬이 감쇠를 지정할 수 없다.** `RigidBodyDesc::Dynamic`은
`RigidBody::dynamic()`으로 매핑되고 그건 `linear_damping: 0.0`이다. `RigidBody`에 필드는 있지만
씬 포맷이 그걸 말할 방법이 없어서, 씬으로 만든 Dynamic 바디는 임펄스를 받으면 무언가 막을 때까지
미끄러진다. mini-arena의 Enemy는 내비 에이전트의 역가속(`acceleration: 8.0`)이 브레이크 역할을
하므로 실사용에 문제가 없고 E2E도 통과하지만, **감쇠에 의존하는 다른 캐릭터는 이 갭을 밟는다.**

`CharacterBody`에 감쇠 필드를 두는 것은 하지 않았다 — `RigidBody.linear_damping`과 같은 개념의
두 번째 소유자를 만드는 일이고, item 32/33이 정리한 방향과 반대다. 올바른 수정은 `EntityDescriptor`가
감쇠를 저작할 수 있게 하는 것(`#[serde(default)]`이면 기존 씬은 그대로 파싱된다)이며, 별도 항목으로
남긴다. → **감사 후속에서 그대로 구현됨** (PR #1769).

**넉백 세기는 계산해서 골랐다.** 흉내는 초속 8로 0.25초를 밀어 총 2유닛을 옮겼다. Enemy 캡슐의
질량이 약 0.42이므로 임펄스 `I`는 `Δv = I/0.42`를 주고, 역가속 8 m/s²에서 정지 거리는
`Δv²/16`이다. 2유닛에 해당하는 값이 `I ≈ 2.4`라 2.5를 썼다. **E2E는 이 선택을 검증하지 못한다** —
임펄스 0.8도, 1.5도, 3.0도 전부 통과했다. 녹화는 적이 근접 두 방에 죽는지를 볼 뿐이라 넉백이 거의
없어도 만족되기 때문이다. 6.0에서 실패한 것은 적이 사거리 밖으로 밀려났기 때문이고, 그게 이 값의
상한을 알려준 유일한 신호다.

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

### 29. 애니메이션 블렌드 트리 (1D 블렌드 스페이스) ✅

**목표:** 현재 `AnimationStateMachine`이 상태 간 크로스페이드만 지원하는 것(로드맵 item
2)을 넘어, 이동 속도 같은 연속 파라미터로 여러 클립을 블렌드

**완료 조건:**
- [x] `BlendTree` 노드 타입 추가 (1D: 단일 파라미터, 최소 2클립)
- [x] `AnimationStateMachine`의 한 상태가 단일 클립 대신 `BlendTree`를 참조 가능
- [x] Scripting API로 블렌드 파라미터 갱신 가능 (예: 이동 속도 → walk/run 블렌드)
- [x] `games/mini-arena` 플레이어의 idle/walk/run 전환을 크로스페이드에서 블렌드 트리로
  교체해 검증
- [ ] 테스트 추가, CI 통과

---

**전제가 틀렸다: 크로스페이드도 동작한 적이 없었다.** 이 항목의 목표는 "크로스페이드만 지원하는
것을 넘어"였는데, 실제로는 출력에 크로스페이드가 없었다. `AnimationStateMachine`은 작성된 이래
`blend_weight`를 `blend_duration`에 걸쳐 성실히 진행시켜 왔지만 `update_skinned_meshes`는 클립
하나만 샘플링하고 그 값을 읽은 적이 없다. mini-arena의 `blend_duration: 0.15`는 아무 효과도 없었다.
그래서 포즈 블렌딩은 이 항목의 추가 기능이 아니라 **빠져 있던 토대**였고, 넣자마자 기존 크로스페이드가
처음으로 동작했다.

**블렌딩은 TRS에서 한다.** 합성된 행렬을 가중 평균하면 회전이 망가진다 — 회전 행렬 둘의 평균은
회전 행렬이 아니라 노드가 쪼그라든다. 변이 검증으로 확인했고, 다른 두 테스트는 그 변이를 못 잡는다.
쿼터니언은 slerp 전에 부호를 맞춘다(`q`와 `-q`는 같은 회전인데 잘못 고르면 구면을 먼 쪽으로 돈다).

**리플렉션 컴포넌트에 필드를 추가하면 씬이 조용히 깨진다.** 리플렉션 역직렬화는 구조체의 모든
필드가 RON에 있기를 요구하는데, 하나라도 없으면 **에러가 아니라 담고 있는 컬렉션이 빈 채로**
돌아온다. `AsmState`에 `blend`를 더하자 mini-arena의 `states`가 `{}`가 됐는데 — 경고도, 실패한
리플레이도 없이 그냥 애니메이션만 안 나왔다. E2E가 못 잡은 이유는 상태 기계에 상태가 있는지
아무도 단언하지 않아서였고, 그 가드(`a_state_machines_states_survive_deserialization`)를 넣었다.
증상이 타입 미등록과 똑같은데 해결책이 다르다는 점도 기록해 둔다 — 등록을 추가해도 안 고쳐진다.

**mini-arena는 상태 셋에서 하나로 줄었다.** 임계값이 플레이어의 실제 속도(0 / 4.5 / 8.1)라 읽는
사람이 추측할 필요가 없다. 속도 6.3에서 캐릭터는 진짜로 절반은 걷고 절반은 뛴다 — 세 상태
버전은 각자 전부라고 주장하는 두 클립을 섞었고 정확히 절반인 순간에만 옳았다.

**스크립팅 조건은 이미 충족돼 있었다.** `Bsengine.asmSetFloat`이 이미 있고 블렌드 트리가 같은
`params_float`를 읽으므로, 새 op이 필요 없었다.

---

### 30. 안정적 에셋 식별자 (리네임에 안 깨지는 참조) ✅

**목표:** 에셋 이름을 바꾸거나 옮겨도 그걸 참조하던 것들이 조용히 깨지지 않게 한다.
item 24 Phase 5에서 이관 — 그 단계의 전제가 틀렸음이 설계 중 드러났다(item 24의 해당
줄 참조). 설계 문서: `docs/superpowers/specs/2026-08-04-stable-asset-identity-design.md`
(단, `docs/`는 gitignore 대상이라 아래 요약이 추적되는 유일한 기록이다).

**왜 이 크기인가:** 저장소 씬 파일의 에셋 참조 32개 중 30개가 `script:` 경로인데 스크립트는
`bevy_asset`을 거치지 않는다. 제대로 덮으려면 **스크립트를 에셋으로 승격**해야 하고, 그
부수 효과로 **스크립트 핫리로드**가 딸려 온다 — 참조의 94%가 스크립트이니 실사용 가치가
가장 큰 핫리로드다.

**다른 엔진 조사 결과와 채택 이유:**
- **Unity**(`.meta`의 GUID만): 에디터 밖에서 에셋만 옮기면 새 GUID를 발급하고 고아 `.meta`를
  지운다 — **복구 불가**, 처방은 버전 관리뿐.
- **Unreal**(경로 + 리디렉터 스텁): 개념은 "옛 경로를 기억한다"와 같다. 실제 고통은 전부
  **정리하지 않을 때** 나온다 — 조회가 느려지고, 쿠킹이 체인을 끊어 패키징 빌드에서만
  로드가 실패하고, 소스 컨트롤 잠금이 fix-up을 막는다.
- **Godot 4**(UID + 경로, UID 우선): 4.4가 **우리와 같은 문제를 같은 방식으로** 풀었다 —
  스크립트/셰이더는 평문이라 UID를 넣을 자리가 없어 전용 `.uid` 사이드카를 도입했다.

**설계 요지:**
- **참조 형식:** 씬 RON은 `(guid, path)` 둘 다 저장, **GUID 우선·경로 폴백**(Godot식).
  경로를 남기므로 씬이 사람에게 읽히고, 사이드카가 전부 없어도 오늘과 똑같이 동작하며,
  기존 씬 10개를 한 번에 갈아엎지 않아도 된다. **스크립팅/MCP API는 계속 경로만 받는다**
  (item 23의 경계 설계 유지).
- **식별자 부여:** `<asset>.meta`(RON, `guid`/`hash`/`former_paths`)를 스캔이 자동 생성.
  명시적 import 단계 없음. **사이드카가 유일한 진실이고 별도 인덱스 파일은 없다** — item 24가
  "같은 사실이 두 곳에 있으면 어긋난다"에 세 번 당했다.
- **해석 순서:** guid 적중 → 경로 폴백 → 옛 경로 → 실패(Phase 4의 상태 조회가 보고).
  JS 안 문자열 리터럴은 guid가 없으니 경로 단계에서 시작한다.
- **고아 복구(두 엔진 모두 사용자에게 떠넘긴 지점):** 에셋 없는 `.meta`와 `.meta` 없는
  에셋의 **해시가 일치하고 짝이 유일할 때만** 다시 이어 붙인다. 모호하면 경고만 하고
  아무것도 하지 않는다.
- **정리 정책(언리얼의 교훈):** 옛 경로 해석은 **항상 경고**하고, `fixup` 명령이 씬 RON을
  재작성하며 `former_paths`를 잘라낸다. **JS 문자열은 자동 재작성하지 않고 위치만 보고한다.**
  두 엔진 모두 이걸 개발 시점 장치로 다룬다(Godot은 익스포트 시 UID 무효, 언리얼은 쿠킹 전
  fix-up 기대).

**완료 조건:**
- [x] **(A)** 사이드카 + 스캔 + 인덱스 + 고아 복구 — PR #1755. 소비자는 아직 없고 플러그인도
  어느 호스트에도 등록하지 않았다(둘 다 의도적이며, "등록을 빠뜨린 게 아니라 결정"임을
  rustdoc 두 곳에 적었다 — item 24에서 그 반대 실수가 두 번 났다).

  **스캔 대상은 allow-list로 정하며, 이 목록을 Phase 3b의 `RELOADABLE_EXTENSIONS`와 합치지
  않는다.** 둘은 다른 질문에 답한다 — "bevy_asset이 리로드할 수 있나" vs "정체성을 가질
  자격이 있나" — 과다 포함이 전자에서는 무해하고 후자에서는 해롭다. `assets/models/CREDITS.md`가
  deny-list로는 안 되는 이유다. sub-item C가 `.js`를 앞쪽 목록으로 옮기면 다시 갈라진다.

  **고아 복구는 짝이 명확할 때만 한다.** 최종 리뷰가 잡은 두 건이 모두 "복구가 조용히 틀린
  일을 한다"는 부류였다: (1) 내용 해시만 보고 확장자를 안 봐서, 삭제된 `main.ron`의 `.meta`가
  바이트가 같은 새 `level2.ron`에 정체성을 넘겨 버렸다 — 가설이 아니라
  `games/net-2p-demo/{client,server}/scene.ron`이 이미 바이트 동일하다. (2) 사이드카의 해시가
  갱신되지 않아, 에셋을 편집한 뒤 옮기면 복구가 Unity 수준으로 퇴화했다. 확장자 일치 요구와
  `size` 기반 재해시로 각각 고쳤고, 정상 상태 해시 비용은 여전히 0이다(에셋당 `stat` 한 번).

  **`.gitignore`의 `*.meta`도 함께 고쳤다.** Visual Studio 템플릿이 링커 아티팩트로 무시하고
  있어서, 두면 사이드카가 영영 커밋되지 않고 클론할 때마다 GUID가 새로 발급돼 모든 참조가
  깨진다 — 무시된 파일은 `git status`에도 안 뜨므로 조용히 그렇게 된다.
- [x] **(B)** 씬 RON에 guid 필드, 해석 순서, 기존 씬 10개 마이그레이션 — 사이드카 30개와
  참조 32개 전부 이관 완료. `AssetRef`는 `(guid, path)` 쌍과 **맨 경로 둘 다** 받는다:
  스크립팅·MCP API는 계속 경로만 다루고(item 23의 경계 설계), 사이드카가 하나도 없는
  프로젝트는 item 30 이전과 똑같이 동작한다. 해석은 guid → 경로 폴백 → 실패 순.

  **`#[serde(untagged)]`은 쓰지 않았다.** 파생이 동작은 하지만 오류가 전부
  `data did not match any variant`로 뭉개지고, 더 나쁘게는 **유효한 쌍 옆의 오타 필드를
  조용히 버린다** — `guid`를 `guuid`로 잘못 쓰면 정체성이 아무 진단 없이 사라진다. 손으로
  `Deserialize`를 써서 받아들이는 철자를 이름 대며 알리게 했다.

  **플러그인 등록만으로는 부족했다.** `AssetIdentityPlugin`과 `ScenePlugin`이 둘 다
  `Startup`인데, 제약 없는 Bevy 스케줄은 **`add_plugins` 순서를 재생하지 않고 정렬한다** —
  즉 "먼저 등록하면 된다"가 아니다. 실측: 명시적 `.after()` 엣지를 지우면 **어느 등록 순서로도**
  실패했다. 게다가 인덱스가 없을 때는 경로로 조용히 폴백하도록 만들어 뒀으므로, 이 실패는
  아무 증상도 남기지 않았을 것이다. `ScenePlugin`이 `.after(build_asset_index)`를 선언하고
  스캔은 `Commands` 대신 리소스를 직접 넣는다.

  **`bevy_asset`과 파일명이 충돌하고 있었다 — 각각은 옳아서 아무도 못 잡았다.**
  `bevy_asset`은 `<asset>.meta`를 자기 `AssetMetaMinimal` 형식으로 예약하는데, sub-item A가
  같은 파일명을 다른 형식으로 썼다. 결과는 경고가 아니라 **로드 실패**다: 플러그인을 켠 뒤
  `games/mini-arena`를 처음 실행하면 `fox.glb.meta`/`glow.wgsl.meta`를 만들고 나서 그
  둘을 못 읽어, **메시도 셰이더도 없이 뜬다** — 에셋 루트 버그와 똑같은 증상이 다른 원인으로.
  스캔은 잘 만든 사이드카를 쓰고 로더는 잘 만든 에셋을 읽으니 어느 쪽 테스트도 걸리지 않았고,
  사이드카가 실제 에셋 옆에 놓이는 순간에야 드러났다. `AssetMetaCheck::Never`로 껐다 —
  이 엔진은 에셋 프로세싱을 하지 않고(`AssetMode::Unprocessed`), 이미 같은 이유로 bevy의
  다른 경로 관례(`file_path`, `BEVY_ASSET_ROOT`)를 덮어쓰고 있다. 테스트는 어느 한쪽이 아니라
  **둘을 한 디렉터리에 놓는 것**이다.

  **MCP 씬 검증기도 함께 고쳤다.** `game_tools.rs`가 스크립트 참조를 생 텍스트
  `script: Some("`로 찾고 있어서, 마이그레이션 후에는 참조를 **하나도 못 찾고** "누락 없음"이라고
  보고했을 것이다 — 검증 도구가 조용히 검증을 멈추는 쪽이 실패하는 쪽보다 나쁘다.

  **동시 실행 경합은 사이드카를 커밋해서 없앴다.** E2E 8개 중 7개가 `games/tilt-run`을
  공유하므로 커밋 전에는 7개 프로세스가 동시에 발급을 시도한다(실측으로는 sub-item A가
  Minor로 고쳐 둔 원자적 쓰기 덕에 그마저도 안전했지만, 커밋이 정답이다 — 클론 간 정체성
  유지가 애초의 요구사항이다). 스캔 비용 실측: mini-arena 9에셋 첫 실행 ~21ms/이후 ~2ms,
  tilt-run 15에셋 ~28-31ms/~3ms — 캐시 없는 설계가 유지된다.
- ~~**(C)** 스크립트를 에셋으로 승격~~ → **item 31로 분리.** A·B를 끝내고 실제 비용이
  보이는 시점에 판단하기로 해 뒀고, B가 그 근거를 줬다 — 아래 item 31의 사유 참조.
- [x] **(D)** 옛 경로 복구 + `fixup` 명령/MCP 툴 — 워처가 기록하고, `load_async`와 씬 해석이
  읽고, `fixup`이 정리한다.

  **옛 경로를 기록하는 주체가 사실상 없었다.** `former_paths`를 써 온 것은 스캔의 고아 복구
  하나뿐이고, 그건 엔진이 **꺼져 있는 동안** 옮겼고 내용이 그대로일 때만 동작한다. 실측:
  커밋된 사이드카 30개의 `former_paths`가 **전부 빈 배열**이었다. 게임을 띄운 채 파일 이름을
  바꾸는 — 즉 사람들이 실제로 하는 — 경로는 아무것도 남기지 않았는데, `drain_asset_changes`가
  리네임 이벤트의 **출발지 경로를 버리고** 있었기 때문이다(목적지만 리로드하면 되니 아무도
  아쉬워하지 않았다). 이걸 먼저 고치지 않았다면 D는 오프라인 케이스만 구제하고, 정상적으로
  리네임한 사람 눈에는 그냥 고장 난 기능으로 보였을 것이다.

  **짝은 늘 있었고 늘 버려지고 있었다.** `notify-debouncer-full`은 리네임을 **두 경로를 모두
  담은 하나의 이벤트로, 옛 경로를 먼저** 보고한다. 그 위에 무엇을 세우기 전에 Windows와 CI의
  Linux 러너 양쪽에서 먼저 실측했다(PR #1759). 매칭은 `EventKind`가 아니라 **짝지어짐 자체**로
  한다 — 백엔드가 종류 이름은 제 식대로 붙여도 되지만, 옛 경로를 빠뜨려서는 안 된다.

  **어차피 동작할 수 없던 잠복 버그가 있었다.** 디바운서의 `FileIdMap` 캐시 루트가 **상대
  경로**로 등록돼 있는데 notify는 항상 CWD 기준 절대 경로를 보고한다. 캐시는 정확한 경로
  일치로 조회하므로 전부 빗나갔고, 리네임 쿠키가 없는 백엔드(Windows)에서 리네임은 서로 무관한
  두 이벤트로 도착했다. 아무도 못 잡은 이유는 **짝이 안 맞아도 목적지 리로드는 멀쩡히 되기
  때문**이고, Linux는 inotify 쿠키가 캐시를 안 거치고 짝을 지어 주므로 CI만으로는 영영 못
  찾았을 종류다.

  **JS는 재작성하지 않고 보고만 한다 — 그리고 그게 D가 존재하는 참조의 부류다.** 문자열
  리터럴에는 정체성을 넣을 자리가 없다. 실행 중인 스크립트 말고는 그 문자들이 경로라는 사실
  자체를 아무도 모르고, `"assets/scenes/" + level + ".ron"`처럼 조립되기도 한다(tilt-run의 레벨
  체인이 그렇다). 인덱스가 고쳐 줄 수 있는 참조가 아니므로 **어디로 갔는지 기억하는 것 외에
  이 부류에 닿는 방법이 없고**, 같은 이유로 기계가 고쳐 줄 수 있는 참조도 아니므로 파일·줄·옛
  경로·현재 위치만 보고한다. `.js` 바이트 불변은 서로 다른 세 곳에서 단언한다.

  **만료가 있는 이유:** 이 설계가 살펴본 세 엔진 중 언리얼의 실제 고통은 **정리하지 않은
  포워딩** 하나에서 전부 나온다 — 리디렉터가 쌓여 조회가 느려지고, 쿠커가 체인을 끊어
  에디터에서는 되던 로드가 패키징 빌드에서만 실패하고, 소스 컨트롤 잠금이 fix-up을 막는다.
  그래서 복구는 **항상 경고**하고(파일에는 여전히 옛 경로가 적혀 있으니 다른 무엇도 알려 주지
  않는다), `fixup`은 그 경고를 쓰는 곳이며, 이것이 `former_paths`가 길어지기만 하지 않고
  **짧아질 수도 있는** 목록인 이유다.

  **잘라내기 규칙은 "`fixup`이 읽을 수 있는 어떤 파일도 더 이상 그 이름을 부르지 않을 때만
  잊는다"이고, 판단 근거는 재작성이 의도한 결과가 아니라 재작성 후 디스크에 남은 것이다.**
  `.js`의 언급은 무기한 붙잡아 둔다(여기서 고칠 수 없는 참조다). 살아남은 `.ron` 언급은 재작성이
  안 먹혔다는 뜻이므로(읽기 전용, 에디터가 잡고 있음, 파싱 실패) 그 기억은 여전히 하중을 받는다.
  그중 하나라도 못 읽으면 아무것도 잘라내지 않는다 — "아무도 이 이름을 안 부른다"는 다 들여다
  보지 않고도 유지되는 결론이 아니고, 잘라내기는 여기서 유일하게 되돌릴 수 없는 동작이다.

  **두 번 돌려도 안전하다 — 아무도 안 들여다보는 쪽까지.** 씬을 재작성하면 그 씬 자체가 제
  사이드카 해시와 어긋난 에셋이 된다. 그래서 `fixup`은 재작성을 한 **바로 그 실행 안에서** 다시
  스캔해 바로잡는다. 안 그러면 "두 번 돌려도 무해"가 거짓이 되는데, 하필 어떤 뻔한 테스트도
  보지 않는 방향으로 거짓이 된다 — 다음 실행이 "할 일 없음"이라 보고할 소스 트리에 파일을 쓴다.
- [x] 에셋을 리네임한 뒤 참조가 복구되는지 보는 E2E 1개 (유닛 테스트로는 안 덮이는 부분) —
  `crates/bsengine-runtime/tests/rename_recovery.rs`.

  **`--replay` 녹화로는 표현할 수 없고, 근사하는 것보다 그렇게 말하는 편이 쓸모 있다.**
  프로토콜에 파일시스템을 건드리는 커맨드가 없고(step/키/마우스/query/assert/wait_until/
  shutdown이 전부), 리플레이 앱은 **워처를 아예 안 올린다** — 시계를 고정해 재현성을 지키는
  유일한 모드에 백그라운드 스레드와 프레임 간 변동을 들이는 값이기 때문이고, 이 테스트 하나
  때문에 그걸 뒤집는 건 tilt-run 레벨5 녹화가 CI에서만 깨지던 걸 고친 결정을 되돌리는 일이다.
  그래서 **가장 정직한 형태**로 만들었다: 진짜 `AssetWatcherPlugin`이 도는 진짜 엔진 앱에서
  실제로 파일을 리네임하고, 그렇게 남은 프로젝트를 **실제 `bsengine-runtime --test --replay`
  바이너리**에 넘겨 녹화가 `wait_until`로(고정 프레임이 아니라) 복구를 확인한다. 리네임 쪽만
  인프로세스인 이유는 `AssetWatcherPlugin`을 등록하는 호스트가 `run_windowed` 하나뿐이고 그건
  창과 GPU를 요구하기 때문이다.

  **링크마다는 이미 덮여 있었고 사슬이 안 덮여 있었다.** 워처·씬 해석·`load_async` 테스트가
  각각 다음 단계가 쓸 상태를 **손으로** 만들어 두므로, 셋 다 통과하면서 사이가 끊어져 있을 수
  있다. `FileIdMap` 버그가 정확히 그렇게 살아남았다 — 주변 유닛 테스트는 전부 통과했고, 진짜
  워처에게 진짜 리네임을 물어본 것이 하나도 없었다.

  **씬의 참조는 맨 경로로 쓴다.** `(guid, path)` 쌍이면 sub-item B의 GUID 조회가 먼저 구제해
  버려 D의 코드에 닿지도 않는다. 관측 대상은 **게임이 실제로 도는 것**이다: 씬은 여전히 옛
  경로를 부르는데 엔티티가 움직이면, 리네임된 파일을 찾아냈다는 뜻이다. "복구했다"와 "복구했다고
  말했다"는 다른 주장이므로 자식 프로세스 stderr의 경고가 두 경로를 모두 담는지도 함께 단언한다.

  **게임은 건드리지 않는다.** 리네임은 커밋된 `.meta`를 옮기고 거기에 한 줄을 붙이는 일이라
  `games/` 안에서 하면 추적 중인 파일을 고치게 되고, 리네임과 복원 사이에서 실패하면 저장소가
  더러워진 채 남는다 — 복원이 막으려던 바로 그 상황이다. 게다가 E2E 8개 중 7개가 공유하는
  `games/tilt-run`과 경합하고, "옛 경로로만 해석되는 참조"는 어떤 실제 게임도 커밋된 채로
  들고 있어선 안 되는 상태다. 임시 디렉터리 픽스처를 만들고 drop에서 지운다.

  **깨뜨리면 실패하는지도 테스트로 남겼다.** 같은 픽스처에서 `former_paths` 한 줄만 지우면
  (스크립트도 씬도 정체성도 그대로) 녹화가 wait에서 타임아웃하며 실패한다. 이게 없으면 위
  테스트는 "복구는 하나도 안 했지만 어쩌다 엔티티가 움직인" 엔진에서도 똑같이 통과한다.
- [x] 테스트 추가, CI 통과

---

### 31. 스크립트를 에셋으로 승격 (+ 스크립트 핫리로드)

**목표:** JS 스크립트를 `std::fs::read_to_string` 직접 읽기에서 `bevy_asset` 에셋으로 옮긴다.
그 부수 효과로 **스크립트 핫리로드**가 딸려 오고, 스크립트도 안정적 식별자를 가질 수 있게 된다.

item 30에서 분리했다. 그쪽 계획이 "A·B가 끝나 실제 비용이 보일 때 분리 여부를 정한다"고
적어 뒀고, B가 그 근거를 줬다.

**왜 별도 항목인가:**
- **성격이 다르다.** item 30의 A·B는 에셋 파이프라인에 정체성을 *얹는* 일이었지만, 이건
  스크립팅 로드 경로를 **동기에서 비동기로 바꾸는** 일이다 — item 24 Phase 2가 네 소비자에
  했던 그 전환이고, 그때 "요청은 한 번, 핸들을 보관, 보관한 핸들을 폴링" 불변식을 새로
  세워야 했다.
- **파급이 크다.** 스크립트는 씬 참조의 94%(32개 중 30개)이고 **E2E 8개가 전부 스크립트
  구동**이다. 스크립트 로드가 프레임을 넘겨 완료되면 씬 로드 시점의 순서가 달라진다.
- **경계에서 나오는 결함이 무섭다.** sub-item B에서 `bevy_asset`과 파일명이 겹친 것만으로
  게임이 메시도 셰이더도 없이 뜨는 실패를 겪었다. 같은 부류가 스크립트에서 나면 게임이
  아예 동작하지 않는다.
- **얻는 것이 정체성과 독립적으로 가치 있다.** 스크립트 핫리로드는 이 엔진이 가질 수 있는
  가장 값진 핫리로드다 — 참조의 94%가 스크립트다.

**완료 조건:**
- [x] `ScriptSource` 에셋 타입 + 로더, `load_scripts`가 `bsengine_asset::load_async` 경유
- [x] item 24 Phase 2가 세운 request-once/retain-handle 불변식 준수
- [x] 스크립트 파일을 저장하면 실행 중인 게임에 반영
- [x] `RELOADABLE_EXTENSIONS`에 `js` 추가 — item 30의 `IDENTIFIED_EXTENSIONS`와 갈라졌다
  (`ron`은 identity 쪽에만 남는다). 둘을 합치지 않는다는 item 30의 판단이 실제로 값을 한 지점
- [x] E2E 8개가 전부 통과 (전부 스크립트 구동이므로 이게 실질 검증선)
- [x] 테스트 추가, CI 통과

**분리 판단이 옳았던 이유, 그리고 우려가 빗나간 지점.** 이 item을 나눈 근거는 "E2E 8개가 전부
스크립트 구동이라 녹화가 깨질 것"이었다. 실제로는 **녹화를 하나도 수정하지 않고** 통과했다 —
item 24 Phase 1이 고정 프레임을 `wait_until` 술어로 바꿔 둔 것이 정확히 이 지연을 흡수한 것이다.
그 전환이 없었다면 여기서 프레임 수를 올리는 유혹에 빠졌을 테고, 그건 로드맵이 명시적으로
금지한 방향이다.

**원자성이 깨진 지점은 하나다.** `handle_scene_load`(`scene_systems.rs`)가 씬을 다시 스폰한 뒤
`load_scripts`를 **인라인 동기 호출**해서, 지금까지 "씬 스폰"과 "스크립트 실행"이 한 단계였다.
`games/tilt-run`이 `loadScene`으로 레벨 다섯 개를 잇고 녹화가 다음 레벨의 스크립트 동작을
단언하므로 여기가 유일한 진짜 위험이었다. 이제 요청만 인라인이고 실행은 폴링 시스템이 한두
프레임 뒤에 한다.

**리로드가 성립하는 건 래퍼가 IIFE이기 때문이다.** 각 스크립트는 `Bsengine._scripts["<엔티티
비트>"]`에 자신을 등록하는 즉시실행 함수로 감싸이므로, 다시 실행하면 항목이 **누적되지 않고
교체된다**. 대신 최상위 `let`/`var`가 전부 초기화되며(`var played = false`가 다시 false),
이건 올바른 기본값이지만 놀랄 수 있으므로 리로드 로그가 그 사실을 직접 말한다.

**브리프에 없었지만 필요했던 것:** 실패한 로드는 에셋을 만들지 않으므로, 나중에 파일을 고쳐도
`Modified`가 아니라 `Added`로 온다. 그 분기가 없으면 **파일명 오타 하나가 실행 내내 고쳐지지
않는다.** `GaveUp` 엔티티에 한정해 처리했고, 이게 가능한 건 실패해도 핸들을 붙잡고 있기
때문이다 — item 24의 셰이더 소비자는 `GaveUp`에서 핸들을 버려서 같은 상황이 진짜 막다른 길이다.

`--test` 앱은 `AssetWatcherPlugin`을 등록하지 않으므로(`test_mode.rs`, 창 있는 호스트 전용),
`js`를 워처 목록에 넣은 것이 리플레이에 영향을 줄 수 없다는 점도 확인했다.

---

### 32. 컴포넌트/op 카탈로그 (중복·설계 드리프트 예방) ✅

**목표:** 새 컴포넌트나 새 스크립팅 op을 만들기 **전에**, 그 개념이 이미 어디에 사는지 알 수 있게 한다.

**이 항목이 생긴 계기는 실제 실패다.** item 27을 설계하면서 `Velocity` 컴포넌트를 새로 만들자고
제안했다. 카탈로그를 세우고 실제로 질의해 보니 **`bsengine_core::Velocity`가 이미 존재하고
등록까지 돼 있었다.** 즉 그 제안은 있는 컴포넌트를 이름까지 똑같이 다시 만드는 것이었다.

그리고 실제 구도는 그보다 나쁘다. "속도"는 **두 서브시스템에 병렬로** 산다:

- `bsengine_core::Velocity { linear: ReflectVec3 }` — 운동학적 속도. `VelocityPlugin`이
  매 프레임 `Transform.translation`에 적분한다.
- Rapier의 속도 — `bsengine_*_velocity` op 18개가 읽고 쓴다.

`Velocity`의 독 주석이 그 경계를 직접 말한다("For physics-driven motion use `bsengine-physics`
instead"). 문서화된 분리이긴 하나, **"velocity 컴포넌트가 있나?"와 "velocity op이 뭘 하나?"의
답이 서로를 언급하지 않는다.** 이 항목을 설계하며 필드 이름만 grep한 탓에 `Velocity`를
놓쳤다는 사실 자체가, 사람이 눈으로 훑는 방식이 왜 실패하는지의 증거다.

**측정된 전제** (설계 전 실측, 추정 아님):

| | 수 |
|---|---|
| `#[derive(..., Component, ...)]` 파생 타입 | 54 (그중 공개 49) |
| 공개 중 `register_type::<>` 등록됨 | 34 |
| **공개 미등록** | **15** (고유 이름 14 + `Name` 중복) |
| `#[op2] pub fn` 스크립팅 op | **298** |

- **미등록 14개는 내부용이 아니다.** `RigidBody`/`Collider`/`PhysicsBodyDesc`/`PhysicsTransform`/
  `PhysicsInput`, `MeshRenderer`/`SkinnedMesh`/`GltfAsset`, `AudioPlayer`/`AudioSource`/
  `PlaybackState`, `AnimationClipLibrary`/`Script`/`Name` — 물리·렌더·오디오 전체 세트다.
  bevy_reflect 도입이 `bsengine-core`를 끝내고 다른 크레이트로 넘어가지 않은 결과이며, 실질
  영향은 인스펙터가 `RigidBody`를 못 보여주고 MCP `set_reflected_component`가 물리를 못 붙이는 것.
- **이름 중복이 이미 하나 있다.** `Name`이 `bsengine-core/src/name.rs`(사용처 **0곳**)와
  `bsengine-scene/src/plugin.rs`(사용처 11곳)에 각각 `pub struct Name(pub String)`으로 정의돼
  있다. 구조도 독 주석상 목적도 같고, 인스펙터는 짧은 타입 이름을 쓰므로 UI에서 구분되지 않는다.
- **드리프트는 컴포넌트보다 op 쪽이 심하다.** 컴포넌트 49개(공개)에 op 298개. `velocity`
  한 개념에 op 18개(전체 벡터 + 축별 변형 + angular 대응)에 더해 컴포넌트가 둘
  (`Velocity`, `AngularVelocity`)이다. `speed`는 `animation_`/`follow_`/`linear_`/`nav_`/
  `nav_angular_` 다섯 개념이 각자 op 쌍을 갖고, 그중 `linear_speed`는 velocity의 크기라 파생
  가능한데도 자기 op을 갖는다.

**설계:** `crates/bsengine-catalog`이 워크스페이스 소스를 `syn`으로 파싱해 색인을 **하나** 만들고
소비자가 둘이다 — MCP 툴 `component_catalog`(설계 시점 질의)와 CI `catalog --check`(기계적 게이트).
색인이 하나이므로 MCP 응답과 CI 판정이 어긋날 수 없다. 소유권 설명은 러스트독에서 추출한다
(`missing_docs` 스윕 덕에 모든 공개 컴포넌트/op에 독 주석이 컴파일러 강제로 존재한다) — 카탈로그
전용 메타데이터는 도입하지 않는다. 선언 지점이 정의 옆이 아니면 반드시 어긋나기 때문이다.

**런타임 레지스트리가 아니라 정적 파싱인 이유:** 레지스트리는 48개 중 34개만 보므로 나머지를
조용히 빠뜨린 채 "전체 목록"이라 답하게 된다. 그리고 예방이 필요한 시점은 코드가 존재하기 전이다.
**텍스트 스캔도 아닌 이유:** 이 설계를 준비하며 grep으로 세어 봤더니 49개 중 14개만 잡고
`Namepub` 같은 파싱 쓰레기를 만들어 냈다.

**완료 조건:**
- [x] `crates/bsengine-catalog` — `syn` 파싱으로 컴포넌트(이름/크레이트/위치/필드/독/등록 여부)와
      op(이름/크레이트/위치/독) 색인 생성
- [x] 개념 색인 — 이름을 snake_case/CamelCase 경계로 분해한 역색인. `velocity` 질의가
      `bsengine-core`의 컴포넌트와 물리 op 양쪽을 함께 반환하는 회귀 테스트로, 이 항목의 계기가
      된 실수(둘이 서로를 언급하지 않는다는 것)를 고정한다
- [x] MCP 툴 `component_catalog` — 개념어 질의 + 전체 나열
- [x] CI `catalog --check`: **R1** 모든 `Component`가 `register_type` 됨(**예외 기제 없음**),
      **R2** 축별(`_x`/`_y`/`_z`) op 신규 추가 금지 래칫
- [x] 공개 미등록 15개 등록 — 게이트를 빨간 채로 머지할 수 없으므로 이 항목의 일부다
- [x] `Name` 중복 정리 — 죽은 `bsengine_core::Name`(사용처 0곳) 삭제, `bsengine_scene::Name` 등록
- [x] 테스트 추가, CI 통과

**예외 기제는 이미 존재하고 이름은 `pub`이다.** 카탈로그를 실제로 워크스페이스에 돌리자
컴포넌트가 49개가 아니라 55개였다. 추가된 6개는 놓친 컴포넌트가 아니라 성격이 다른 것들이었다 —
다섯은 `pub(crate)`이거나 private(`AudioHandle`, `PhysicsHandles`, `GltfLoaded`, `PendingGltf`,
`ScriptLoad`)이고, 하나는 `bsengine-ecs`의 `#[cfg(test)]` 픽스처(`Position`)다.

비공개 컴포넌트는 **구조적으로 내부용**이다. 다른 크레이트가 이름을 부를 수 없으므로 공용 등록
함수에 등록하는 것 자체가 불가능하고, 씬 파일도 인스펙터도 MCP도 닿지 못한다. 그래서 R1은
**공개 컴포넌트에만** 적용한다. 이것이 이 설계가 처음 제안했다 철회한 허용 목록 파일보다 엄격히
나은 이유는, 선언이 정의 지점에 있고 **컴파일러가 강제**하며, 예외를 원하는 사람이 별도 파일에
줄을 추가하는 게 아니라 타입을 실제로 비공개로 만들어야 하기 때문이다.

테스트 픽스처는 설계 표면이 아니므로 카탈로그에서 제외한다. 넣으면 개념 질의에 엔진이 갖지도
않은 `Position` 컴포넌트가 나온다.

**실측 확정:** 컴포넌트 54개(공개 49) / 등록 34 / **공개 미등록 15**(고유 이름 14 + `Name` 중복) /
op 298 / 축별 op 45.

**등록 대상 판단.** 공개 미등록 15개 중 11개는 로컬 평범한 데이터라 그냥 등록된다
(`RigidBody`/`Collider`의 필드 타입은 로컬 enum이고, `PhysicsTransform`/`PhysicsInput`의
`Vec3`/`Quat`는 `ReflectVec3`/`ReflectQuat`가 이미 등록돼 있다). 외부 타입에 막히는 건
`AudioSource { data: StaticSoundData }`(kira) **한 건뿐**이고, 그조차 `#[reflect(ignore)]`로
해결될 가능성이 높다. `SkinnedMesh`/`AnimationClipLibrary` 둘은 등록 가능성이 아니라 값어치의
문제다(정점 5만 개를 인스펙터에 펼치는 게 의미가 있는가). 나머지 둘은 중복된 `Name`이다.

허용 목록 파일을 두지 않는 이유도 같은 자리에 적어 둔다. 쓰이지 않을 예외 목록을 미리 만들면
등록하기 귀찮을 때 쓰는 탈출구가 될 뿐이다. `pub` 여부라는 기존 문법이 그 역할을 이미 하고 있고,
그쪽이 훨씬 정직하다.

**실행은 Red 우선.** ① `--check`가 R1 위반 15건으로 실패 → ② 평범한 11개 등록 → ③ `AudioSource`가
`StaticSoundData` 때문에 실패하는 것을 확인한 뒤 `#[reflect(ignore)]`가 되는지 컴파일러에게 묻는다
(코드베이스에 선례가 없으므로 가정하지 않는다) → ④ 애매한 둘을 판단 → ⑤ `Name` 중복 정리 →
⑥ 게이트를 CI에 붙인다. 죽은 타입을 등록해 R1을 만족시키는 것은 규칙을 지키되 목적을 배신하는 일이다.

**카탈로그가 구축 도중에 찾아낸 것들.** 감사를 시작하기도 전에 R1을 초록으로 만드는 과정에서
나왔다. 이것이 이 항목의 값을 실증한다.

1. **세이브 시스템이 아무것도 저장하지 않고 있었다.** `Name`이 두 번 정의돼 있었고
   (`bsengine-core`와 `bsengine-scene`, 둘 다 `pub struct Name(pub String)`), 씬 스폰은
   `bsengine_scene::Name`을 붙이는데 `bsengine-scripting/src/save.rs`만 `bsengine_core::Name`을
   임포트했다. 그래서 `save_world`의 쿼리가 실제 게임 월드에서 **한 엔티티도 매칭하지 못했다** —
   세이브 파일에 빈 엔티티 목록이 쓰였다. 두 타입이 구조가 같아 컴파일은 통과했고,
   `save.rs`의 테스트는 자기가 `bsengine_core::Name`을 직접 spawn해서 전부 통과했다.
   **진짜 시스템을 구동하지 않는 테스트가 코드가 위반하는 성질을 인증한 사례**이며, 회귀
   테스트(`a_scene_spawned_entity_is_actually_saved`)는 `spawn_scene_entities`를 실제로 호출한다.
   변이 검증: 씬이 붙이지 않는 `Name`을 쿼리하게 만들면 세이브 파일이 비고 테스트가 실패한다.
2. **ECS 오디오 경로 전체가 도달 불가능했다.** `AudioSource`를 만들거나 붙이는 코드가
   워크스페이스에 없어 `start_playback`의 쿼리가 영원히 매칭되지 않았다. 소리는 스크립팅이
   `AudioWorld`를 직접 호출해 난다. 자체 `PlaybackState` enum은 `kira::sound::PlaybackState`의
   그림자 복제였다. 삭제했다 — 등록했다면 동작하지 않는 진입점을 광고하는 셈이었다.
3. **`Velocity`가 두 서브시스템에 병렬로 존재한다.** 위 참조. 이건 삭제 대상이 아니라
   감사에서 다룰 설계 판단이다.

**CI가 판별하지 못하는 것을 명시해 둔다.** 중복 그 자체는 기계가 판별하지 못한다. `linear_speed`가
velocity의 크기라는 것, 두 `Name`이 같은 개념이라는 것은 판단이다. R1/R2는 위생 규칙이지 중복
탐지기가 아니며, **게이트가 초록인 것을 "중복 없음"으로 읽는 것이 이 도구의 가장 그럴듯한 실패
방식**이다. `--check` 출력에 무엇을 검사하지 않는지 한 줄 적는다.

**최종 상태.** 컴포넌트 49개 전부 등록(R1 초록), op 298개, 축별 45개가 기준선에 고정(R2 래칫).
`catalog --check`가 CI에서 clippy 뒤·빌드 앞에 돈다 — `syn`만 쓰므로 값싸고 빨리 실패한다.
질의는 두 경로다: 에이전트용 MCP 툴 `component_catalog`, 사람용 `catalog --concept <word>`.

**첫 실사용이 item 27에 답을 줬다.** `catalog --concept grounded` → *"nothing owns this yet."*
반면 `--concept velocity`는 *"velocity is spread across 2 crates"*를 출력한다. 즉 `Grounded`는
진짜 새 개념이 맞고, `Velocity`는 만들지 말았어야 했다 — 이 도구가 없었다면 둘 다 몰랐다.

**등록 위치는 한 곳이 아니다.** 크레이트 그래프가 정한다. `bsengine-scene`은 physics/render/audio에
의존하지 않고 `bsengine-scripting`은 거꾸로 scene에 의존하므로(순환), 각 크레이트가 자기 플러그인에서
등록한다 — 단 그 플러그인이 **헤드리스 `--test` 앱에도 추가되는 경우에만**이다. `GltfPlugin`과
`RenderPlugin`은 창 있는 호스트 전용이라 gltf 타입 셋은 `register_gameplay_reflect_types`로 갔다.
`MeshRenderer`만 창 있는 쪽에서만 등록된다(GPU 핸들이라 옳다). **알려진 한계: R1은 "어느 호스트가
등록하는가"를 구분하지 못한다** — 소스 어디든 `register_type::<T>`가 있으면 등록된 것으로 센다.

**`Vec3`/`Quat` 필드는 `ReflectVec3`/`ReflectQuat`로 바꿔야 했다.** glam 타입은 `Reflect`를 구현하지
않고, 이 저장소는 이미 그 래퍼를 갖고 있었다(`bsengine_core::reflect_glam`). `Collider`의
`half_extents`를 포함해 물리 쪽 필드 타입이 바뀌었다 — 등록보다 큰 변경이지만 기존 관례를 따른 것이고
대안이 없다.

**`#[reflect(ignore)]` 선례를 이 항목이 세웠다.** `SkinnedMesh`의 정점별 대량 필드와
`AnimationClipLibrary`의 `clips`가 대상이고, 컴포넌트의 존재와 식별 정보만 노출한다. 다만
`#[reflect(ignore)]`는 무시된 필드에 `Default`를 요구하므로 만능이 아니다 — `AudioSource`의
`StaticSoundData`(kira)가 정확히 거기서 막혔고, 그게 오디오 경로가 죽었다는 사실을 드러냈다.

**범위 밖:** 감사 결과의 *수정*. 카탈로그를 만든 뒤 49개와 298개를 훑어 findings를 내고, 거기서
나오는 수정(velocity op 정리, `Name` 중복 제거)은 각자 별개 항목이 된다.
27개짜리 op 정리를 카탈로그 구축에 끼워 넣으면 둘 다 망한다.

---

### 33. 운동학 운동 스택 제거 (Rapier로 일원화)

**목표:** `bsengine-core`/`bsengine-app`의 운동학 물리 스택을 지우고 운동을 Rapier 한 곳으로 모은다.

아래 감사 결과 A가 근거다. 속도·감쇠·중력·충격량이 두 서브시스템에 각각 구현돼 있고 서로를
참조하지 않는다. item 27(캐릭터 컨트롤러)이 어느 쪽에 설지 골라야 해서 지금 정한다.

**제거 전에 잰 것.** 추정이 아니라 실측이며, 이 수치가 제거를 가능하게 한다:

| | 실사용처 |
|---|---|
| `Velocity` | `games/cube-roller` 1곳뿐 (나머지는 등록·인스펙터 나열) |
| `AngularVelocity`·`GravityScale`·`ExternalImpulse`·`Mass` | 게임 사용 0곳 |
| `Damping` | 스크립팅 `setDamping` 1곳 |
| 스크립팅 velocity op 18개 | **전부 Rapier행**(`PhysicsWorld::set_linvel`) — 운동학 컴포넌트를 건드리지 않는다 |

즉 `tilt-run`·`mini-arena`의 스크립트는 이 제거의 영향을 받지 않는다. 처음 우려했던 "물리 바디 없이
움직이는 UI·카메라·장식"은 **하나도 없었다.**

**감사가 추가로 드러낸 어휘 충돌:** `setGravityScale` op은 `GravityScale` **컴포넌트가 아니라**
Rapier로 간다(`pw.set_gravity_scale`). 이름이 같은데 다른 시스템을 건드린다.

**완료 조건:**
- [ ] `games/cube-roller`를 Rapier로 포팅 — **먼저 한다.** 그래야 삭제 시점에 의존자가 없다
- [ ] `bsengine-core`에서 `Velocity`/`AngularVelocity`/`Damping`/`GravityScale`/`Gravity`(리소스)/
      `ExternalImpulse`/`Mass` 삭제
- [ ] `bsengine-app`의 velocity·angular_velocity·damping·gravity·external_impulse 플러그인 삭제
- [ ] 스크립팅 `setDamping` 제거 — Rapier용 `setLinearDamping`이 이미 있어 중복이다
- [ ] 리플렉션 등록과 인스펙터 항목 정리 (`catalog --check`가 초록을 유지해야 한다)
- [ ] 테스트 추가, CI 통과, E2E 8개 통과

**item 27이 이 항목의 첫 소비자다.** 캐릭터 컨트롤러는 Rapier의 `KinematicCharacterController`
위에 서고, 운동학 컴포넌트를 다시 만들지 않는다.

---

### 34. 힘이 스텝을 넘어 누적되던 문제 ✅

**목표:** `addForce`가 문서에 적힌 대로 "이번 스텝에" 작용하게 한다.

item 25/27이 이걸 우회하고 지나갔고, item 27의 설계 문서는 "수정법은 알지만 레벨 재튜닝이
필요해 보류"라고 적었다. **문서화는 수정이 아니다.**

**엔진이 거짓말을 하고 있었다.** `apply_force`의 doc comment는 `for the current step`이라고
썼지만 Rapier의 `add_force`는 `reset_forces`를 부를 때까지 남는다. `PhysicsWorld::step`은
부르지 않았다. 그래서 힘은 **영원히** 쌓였고, `release_key`조차 그걸 걷어내지 못했다.

**콘텐츠가 그 거짓말에 맞춰 튜닝돼 있었다.** `tilt-run`의 `FORCE_MAGNITUDE`는 0.045였다 —
정상값의 1/50이다. 그 값이 성립한 유일한 이유가 누적이었고, 녹화 7개는 "키를 20프레임 누르고
330프레임 코스트"처럼 적혀 있었지만 그 330프레임은 코스트가 아니라 **계속 추진 중**이었다.

**완료 조건:**
- [x] `PhysicsWorld::step`이 스텝 뒤 모든 바디의 힘·토크를 리셋 (`wake_up: false` — 안 느끼던
      힘을 지우는 게 바디를 깨울 이유는 아니다). 시스템이 아니라 `step` 안에 둔 이유는
      `step`을 직접 부르는 테스트까지 같은 물리를 받게 하기 위해서다
- [x] Red 먼저 — 같은 힘을 4스텝 걸었을 때 속도 증가분이 일정한지. 수정 전 증가분은 정확히
      **1:2:3:4**로 나왔고, 한 번 건 힘은 11스텝 동안 밀었다. 토크 판과 "한 번 건 힘은 한 번만
      작용한다" 판까지 3개
- [x] 거짓 문서 3곳 정정 (`world.rs`의 `apply_force`/`reset_forces`, `ops.rs`의 `ResetForces`와
      JS 주석). `resetForces` op은 남긴다 — 같은 프레임에 이미 걸린 힘을 스텝 전에 무르는
      좁은 용도가 여전히 있고, 스크립팅 API를 일방적으로 깨지 않는다
- [x] `tilt-run` 재튜닝 — `FORCE_MAGNITUDE` 0.045 → 2.5, 녹화 6개의 프레임 수 재산출
- [x] E2E 8개 전부 통과, `FORCE_MAGNITUDE = 0` 변이에서 tilt-run 7개 전부 실패

**발견: 녹화 2개가 아무것도 검증하지 않고 있었다.** `level2-clear`는 `z > 9.0`을,
`level3-clear`는 `z > 7.0`을 어서트하는데 **두 레벨 모두 공의 시작 z가 10이다.** 키를 하나도
누르지 않아도 참이다. 클리어해서 다음 레벨이 로드돼도 그 시작 z가 또 10·8이라 위치만으로는
둘을 구분할 수 없다. 다음 레벨에만 존재하는 엔티티(`MovingPlatform`/`MovingObstacle`)를
질의하도록 바꿨다 — 없으면 `null is not numeric`으로 실패한다.

이 둘은 이번 수정에서도 **혼자만 통과했다.** 5개가 빨갛게 죽는 동안 조용히 초록이었고,
그게 이 녹화들이 8일 동안 아무것도 지키지 않았다는 유일한 신호였다.

---

### 35. 커밋된 에셋 해시가 어느 플랫폼에서도 맞지 않던 문제 ✅

**목표:** 사이드카에 기록된 blake3가 실제로 무언가를 보증하게 한다.

item 34를 하다 곁가지로 발견했다. `cube-evader`의 `player.js.meta` 하나가 어긋나 있길래
"스크립트만 고치고 meta를 안 갱신했나 보다" 하고 넘어갈 뻔했다. **한 개가 아니라 31개 전부였다.**

**증상은 "게임을 돌리면 트리가 더러워진다"였다.** 이건 잡음처럼 보이지만, 실제로는 무결성
검사가 매 실행마다 전부 실패하고 있다는 신호였다.

**원인.** `measure_file`은 파일의 **원본 바이트**를 해싱하고, 그 결과가 git에 커밋된다.
그런데 `.gitattributes`는 `.rs`/`.toml`/`.json`에만 `eol=lf`를 걸어 두고 정작 에셋 시스템이
해싱하는 `.js`/`.ron`/`.wgsl`/`.meta`는 `* text=auto`에 맡겨 두었다. Windows 체크아웃은
CRLF, Linux는 LF를 받으므로 **커밋된 해시는 둘 중 한쪽에서만 맞을 수 있다.** 기록된 값은
전부 CRLF에서 뜬 것이었고, 따라서 **Linux(=CI)에서는 지금까지 모든 텍스트 에셋이 "변경됨"이었다.**

**완료 조건:**
- [x] `.gitattributes`에 `.js`/`.ron`/`.wgsl`/`.meta` `eol=lf` 추가. 바이너리 에셋
      (`.glb`/`.wav`/`.png`/…)은 `binary`로 명시 — 메시나 사운드 안의 CRLF 치환은 diff가
      아니라 조용한 손상이고, `text=auto`가 여태 맞게 추측해 온 것과 명시된 것은 다르다
- [x] 31개 사이드카를 LF 바이트로 재생성 (각 프로젝트를 한 번씩 로드)
- [x] 회귀 가드 — `committed_sidecars.rs`가 커밋된 사이드카 전부를 다시 측정한다. 실패
      메시지가 두 원인을 구분한다: **차이가 전부 그 파일의 줄 수와 같으면 줄바꿈 문제,
      한 파일만 어긋나면 스캔 없이 편집된 것**
- [x] 변이 검증 2종 — 해시 한 글자 변경, 에셋을 CRLF로 되돌리기. 둘 다 실패한다
- [x] 커밋 후 프로젝트를 돌려도 트리가 그대로임을 확인 (증상 소멸), E2E 8개 통과

**교훈은 34번과 같은 모양이다.** 어긋난 값 하나를 고치는 것과 왜 어긋났는지 묻는 것은
비용이 30배 다르고, 후자만이 다시 어긋나지 않게 한다.

---

### 36. 헤드리스 렌더링 + 픽셀 리드백

**목표:** 렌더러가 실제로 낸 픽셀을 테스트가 읽는다.

설계: [docs/superpowers/specs/2026-08-06-headless-render-readback-design.md](docs/superpowers/specs/2026-08-06-headless-render-readback-design.md)

item 28(파티클)을 시작하려다 "작업의 절반이 관찰할 수 없는 GPU 파이프라인"이라고 보고했는데,
조사해 보니 **관찰 불가능성은 파티클의 성질이 아니라 이 저장소가 픽셀을 읽어본 적이 없다는
사실**이었다. `copy_texture_to_buffer`가 저장소 어디에도 없다. 헤드리스 GPU 디바이스는 이미
있고 테스트가 쓰고 있다.

**창 결합이 좁다.** `WgpuSurface`가 창을 쓰는 유일한 곳이 `create_surface`이고 보관 필드는
이미 `_window`다 — 살려두려고 들고 있을 뿐 읽지 않는다. 스왑체인을 만지는 곳은 넷,
`self.config` 사용처는 여섯 곳뿐이다. `egui-winit` 의존도 없어(raw input을 손으로 조립한다)
egui가 이미 창에 독립적이다.

**`render_frame` 하나가 그림자·포인트라이트 큐브 그림자·스카이박스·텍스처·커스텀 셰이더·
블룸·SSAO·톤매핑·HUD를 전부 구동한다.** 창 하나만 걷어내면 이 전부가 픽셀로 검증 가능해진다.

**완료 조건:**
- [ ] `Output` enum으로 창/오프스크린을 가르고 `new_offscreen(w, h)` 추가 — 파이프라인
      구축은 창 경로와 **공유한다**. 복제본을 만들면 테스트가 진짜 렌더러를 검사하지 않는다
- [ ] `offscreen.rs`의 `read_pixels()` — 행 256바이트 정렬. 테스트 해상도는 200×150
      (`200*4 = 800`, `800 % 256 = 32`이라 패딩 경로를 강제한다. 폭이 64의 배수면
      패딩을 빠뜨린 코드도 통과한다)
- [ ] 픽셀 테스트 14개 — 기본 4, 라이팅·그림자 3, 재질 3, 포스트프로세스 3, UI 1.
      각각 변이로 검증
- [ ] 어댑터가 그리지 못하면 어댑터 이름과 함께 실패 — **조용한 skip 금지**
- [ ] 창 경로 회귀 없음(E2E 8개, 에디터 825개), CI 양쪽 러너에서 픽셀 테스트 통과

**가장 큰 위험은 CI가 실제로 래스터라이즈하는가다.** 지금 CI의 wgpu 테스트들은 디바이스를
만들기만 하고 그린 적이 없다. 그래서 첫 커밋을 `Output` 분리 + 리드백 + 클리어 색 테스트
하나까지만 하고 즉시 푸시해 CI에게 먼저 묻는다.

---

### 37. 투명도 (1급 기능)

**목표:** 씬이 투명도를 저작할 수 있고, 화면에 실제로 비쳐 보인다.

36의 리드백 위에 선다. 카탈로그가 `opacity`/`alpha`/`transparent`/`transparency` 전부
`nothing owns this yet`이라 답했으므로 새 어휘 도입이 맞다. 다만 `blend`는 이미
`AnimationStateMachine`(블렌드 트리)이 갖고 있으니 머티리얼 쪽에서 그 단어는 쓰지 않는다 —
감사가 찾아낸 `setGravityScale` 류의 어휘 충돌을 새로 만들지 않기 위해서다.

**완료 조건:**
- [ ] `Material.opacity: f32` — `metallic`/`roughness`와 같은 결이다. `base_color`를
      Vec4로 바꾸지 않는다: `ReflectColor`는 `emissive`와 라이트 색이 공유하는데 거기
      알파는 의미가 없다. **씬이 `Material`을 리플렉트로 저작하는 곳은 하나도 없고**
      전부 Rust 구조체 리터럴 8곳이라, 필드 추가는 컴파일 에러로 잡히는 안전한 종류다
      (item 29를 물었던 함정이 여기엔 없다)
- [ ] `EntityDescriptor`에 `opacity` — 기본값 있는 plain serde라 기존 씬이 그대로 파싱된다
- [ ] 알파 블렌드 파이프라인 + 뒤에서 앞으로 정렬된 투과 패스 (깊이 테스트 켬, 깊이 쓰기 끔).
      순서는 불투명 → 스카이박스 → 투과 → 포스트프로세스
- [ ] 기존 게임 하나에서 실제로 비쳐 보이는 물체
- [ ] 36의 리드백으로 검증 — 반투명 물체 뒤의 색이 실제로 섞여 나오는가
- [ ] 알려진 한계 기록: 투명 물체는 그림자를 드리우지 않는다(섀도우 패스는 불투명 전용)

---

## 컴포넌트/op 감사 결과 (2026-08-05)

item 32의 카탈로그로 공개 컴포넌트 49개와 op 298개를 훑은 결과다. 항목 32는 **도구를 세웠을 뿐**이고
이 절이 그 도구의 첫 산출물이다. 각 발견은 실제로 코드를 읽어 확인했으며, 확인 방법을 함께 적는다.

### A. 운동 시스템이 두 벌 있다 — 가장 큰 발견

`bsengine-core` + `bsengine-app`에 **완결된 운동학 물리 스택**이 있고, `bsengine-physics`의 Rapier와
병렬로 돈다. 둘은 독 주석 밖에서 서로를 참조하지 않는다.

| 개념 | 운동학 쪽 | Rapier 쪽 |
|---|---|---|
| 속도 | `Velocity`, `AngularVelocity` (플러그인이 `Transform`에 적분) | `bsengine_*_velocity` op 18개 |
| 감쇠 | `Damping` + `DampingPlugin` (`Velocity`를 감쇠) | `RigidBody.linear_damping`/`angular_damping` |
| 중력 | `Gravity`(리소스) + `GravityScale`(컴포넌트) → `Velocity` | Rapier 내장 |
| 충격량 | `ExternalImpulse` + `Mass` | `bsengine_apply_impulse_at_point` |

확인: `crates/bsengine-app/src/{damping,gravity,angular_velocity,external_impulse}.rs`의 시스템들이
전부 살아 있고 `bsengine_core::Velocity`를 읽고 쓴다. `Velocity`의 독 주석이 경계를 명시한다
("For physics-driven motion use `bsengine-physics` instead").

**이건 버그가 아니라 결정이 필요한 설계 사실이다.** 그리고 **item 27(캐릭터 컨트롤러)이 어느 쪽에
설지 골라야 한다** — 카탈로그 없이 설계했다면 이 분기를 못 본 채 결정했을 것이다. 실제로 item 27
설계 중 `Velocity` 컴포넌트를 새로 만들자는 제안이 나왔고, 그건 이미 있는 이름이었다.

### B. 스크립팅 API와 컴포넌트가 같은 것을 다르게 부른다

| API가 부르는 이름 | 컴포넌트의 이름 | op 수 |
|---|---|---|
| `position` | `Transform.translation` (독 주석은 "Local-space position") | 12 |
| `euler` | `Transform.rotation` (`ReflectQuat`) | 11 |
| `ao` | `AmbientOcclusion` | 10 |

확인: `bsengine_get_position_x`가 트랜스폼 스냅샷의 `.0.x`를 읽는다(`ops.rs:2678`).

**이게 카탈로그 자신의 실효성을 깎는다.** `catalog --concept position`이 "컴포넌트 없음, op 12개"라고
답하는데, 개념에는 분명히 컴포넌트가 있다 — 다른 단어로. 즉 "이미 있나?"라는 질문에 **오답**을 준다.
어휘를 맞추거나(파급이 크다: 씬 파일·스크립트 호환), 카탈로그에 별칭 표를 두거나 둘 중 하나다.

### C. 구조가 완전히 같은 컴포넌트 쌍

`PhysicsInput`과 `PhysicsTransform`이 둘 다 `{ translation: ReflectVec3, rotation: ReflectQuat }`다.
한쪽은 물리로 들어가는 입력, 다른 쪽은 물리가 쓴 출력이라 방향만 다르다. 이름으로는 구분되지만
인스펙터에서 나란히 보면 어느 쪽이 어느 쪽인지 알 수 없다.

(`CustomShader`/`GltfAsset`이 둘 다 `{path: String}`, `Name`/`ScriptPath`가 둘 다 `(String)`인 것은
의미가 명확히 달라 문제로 보지 않는다.)

### D. `rotation`이 컴포넌트 4개에 흩어져 있다

`Transform`, `PhysicsInput`, `PhysicsTransform`, `Skybox`. A와 C의 결과이므로 별도 항목은 아니다.

### 후속 처리 결과 (2026-08-06)

**A(운동 시스템 두 벌)** → item 33으로 처리. 운동학 스택을 제거하고 Rapier로 일원화했다.

**B(어휘 불일치)** → **필드가 API의 이름을 따랐다.** `position`이 이겼다 — 스크립팅 API가 이미 그렇게
부르고 있었고 `Transform.translation`의 독 주석조차 "Local-space position"이라고 적혀 있었다.
`Transform`/`PhysicsTransform`/`PhysicsInput`/`TransformDescriptor`가 `position`으로 바뀌었고
`Transform::from_translation`은 `from_position`이 됐으며 모든 씬이 따라왔다.

glTF의 `NodeTransform`만 `translation`을 유지한다 — glTF 문서 자체의 어휘를 미러링하는 타입이라
바꾸면 임포터가 읽는 포맷과 어긋난다.

이제 `catalog --concept position`이 `Transform`·`PhysicsTransform`·`PhysicsInput`을 답한다.
전에는 "컴포넌트 없음, op 12개"였다.

**C(`PhysicsInput`/`PhysicsTransform` 동일 구조)** → 합치지 않았다. 하나는 우리가 쓰고 하나는
시뮬레이션이 쓰므로 합치면 읽기/쓰기가 충돌한다. 제거할 중복이 아니라 구분이 안 되는 것이
문제였으므로, 각자 첫 줄에 방향을 말하고 서로를 가리킨다.

**추가로 닫은 갭:** 씬이 `linear_damping`을 저작할 수 없었다. `RigidBody`에는 필드가 있는데
씬 포맷이 말할 방법이 없어 씬으로 만든 Dynamic 바디는 감쇠가 0이었다(item 27이 여기 걸려
내비 에이전트의 역가속으로 우회했다). `EntityDescriptor`가 기본값 필드로 노출한다 — 평범한
serde라 item 29를 문 리플렉션 컴포넌트와 달리 필드 추가가 아무것도 깨뜨리지 않는다.

**대규모 이름 변경을 안전하게 하는 법.** 넓게 치환하고 **컴파일러가 실수를 찾게** 했다.
`NodeTransform`에는 `position` 필드가 없고 glam의 `.translation()`은 메서드라, 잘못 바꾼 곳은
전부 빌드 에러가 된다 — 조용한 동작 변화가 아니라. 손으로 고칠 것은 셋만 남았다: 생성자 본문,
필드 초기화 축약형, 그리고 테스트 초기화. 마지막 것은 `cargo build --all`이 테스트를 컴파일하지
않아 `clippy --all-targets`에서야 드러났다.

### 남은 후속 항목 후보

우선순위 순. 아직 번호를 붙이지 않았다 — item 27의 설계 결과가 A의 답을 일부 정하기 때문이다.

1. **A의 결정**: 운동학 스택과 Rapier 스택의 관계를 정한다. 통합·명시적 분리·한쪽 제거 중 하나.
   item 27이 이 결정의 첫 소비자다.
2. **B의 결정**: 어휘를 맞출지, 카탈로그에 별칭을 둘지.
3. **C**: `PhysicsInput`/`PhysicsTransform`의 독 주석을 강화하거나 이름을 방향이 드러나게 바꾼다.
4. **축별 op 45개 축소**: R2가 신규 유입을 막고 있으므로 급하지 않다. A·B가 정해진 뒤에 한다.

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
| 23. `bevy_asset` adoption: `LoadedGltf`/`ShaderSource`/`AudioSourceAsset`/`TextureAsset` migrated to `Handle<T>`-based `AssetLoader`s (glTF, custom WGSL shaders, audio, textures incl. skybox), replacing 9 separate direct `std::fs::read` call sites; scene RON/scripting API/MCP tool surfaces stay path-string based, `AssetServer` converts at the boundary; sync-blocking initial load preserved (no behavior change); `games/tilt-run`'s 7 E2E replays all pass, `games/mini-arena`'s replay reliably fails on this environment (confirmed via 10 master + 14 HEAD runs) but proven — via reproduction on unmodified master — to be a pre-existing, wall-clock-timing-dependent flakiness unrelated to this migration; also surfaced local-only test stack overflows, later root-caused (libtest runs tests on spawned threads that never saw the `/STACK:` main-thread setting) and fixed on branch `fix/test-thread-stack-size` — see item 23's own notes above | 2026-07-31 | PR #TBD |
