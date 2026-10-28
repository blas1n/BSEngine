//! This provides a multithreaded task runtime, which uses the `waitAtomic()`
//! browser api to implement `send_local`, and executes tasks in a mix of
//! promises and browser microtasks.

use alloc::sync::Arc;
use alloc::sync::Weak;
use core::cell::UnsafeCell;
use core::sync::atomic::AtomicI32;
use core::sync::atomic::Ordering;

use async_task::{Runnable, Task};
use wasm_bindgen::prelude::*;

use crate::queue::Queue;

/// The state of a LocalRunnable which is waiting to be scheduled.
const WAITING: i32 = 0;

/// The state of a LocalRunnable which is in the process of being scheduled or
/// ran. This is the only state in which the `LocalRunnable::runnable` field may
/// be modified, and represents a mutex-lock on the local-runnable.
const LOCKED: i32 = 1;

/// The state of a LocalRunnable which has been scheduled and is ready to run.
const READY: i32 = 2;

/// A local runner polls a runner to completion on a single thread, but can be
/// woken from any thread.
///
/// When a runnable is scheduled, it is stored and we use atomics to cause a
/// promise on the owning thread to resolve, after which the runnable is
/// executed and a new promise is created.
pub(crate) struct LocalRunnable {
    /// The atomic that is used to lock the runnable, and wake the promise to
    /// allow execution to progress on the owning thread.
    state: AtomicI32,
    /// Contains the runnable that will be executed when the promise resolves.
    runnable: UnsafeCell<Option<Runnable>>,
    /// A continuation with a weak self-reference that calls `run_to_completion`.
    continuation: UnsafeCell<Closure<dyn FnMut(JsValue)>>,
}

impl LocalRunnable {
    /// Creates a new local runnable.
    fn new() -> Arc<LocalRunnable> {
        let this = Arc::new(Self {
            state: AtomicI32::new(WAITING),
            runnable: UnsafeCell::new(None),
            continuation: UnsafeCell::new(Closure::new(drop)),
        });
        let this_weak = Arc::downgrade(&this);
        // SAFETY: We have trivial exclusive access at this point.
        let continuation_ref = unsafe { &mut *this.continuation.get() };
        *continuation_ref = Closure::new(move |_| {
            // SAFETY: This will always execute on the same thread as
            // the caller, and we are allowed to call it recursively.
            unsafe { LocalRunnable::run_to_completion(&this_weak) }
        });
        this
    }

    /// Stores the runnable and causes the `waitAsync()` promise in
    /// `wait` to resolve on the owning thread, and allowing `run_to_completion`
    /// to progress.
    ///
    /// This is called as part of an `async-task` schedule function, and
    /// expect `async-task` not to call this again until the runnable runs.
    ///
    /// More generally, we also assume that there is only one instance of
    /// `schedule` and at most one instance of `run_to_completion` executing at
    /// once on different threads. It expects not to contend with itself.
    fn schedule(&self, runnable: Runnable) {
        // Ideally, we want to take the state from WAITING to LOCKED here, so
        // that we can safely write the runnable into the local field. There are
        // three possible valid cases to handle.
        loop {
            match self.state.swap(LOCKED, Ordering::Relaxed) {
                // If we  were in the WAITING state, we have sucessfully aquired
                // the lock and can progress to storing the runnable.
                WAITING => break,
                // If we were in the LOCKED state (extreamly unlikely) then we
                // may have conflicted with `run_to_completion` or even another
                // instance of `schedule` (although this could only result from
                // a bug).
                //
                // In either case, we simply spin while the lock is held. The
                // chance that the local runnable remains locked for multiple
                // iterations of this loop is extreamly low.
                LOCKED => continue,
                // If we were in the READY state, that's implies that there are
                // somehow multiple copies of `runnable` floating around. This
                // is almost certantly a bug, and we have no choice but to
                // panic.
                READY => panic!("tried to schedule already scheduled task"),
                // Any other state is invalid.
                _ => panic!("invalid local runnable state"),
            }
        }

        // SAFETY: The spin-lock above ensures we have exlusive access to this
        // value, which allows us to alias it mutably.
        let runnable_ref = unsafe { &mut *self.runnable.get() };
        debug_assert!(runnable_ref.is_none());
        *runnable_ref = Some(runnable);

        // Now we can release the lock. This uses the `Release` memory ordering
        // to ensure that other theads can access the runnable after loading the
        // state.
        let state = self.state.swap(READY, Ordering::Release);
        debug_assert_eq!(state, LOCKED);

        // Wake the promise created by `waitAsync()` in `Self::wait`.
        //
        // SAFETY: The provided pointer is an atomic which it is correct to call
        // wasm's `memory.atomic.notify` instruction on.
        unsafe {
            core::arch::wasm32::memory_atomic_notify(self.state.as_ptr(), 1);
        }
    }

    /// Polls the runnable to completion on the thread on which it is first run.
    ///
    /// # Safety
    ///
    /// This must be called exactly once (excluding recursion) on the thread the
    /// local runnable was created on.
    ///
    /// We assume that there is only one instance of `run_to_completion` and at
    /// most one instance of `schedule` executing on different threads. It
    /// expects not to contend with itself.
    unsafe fn run_to_completion(this_weak: &Weak<Self>) {
        // We rely on the Arc captured by the schedule function to keep `Self`
        // alive. When the future is dropped, the schedule function is too,
        // which will cause this upgrade to fail, allowing us to exit early.
        let Some(this) = this_weak.upgrade() else {
            return;
        };

        // Ideally, we want to take the state from READY to LOCKED here, so
        // that we can safely read the runnable from the local field. As with
        // `schedule`, there are three possible valid cases to handle.
        loop {
            // This uses the `Acquire` memory ording to ensure we can access the
            // runnable after the load.
            match this.state.swap(LOCKED, Ordering::Acquire) {
                // If we were in the WAITING state, there's no runnable to
                // read. We have no choice but to panic.
                WAITING => panic!("tried to run a task that wasn't scheduled"),
                // If we were in the LOCKED state (extreamly unlikely) then we may
                // have conflicted with `schedule` or even another instance of
                // `run_to_completion` (although this could only result from a bug).
                //
                // In either case, we simply spin while the lock is held. The
                // chance that the local runnable remains locked for multiple
                // iterations of this loop is extreamly low.
                LOCKED => continue,
                // IF we were in the READY state, we have sucessfully aquired
                // the lock and can progress to retriving the runnable.
                READY => break,
                // Any other state is invalid.
                _ => panic!("invalid local runnable state"),
            }
        }

        // SAFETY: The spin-lock above ensures we have exlusive access to this
        // value, which allows us to alias it mutably.
        let runnable_ref = unsafe { &mut *this.runnable.get() };
        let runnable = core::mem::take(runnable_ref).unwrap();

        // Now we can release the lock.
        let state = this.state.swap(WAITING, Ordering::Relaxed);
        debug_assert_eq!(state, LOCKED);

        // Having released the lock, we can execute the runnable. While
        // executing, it's often the case that `schedule` will be called again.
        // Some futures always reschedule themselves.
        runnable.run();

        // If this function is the only thing keeping the local runnable
        // alive, the future has been dropped; don't schedule a
        // condinuation.
        //
        // This _theoretically_ prevents a js-side memory leak, where we
        // create promises for futures that will never resolve.
        if this_weak.strong_count() == 1 {
            return;
        }

        // Otherwise, we need to set up the continuation for this future on this
        // thread. The call to `wait` returns a promise if the future has not
        // yet been re-scheduled, and `None` if it has.
        if let Some(promise) = this.wait() {
            // SAFETY: This continuation is only accessed immutably after
            // construction.
            let continuation_ref = unsafe { &*this.continuation.get() };
            drop(promise.then(continuation_ref));
        } else {
            // If the future has already been rescheduled, we can add it back to
            // the queue. This will cause it to execute again in the next
            // browser microtask.
            Queue::enqueue(Job::Local(this_weak.clone()))
        }
    }

    /// An ergonomic wrapper around the `waitAsync()` api. When `schedule` has
    /// been called, this returns `None`. Otherwise it returns a promise that
    /// resolves after `schedule` is next called.
    fn wait(&self) -> Option<js_sys::Promise> {
        let mem = wasm_bindgen::memory().unchecked_into::<js_sys::WebAssembly::Memory>();
        let array = js_sys::Int32Array::new(&mem.buffer());
        let ptr = &self.state;
        let result = Atomics::wait_async(&array, ptr.as_ptr() as u32 / 4, WAITING);
        if result.async_() {
            return Some(result.value());
        } else {
            return None;
        }

        #[wasm_bindgen]
        extern "C" {
            type Atomics;
            type WaitAsyncResult;

            #[wasm_bindgen(static_method_of = Atomics, js_name = waitAsync)]
            fn wait_async(buf: &js_sys::Int32Array, index: u32, value: i32) -> WaitAsyncResult;

            #[wasm_bindgen(static_method_of = Atomics, js_name = waitAsync, getter)]
            fn get_wait_async() -> JsValue;

            #[wasm_bindgen(method, getter, structural, js_name = async)]
            fn async_(this: &WaitAsyncResult) -> bool;

            #[wasm_bindgen(method, getter, structural)]
            fn value(this: &WaitAsyncResult) -> js_sys::Promise;
        }
    }
}

/// Multithreaded jobs can be either Send or Local (eg !Send). Both of these get
/// mixed together in the thread-local queues, but waking a Local job is more
/// complex.
pub(crate) enum Job {
    Send(Runnable),
    Local(Weak<LocalRunnable>),
}

impl Job {
    #[inline]
    pub fn spawn<F>(future: F) -> Task<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        // For send futures, we can just add them to the current thread's queue.
        // This will cause them to execute on whatever thread wakes them, which
        // may not be the same thread that they were created on.
        let schedule = |runnable| Queue::enqueue(Job::Send(runnable));
        let (runnable, task) = async_task::spawn(future, schedule);
        runnable.schedule();
        task
    }

    #[inline]
    pub fn spawn_local<F>(future: F) -> Task<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        // Instead of directly scheduling !Send futures, we create a
        // LocalRunnable that will keep their execution on this thread.
        //
        // Note: This returns an Arc<LocalRunnable>
        let local_runnable = LocalRunnable::new();

        let schedule = {
            // The schedule function takes a clone of the local runnable, which
            // ensures that it will stay alive until the future is polled to
            // completion or canceled.
            let local_runnable = local_runnable.clone();
            // We defer scheduling to the local runnable.
            move |runnable| local_runnable.schedule(runnable)
        };

        // SAFETY: The future is `!Send`, so we must ensure that the `Runnable`
        // is used and dropped only on this thread. The `LocalRunnable` ensures
        // this.
        //
        // None of the other safety requires apply:
        // + The future and scheduler are 'static
        // + The scheduler is Send and Sync
        let (runnable, task) = unsafe { async_task::spawn_unchecked(future, schedule) };

        // Having created our first runnable, we insert it into the `LocalRunnable`.
        runnable.schedule();

        // In order to actually run the `LocalRunnable`, we have to add it to the queue.
        //
        // Note: We only do this when we are sure we are on the thread that
        // created the runnable.
        Queue::enqueue(Job::Local(Arc::downgrade(&local_runnable)));

        task
    }

    #[inline]
    pub fn run(self) {
        match self {
            Job::Send(send_runnable) => {
                send_runnable.run();
            }
            Job::Local(local_runnable) => {
                // SAFETY: The requirements for this are checked when adding a
                // local job to the queue.
                unsafe { LocalRunnable::run_to_completion(&local_runnable) };
            }
        }
    }
}
