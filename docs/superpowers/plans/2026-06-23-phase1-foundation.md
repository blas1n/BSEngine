# BSEngine Phase 1: Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cargo 워크스페이스 구성 후 `bsengine-core`, `bsengine-ecs`, `bsengine-app` 크레이트를 구현해 ECS 시스템이 동작하는 최소 엔진 루프 실행.

**Architecture:** `bevy_app::App`과 `bevy_app::Plugin`을 직접 활용. `bsengine-ecs`는 `bevy_ecs` re-export 래퍼, `bsengine-app`은 BSEngine 전용 초기화를 얹은 `bevy_app` 래퍼. `BsPlugin = bevy_app::Plugin`으로 시그니처(`&mut App`) 문제와 순환 의존성을 모두 회피.

**Tech Stack:** Rust 2021 edition, bevy_app 0.14, bevy_ecs 0.14, tracing 0.1, tracing-subscriber 0.3

---

## 파일 구조

```
bsengine/
├── Cargo.toml                          ← 워크스페이스 루트 (신규)
└── crates/
    ├── bsengine-core/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs                  ← 공통 타입, 로깅 re-export
    │       └── logging.rs              ← tracing 초기화
    ├── bsengine-ecs/
    │   ├── Cargo.toml
    │   └── src/
    │       └── lib.rs                  ← bevy_ecs prelude re-export
    └── bsengine-app/
        ├── Cargo.toml
        └── src/
            ├── lib.rs                  ← BsPlugin, App re-export, BSEngine 초기화
            └── main.rs                 ← 바이너리 진입점
```

---

## Task 1: 워크스페이스 초기화

**Files:**
- Create: `Cargo.toml`
- Create: `crates/bsengine-core/Cargo.toml`
- Create: `crates/bsengine-ecs/Cargo.toml`
- Create: `crates/bsengine-app/Cargo.toml`

- [ ] **Step 1: 워크스페이스 Cargo.toml 생성**

`Cargo.toml` (루트):
```toml
[workspace]
resolver = "2"
members = [
    "crates/bsengine-core",
    "crates/bsengine-ecs",
    "crates/bsengine-app",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["blas1n"]
license = "MIT"

[workspace.dependencies]
bsengine-core = { path = "crates/bsengine-core" }
bsengine-ecs  = { path = "crates/bsengine-ecs" }
bevy_app      = { version = "0.14", default-features = false }
bevy_ecs      = { version = "0.14", default-features = false }
tracing       = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
```

- [ ] **Step 2: 각 크레이트 Cargo.toml 생성**

`crates/bsengine-core/Cargo.toml`:
```toml
[package]
name = "bsengine-core"
version.workspace = true
edition.workspace = true

[dependencies]
tracing.workspace = true
tracing-subscriber.workspace = true
```

`crates/bsengine-ecs/Cargo.toml`:
```toml
[package]
name = "bsengine-ecs"
version.workspace = true
edition.workspace = true

[dependencies]
bsengine-core.workspace = true
bevy_ecs.workspace = true
```

`crates/bsengine-app/Cargo.toml`:
```toml
[package]
name = "bsengine-app"
version.workspace = true
edition.workspace = true

[[bin]]
name = "bsengine"
path = "src/main.rs"

[dependencies]
bsengine-core.workspace = true
bsengine-ecs.workspace = true
bevy_app.workspace = true
```

- [ ] **Step 3: 디렉토리와 빈 소스 파일 생성**

```bash
mkdir -p crates/bsengine-core/src crates/bsengine-ecs/src crates/bsengine-app/src
echo "// bsengine-core" > crates/bsengine-core/src/lib.rs
echo "// bsengine-ecs" > crates/bsengine-ecs/src/lib.rs
echo "// bsengine-app" > crates/bsengine-app/src/lib.rs
echo "fn main() {}" > crates/bsengine-app/src/main.rs
```

- [ ] **Step 4: 빌드 확인**

```bash
cargo build
```
Expected: 컴파일 성공.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/
git commit -m "build: initialize cargo workspace with core/ecs/app crates"
```

---

## Task 2: bsengine-core — 로깅

**Files:**
- Create: `crates/bsengine-core/src/logging.rs`
- Modify: `crates/bsengine-core/src/lib.rs`

- [ ] **Step 1: 테스트 작성**

`crates/bsengine-core/src/logging.rs`:
```rust
use std::sync::OnceLock;

static LOGGING_INIT: OnceLock<()> = OnceLock::new();

/// tracing 로깅 초기화. 중복 호출 안전.
pub fn init_logging() {
    LOGGING_INIT.get_or_init(|| {
        use tracing_subscriber::{fmt, EnvFilter};
        fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new("bsengine=debug,warn")),
            )
            .init();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_does_not_panic_on_first_call() {
        init_logging();
    }

    #[test]
    fn init_is_idempotent() {
        init_logging();
        init_logging(); // 두 번 호출해도 panic 없어야 함
    }
}
```

- [ ] **Step 2: 테스트 실패 확인**

```bash
cargo test -p bsengine-core
```
Expected: FAIL — `logging` 모듈 없음.

- [ ] **Step 3: lib.rs 구성**

`crates/bsengine-core/src/lib.rs`:
```rust
pub mod logging;
pub use logging::init_logging;

pub use tracing::{debug, error, info, trace, warn};
```

- [ ] **Step 4: 테스트 통과 확인**

```bash
cargo test -p bsengine-core
```
Expected: test result: ok. 2 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/bsengine-core/
git commit -m "feat(core): add tracing-based logging with idempotent init"
```

---

## Task 3: bsengine-ecs — bevy_ecs re-export

**Files:**
- Modify: `crates/bsengine-ecs/src/lib.rs`

- [ ] **Step 1: 테스트 작성**

`crates/bsengine-ecs/src/lib.rs`:
```rust
pub use bevy_ecs::prelude::{
    Bundle, Commands, Component, Entity, Event, EventReader, EventWriter,
    Query, Res, ResMut, Resource, With, Without, World,
};
pub use bevy_ecs::schedule::{IntoSystemConfigs, Schedule, ScheduleLabel, SystemSet};
pub use bevy_ecs::system::IntoSystem;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct Position { x: f32, y: f32 }

    #[derive(Resource, Default)]
    struct Counter(u32);

    #[test]
    fn component_and_resource_are_accessible() {
        let mut world = World::new();
        world.init_resource::<Counter>();

        let entity = world.spawn(Position { x: 1.0, y: 2.0 }).id();

        let pos = world.get::<Position>(entity).unwrap();
        assert_eq!(pos.x, 1.0);

        let counter = world.resource::<Counter>();
        assert_eq!(counter.0, 0);
    }

    #[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
    struct TestSchedule;

    #[test]
    fn system_runs_against_world() {
        fn increment(mut counter: ResMut<Counter>) {
            counter.0 += 1;
        }

        let mut world = World::new();
        world.init_resource::<Counter>();

        let mut schedule = Schedule::new(TestSchedule);
        schedule.add_systems(increment);
        schedule.run(&mut world);

        assert_eq!(world.resource::<Counter>().0, 1);
    }
}
```

- [ ] **Step 2: 테스트 실패 확인**

```bash
cargo test -p bsengine-ecs
```
Expected: FAIL — re-export 없어서 타입 미해결.

- [ ] **Step 3: 테스트 통과 확인**

```bash
cargo test -p bsengine-ecs
```
Expected: test result: ok. 2 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/bsengine-ecs/
git commit -m "feat(ecs): add bevy_ecs prelude re-exports with smoke tests"
```

---

## Task 4: bsengine-app — App 래퍼 + BsPlugin

**Files:**
- Create: `crates/bsengine-app/src/app.rs`
- Modify: `crates/bsengine-app/src/lib.rs`
- Modify: `crates/bsengine-app/src/main.rs`

`bevy_app::App`을 그대로 사용하고, BSEngine 전용 초기화(로깅, 기본 스케줄)를 추가하는 `BsApp::new()`를 제공. `BsPlugin`은 `bevy_app::Plugin`의 별칭.

- [ ] **Step 1: 테스트 작성**

`crates/bsengine-app/src/app.rs`:
```rust
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::Resource;
use bsengine_core::init_logging;

/// BsPlugin = bevy_app::Plugin.
/// build(&self, app: &mut App) 시그니처로 시스템/리소스 등록.
pub use bevy_app::Plugin as BsPlugin;
pub use bevy_app::{Startup, Update, PreUpdate, PostUpdate, Last};

/// BSEngine 전용 App 초기화.
pub fn new_app() -> App {
    init_logging();
    App::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Score(u32);

    struct ScorePlugin;
    impl Plugin for ScorePlugin {
        fn build(&self, app: &mut App) {
            app.init_resource::<Score>();
        }
    }

    #[test]
    fn plugin_registers_resource() {
        let mut app = new_app();
        app.add_plugins(ScorePlugin);
        // update 한 번 돌려 Startup 등 내부 스케줄 처리
        app.update();
        assert!(app.world().get_resource::<Score>().is_some());
    }

    #[test]
    fn system_runs_in_update_schedule() {
        fn increment(mut score: bevy_ecs::prelude::ResMut<Score>) {
            score.0 += 1;
        }

        struct IncrPlugin;
        impl Plugin for IncrPlugin {
            fn build(&self, app: &mut App) {
                app.init_resource::<Score>()
                   .add_systems(Update, increment);
            }
        }

        let mut app = new_app();
        app.add_plugins(IncrPlugin);

        app.update();
        app.update();
        app.update();

        assert_eq!(app.world().resource::<Score>().0, 3);
    }
}
```

- [ ] **Step 2: 테스트 실패 확인**

```bash
cargo test -p bsengine-app
```
Expected: FAIL — `app` 모듈 없음.

- [ ] **Step 3: lib.rs 구성**

`crates/bsengine-app/src/lib.rs`:
```rust
pub mod app;
pub use app::{new_app, BsPlugin, Last, PostUpdate, PreUpdate, Startup, Update};

// 편의를 위해 bevy_ecs 핵심 타입도 re-export
pub use bsengine_ecs::{
    Bundle, Commands, Component, Entity, Query, Res, ResMut, Resource, With, Without,
};
```

- [ ] **Step 4: 테스트 통과 확인**

```bash
cargo test -p bsengine-app
```
Expected: test result: ok. 2 passed.

- [ ] **Step 5: main.rs 구성**

`crates/bsengine-app/src/main.rs`:
```rust
use bsengine_app::new_app;
use bsengine_core::info;

fn main() {
    let mut app = new_app();
    info!("BSEngine starting");
    // Phase 2에서 WindowPlugin 추가 후 app.run() 으로 교체
    app.update();
    info!("BSEngine ready");
}
```

- [ ] **Step 6: 실행 확인**

```bash
cargo run -p bsengine-app
```
Expected: 로그 출력 후 종료. panic 없음.

- [ ] **Step 7: Commit**

```bash
git add crates/bsengine-app/
git commit -m "feat(app): add BSEngine App wrapper with BsPlugin and schedule support"
```

---

## Task 5: 통합 테스트 — 플러그인 간 시스템 순서

`BsPlugin` 두 개가 서로 순서 의존성을 선언하고 올바른 순서로 실행되는지 검증.

**Files:**
- Create: `crates/bsengine-app/tests/plugin_ordering.rs`

- [ ] **Step 1: 테스트 작성**

`crates/bsengine-app/tests/plugin_ordering.rs`:
```rust
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemSet;
use bsengine_app::{new_app, Update};

/// 실행 순서를 기록하는 리소스
#[derive(Resource, Default)]
struct ExecutionLog(Vec<&'static str>);

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum GameSet {
    Physics,
    Render,
}

fn physics_system(mut log: ResMut<ExecutionLog>) {
    log.0.push("physics");
}

fn render_system(mut log: ResMut<ExecutionLog>) {
    log.0.push("render");
}

struct PhysicsPlugin;
impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, physics_system.in_set(GameSet::Physics));
    }
}

struct RenderPlugin;
impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, GameSet::Render.after(GameSet::Physics))
           .add_systems(Update, render_system.in_set(GameSet::Render));
    }
}

#[test]
fn render_runs_after_physics() {
    let mut app = new_app();
    app.init_resource::<ExecutionLog>()
       .add_plugins((PhysicsPlugin, RenderPlugin));

    app.update();

    let log = app.world().resource::<ExecutionLog>();
    assert_eq!(log.0, vec!["physics", "render"]);
}

#[test]
fn order_holds_across_multiple_frames() {
    let mut app = new_app();
    app.init_resource::<ExecutionLog>()
       .add_plugins((PhysicsPlugin, RenderPlugin));

    app.update();
    app.update();
    app.update();

    let log = app.world().resource::<ExecutionLog>();
    assert_eq!(log.0, vec![
        "physics", "render",
        "physics", "render",
        "physics", "render",
    ]);
}
```

- [ ] **Step 2: 테스트 실패 확인**

```bash
cargo test -p bsengine-app --test plugin_ordering
```
Expected: FAIL — 컴파일 에러 또는 순서 불일치.

- [ ] **Step 3: 테스트 통과 확인**

bevy_ecs 스케줄러가 `after()` 선언을 처리하므로 별도 구현 없이 통과해야 함.

```bash
cargo test -p bsengine-app --test plugin_ordering
```
Expected: test result: ok. 2 passed.

- [ ] **Step 4: 전체 테스트 실행**

```bash
cargo test
```
Expected: 모든 크레이트 테스트 PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/bsengine-app/tests/
git commit -m "test: verify plugin system ordering via bevy_ecs DAG scheduler"
```

---

## 완료 기준

- [ ] `cargo build` 경고 없이 성공
- [ ] `cargo test` 전체 PASS (core 2 + ecs 2 + app 2 + integration 2 = 8개)
- [ ] `cargo run -p bsengine-app` 실행 후 정상 종료
- [ ] `plugin_ordering` 테스트: render가 항상 physics 이후 실행됨 확인

## 다음 페이즈

Phase 2 — Platform: `bsengine-window` (winit), `bsengine-input`
