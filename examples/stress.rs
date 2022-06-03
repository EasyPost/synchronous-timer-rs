use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use synchronous_timer::Timer;

const TARGET: u32 = 2_000_000;

fn main() {
    let mut t = Timer::new();
    let val = Arc::new(AtomicU32::default());
    for _ in 0..TARGET {
        let their_val = Arc::clone(&val);
        t.schedule_immediately(move || {
            their_val.fetch_add(1, Ordering::SeqCst);
        });
    }
    while val.load(Ordering::SeqCst) != TARGET {
        std::thread::sleep(Duration::from_millis(10));
    }
}
