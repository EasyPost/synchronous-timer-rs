use std::collections::BinaryHeap;
use std::panic::UnwindSafe;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use parking_lot::{Condvar, Mutex};

use crate::executor::Executor;
use crate::task::{Task, TaskCallable, TaskGuard};

/// The main structure of this library, a `Timer` handles scheduling one-off and repeating tasks,
/// which are executed on a background thread. Tasks should be short-lived (as they block the
/// thread) synchronous functions.
pub struct Timer {
    executor_thread: Option<std::thread::JoinHandle<()>>,
    shared: Arc<Mutex<TimerShared>>,
    changed: Arc<Condvar>,
}

pub(crate) struct TimerShared {
    pub tasks: BinaryHeap<Task>,
    pub done: bool,
    pub next_id: u64,
}

impl TimerShared {
    #[inline(always)]
    fn with_capacity(cap: usize) -> Self {
        Self {
            tasks: if cap == 0 {
                // Avoid allocating in this case
                BinaryHeap::new()
            } else {
                BinaryHeap::with_capacity(cap)
            },
            done: false,
            next_id: 1,
        }
    }
}

impl Timer {
    /// Construct a new Timer. This will immediately start a background thread
    /// for executing tasks, which will be shut down on drop.
    #[inline(always)]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Construct a new Timer with underlying capacity for the given number of tasks
    /// as a microoptimization. This will immediately start a background thread for
    /// executing tasks, which will be shut down on drop.
    pub fn with_capacity(cap: usize) -> Self {
        let shared = Arc::new(Mutex::new(TimerShared::with_capacity(cap)));
        let changed = Arc::new(Condvar::new());
        let executor = Executor::new(Arc::clone(&shared), Arc::clone(&changed));
        let executor_thread = Some(
            std::thread::Builder::new()
                .name("timer-executor".into())
                .spawn(|| executor.run_until_done())
                .unwrap(),
        );
        Self {
            shared,
            changed,
            executor_thread,
        }
    }

    fn push(&mut self, callable: TaskCallable, next: Instant) -> TaskGuard {
        let mut shared = self.shared.lock();
        let id = shared.next_id;
        shared.next_id += 1;
        let handle = Task::new(id, next, callable);
        let guard = handle.guard();
        shared.tasks.push(handle);
        drop(shared);
        self.changed.notify_one();
        guard
    }

    /// Schedule a task to run once, after the given duration
    pub fn schedule_in<F: FnOnce() + UnwindSafe + Send + 'static>(
        &mut self,
        duration: Duration,
        f: F,
    ) -> TaskGuard {
        let callable = TaskCallable::new_once(f);
        self.push(callable, Instant::now() + duration)
    }

    /// Schedule a task to run at a given wall-clock time. This will be converted
    /// to an Instant and run according to the monotonic clock, so may have... somewhat
    /// unpredictable behavior around leap seconds.
    pub fn schedule_at<F: FnOnce() + UnwindSafe + Send + 'static>(
        &mut self,
        system_time: SystemTime,
        f: F,
    ) -> TaskGuard {
        let callable = TaskCallable::new_once(f);
        let now = SystemTime::now();
        let when = match system_time.duration_since(now) {
            Ok(d) => Instant::now() + d,
            Err(_) => Instant::now(),
        };
        self.push(callable, when)
    }

    /// Schedule a task to run periodically, after every interval
    pub fn schedule_repeating<F: FnMut() + UnwindSafe + Send + 'static>(
        &mut self,
        interval: Duration,
        f: F,
    ) -> TaskGuard {
        let callable = TaskCallable::new_repeating(f, interval);
        self.push(callable, Instant::now() + interval)
    }

    /// Schedule a task to run as soon as possible
    pub fn schedule_immediately<F: FnOnce() + UnwindSafe + Send + 'static>(&mut self, f: F) {
        let callable = TaskCallable::new_once(f);
        self.push(callable, Instant::now()).detach()
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Timer {
    /// Drop the timer and shut down the background thread
    fn drop(&mut self) {
        if let Some(handle) = self.executor_thread.take() {
            let mut s = self.shared.lock();
            s.done = true;
            self.changed.notify_one();
            drop(s);
            if let Err(e) = handle.join() {
                log::error!("Error joining timer thread: {:?}", e);
            }
        }
    }
}
