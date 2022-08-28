//! Provides the [`TimerEvent`] event
use chrono::offset::Utc;
use chrono::{Datelike, Duration};
use tracing::info;

/// This type of events are broadcasted during specific datetime, allowing the bot to run datetime specific
/// tasks
#[derive(Debug, Clone)]
pub enum TimerEvent {
    /// Sent at the start of Sunday at UTC time
    Weekly,
}

crate::signal!(TimerSignal, TimerRecv, TimerEvent);

/// Start the loop for broadcasting [`TimerEvent`]
pub async fn start_loop(signal: TimerSignal) {
    tokio::spawn(async move {
        info!("Starting timer loop");

        loop {
            // Calculating amount of time until next sunday
            let now = Utc::now();
            let until_midnight = (now + Duration::days(1)).date().and_hms(0, 0, 0) - now;
            let days_util_sunday = now.date().weekday().num_days_from_sunday();
            let days_util_sunday = i64::try_from(days_util_sunday).expect("Failed to convert u64 to i64");
            let until_sunday = until_midnight + Duration::days(6 - days_util_sunday);
            let until_sunday = until_sunday
                .to_std()
                .expect("Failed to convert chrono::Duration to std Duration");

            info!(
                "Currently {}, duration until next utc sunday: {}",
                now.format("%Y %b %d (%a) %T"),
                util::string::fmt_second(until_sunday.as_secs().try_into().unwrap())
            );

            // Wait until next sunday and broadcast a weekly event
            tokio::time::sleep(until_sunday).await;
            signal.signal(TimerEvent::Weekly);
        }
    });
}
