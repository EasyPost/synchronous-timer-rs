use std::time::Duration;
use synchronous_timer::Timer;

fn tick_with(s: &'static str) {
    println!("tick from thread {:?}: {}", std::thread::current().id(), s);
}

fn main() {
    let mut t = Timer::new();
    println!(
        "starting timer on thread {:?}; will run for 10 seconds",
        std::thread::current().id()
    );
    t.schedule_repeating(Duration::from_secs(1), || tick_with("1"))
        .detach();
    t.schedule_repeating(Duration::from_millis(500), || tick_with("0.5"))
        .detach();
    let tick_2_handle = t.schedule_repeating(Duration::from_secs(2), || tick_with("2"));
    std::thread::sleep(Duration::from_secs(5));
    t.schedule_immediately(|| println!("tick 2 should stop now"));
    drop(tick_2_handle);
    std::thread::sleep(Duration::from_secs(5));
}
