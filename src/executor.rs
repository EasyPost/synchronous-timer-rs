use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};
use smallvec::SmallVec;

use crate::task::{Ready, Task};
use crate::timer::TimerShared;

// This value is the worst-case for how "late" an item can be in case we happen to miss the condvar
// notification and it's added while we're executing another item.
const DEFAULT_LOOP_TIME: Duration = Duration::from_millis(500);

// In testing, there's a big (50%) speedup going from 1 to 4 items per loop (from locking
// amortization), but basically no performance difference between 4 and 16, then a gradual falloff.
// 8 seems to be a nice spot in the middle. This might be best off tuned based on system stuff, but
// _shrug_
const MAX_PER_LOOP: usize = 8;

#[derive(Debug)]
enum NextAction {
    ExecuteSome(SmallVec<[Task; MAX_PER_LOOP]>),
    SleepAtLeast(Duration, u64),
    Exit,
}

pub(crate) struct Executor {
    changed: Arc<Condvar>,
    shared: Arc<Mutex<TimerShared>>,
}

impl Executor {
    pub fn new(shared: Arc<Mutex<TimerShared>>, changed: Arc<Condvar>) -> Self {
        Self { changed, shared }
    }

    fn get_next_action(&self) -> NextAction {
        let mut shared = self.shared.lock();
        if shared.done {
            return NextAction::Exit;
        }
        let next_id = shared.next_id;
        let mut ready = SmallVec::new();
        let now = Instant::now();
        loop {
            if ready.len() == MAX_PER_LOOP {
                break;
            }
            match shared.tasks.peek().map(|t| t.ready(now)) {
                Some(Ready::Now) => {
                    // There's no condition where this isn't Some(task) since we just peeked it,
                    // but BinaryHeap has no operation to avoid this Option
                    if let Some(task) = shared.tasks.pop() {
                        ready.push(task)
                    }
                }
                Some(Ready::In(d)) => {
                    if ready.is_empty() {
                        return NextAction::SleepAtLeast(d, next_id);
                    } else {
                        break;
                    }
                }
                None => break,
            }
        }
        if ready.is_empty() {
            NextAction::SleepAtLeast(DEFAULT_LOOP_TIME, next_id)
        } else {
            NextAction::ExecuteSome(ready)
        }
    }

    pub fn run_until_done(self) {
        loop {
            // Grab some items (this will briefly hold the lock while it's grabbing them)
            let action = self.get_next_action();
            match action {
                NextAction::Exit => break,
                NextAction::ExecuteSome(items) => {
                    // Execute those items serially. This will not hold the lock
                    let remainders = items
                        .into_iter()
                        .filter_map(|item| {
                            if item.dropped() {
                                log::debug!("encountered dropped task {}", item.id());
                                return None;
                            }
                            match std::panic::catch_unwind(|| item.run()) {
                                Ok(remainder) => remainder,
                                Err(e) => {
                                    log::error!("uncaught panic when running task: {:?}", e);
                                    None
                                }
                            }
                        })
                        .collect::<SmallVec<[Task; MAX_PER_LOOP]>>();
                    // Reinsert any periodic timers to the list in one big chunk
                    if !remainders.is_empty() {
                        let mut s = self.shared.lock();
                        for item in remainders {
                            s.tasks.push(item);
                        }
                    }
                }
                NextAction::SleepAtLeast(d, seen_epoch) => {
                    // Wait for the next item to be ready. This will only briefly hold the lock to
                    // check for shutdown.
                    let mut shared = self.shared.lock();
                    if shared.done {
                        break;
                    }
                    // This means someone changed the structure between when we read it at the top
                    // and here, so let's rescan
                    if shared.next_id != seen_epoch {
                        continue;
                    }
                    if !self
                        .changed
                        .wait_until(&mut shared, Instant::now() + d)
                        .timed_out()
                    {
                        log::debug!("something changed");
                    }
                }
            }
        }
    }
}
