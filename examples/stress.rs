use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use synchronous_timer::Timer;

const TARGET: u32 = 200_000;

fn main() {
    let mut t = Timer::new();
    let val = Arc::new(AtomicU32::default());
    let at = SystemTime::now() + Duration::from_millis(15);
    for _ in 0..TARGET {
        let their_val = Arc::clone(&val);
        t.schedule_immediately(move || {
            their_val.fetch_add(1, Ordering::SeqCst);
        });
        let their_val = Arc::clone(&val);
        t.schedule_at(at, move || {
            their_val.fetch_add(1, Ordering::SeqCst);
        })
        .detach();
    }
    while val.load(Ordering::SeqCst) != TARGET * 2 {
        std::thread::sleep(Duration::from_millis(10));
    }
}
