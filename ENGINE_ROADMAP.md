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
