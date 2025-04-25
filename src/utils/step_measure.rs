use log::{debug, log_enabled, Level};
use std::time::{Duration, Instant};

fn format_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    let secs = duration.as_secs();
    let mins = secs / 60;
    let secs_rem = secs % 60;
    let millis_rem = duration.subsec_millis();

    if millis < 1_000 {
        format!("{millis} ms")
    } else if secs < 60 {
        format!("{secs}.{millis_rem:03} s")
    } else {
        format!("{mins}:{secs_rem:02}.{millis_rem:03} min")
    }
}

pub struct StepMeasure {
    enabled: bool,
    msg: String,
    start: Instant,
}

impl StepMeasure {
    pub fn new(msg: &str) -> Self {
        Self {
            enabled: log_enabled!(Level::Debug),
            msg: msg.to_string(),
            start: Instant::now(),
        }
    }

    pub fn tick(&mut self, msg: &str) {
        if self.enabled {
            debug!("{} in {}", self.msg, format_duration(self.start.elapsed()));
            self.msg = msg.to_string();
            self.start = Instant::now();
        }
    }

    pub fn stop(&mut self) {
        if self.enabled && !self.msg.is_empty() {
            debug!("{} in {}", self.msg, format_duration( self.start.elapsed()));
            self.enabled = false;
        }
    }
}

impl Drop for StepMeasure {
    fn drop(&mut self) {
        self.stop();
    }
}