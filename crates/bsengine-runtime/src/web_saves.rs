//! A browser build's saves, kept in the page's IndexedDB.
//!
//! A page has no file system, so a script's `Bsengine.save` there lands in a
//! [`SaveStore`] instead of a `SaveDir` -- and the store lives only as long as
//! the page unless something keeps it. Unity's WebGL player and Godot's web
//! export both keep saves in IndexedDB (Emscripten's IDBFS, mounted at
//! `persistentDataPath` / `user://`), and both work round IndexedDB being
//! asynchronous the same way: read everything into memory before the game
//! starts, let every save change memory at once, write it back behind. This
//! does the same, without the file system in between: [`open`] reads the
//! database into the store before the app is built, and [`flush`] -- a system
//! at the end of every frame -- writes back whatever was saved during it.
//!
//! One database per game, named for the project (`bsengine-saves/<name>`):
//! IndexedDB is per origin, and two games served from the same site would
//! otherwise read each other's `checkpoint.json`. Godot separates them the
//! same way, by a per-project `app_userdata/<name>` folder in one database.

use std::cell::RefCell;

use bevy_ecs::prelude::ResMut;
use bsengine_core::SaveStore;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

/// The object store holding the save files: key = the save's path as the
/// store normalised it, value = its text.
const STORE: &str = "saves";

thread_local! {
    /// The opened database, for [`flush`]. Not a resource: a JS object is not
    /// `Send`, and the browser build has one thread anyway.
    static DB: RefCell<Option<web_sys::IdbDatabase>> = const { RefCell::new(None) };
}

/// The database a project's saves are kept in.
fn database_name(project: &str) -> String {
    format!("bsengine-saves/{project}")
}

/// Reads `project`'s saves back from IndexedDB into a store, and keeps the
/// database open for [`flush`]. A page that cannot have IndexedDB -- some
/// private windows refuse it -- still gets a store, so saving and loading
/// work for as long as the page is open, with a warning that it will not
/// last. That is the same fallback Godot's web export takes.
pub async fn open(project: &str) -> SaveStore {
    match read_back(project).await {
        Ok((db, files)) => {
            DB.with(|slot| *slot.borrow_mut() = Some(db));
            let store = SaveStore::with_files(files);
            // Counted from the store the game gets, not from what the database
            // returned: this line is what the browser smoke test reads to say
            // the saves came back, and a count taken before the store was made
            // would say so for a store that dropped them.
            tracing::info!(
                "web saves: {} restored from IndexedDB ({})",
                store.len(),
                database_name(project)
            );
            store
        }
        Err(e) => {
            tracing::warn!(
                "web saves: IndexedDB is unavailable ({}); saves last only until the page closes",
                describe(&e)
            );
            SaveStore::default()
        }
    }
}

async fn read_back(
    project: &str,
) -> Result<(web_sys::IdbDatabase, Vec<(String, String)>), JsValue> {
    let factory = web_sys::window()
        .ok_or("no window")?
        .indexed_db()?
        .ok_or("no IndexedDB")?;
    let request = factory.open_with_u32(&database_name(project), 1)?;
    // Version 1 is the only one, so the only upgrade is the database being
    // made: give it its one store.
    let upgrade = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
        let db = e
            .target()
            .and_then(|t| t.dyn_into::<web_sys::IdbOpenDbRequest>().ok())
            .and_then(|r| r.result().ok())
            .and_then(|r| r.dyn_into::<web_sys::IdbDatabase>().ok());
        if let Some(db) = db {
            if let Err(e) = db.create_object_store(STORE) {
                tracing::warn!("web saves: cannot create the store: {}", describe(&e));
            }
        }
    });
    request.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
    let db: web_sys::IdbDatabase = completed(&request).await?.dyn_into()?;
    drop(upgrade);

    // Both requests before either is awaited: a transaction commits once it
    // has nothing pending at the end of a task, and an `await` in between
    // would find it already finished (`TransactionInactiveError`). Both list
    // in key order, so the two arrays line up.
    let store = db.transaction_with_str(STORE)?.object_store(STORE)?;
    let keys = store.get_all_keys()?;
    let values = store.get_all()?;
    let keys = js_sys::Array::from(&completed(&keys).await?);
    let values = js_sys::Array::from(&completed(&values).await?);
    let files = keys
        .iter()
        .zip(values.iter())
        .filter_map(|(k, v)| Some((k.as_string()?, v.as_string()?)))
        .collect();
    Ok((db, files))
}

/// Writes back every save made this frame -- each file once, as it is now
/// (`SaveStore::take_written`) -- in one transaction. Fire and forget, as
/// IDBFS's sync is: the store already holds the save, so the game never
/// waits on the database. A write the browser refuses (the quota) is logged.
pub fn flush(mut store: ResMut<SaveStore>) {
    let written = store.take_written();
    if written.is_empty() {
        return;
    }
    DB.with(|slot| {
        let Some(db) = slot.borrow().clone() else {
            // The page has no IndexedDB (see `open`): the store is all there
            // is, and it already holds the save.
            return;
        };
        let transaction =
            match db.transaction_with_str_and_mode(STORE, web_sys::IdbTransactionMode::Readwrite) {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("web saves: cannot write back: {}", describe(&e));
                    return;
                }
            };
        let paths: Vec<String> = written.iter().map(|(p, _)| p.clone()).collect();
        let on_error = Closure::once_into_js(move |_: web_sys::Event| {
            tracing::warn!("web saves: IndexedDB refused the write of {paths:?}");
        });
        transaction.set_onerror(Some(on_error.unchecked_ref()));
        let store = match transaction.object_store(STORE) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("web saves: cannot write back: {}", describe(&e));
                return;
            }
        };
        for (path, contents) in written {
            if let Err(e) =
                store.put_with_key(&JsValue::from_str(&contents), &JsValue::from_str(&path))
            {
                tracing::warn!("web saves: cannot write back {path:?}: {}", describe(&e));
            }
        }
    });
}

/// Resolves with `request`'s result once it succeeds.
async fn completed(request: &web_sys::IdbRequest) -> Result<JsValue, JsValue> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let done = request.clone();
        let on_success = Closure::once_into_js(move |_: web_sys::Event| {
            let _ = resolve.call1(&JsValue::NULL, &done.result().unwrap_or(JsValue::UNDEFINED));
        });
        let on_error = Closure::once_into_js(move |_: web_sys::Event| {
            let _ = reject.call1(
                &JsValue::NULL,
                &JsValue::from_str("the IndexedDB request failed"),
            );
        });
        request.set_onsuccess(Some(on_success.unchecked_ref()));
        request.set_onerror(Some(on_error.unchecked_ref()));
    });
    wasm_bindgen_futures::JsFuture::from(promise).await
}

fn describe(e: &JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}
