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

### 8. Runtime Inspector / Editor ✅

**목표:** 런타임에 엔티티/컴포넌트를 inspect 및 수정 가능

**완료 조건:**
- [x] 엔티티 목록 패널
- [x] 컴포넌트 프로퍼티 인라인 편집
- [x] 플레이 중 값 변경이 즉시 반영
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
| 8. Runtime Inspector / Editor | 2026-07-06 | [#1669](https://github.com/blas1n/BSEngine/pull/1669) |
