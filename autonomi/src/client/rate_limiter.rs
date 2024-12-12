use ant_networking::target_arch::{sleep, Duration, Instant};

pub struct RateLimiter {
    last_request_time: Option<Instant>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            last_request_time: None,
        }
    }

    pub async fn wait_interval_since_last_request(&mut self, interval_in_ms: u64) {
        if let Some(last_request_time) = self.last_request_time {
            let elapsed_time = last_request_time.elapsed();

            let interval = Duration::from_millis(interval_in_ms);

            if elapsed_time < interval {
                sleep(interval - elapsed_time).await;
            }
        }

        self.last_request_time = Some(Instant::now());
    }
}
