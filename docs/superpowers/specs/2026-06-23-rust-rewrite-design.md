# BSEngine Rust Rewrite — Design Spec

## 배경

BSEngine은 Unity 컴포넌트 시스템 + Unreal RHI 추상화를 결합한 C++17 게임 엔진 프로젝트.
2022년 2월 렌더링 레이어가 stub 상태에서 중단됨.

**재시작 결정 사항:**
- 언어를 Rust로 전환 (메모리 안전성, Cargo 생태계, 순수 ECS와의 궁합)
- 검증된 크레이트(`bevy_ecs`, `wgpu`, `winit`) 기반으로 구축
- AI Native 게임 개발 경험을 1급 목표로 설정

---

## 설계 철학

| 목표 | 방법 |
|------|------|
| 게임을 만들 수 있는 엔진 | 인프라보다 AI native 레이어에 집중 |
| AI로 게임 만들기 편함 (Goal B) | MCP 서버 + 타입 안전 TypeScript API |
| AI friendly 엔진 개발 (Method A) | 명확한 크레이트 경계, bevy_ecs 패턴 |
| 메모리 안전성 | Rust borrow checker + bevy_ecs 스케줄러 |
| 그래픽 API 교체 가능 | RHI trait 추상화, wgpu가 기본 구현 |

---

## 크레이트 구조

```
bsengine/
├── Cargo.toml                  # 워크스페이스
├── project.toml                # 게임 프로젝트 설정
└── crates/
    ├── bsengine-core           # 기반 타입, 로깅, 이벤트
    ├── bsengine-ecs            # bevy_ecs 래퍼 (ECS API)
    ├── bsengine-scene          # RON 씬 직렬화/역직렬화
    ├── bsengine-asset          # GLTF/텍스처/오디오 에셋 로딩
    ├── bsengine-rhi            # RHI trait 정의 (교체 인터페이스)
    ├── bsengine-rhi-wgpu       # wgpu 구현체 (기본 렌더 백엔드)
    ├── bsengine-render         # 씬 → 드로우 콜 변환
    ├── bsengine-window         # winit 래퍼
    ├── bsengine-input          # 입력 ECS 시스템
    ├── bsengine-scripting      # Deno/TypeScript 임베딩
    ├── bsengine-plugin         # 플러그인 로딩 (plugin.toml)
    ├── bsengine-mcp            # MCP 서버 (AI Native 핵심)
    └── bsengine-app            # 진입점, App 빌더, DI 컨테이너
```

### C++ 모듈 → Rust 크레이트 대응

| C++ 모듈 | Rust 크레이트 | 비고 |
|----------|--------------|------|
| Core | `bsengine-core` + `tracing` | 이벤트는 bevy_ecs 내장 |
| Framework (Component/Manager) | `bevy_ecs` | 패턴 자체를 대체 |
| Engine (Entity/Scene/ECS) | `bsengine-ecs` + `bsengine-scene` | |
| Window | `bsengine-window` (winit) | |
| Input | `bsengine-input` | |
| Thread | `tokio` / `rayon` | |
| Plugin | `bsengine-plugin` + `plugin.toml` | |
| Render | `bsengine-render` | |
| RHI | `bsengine-rhi` + `bsengine-rhi-wgpu` | |
| Launch | `bsengine-app` | |
| _(신규)_ | `bsengine-scripting` | TypeScript 게임 로직 |
| _(신규)_ | `bsengine-mcp` | AI Native 도구 |

---

## ECS 아키텍처

`bevy_ecs`를 standalone으로 사용. `bsengine-ecs`는 API 추상화 래퍼로, 향후 ECS 구현체 교체 시 게임 코드 영향 최소화.

```rust
// bsengine-ecs가 노출하는 API (bevy_ecs 래핑)
pub use bevy_ecs::prelude::{Component, Resource, Query, Res, ResMut, Commands};
pub use bevy_ecs::schedule::{IntoSystemConfigs, SystemSet};

pub struct World(bevy_ecs::world::World);
pub struct App(bevy_ecs::app::App);
```

### 시스템 실행 순서 — DAG 기반 위상 정렬

각 플러그인이 자신의 실행 순서 의존성을 선언하면 bevy_ecs 스케줄러가 DAG를 빌드해 위상 정렬 실행. 의존성 없는 시스템은 자동 병렬 실행.

```rust
impl BsPlugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app
            .configure_sets(Update,
                PhysicsSet::Simulate.before(PhysicsSet::SyncTransforms)
            )
            .add_systems(Update,
                simulate.in_set(PhysicsSet::Simulate)
            )
            .add_systems(Update,
                sync_transforms.in_set(PhysicsSet::SyncTransforms)
            );
    }
}

impl BsPlugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        // Physics 완료 이후 실행됨을 선언
        app.add_systems(PostUpdate,
            render.after(PhysicsSet::SyncTransforms)
        );
    }
}
```

### DI — VContainer 대응

bevy_ecs의 `Resource`가 VContainer와 동일한 역할. 플러그인이 서비스를 등록하면 시스템에 자동 주입.

```rust
// 서비스 등록 (Plugin::build에서)
app.insert_resource(AudioManager::new());

// 시스템에서 자동 주입
fn audio_system(audio: Res<AudioManager>, query: Query<&AudioSource>) {
    // ...
}
```

---

## 플러그인 시스템 (Unreal 스타일)

### 디스크립터 파일

```toml
# plugins/my-physics/plugin.toml
[plugin]
name = "MyPhysics"
version = "0.1.0"

[plugin.dependencies]
bsengine-core = "*"
```

```toml
# project.toml — 프로젝트 레벨 활성화
[project]
name = "MyGame"

[plugins]
enabled = ["MyPhysics", "MyAudio", "bsengine-rhi-wgpu"]
```

### 플러그인 트레이트

```rust
// bsengine-app (코어)
pub trait BsPlugin {
    fn build(&self, app: &mut App);
}

// bsengine-mcp (선택적)
pub trait McpPlugin: BsPlugin {
    fn register_mcp(&self, registry: &mut McpRegistry);
}
```

엔진 코어(`bsengine-app`)는 MCP를 모름. MCP 지원이 필요한 플러그인만 `McpPlugin`을 추가 구현. Shipping 빌드에서 `bsengine-mcp` 크레이트 제외 가능.

> **플러그인 컴파일 모델**: 플러그인은 Rust 크레이트이므로 컴파일 타임에 `Cargo.toml`에 선언되어야 함. `project.toml`은 런타임 활성화 여부를 제어 — 비활성화 플러그인은 컴파일되지만 `build()`가 호출되지 않음. Unreal 플러그인 시스템과 동일한 모델.

---

## 데이터 포맷

| 역할 | 포맷 | 이유 |
|------|------|------|
| 씬 (엔티티/컴포넌트 배치) | RON | bevy_ecs reflection 내장, 중첩에 간결 |
| 3D 에셋 (메시/애니메이션/머티리얼) | GLTF | 산업 표준, Blender 직수출 |
| 텍스처 / 오디오 | PNG, WAV 등 | 표준 포맷 |
| 엔진/플러그인 설정 | TOML | 평탄한 설정에 적합 |
| 게임 로직 | TypeScript | Deno 임베딩 |

### RON 씬 예시

```ron
Scene(
    entities: [
        (
            name: "Player",
            components: [
                Transform(position: (0.0, 1.0, 0.0), rotation: (0.0, 0.0, 0.0, 1.0)),
                Health(max_hp: 100, current_hp: 100),
                MeshRenderer(mesh: "player.glb", material: "player_mat"),
            ],
        ),
        (
            name: "Camera",
            components: [
                Transform(position: (0.0, 5.0, -10.0)),
                Camera(fov: 60.0),
            ],
        ),
    ],
)
```

---

## 그래픽 API (RHI)

`bsengine-rhi`가 교체 가능한 trait 인터페이스를 정의. `bsengine-rhi-wgpu`가 기본 구현.
wgpu는 내부적으로 Metal(macOS), Vulkan, DX12를 추상화.

```rust
// bsengine-rhi
pub trait RHI: Send + Sync {
    fn create_mesh(&self) -> Box<dyn RHIMesh>;
    fn create_shader(&self, src: &str) -> Box<dyn RHIShader>;
    fn create_texture(&self) -> Box<dyn RHITexture>;
}

// bsengine-rhi-wgpu
pub struct WgpuRHI { device: wgpu::Device, queue: wgpu::Queue }
impl RHI for WgpuRHI { ... }
```

플러그인으로 RHI 주입:
```rust
App::new()
    .add_plugin(WgpuRHIPlugin)  // Resource로 RHI 등록
    .run()
```

| 플랫폼 | wgpu 백엔드 | 비고 |
|--------|------------|------|
| macOS | Metal | wgpu가 자동 선택 |
| Windows | DX12 / Vulkan | wgpu가 자동 선택 |
| Linux | Vulkan | |

향후 커스텀 백엔드 필요 시 `RHI` trait 구현체 추가만으로 교체 가능.

---

## TypeScript 스크립팅

`deno_core`로 Deno 런타임 임베딩. 게임 로직을 TypeScript로 작성.

```typescript
// scripts/player.ts
import { Entity, Transform, Health } from "@bsengine/api";

export function onUpdate(entity: Entity, dt: number) {
    const transform = entity.getComponent(Transform);
    const health = entity.getComponent(Health);

    if (health.current <= 0) {
        entity.destroy();
    }
}
```

`@bsengine/api` TypeScript 타입 정의가 AI 코드 생성의 가이드 역할. AI가 `getComponent()` 호출 시 어떤 필드가 있는지 타입으로 검증 가능.

---

## AI Native — MCP 서버

### 아키텍처

```
bsengine-mcp
  └─ McpRegistry (도구 레지스트리)
       ├─ 엔진 코어 도구 (씬, 에셋, 스키마)
       ├─ 스크립팅 도구 (컴파일, 린트)
       └─ 플러그인 도구 (McpPlugin 구현체에서 등록)
```

### 핵심 도구 목록

**씬 조작**
```
create_entity(name, parent?)          엔티티 생성 → main.ron 업데이트
add_component(entity, type, fields)   컴포넌트 추가
remove_component(entity, type)        컴포넌트 제거
query_entities(filter?)               씬 상태 조회
```

**스키마 — AI 생성 정확도의 핵심**
```
get_schema(component_type)    컴포넌트 필드 정의 반환 (타입, 기본값, 설명)
list_components()             등록된 모든 컴포넌트 목록
get_script_types()            TypeScript API 타입 정의 반환
```

**스크립팅**
```
compile_script(path)          TypeScript 컴파일 → 에러 목록 { line, col, message }
lint_script(path)             Deno lint 실행 → 경고 목록
check_scene_scripts()         씬에 연결된 모든 스크립트 일괄 검사
run_script(path, args?)       스크립트 실행 결과 반환
```

**플러그인**
```
list_plugins()                활성/비활성 플러그인 목록
enable_plugin(name)           project.toml 업데이트 후 재로드
```

### 플러그인 MCP 등록

```rust
// 플러그인이 McpPlugin 구현 시 도구 자동 노출
impl McpPlugin for PhysicsPlugin {
    fn register_mcp(&self, registry: &mut McpRegistry) {
        registry.register(mcp_tool! {
            name: "set_gravity",
            description: "Set global gravity vector",
            params: { x: f32, y: f32, z: f32 },
            handler: |params, world| {
                world.resource_mut::<PhysicsWorld>()
                    .set_gravity(Vec3::new(params.x, params.y, params.z));
                Ok(json!({ "ok": true }))
            }
        });
    }
}
```

### 실제 AI 워크플로우

```
사용자: "체력 100인 플레이어 엔티티 만들어줘"

Claude Code:
  1. get_schema("Transform") → { position: Vec3, rotation: Quat, scale: Vec3 }
  2. get_schema("Health")    → { max_hp: u32, current_hp: u32 }
  3. create_entity("Player")
  4. add_component("Player", "Transform", { position: [0, 1, 0] })
  5. add_component("Player", "Health", { max_hp: 100, current_hp: 100 })
  → main.ron 자동 업데이트, 에디터에 반영
```

---

## 진입점 흐름

```rust
fn main() {
    App::new()
        // RHI 선택 (플러그인으로 주입)
        .add_plugin(WgpuRHIPlugin)
        // 엔진 코어 플러그인
        .add_plugin(WindowPlugin::default())
        .add_plugin(InputPlugin::default())
        .add_plugin(ScriptingPlugin::default())
        // 게임 플러그인 (project.toml에서 로드)
        .load_plugins_from("project.toml")
        // 씬 로드
        .load_scene("scenes/main.ron")
        // 개발 모드에서만 MCP 서버 활성화
        .add_plugin_if(cfg!(debug_assertions), McpServerPlugin::default())
        .run();
}
```

---

## 범위 외 (이번 스펙에서 제외)

- 에디터 UI (추후 별도 스펙)
- 오디오 시스템 (플러그인으로 추가 예정)
- 물리 엔진 (플러그인으로 추가 예정)
- 네트워크 (추후)
- 모바일 플랫폼 (추후)
