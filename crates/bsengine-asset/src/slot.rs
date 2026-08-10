//! One asset request, and what became of it.
//!
//! Five places in this engine spelled this state machine out by hand: custom
//! shaders, the skybox and the texture cache in `bsengine-render`, scripts in
//! `bsengine-scripting`, and glTF in `bsengine-gltf`. Each carried its own copy
//! of the same paragraph about why a failed load must never be re-requested.
//! That paragraph lives once now, on [`AssetSlot::GaveUp`].

use bevy_asset::{Asset, AssetServer, Assets, Handle};

/// One asset request, and what became of it.
///
/// Owns the *load* lifecycle only. How a slot is found — by path in a map, by
/// entity as a component, or as one global slot — is the caller's business, and
/// so is everything that happens after the bytes arrive.
///
/// # Why `Ready` does not mean "usable"
///
/// Compiling a shader and uploading a texture need a GPU that loading does not,
/// and both can happen frames after the load itself finishes. A slot that
/// waited for them would sit in `Loading` while a surface was missing — and
/// `bsengine-render`'s `rebuild_modified_shaders` only visits paths that have
/// left `Loading`, so folding the GPU step in here would switch shader hot
/// reload off for exactly as long as the window took to appear.
///
/// So `Ready` means the asset arrived, nothing more. Whether it compiled,
/// uploaded, ran or spawned is the caller's to track, because only the caller
/// knows what those words mean.
pub enum AssetSlot<A: Asset> {
    /// Requested exactly once. Polled, never re-requested.
    Loading(Handle<A>),
    /// The asset arrived. Says nothing about what the caller made of it.
    ///
    /// The handle is retained rather than dropped because
    /// `AssetEvent::Modified` only fires while a strong handle exists; dropping
    /// it makes `AssetServer::reload` on this path a silent no-op (measured by
    /// `reload_emits_modified_only_while_a_handle_is_retained` in
    /// `bsengine-gltf`).
    Ready(Handle<A>),
    /// The load failed, and will never be asked for again.
    ///
    /// Re-asking `AssetServer` for a failed path resets it to `Loading` and
    /// starts the load over (`bevy_asset` 0.14.2, `server/info.rs:216-221`).
    /// `LoadState::Failed` is set in `PreUpdate` while these are polled in
    /// `Update`, so a loop that re-requests erases the failure before it can
    /// observe it — retrying a missing file forever and spawning a fresh
    /// filesystem task every frame. Holding this state is what stops that.
    ///
    /// The handle is kept even here. Never re-requesting is not the same as
    /// giving up on the file: if it later appears on disk, that arrives as an
    /// `AssetEvent` naming this asset id, and a caller can only recognise it as
    /// *its* file by matching the handle it still holds. `bsengine-scripting`
    /// recovers a given-up script exactly that way.
    GaveUp(Handle<A>),
}

/// Written by hand rather than derived so it does not demand `A: Debug` —
/// `LoadedGltf` and `ScriptSource` are not — and so a failing assertion names
/// the *path*, which is what a reader of that message wants to know.
impl<A: Asset> std::fmt::Debug for AssetSlot<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Loading(h) => write!(f, "Loading({:?})", h.path()),
            Self::Ready(h) => write!(f, "Ready({:?})", h.path()),
            Self::GaveUp(h) => write!(f, "GaveUp({:?})", h.path()),
        }
    }
}

/// What [`AssetSlot::poll`] just did, so a caller can act exactly once.
///
/// A transition rather than a state: every caller of this type does one thing
/// when the asset lands (compile it, upload it, run it, spawn from it) and
/// warns once when it does not. Reporting the state instead would make each
/// caller remember separately whether it had already acted — which is what
/// `bsengine-scripting` used its `Loading` variant for before this existed.
#[derive(Debug)]
pub enum Polled {
    /// Still in flight, or already settled and acted on. Nothing to do.
    Nothing,
    /// The asset arrived on this call. Fires once per slot.
    Arrived,
    /// The load failed on this call. Fires once per slot.
    ///
    /// Carries the message rather than logging it: the five callers word their
    /// warnings differently — `[texture] 'x' failed to load` against
    /// `[scripting] giving up on x` — and some tests read those words.
    Failed(String),
}

impl<A: Asset> AssetSlot<A> {
    /// Requests `path` once and returns a `Loading` slot.
    pub fn requesting(server: &AssetServer, path: &str) -> Self {
        Self::Loading(crate::load_async::<A>(server, path))
    }

    /// Wraps a handle someone else requested.
    ///
    /// For callers whose request goes through [`crate::load`] with a
    /// [`crate::LoadMode`] and a custom loader, rather than through
    /// [`crate::load_async`].
    pub fn from_handle(handle: Handle<A>) -> Self {
        Self::Loading(handle)
    }

    /// Advances the slot, reporting what changed on this call.
    pub fn poll(&mut self, server: &AssetServer, assets: &Assets<A>) -> Polled {
        // Cloned out before the state is written back: a `Handle` is
        // refcounted, so this is a bump rather than a copy of the asset.
        let Self::Loading(handle) = self else {
            return Polled::Nothing;
        };
        let handle = handle.clone();

        if assets.get(&handle).is_some() {
            *self = Self::Ready(handle);
            return Polled::Arrived;
        }
        // Absent is inconclusive — still loading, or failed. Only the load
        // state tells those apart, and it is read from the handle already held,
        // never from a fresh request.
        if let bevy_asset::LoadState::Failed(e) = server.load_state(&handle) {
            *self = Self::GaveUp(handle);
            return Polled::Failed(e.to_string());
        }
        Polled::Nothing
    }

    /// Records that the asset is present after all.
    ///
    /// For a caller that learned it from an `AssetEvent` rather than from
    /// [`Self::poll`]: a file that was missing and has since been created
    /// arrives that way, and a `GaveUp` slot has to be able to leave that state
    /// or the next frame would forget the recovery just performed.
    /// `bsengine-scripting` uses this when a given-up script's file appears.
    ///
    /// This never re-requests, which is the thing that must not happen — it
    /// only writes down what an event already proved.
    pub fn mark_arrived(&mut self) {
        *self = Self::Ready(self.handle().clone());
    }

    /// The retained handle. Present in every state — see
    /// [`Self::GaveUp`] for why a failed load keeps one too.
    pub fn handle(&self) -> &Handle<A> {
        match self {
            Self::Loading(h) | Self::Ready(h) | Self::GaveUp(h) => h,
        }
    }

    /// Whether the asset has arrived.
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    /// Whether this slot reached a terminal failure.
    ///
    /// Distinct from "not ready": a slot still loading is also not ready, and a
    /// test that cannot tell those apart cannot tell a give-up from an infinite
    /// retry, which is the failure mode this state exists to prevent.
    pub fn gave_up(&self) -> bool {
        matches!(self, Self::GaveUp(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::AssetPlugin;
    use crate::test_support::{unique, ProbeDir};
    use crate::types::TextureAsset;
    use bsengine_app::new_app;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    /// A hang guard, not a budget: what is being waited for is one tiny local
    /// file being read, or one that is not there at all.
    const DEADLINE: Duration = Duration::from_secs(10);

    fn png_bytes() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([7, 8, 9, 255]));
        let mut buffer = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut buffer, image::ImageFormat::Png)
            .expect("encode probe png");
        buffer.into_inner()
    }

    /// A probe directory under the process CWD, because the asset root *is* the
    /// CWD (see [`crate::plugin`]) and a path outside it is not addressable as
    /// an asset path at all. `.gitignore` covers the name [`unique`] produces.
    fn probe_project(tag: &str, with_asset: bool) -> (String, ProbeDir) {
        let project = unique(tag);
        let root = PathBuf::from(&project);
        let guard = ProbeDir(root.clone());
        std::fs::create_dir_all(&root).expect("create probe directory");
        if with_asset {
            std::fs::write(root.join("probe.png"), png_bytes()).expect("write probe asset");
        }
        (project, guard)
    }

    /// An app with a real `AssetServer` over a real directory.
    fn app() -> bsengine_app::App {
        let mut app = new_app();
        app.add_plugins(AssetPlugin);
        app.update();
        app
    }

    /// Polls `slot` once per frame until `done` says to stop, returning every
    /// transition `poll` reported along the way.
    ///
    /// Loads finish on a background thread, so the frame an asset lands on is
    /// not fixed; the deadline keeps a broken slot from hanging the suite.
    fn poll_until(
        app: &mut bsengine_app::App,
        slot: &mut AssetSlot<TextureAsset>,
        done: impl Fn(&AssetSlot<TextureAsset>) -> bool,
    ) -> Vec<Polled> {
        let deadline = Instant::now() + DEADLINE;
        let mut seen = Vec::new();
        loop {
            app.update();
            let server = app.world().resource::<AssetServer>().clone();
            let assets = app.world().resource::<Assets<TextureAsset>>();
            seen.push(slot.poll(&server, assets));
            if done(slot) || Instant::now() >= deadline {
                return seen;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// How many of `seen` were arrivals, and how many were failures.
    fn tally(seen: &[Polled]) -> (usize, usize) {
        (
            seen.iter().filter(|p| matches!(p, Polled::Arrived)).count(),
            seen.iter()
                .filter(|p| matches!(p, Polled::Failed(_)))
                .count(),
        )
    }

    #[test]
    fn a_failed_load_reaches_gave_up_and_stays_there() {
        let (project, _guard) = probe_project("slot-missing", false);
        let mut app = app();
        let mut slot = {
            let server = app.world().resource::<AssetServer>();
            AssetSlot::<TextureAsset>::requesting(server, &format!("{project}/absent.png"))
        };

        let seen = poll_until(&mut app, &mut slot, |s| s.gave_up());
        assert!(
            slot.gave_up(),
            "a missing file should reach GaveUp, not poll forever"
        );
        assert_eq!(tally(&seen).1, 1, "the failure should be reported once");

        // And stays there: nothing may re-request it, because re-requesting a
        // failed path restarts the load and erases the failure.
        for _ in 0..5 {
            app.update();
            let server = app.world().resource::<AssetServer>().clone();
            let assets = app.world().resource::<Assets<TextureAsset>>();
            assert!(
                matches!(slot.poll(&server, assets), Polled::Nothing),
                "polling a given-up slot should report nothing"
            );
            assert!(slot.gave_up(), "a given-up slot should not leave GaveUp");
        }
    }

    #[test]
    fn a_given_up_slot_still_holds_its_handle() {
        // Never re-requesting is not the same as forgetting the file. When a
        // missing path is later created, that arrives as an `AssetEvent` naming
        // an asset id, and the only way a caller recognises it as *its* file is
        // the handle it kept -- which is how `bsengine-scripting` brings a
        // given-up script back to life.
        let (project, _guard) = probe_project("slot-gaveup-handle", false);
        let mut app = app();
        let path = format!("{project}/absent.png");
        let mut slot = {
            let server = app.world().resource::<AssetServer>();
            AssetSlot::<TextureAsset>::requesting(server, &path)
        };

        poll_until(&mut app, &mut slot, |s| s.gave_up());
        assert!(slot.gave_up(), "precondition: the load must fail");
        assert_eq!(
            slot.handle().path().map(ToString::to_string).as_deref(),
            Some(path.as_str()),
            "a given-up slot must keep the handle naming the file it wanted"
        );
    }

    #[test]
    fn failure_is_reported_exactly_once() {
        // The reason the five callers' warnings fire once instead of every
        // frame. A per-frame warning for a path that will never load is how the
        // log that should carry that line gets buried.
        let (project, _guard) = probe_project("slot-once-fail", false);
        let mut app = app();
        let mut slot = {
            let server = app.world().resource::<AssetServer>();
            AssetSlot::<TextureAsset>::requesting(server, &format!("{project}/absent.png"))
        };

        // Deliberately keeps polling well past the transition rather than
        // stopping at it, which is the only way a second report could show up.
        let mut seen = poll_until(&mut app, &mut slot, |s| s.gave_up());
        for _ in 0..20 {
            app.update();
            let server = app.world().resource::<AssetServer>().clone();
            let assets = app.world().resource::<Assets<TextureAsset>>();
            seen.push(slot.poll(&server, assets));
        }
        assert_eq!(
            tally(&seen).1,
            1,
            "a failed load should report Failed exactly once"
        );
    }

    #[test]
    fn arrival_is_reported_exactly_once_and_retains_the_handle() {
        // Arriving twice is how a script would run twice and a texture would
        // upload twice. Retaining the handle is what keeps hot reload alive.
        let (project, _guard) = probe_project("slot-arrive", true);
        let mut app = app();
        let mut slot = {
            let server = app.world().resource::<AssetServer>();
            AssetSlot::<TextureAsset>::requesting(server, &format!("{project}/probe.png"))
        };

        let mut seen = poll_until(&mut app, &mut slot, |s| s.is_ready());
        assert!(slot.is_ready(), "the probe asset should arrive");
        for _ in 0..20 {
            app.update();
            let server = app.world().resource::<AssetServer>().clone();
            let assets = app.world().resource::<Assets<TextureAsset>>();
            seen.push(slot.poll(&server, assets));
        }

        assert_eq!(
            tally(&seen).0,
            1,
            "an arrival should be reported exactly once"
        );
        assert_eq!(
            slot.handle().path().map(ToString::to_string).as_deref(),
            Some(format!("{project}/probe.png").as_str()),
            "a Ready slot must keep the handle naming its file, or \
             AssetEvent::Modified stops firing and hot reload dies silently"
        );
    }
}
