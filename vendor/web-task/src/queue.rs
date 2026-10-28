//! Provides a local task queue that periodically executes a batch of queued
//! jobs within a browser microtask.
//!
//! When multithreaded (using the atomics target feature) this queue is
//! thread-local.

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use core::cell::{Cell, OnceCell, RefCell};
use core::mem;
use core::ops::DerefMut;
use wasm_bindgen::prelude::*;

use crate::runtime::Job;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen]
    fn queueMicrotask(closure: &Closure<dyn FnMut(JsValue)>);

    type Global;
}

struct QueueState {
    // The queue of Tasks which are to be run in order. In practice this is all the
    // synchronous work of futures, and each `Task` represents calling `poll` on
    // a future "at the right time".
    jobs: RefCell<VecDeque<Job>>,

    // This flag indicates whether we've scheduled `run_all` to run in the future.
    // This is used to ensure that it's only scheduled once.
    is_scheduled: Cell<bool>,
}

impl QueueState {
    fn run_all(&self) {
        // "consume" the schedule
        let _was_scheduled = self.is_scheduled.replace(false);
        debug_assert!(_was_scheduled);

        // Takes everything already scheduled and runs it. Tasks scheduled after
        // this point will run in the next microtask.
        let jobs = mem::take(self.jobs.borrow_mut().deref_mut());
        for job in jobs {
            job.run();
        }

        // All of the Tasks have been run, so it's now possible to schedule the
        // next tick again
    }
}

pub(crate) struct Queue {
    state: Rc<QueueState>,
    closure: Closure<dyn FnMut(JsValue)>,
}

impl Queue {
    // Schedule a task to run on the next tick.
    pub(crate) fn enqueue(job: Job) {
        Queue::with(|queue| {
            queue.state.jobs.borrow_mut().push_back(job);
            if !queue.state.is_scheduled.replace(true) {
                queueMicrotask(&queue.closure);
            }
        })
    }
}

impl Queue {
    fn new() -> Self {
        let state = Rc::new(QueueState {
            is_scheduled: Cell::new(false),
            jobs: RefCell::new(VecDeque::new()),
        });
        Self {
            closure: {
                let state = Rc::clone(&state);
                Closure::new(move |_| state.run_all())
            },
            state,
        }
    }

    pub fn with<R>(f: impl FnOnce(&Self) -> R) -> R {
        struct Wrapper<T>(OnceCell<T>);

        #[cfg(not(target_feature = "atomics"))]
        unsafe impl<T> Sync for Wrapper<T> {}

        #[cfg(not(target_feature = "atomics"))]
        unsafe impl<T> Send for Wrapper<T> {}

        #[cfg_attr(target_feature = "atomics", thread_local)]
        static QUEUE: Wrapper<Queue> = Wrapper(OnceCell::new());

        f(&QUEUE.0.get_or_init(Queue::new))
    }
}
