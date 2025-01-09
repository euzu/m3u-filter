use std::sync::Arc;
use std::str::FromStr;
use std::time::{Duration, Instant, SystemTime};
use chrono::{DateTime, FixedOffset, Local};
use cron::Schedule;
use log::error;
use crate::exit;
use crate::model::config::{Config, ProcessTargets};
use crate::processing::playlist_processor::exec_processing;

fn datetime_to_instant(datetime: DateTime<FixedOffset>) -> Instant {
    // Convert DateTime<FixedOffset> to SystemTime
    let target_system_time: SystemTime = datetime.into();

    // Get the current SystemTime
    let now_system_time = SystemTime::now();

    // Calculate the duration between now and the target time
    let duration_until = target_system_time
        .duration_since(now_system_time)
        .unwrap_or_else(|_| Duration::from_secs(0));

    // Get the current Instant and add the duration to calculate the target Instant
    Instant::now() + duration_until
}

pub async fn start_scheduler(client: Arc<reqwest::Client>, expression: &str, config: Arc<Config>, targets: Arc<ProcessTargets>) -> ! {
    match Schedule::from_str(expression) {
        Ok(schedule) => {
            let offset = *Local::now().offset();
            loop {
                let mut upcoming = schedule.upcoming(offset).take(1);
                if let Some(datetime) = upcoming.next() {
                    actix_web::rt::time::sleep_until(actix_rt::time::Instant::from(datetime_to_instant(datetime))).await;
                    exec_processing(Arc::clone(&client), Arc::clone(&config), Arc::clone(&targets)).await;
                 }
            }
        }
        Err(err) => exit!("Failed to start scheduler: {}", err)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::atomic::{AtomicU8, Ordering};
    use chrono::Local;
    use cron::Schedule;
    use crate::api::scheduler::datetime_to_instant;

    #[actix_rt::test]
    async fn test_run_scheduler() {
        // Define a cron expression that runs every second
        let expression = "0/1 * * * * * *"; // every second

        let runs = AtomicU8::new(0);
        let run_me = || runs.fetch_add(1, Ordering::Relaxed);

        let start = std::time::Instant::now();
        match Schedule::from_str(expression) {
            Ok(schedule) => {
                let offset = *Local::now().offset();
                loop {
                    let mut upcoming = schedule.upcoming(offset).take(1);
                    if let Some(datetime) = upcoming.next() {
                        actix_web::rt::time::sleep_until(actix_rt::time::Instant::from(datetime_to_instant(datetime))).await;
                        run_me();
                    }
                    if runs.load(Ordering::Relaxed) == 6 {
                        break;
                    }
                }
            }
            Err(_) => {}
        };
        let duration = start.elapsed();

        assert!(runs.load(Ordering::Relaxed) == 6, "Failed to run");
        assert!(duration.as_secs() > 4, "Failed time");
    }
}