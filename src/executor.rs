use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};

use crate::task::Ready;
use crate::timer::TimerShared;

const DEFAULT_LOOP_TIME: Duration = Duration::from_millis(500);

#[derive(Debug)]
enum NextAction {
    ExecuteTop,
    SleepAtLeast(Duration, u64),
}

pub(crate) struct Executor {
    changed: Arc<Condvar>,
    shared: Arc<Mutex<TimerShared>>,
}

impl Executor {
    pub fn new(shared: Arc<Mutex<TimerShared>>, changed: Arc<Condvar>) -> Self {
        Self { changed, shared }
    }

    pub fn run_until_done(self) {
        loop {
            let action = {
                let shared = self.shared.lock();
                if shared.done {
                    break;
                }
                let next_id = shared.next_id;
                match shared.tasks.peek().map(|t| t.ready()) {
                    Some(Ready::Now) => NextAction::ExecuteTop,
                    Some(Ready::In(d)) => NextAction::SleepAtLeast(d, next_id),
                    None => NextAction::SleepAtLeast(DEFAULT_LOOP_TIME, next_id),
                }
            };
            match action {
                NextAction::ExecuteTop => {
                    let item = {
                        let mut s = self.shared.lock();
                        s.tasks.pop()
                    };
                    if let Some(item) = item {
                        // someone must've dropped the top item
                        if !item.ready().now() {
                            let mut s = self.shared.lock();
                            s.tasks.push(item);
                            continue;
                        }
                        if item.dropped() {
                            log::debug!("encountered dropped task {}", item.id());
                            continue;
                        }
                        match std::panic::catch_unwind(|| item.run()) {
                            Ok(Some(remainder)) => {
                                let mut s = self.shared.lock();
                                s.tasks.push(remainder);
                            }
                            Ok(None) => {}
                            Err(e) => {
                                log::error!("uncaught panic when running task: {:?}", e);
                            }
                        };
                    }
                }
                NextAction::SleepAtLeast(d, seen_epoch) => {
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
