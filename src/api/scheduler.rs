use std::str::FromStr;
use std::time::Duration;
use actix_web::web::Data;
use chrono::Local;
use cron::Schedule;
use crate::api::api_model::AppState;
use crate::processing::playlist_processor::exec_processing;

pub(crate) async fn start_scheduler(expression: &str, data: Data<AppState>) -> ! {
    let schedule = Schedule::from_str(expression).unwrap();
    let offset = *Local::now().offset();
    loop {
        let mut upcoming = schedule.upcoming(offset).take(1);
        actix_rt::time::sleep(Duration::from_millis(500)).await;
        let local = &Local::now();

        if let Some(datetime) = upcoming.next() {
            if datetime.timestamp() <= local.timestamp() {
                exec_processing(data.config.clone(), data.targets.clone());
            }
        }
    }
}