This is a library, with an API inspired by [timer.rs](https://github.com/Yoric/timer.rs), for scheduling jobs that run synchronously on a background thread.

[![CI](https://github.com/EasyPost/synchronous-timer-rs/workflows/CI/badge.svg?branch=master)](https://github.com/EasyPost/synchronous-timer-rs/actions/workflows/ci.yml)
[![Documentation](https://docs.rs/synchronous-timer/badge.svg)](https://docs.rs/synchronous-timer)
[![crates.io](https://img.shields.io/crates/v/synchronous-timer.svg)](https://crates.io/crates/synchronous-timer)

Example:

```rust
use std::time::Duration;
use synchronous_timer::Timer;

fn main() {
    let mut timer = Timer::new();
    timer
        .schedule_in(Duration::from_secs(5), || {
            println!("I will run on the background thread in 5 seconds")
        })
        .detach();
    timer.schedule_immediately(|| println!("I will run on the background thread right now"));
    let handle = timer.schedule_in(Duration::from_secs(1), || println!("I will never run"));
    drop(handle);
    std::thread::sleep(Duration::from_secs(6));
}
```

This work is licensed under the ISC license, a copy of which can be found in [LICENSE.txt](LICENSE.txt).
