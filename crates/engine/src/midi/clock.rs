//! The internal tempo: a thread that sends MIDI clock ticks (24 per quarter
//! note) to the forwarder. It runs while a pattern mode is on; the pattern
//! engine only uses its ticks while no external clock arrives.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::forwarder::FwdMsg;
use super::pattern::{ClockMsg, ClockOrigin, PPQN};

/// Asks Windows for 1 ms timer resolution while the clock runs; the default
/// of about 15.6 ms would make the internal tempo audibly uneven.
struct TimerResolution;

#[cfg(windows)]
#[link(name = "winmm")]
extern "system" {
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}

impl TimerResolution {
    fn new() -> Self {
        #[cfg(windows)]
        // SAFETY: plain Win32 call without pointers; undone in Drop.
        unsafe {
            timeBeginPeriod(1);
        }
        TimerResolution
    }
}

impl Drop for TimerResolution {
    fn drop(&mut self) {
        #[cfg(windows)]
        // SAFETY: matches the timeBeginPeriod in `new`.
        unsafe {
            timeEndPeriod(1);
        }
    }
}

pub struct InternalClock {
    stop: Arc<AtomicBool>,
    bpm: Arc<AtomicU32>,
    join: Option<JoinHandle<()>>,
}

impl InternalClock {
    pub fn start(bpm: f32, tx: Sender<FwdMsg>) -> InternalClock {
        let stop = Arc::new(AtomicBool::new(false));
        let bpm = Arc::new(AtomicU32::new(bpm.to_bits()));
        let (stop_flag, bpm_value) = (stop.clone(), bpm.clone());
        let join = thread::Builder::new()
            .name("internal-clock".into())
            .spawn(move || {
                let _resolution = TimerResolution::new();
                let mut next = Instant::now();
                while !stop_flag.load(Ordering::Relaxed) {
                    let bpm = f32::from_bits(bpm_value.load(Ordering::Relaxed)).clamp(20.0, 300.0) as f64;
                    next += Duration::from_secs_f64(60.0 / (bpm * PPQN as f64));
                    let now = Instant::now();
                    if next > now {
                        thread::sleep(next - now);
                    } else if now - next > Duration::from_millis(100) {
                        // Fell far behind (system sleep, debugger): do not rush to catch up.
                        next = now;
                    }
                    if tx
                        .send(FwdMsg::Clock(ClockOrigin::Internal, ClockMsg::Tick))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("spawn internal clock thread");
        InternalClock {
            stop,
            bpm,
            join: Some(join),
        }
    }

    pub fn set_bpm(&self, bpm: f32) {
        self.bpm.store(bpm.to_bits(), Ordering::Relaxed);
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for InternalClock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_at_the_set_tempo() {
        let (tx, rx) = std::sync::mpsc::channel();
        // 300 BPM = 120 ticks per second.
        let clock = InternalClock::start(300.0, tx);
        let start = Instant::now();
        let mut ticks = 0;
        while start.elapsed() < Duration::from_millis(500) {
            if rx.recv_timeout(Duration::from_millis(50)).is_ok() {
                ticks += 1;
            }
        }
        clock.stop();
        assert!((50..=70).contains(&ticks), "expected about 60 ticks, got {ticks}");
    }
}
