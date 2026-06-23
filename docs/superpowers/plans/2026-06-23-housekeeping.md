# BSEngine Housekeeping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 4년 만에 프로젝트를 재개하기 위한 기반 정비 — 코드 버그 수정, 빌드 시스템 복구, 문서화, OpenGL 의존성 추가.

**Architecture:** 기존 Render/RHI 분리 구조 유지. RHI는 그래픽 API 추상화 레이어, Render는 씬 그래프 → 드로우 콜 변환 레이어. OpenGL 구현체를 첫 번째 RHI로 구현 (이번 계획에서는 의존성 추가까지만). 추상화 덕분에 이후 Vulkan 등 추가 가능.

**Tech Stack:** C++17, CMake 3.12+, vcpkg, glad (OpenGL function loader, system OpenGL 불필요 — macOS/Windows 모두 시스템 제공)

---

## Task 1: RHI.h 버그 수정

`Source/Runtime/RHI/include/RHI.h`의 복붙 실수 — CreateShader/CreateTexture가 `RHIMesh*`를 반환.

**Files:**
- Modify: `Source/Runtime/RHI/include/RHI.h`

- [ ] **Step 1: 파일 열어 현재 상태 확인**

현재 내용:
```cpp
virtual RHIMesh* CreateMesh()    { return nullptr; }
virtual RHIMesh* CreateShader()  { return nullptr; }  // 버그: RHIShader*여야 함
virtual RHIMesh* CreateTexture() { return nullptr; }  // 버그: RHITexture*여야 함
```

- [ ] **Step 2: 반환 타입 수정**

`Source/Runtime/RHI/include/RHI.h`를 다음으로 교체:
```cpp
#pragma once

#include "Core.h"

class RHIMesh;
class RHIShader;
class RHITexture;

class RHI_API RHI
{
public:
    virtual ~RHI() = default;

    virtual RHIMesh*    CreateMesh()    { return nullptr; }
    virtual RHIShader*  CreateShader()  { return nullptr; }
    virtual RHITexture* CreateTexture() { return nullptr; }
};
```

- [ ] **Step 3: Commit**

```bash
git add Source/Runtime/RHI/include/RHI.h
git commit -m "fix: correct CreateShader/CreateTexture return types in RHI"
```

---

## Task 2: Texture.h 버그 수정

`Source/Runtime/RHI/include/Texture.h`에 세 가지 문제:
1. `enum class TextureCreateFlags` 끝에 `;` 누락
2. 생성자 파라미터 `iPixelFormat` → 존재하지 않는 타입 (올바른 타입: `PixelFormat`)
3. 생성자에 `public:` 접근 지정자 누락

**Files:**
- Modify: `Source/Runtime/RHI/include/Texture.h`

- [ ] **Step 1: 수정 적용**

`Source/Runtime/RHI/include/Texture.h`를 다음으로 교체:
```cpp
#pragma once

#include "BSMath/Vector.h"
#include "RHIDef.h"

using BSBase::uint32;

enum class TextureCreateFlags : uint32
{
    None = 0,
    RenderTargetable              = 1 << 0,
    ResolveTargetable             = 1 << 1,
    DepthStencilTargetable        = 1 << 2,
    ShaderResource                = 1 << 3,
    SRGB                          = 1 << 4,
    CPUWritable                   = 1 << 5,
    NoTiling                      = 1 << 6,
    VideoDecode                   = 1 << 7,
    Dynamic                       = 1 << 8,
    InputAttachmentRead           = 1 << 9,
    Foveation                     = 1 << 10,
    Memoryless                    = 1 << 11,
    GenerateMipCapable            = 1 << 12,
    FastVRAMPartialAlloc          = 1 << 13,
    DisableSRVCreation            = 1 << 14,
    DisableDCC                    = 1 << 15,
    UAV                           = 1 << 16,
    Presentable                   = 1 << 17,
    CPUReadback                   = 1 << 18,
    OfflineProcessed              = 1 << 19,
    FastVRAM                      = 1 << 20,
    HideInVisualizeTexture        = 1 << 21,
    Virtual                       = 1 << 22,
    TargetArraySlicesIndependently = 1 << 23,
    Shared                        = 1 << 24,
    NoFastClear                   = 1 << 25,
    DepthStencilResolveTarget     = 1 << 26,
    Streamable                    = 1 << 27,
    NoFastClearFinalize           = 1 << 28,
    AFRManual                     = 1 << 29,
    ReduceMemoryWithTilingMode    = 1 << 30,
    Transient                     = 1u << 31,
};

class RHI_API RHITexture
{
public:
    RHITexture(uint32 inNumMips, uint32 inNumSamples, PixelFormat inFormat,
               TextureCreateFlags inFlags, const ClearValue& inClearValue)
        : clearValue(inClearValue)
        , numMips(inNumMips)
        , numSamples(inNumSamples)
        , flags(inFlags)
        , format(inFormat) {}

    virtual ~RHITexture() = default;

    virtual void* GetNativeResource() const noexcept { return nullptr; }
    virtual void* GetShaderResourceView() const noexcept { return nullptr; }
    virtual IntVector GetSize() const noexcept = 0;

    ClearValue         GetClearValue()  const noexcept { return clearValue; }
    uint32             GetNumMipmaps()  const noexcept { return numMips; }
    uint32             GetNumSamples()  const noexcept { return numSamples; }
    TextureCreateFlags GetFlags()       const noexcept { return flags; }
    PixelFormat        GetFormat()      const noexcept { return format; }

private:
    ClearValue         clearValue;
    uint32             numMips;
    uint32             numSamples;
    TextureCreateFlags flags;
    PixelFormat        format;
};
```

- [ ] **Step 2: Commit**

```bash
git add Source/Runtime/RHI/include/Texture.h
git commit -m "fix: enum class semicolon, PixelFormat type, public access in RHITexture"
```

---

## Task 3: 누락된 CMakeLists.txt 생성 (RHI)

`Source/Runtime/CMakeLists.txt`에 `add_subdirectory(RHI)`가 있지만 파일이 없어 빌드가 깨짐.

**Files:**
- Create: `Source/Runtime/RHI/CMakeLists.txt`

- [ ] **Step 1: 기존 모듈 패턴 확인**

Core의 CMakeLists.txt 패턴:
```cmake
register_library()
target_link_libraries(Core PUBLIC BSBase::BSBase BSMath::BSMath fmt::fmt-header-only
                           PRIVATE nlohmann_json::nlohmann_json spdlog::spdlog utf8cpp)
```

RHI는 Core(Assertion 포함)와 BSMath(Color, Vector)가 필요.

- [ ] **Step 2: 파일 생성**

`Source/Runtime/RHI/CMakeLists.txt`:
```cmake
# Source/Runtime/RHI

register_library()

target_link_libraries(RHI PUBLIC Core BSMath::BSMath)
```

- [ ] **Step 3: Commit**

```bash
git add Source/Runtime/RHI/CMakeLists.txt
git commit -m "build: add missing RHI CMakeLists.txt"
```

---

## Task 4: 누락된 CMakeLists.txt 생성 (Render)

**Files:**
- Create: `Source/Runtime/Render/CMakeLists.txt`

- [ ] **Step 1: 의존성 파악**

`RenderManager.h`가 `Manager.h`(Framework)와 `RHI`를 사용하고, `GetRHI()`를 public으로 노출하므로 RHI는 PUBLIC 링크.

- [ ] **Step 2: 파일 생성**

`Source/Runtime/Render/CMakeLists.txt`:
```cmake
# Source/Runtime/Render

register_library()

target_link_libraries(Render PUBLIC RHI Framework)
```

- [ ] **Step 3: Commit**

```bash
git add Source/Runtime/Render/CMakeLists.txt
git commit -m "build: add missing Render CMakeLists.txt"
```

---

## Task 5: OpenGL 의존성 추가

OpenGL 자체는 시스템 제공(`opengl32` on Windows, `OpenGL.framework` on macOS)이므로 별도 SDK 불필요. 단, OpenGL 함수 포인터 로더인 `glad`가 필요.

**Files:**
- Modify: `vcpkg.json`

- [ ] **Step 1: 의존성 추가**

`vcpkg.json`을 다음으로 교체:
```json
{
  "name": "bsengine",
  "version-string": "0.1.0",
  "description": "Game engine combining Unity component model with Unreal RHI abstraction",
  "homepage": "https://github.com/blas1n/BSEngine",
  "license": "MIT",
  "dependencies": [
    "bsbase",
    "bsmath",
    "fmt",
    "gtest",
    "nlohmann-json",
    "spdlog",
    "utfcpp",
    "glad"
  ]
}
```

- [ ] **Step 2: 플랫폼별 OpenGL 링크 참고**

OpenGL 자체는 CMake의 `find_package(OpenGL REQUIRED)`로 찾을 수 있음. OpenGL RHI 모듈 CMakeLists.txt에서:
```cmake
find_package(OpenGL REQUIRED)
find_package(glad CONFIG REQUIRED)
target_link_libraries(OpenGLRHI PRIVATE RHI OpenGL::GL glad::glad)
```
macOS에서는 추가로 `-framework Cocoa -framework IOKit`가 필요할 수 있음 (Window 모듈에서 처리 예정).

- [ ] **Step 3: Commit**

```bash
git add vcpkg.json
git commit -m "build: add glad dependency for upcoming OpenGL RHI"
```

---

## Task 6: README.md 전면 재작성

**Files:**
- Modify: `README.md`

- [ ] **Step 1: 재작성**

`README.md`를 다음으로 교체:
```markdown
# BSEngine

[![BSEngine Test](https://github.com/blas1n/BSEngine/workflows/BSEngine%20Test/badge.svg)](https://github.com/blas1n/BSEngine/actions)
[![Codacy Badge](https://app.codacy.com/project/badge/Grade/1fc8e08bafaa476986f2197e56cd4137)](https://www.codacy.com/gh/blas1n/BSEngine/dashboard)

C++17 게임 엔진. Unity의 컴포넌트 시스템과 Unreal의 RHI 추상화를 결합한 구조.

## 특징

- **Unity-style Component System** — `Entity::AddComponent<T>()`로 컴포넌트 동적 부착, 런타임 리플렉션 없이 타입 기반 팩토리로 구현
- **Unreal-style RHI Abstraction** — Render/RHI 레이어 분리로 OpenGL, Vulkan 등 다수 그래픽 API를 추상화
- **ECS** — Entity, Scene, SceneManager 기반의 씬 관리, 비동기 씬 로딩 지원
- **Plugin 시스템** — 그래픽 API 구현체를 DLL 플러그인으로 로드

## 모듈 구조

```
Core        → 로깅, JSON, Delegate/Event, 타이머
Framework   → 컴포넌트/매니저 베이스
Engine      → Entity, Scene, Transform, SceneManager
Window      → 플랫폼 윈도우 관리
Input       → 키보드/마우스/축 입력
Thread      → 스레드 풀, 비동기 태스크
Plugin      → 플러그인 로딩 (WIP)
Render      → 씬 → 드로우 콜 변환 (WIP)
RHI         → 그래픽 API 추상화 레이어 (WIP)
```

## 빌드

**요구사항**
- CMake 3.12+
- vcpkg
- C++17 컴파일러 (MSVC / Clang)
- OpenGL은 시스템 제공 (별도 SDK 불필요)

**빌드**
```bash
cmake -B build -S . -DCMAKE_TOOLCHAIN_FILE=<vcpkg-root>/scripts/buildsystems/vcpkg.cmake
cmake --build build
```

## 라이선스

MIT
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: rewrite README with architecture overview and build instructions"
```

---

## Task 7: ARCHITECTURE.md 작성

**Files:**
- Create: `docs/ARCHITECTURE.md`

- [ ] **Step 1: 파일 생성**

`docs/ARCHITECTURE.md`:
```markdown
# BSEngine Architecture

## 설계 철학

Unity의 사용성과 Unreal의 엔지니어링을 결합하는 것이 목표.

- **컴포넌트 시스템** — Unity처럼 `AddComponent<T>()` / `GetComponent<T>()`로 런타임 조합
- **Render/RHI 분리** — Unreal처럼 고수준 Render 레이어와 저수준 RHI 레이어를 분리해 그래픽 API를 플러그인으로 교체 가능하게 설계

## 레이어 구조

```
┌─────────────────────────────────┐
│          Game Logic             │  사용자 코드
├─────────────────────────────────┤
│  Engine (Entity / Scene / ECS)  │  씬 관리, 엔티티 수명주기
├──────────────┬──────────────────┤
│    Render    │     Input        │  씬 → 드로우 콜 / 입력 처리
├──────────────┤                  │
│     RHI      │                  │  그래픽 API 추상화
├──────────────┴──────────────────┤
│  Window / Thread / Plugin       │  플랫폼 레이어
├─────────────────────────────────┤
│  Core / Framework               │  유틸리티, 베이스 클래스
└─────────────────────────────────┘
```

## 컴포넌트 시스템

`REGISTER_COMPONENT(MyComp)` 매크로를 `.cpp` 파일에 선언하면 static initializer가 이름 → 팩토리 함수를 전역 레지스트리에 등록. 이후 `Entity::AddComponent<MyComp>()`가 타입 이름을 키로 팩토리를 호출.

```cpp
// MyComponent.cpp
REGISTER_COMPONENT(MyComponent)
```

```cpp
// 사용
Entity* entity = scene->AddEntity("Player");
auto* comp = entity->AddComponent<MyComponent>();
```

## Render / RHI 분리

RHI는 순수 가상 인터페이스. 구체 구현체(OpenGL, Vulkan 등)가 Plugin DLL 형태로 제공되고, 엔진 시작 시 `RenderManager::SetRHI()`로 주입.

```
엔진 시작
  └→ PluginManager가 OpenGLRHI.dll 로드
       └→ DLL이 RHI 구현체를 생성해 RenderManager::SetRHI() 호출
            └→ RenderManager::Update()에서 RHI를 통해 드로우 콜 발행
```

## 씬 관리

- `SceneManager`가 현재 씬을 더블버퍼링으로 관리 (Update 중 씬 전환 안전성 확보)
- `Scene::Load()`는 JSON 파일에서 Entity/Component를 비동기로 복원
- Entity 이름 변경 시 `onChangedName` 이벤트로 Scene의 내부 맵을 자동 갱신

## 그래픽 API 타겟

| 플랫폼  | API     | 비고                              |
|---------|---------|-----------------------------------|
| macOS   | OpenGL  | 4.1 Core Profile (시스템 제공)    |
| Windows | OpenGL  | 4.6 (시스템 제공, glad로 로드)    |
| -       | Vulkan  | 향후 추가 가능 (추상화로 용이)    |
```

- [ ] **Step 2: Commit**

```bash
git add docs/ARCHITECTURE.md
git commit -m "docs: add architecture overview document"
```

---

## Task 8: GitHub Issues 정비

`gh` CLI가 없으면 https://github.com/blas1n/BSEngine/issues 에서 직접 수행.

**현재 이슈 상태:**
| # | 제목 | 조치 |
|---|------|------|
| 151 | Create render module | 유지 (Render 구현 시 사용) |
| 103 | Documentation | **Close** — 이번 작업으로 완료 |
| 95  | Create allocator | 유지 (Core 고도화 시 사용) |
| 15  | Implement Audio Manager | 유지 (장기 계획) |

- [ ] **Step 1: OpenGL RHI 이슈 생성**

새 이슈 생성:
- **Title:** `Implement OpenGL RHI`
- **Body:**
```
## 목표
RHI 추상 인터페이스의 첫 번째 구현체로 OpenGL 작성.
macOS (OpenGL 4.1), Windows (OpenGL 4.6) 지원.

## 구현 범위
- [ ] OpenGLRHI 클래스 (RHI 상속)
- [ ] OpenGLRHIMesh (RHIMesh 상속) — VAO/VBO/EBO
- [ ] OpenGLRHIShader (RHIShader 상속) — GLSL 컴파일/링크
- [ ] OpenGLRHITexture (RHITexture 상속) — glTexImage2D
- [ ] Plugin DLL 진입점 → RenderManager::SetRHI() 호출
- [ ] OpenGL Context 생성 (WGL on Windows, NSOpenGL on macOS)
- [ ] 기본 렌더 루프 (clear → draw → swap)

## 의존성
glad (vcpkg), OpenGL은 시스템 제공
```

- [ ] **Step 2: Issue #103 닫기**

GitHub 웹에서 #103 "Documentation"을 Close with comment:
```
README 전면 재작성 및 docs/ARCHITECTURE.md 추가로 완료.
```

- [ ] **Step 3: 남은 이슈에 설명 추가**

각 이슈에 현재 상태 코멘트 추가 (선택):
- #151 — "OpenGL RHI 이슈(#NEW)가 선행 작업. RHI 완료 후 진행."
- #95 — "커스텀 얼로케이터. 렌더링 구현 안정화 후 검토."

---

## 완료 기준

- [ ] `cmake --build build` 가 경고 없이 성공
- [ ] `Core` / `Engine` / `Render` / `RHI` 모듈 모두 빌드됨
- [ ] README와 ARCHITECTURE.md가 현재 코드 구조와 일치
- [ ] GitHub에 OpenGL RHI 이슈 존재, Documentation 이슈 닫힘
