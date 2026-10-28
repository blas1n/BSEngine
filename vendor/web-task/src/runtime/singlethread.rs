//! This provides a single-threaded task runtime, which executes all tasks on
//! the main thread in browser microtasks.

use async_task::{Runnable, Task};

use crate::queue::Queue;

pub struct Job {
    runnable: Runnable,
}

impl Job {
    #[inline]
    pub fn spawn<F>(future: F) -> Task<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let schedule = |runnable| Queue::enqueue(Job { runnable });
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
        let schedule = |runnable| Queue::enqueue(Job { runnable });
        // SAFETY: This runtime is single threaded, so the runnable must be used
        // and dropped on the main thread.
        //
        // No other safety requirements apply:
        // + The future and scheduler are 'static
        // + The scheduler is Send and Sync
        let (runnable, task) = unsafe { async_task::spawn_unchecked(future, schedule) };
        runnable.schedule();
        task
    }

    #[inline]
    pub fn run(self) {
        self.runnable.run();
    }
}
