use std::time::{Duration, Instant};
use log::{debug, log_enabled, Level};

fn format_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    let secs = duration.as_secs();
    let mins = secs / 60;
    let secs_rem = secs % 60;
    let millis_rem = duration.subsec_millis();

    if millis < 1_000 {
        format!("{} ms", millis)
    } else if secs < 60 {
        format!("{}.{:03} s", secs, millis_rem)
    } else {
        format!("{}:{:02}.{:03} min", mins, secs_rem, millis_rem)
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
        if self.enabled  {
            debug!("{} in {}", self.msg, format_duration(self.start.elapsed()));
            self.msg = msg.to_string();
            self.start = Instant::now();
        }
    }
}