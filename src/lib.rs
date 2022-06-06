//! This module implements a relatively simple synchronous Timer/Scheduler backed by the standard library BinaryHeap type. It is suitable for a reasonably large number of tasks, but you should really use some kind of timer-wheel implementation if you want to have millions and millions of tasks.
//!
//! # Panics
//! Panics in a scheduled task will be caught and logged; repeating task will *not* be rerun after they panics.
//!
mod executor;
mod task;
mod timer;

pub use task::TaskGuard;
pub use timer::Timer;

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, SystemTime};

    use super::Timer;

    #[test]
    fn test_once() {
        let mut t = Timer::new();
        let h = Arc::new(AtomicU32::new(0));
        let h2 = Arc::clone(&h);
        t.schedule_in(Duration::from_millis(10), move || {
            h2.fetch_add(1, Ordering::SeqCst);
        })
        .detach();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(h.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_repeating() {
        let mut t = Timer::new();
        let h = Arc::new(AtomicU32::new(0));
        let h2 = Arc::clone(&h);
        t.schedule_repeating(Duration::from_millis(10), move || {
            h2.fetch_add(1, Ordering::SeqCst);
        })
        .detach();
        std::thread::sleep(Duration::from_millis(100));
        let ran = h.load(Ordering::SeqCst);
        assert!(ran > 7);
        assert!(ran < 11);
    }

    #[test]
    fn test_drop() {
        let mut t = Timer::new();
        let h = Arc::new(AtomicU32::new(0));
        let h2 = Arc::clone(&h);
        let guard = t.schedule_in(Duration::from_millis(10), move || {
            h2.fetch_add(1, Ordering::SeqCst);
        });
        drop(guard);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(h.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_schedule_immediately() {
        let mut t = Timer::new();
        let h = Arc::new(AtomicU32::new(0));
        let h2 = Arc::clone(&h);
        t.schedule_immediately(move || {
            h2.fetch_add(1, Ordering::SeqCst);
        });
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(h.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_schedule_at() {
        let mut t = Timer::new();
        let h = Arc::new(AtomicU32::new(0));
        let h2 = Arc::clone(&h);
        t.schedule_at(SystemTime::now() + Duration::from_millis(50), move || {
            h2.fetch_add(1, Ordering::SeqCst);
        })
        .detach();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(h.load(Ordering::SeqCst), 1);
    }
}
