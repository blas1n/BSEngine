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
| 17. `attach_physics_body`/`detach_physics_body` MCP tools; `EntityInfo`/`build_entity_descriptors` extended so physics bodies survive `save_scene` (were always dropped); fixed `EditorCommand::LoadScene`'s own separate scene-loading path, which never spawned `PhysicsBodyDesc` from loaded rigidbody/collider RON at all | 2026-07-30 | branch `feat/physics-body-mcp` (PR #TBD) |
