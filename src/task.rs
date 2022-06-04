use std::panic::UnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
struct TaskState {
    running: Arc<AtomicBool>,
    dropped: Arc<AtomicBool>,
}

pub(crate) enum TaskCallable {
    Once(Box<dyn FnOnce() + UnwindSafe + Send + 'static>),
    Repeating(Box<dyn FnMut() + UnwindSafe + Send + 'static>, Duration),
}

impl TaskCallable {
    pub fn new_once<F: FnOnce() + UnwindSafe + Send + 'static>(f: F) -> Self {
        Self::Once(Box::new(f))
    }

    pub fn new_repeating<F: FnMut() + UnwindSafe + Send + 'static>(
        f: F,
        interval: Duration,
    ) -> Self {
        Self::Repeating(Box::new(f), interval)
    }
}

impl std::fmt::Debug for TaskCallable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Once(_) => write!(f, "TaskCallable::Once(<unformattable>)"),
            Self::Repeating(_, i) => write!(f, "TaskCallable::Repeating(<unformattable>, {:?})", i),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Ready {
    Now,
    In(Duration),
}

#[derive(Debug)]
pub(crate) struct Task {
    task_id: u64,
    next_execution: Instant,
    task: TaskState,
    callable: TaskCallable,
}

impl Task {
    pub fn new(task_id: u64, next_execution: Instant, callable: TaskCallable) -> Self {
        Self {
            task_id,
            next_execution,
            task: TaskState::default(),
            callable,
        }
    }

    /// Run this task. If there is a "next_execution", return a new TaskHandle with the fields
    /// updated
    pub fn run(self) -> Option<Task> {
        let task_id = self.task_id;
        let task = self.task;
        let was_running = task.running.swap(true, Ordering::Acquire);
        if was_running {
            log::error!("encountered a running task (a.k.a. a panic); not running again");
            return None;
        }
        match self.callable {
            TaskCallable::Repeating(mut f, interval) => {
                let next_execution = Instant::now() + interval;
                f();
                task.running.store(false, Ordering::Release);
                Some(Task {
                    task_id,
                    next_execution,
                    task,
                    callable: TaskCallable::Repeating(f, interval),
                })
            }
            TaskCallable::Once(f) => {
                f();
                task.running.store(false, Ordering::Release);
                None
            }
        }
    }

    pub fn id(&self) -> u64 {
        self.task_id
    }

    pub fn dropped(&self) -> bool {
        self.task.dropped.load(Ordering::Relaxed)
    }

    pub fn ready(&self, now: Instant) -> Ready {
        if now > self.next_execution {
            Ready::Now
        } else {
            Ready::In(self.next_execution - now)
        }
    }

    pub fn guard(&self) -> TaskGuard {
        TaskGuard::new(self.task_id, Arc::clone(&self.task.dropped))
    }
}

impl PartialEq<Task> for Task {
    fn eq(&self, other: &Task) -> bool {
        self.task_id == other.task_id
    }
}

impl PartialOrd<Task> for Task {
    fn partial_cmp(&self, other: &Task) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Task {}

impl Ord for Task {
    fn cmp(&self, other: &Task) -> std::cmp::Ordering {
        match self.next_execution.cmp(&other.next_execution).reverse() {
            std::cmp::Ordering::Equal => self.task_id.cmp(&other.task_id).reverse(),
            other => other,
        }
    }
}

#[derive(Debug)]
/// A `TaskGuard` represents a handle to a future task. When it is dropped, we will attempt to cancel that task. If you would like the task to continue running in the background, use the `.detach()` method
pub struct TaskGuard {
    task_id: u64,
    dropped: Option<Arc<AtomicBool>>,
}

impl TaskGuard {
    fn new(task_id: u64, dropped: Arc<AtomicBool>) -> Self {
        Self {
            task_id,
            dropped: Some(dropped),
        }
    }

    /// Get the ID of the underlying task, for debugging
    pub fn task_id(&self) -> u64 {
        self.task_id
    }

    /// Detach this `TaskGuard` from the underlying `Task` so that dropping this guard will no
    /// longer cancel the task.
    pub fn detach(mut self) {
        self.dropped.take();
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        if let Some(dropped) = self.dropped.take() {
            dropped.store(true, Ordering::Relaxed);
        }
    }
}
