// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::target_arch::interval;
use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use prometheus_client::metrics::{family::Family, gauge::Gauge};
use std::time::{Duration, Instant};
use strum::IntoEnumIterator;

const UPDATE_INTERVAL: Duration = Duration::from_secs(20);

/// A struct to record the the number of reports against our node across different time frames.
pub struct ShunnedCountAcrossTimeFrames {
    metric: Family<TimeFrame, Gauge>,
    tracked_values: Vec<TrackedValue>,
}

struct TrackedValue {
    time: Instant,
    least_bucket_it_fits_in: TimeFrameType,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct TimeFrame {
    time_frame: TimeFrameType,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, EncodeLabelValue, strum::EnumIter)]
pub enum TimeFrameType {
    LastTenMinutes,
    LastHour,
    LastSixHours,
    LastDay,
    LastWeek,
    Indefinite,
}

impl TimeFrameType {
    #[cfg(not(test))]
    fn get_duration_sec(&self) -> u64 {
        match self {
            TimeFrameType::LastTenMinutes => 10 * 60,
            TimeFrameType::LastHour => 60 * 60,
            TimeFrameType::LastSixHours => 6 * 60 * 60,
            TimeFrameType::LastDay => 24 * 60 * 60,
            TimeFrameType::LastWeek => 7 * 24 * 60 * 60,
            TimeFrameType::Indefinite => u64::MAX,
        }
    }

    #[cfg(test)]
    fn get_duration_sec(&self) -> u64 {
        match self {
            TimeFrameType::LastTenMinutes => 2,
            TimeFrameType::LastHour => 4,
            TimeFrameType::LastSixHours => 6,
            TimeFrameType::LastDay => 8,
            TimeFrameType::LastWeek => 10,
            TimeFrameType::Indefinite => u64::MAX,
        }
    }

    fn next_time_frame(&self) -> Self {
        match self {
            TimeFrameType::LastTenMinutes => TimeFrameType::LastHour,
            TimeFrameType::LastHour => TimeFrameType::LastSixHours,
            TimeFrameType::LastSixHours => TimeFrameType::LastDay,
            TimeFrameType::LastDay => TimeFrameType::LastWeek,
            TimeFrameType::LastWeek => TimeFrameType::Indefinite,
            TimeFrameType::Indefinite => TimeFrameType::Indefinite,
        }
    }
}

impl ShunnedCountAcrossTimeFrames {
    pub fn spawn_background_task(
        time_based_shunned_count: Family<TimeFrame, Gauge>,
    ) -> tokio::sync::mpsc::Sender<()> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        tokio::spawn(async move {
            let mut shunned_metrics = ShunnedCountAcrossTimeFrames {
                metric: time_based_shunned_count,
                tracked_values: Vec::new(),
            };
            let mut update_interval = interval(UPDATE_INTERVAL);
            update_interval.tick().await;

            loop {
                tokio::select! {
                    _ = rx.recv() => {
                        shunned_metrics.record_shunned();

                    }
                    _ = update_interval.tick() => {
                        shunned_metrics.update();
                    }
                }
            }
        });
        tx
    }

    pub fn record_shunned(&mut self) {
        let now = Instant::now();
        self.tracked_values.push(TrackedValue {
            time: now,
            least_bucket_it_fits_in: TimeFrameType::LastTenMinutes,
        });

        for variant in TimeFrameType::iter() {
            let time_frame = TimeFrame {
                time_frame: variant,
            };
            self.metric.get_or_create(&time_frame).inc();
        }
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        let mut idx_to_remove = Vec::new();

        for (idx, tracked_value) in self.tracked_values.iter_mut().enumerate() {
            let time_elapsed_since_adding = now.duration_since(tracked_value.time).as_secs();

            if time_elapsed_since_adding > tracked_value.least_bucket_it_fits_in.get_duration_sec()
            {
                let time_frame = TimeFrame {
                    time_frame: tracked_value.least_bucket_it_fits_in,
                };
                self.metric.get_or_create(&time_frame).dec();

                let new_time_frame = tracked_value.least_bucket_it_fits_in.next_time_frame();
                if new_time_frame == TimeFrameType::Indefinite {
                    idx_to_remove.push(idx);
                } else {
                    tracked_value.least_bucket_it_fits_in = new_time_frame;
                }
            }
        }
        // remove the ones that are now indefinite
        for idx in idx_to_remove {
            self.tracked_values.remove(idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_should_move_to_next_state() -> eyre::Result<()> {
        let mut shunned_metrics = ShunnedCountAcrossTimeFrames {
            metric: Family::default(),
            tracked_values: Vec::new(),
        };
        shunned_metrics.record_shunned();

        let current_state = shunned_metrics.tracked_values[0].least_bucket_it_fits_in;
        assert!(matches!(current_state, TimeFrameType::LastTenMinutes));
        // all the counters should be 1
        for variant in TimeFrameType::iter() {
            let time_frame = TimeFrame {
                time_frame: variant,
            };
            assert_eq!(shunned_metrics.metric.get_or_create(&time_frame).get(), 1);
        }

        println!(
            "current_state: {current_state:?}. Sleeping for {} sec",
            current_state.get_duration_sec() + 1
        );
        std::thread::sleep(std::time::Duration::from_secs(
            current_state.get_duration_sec() + 1,
        ));
        shunned_metrics.update();
        let current_state = shunned_metrics.tracked_values[0].least_bucket_it_fits_in;
        assert!(matches!(current_state, TimeFrameType::LastHour));
        // all the counters except LastTenMinutes should be 1
        for variant in TimeFrameType::iter() {
            let time_frame = TimeFrame {
                time_frame: variant,
            };
            if variant == TimeFrameType::LastTenMinutes {
                assert_eq!(shunned_metrics.metric.get_or_create(&time_frame).get(), 0);
            } else {
                assert_eq!(shunned_metrics.metric.get_or_create(&time_frame).get(), 1);
            }
        }

        println!(
            "current_state: {current_state:?}. Sleeping for {} sec",
            current_state.get_duration_sec() + 1
        );
        std::thread::sleep(std::time::Duration::from_secs(
            current_state.get_duration_sec() + 1,
        ));
        shunned_metrics.update();
        let current_state = shunned_metrics.tracked_values[0].least_bucket_it_fits_in;
        assert!(matches!(current_state, TimeFrameType::LastSixHours));
        // all the counters except LastTenMinutes and LastHour should be 1
        for variant in TimeFrameType::iter() {
            let time_frame = TimeFrame {
                time_frame: variant,
            };
            if variant == TimeFrameType::LastTenMinutes || variant == TimeFrameType::LastHour {
                assert_eq!(shunned_metrics.metric.get_or_create(&time_frame).get(), 0);
            } else {
                assert_eq!(shunned_metrics.metric.get_or_create(&time_frame).get(), 1);
            }
        }

        println!(
            "current_state: {current_state:?}. Sleeping for {} sec",
            current_state.get_duration_sec() + 1
        );
        std::thread::sleep(std::time::Duration::from_secs(
            current_state.get_duration_sec() + 1,
        ));
        shunned_metrics.update();
        let current_state = shunned_metrics.tracked_values[0].least_bucket_it_fits_in;
        assert!(matches!(current_state, TimeFrameType::LastDay));
        // all the counters except LastTenMinutes, LastHour and LastSixHours should be 1
        for variant in TimeFrameType::iter() {
            let time_frame = TimeFrame {
                time_frame: variant,
            };
            if variant == TimeFrameType::LastTenMinutes
                || variant == TimeFrameType::LastHour
                || variant == TimeFrameType::LastSixHours
            {
                assert_eq!(shunned_metrics.metric.get_or_create(&time_frame).get(), 0);
            } else {
                assert_eq!(shunned_metrics.metric.get_or_create(&time_frame).get(), 1);
            }
        }

        println!(
            "current_state: {current_state:?}. Sleeping for {} sec",
            current_state.get_duration_sec() + 1
        );
        std::thread::sleep(std::time::Duration::from_secs(
            current_state.get_duration_sec() + 1,
        ));
        shunned_metrics.update();
        let current_state = shunned_metrics.tracked_values[0].least_bucket_it_fits_in;
        assert!(matches!(current_state, TimeFrameType::LastWeek));
        // all the counters except LastTenMinutes, LastHour, LastSixHours and LastDay should be 1
        for variant in TimeFrameType::iter() {
            let time_frame = TimeFrame {
                time_frame: variant,
            };
            if variant == TimeFrameType::LastTenMinutes
                || variant == TimeFrameType::LastHour
                || variant == TimeFrameType::LastSixHours
                || variant == TimeFrameType::LastDay
            {
                assert_eq!(shunned_metrics.metric.get_or_create(&time_frame).get(), 0);
            } else {
                assert_eq!(shunned_metrics.metric.get_or_create(&time_frame).get(), 1);
            }
        }

        println!(
            "current_state: {current_state:?}. Sleeping for {} sec",
            current_state.get_duration_sec() + 1
        );
        std::thread::sleep(std::time::Duration::from_secs(
            current_state.get_duration_sec() + 1,
        ));
        shunned_metrics.update();
        assert_eq!(shunned_metrics.tracked_values.len(), 0);
        // all the counters except Indefinite should be 0
        for variant in TimeFrameType::iter() {
            let time_frame = TimeFrame {
                time_frame: variant,
            };
            if variant == TimeFrameType::Indefinite {
                assert_eq!(shunned_metrics.metric.get_or_create(&time_frame).get(), 1);
            } else {
                assert_eq!(shunned_metrics.metric.get_or_create(&time_frame).get(), 0);
            }
        }

        Ok(())
    }
}
