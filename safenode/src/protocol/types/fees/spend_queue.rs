// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! An implementation of a spend queue for SNT spends.
//!
//! The purpose of this spend queue is to further enhance
//! the supply/demand properties of the payment network.
//!
//! It allows for Clients to set priority for their spends,
//! and for the network to moderate the load of incoming spends
//! in times of high demand.
//!
//! Additionally, the network can now be even nimbler than with
//! only the growth-parameters-based supply/demand properties of the fees
//! (see op_cost::required_tokens). The spend queue will give an immediate
//! response to high demand from Clients.
//!
//! Knock on effects are increased incentives for nodes to join
//! the network, as high demand increases the rewards paid to Nodes.

use super::SpendPriority;

use priority_queue::PriorityQueue;
use std::{hash::Hash, time::Duration};
use tokio::time::Instant;

/// The queue of pending spends, sorted by
/// the fee paid.
///
/// Implemented with generic arg to simplify testing,
/// as the `SignedSpend` type, that we know spend queue will
/// be used with, requires lots of plumbing to setup.
#[derive(custom_debug::Debug)]
pub struct SpendQ<T: Eq + Hash> {
    #[debug(skip)]
    queue: PriorityQueue<T, u64>,
    #[debug(skip)]
    snapshot: SpendQSnapshot,
    last_pop: Instant,
}

/// A snapshot of the sorted fees in the spend queue.
/// Used to calculate the stats of the spend queue.
#[derive(Clone, custom_debug::Debug)]
pub struct SpendQSnapshot {
    #[debug(skip)]
    #[cfg(test)]
    queue: Vec<u64>,
    stats: SpendQStats,
}

/// Stats of the spend queue, listing the number of spends currently queued,
/// the highest and lowest fee in the queue, as well as the average fee and
/// standard deviation.
/// The spend queue stats is not meant to return 100% consistent data,
/// but an approximation which is good enough.
///
/// If the queue becomes empty, the last value popped will be used
/// for `high`, `low` and `avg`, with a zero `std_dev`.
/// Before any item has been pushed to the queue, the value with which
/// the spend queue was instantiated will be used. And if spend queue was
/// instantiated with no value, the passed in fallback value (current op_cost)
/// will be used.
#[derive(Copy, Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpendQStats {
    /// The highest fee in the queue.
    pub highest: u64,
    /// The second highest fee in the queue.
    pub high: u64,
    /// The third highest fee in the queue.
    pub medium_high: u64,
    /// The average fee in the queue.
    pub avg: u64,
    /// The third lowest fee in the queue.
    pub medium_low: u64,
    /// The second lowest fee in the queue.
    pub low: u64,
    /// The lowest fee in the queue.
    pub lowest: u64,
    /// The number of spends currently in the queue.
    pub len: usize,
    /// The standard deviation of the fees in the queue.
    pub std_dev: u64,
}

impl SpendQStats {
    /// Validate that the fee is high enough to push it to the spend queue,
    /// otherwise it is dropped and an error returned to Client.
    /// Returns if the fee paid was valid or not, and the currently lowest valid fee.
    ///
    /// The basic rule for now is that if the paid fee is over 10% less
    /// than the lowest fee in queue minus std dev, then we don't accept the fee.
    /// This is a temporary diff value/design.
    /// (And yes, this means that for now, the Client can try to get away cheaper by sending in up to 10% less than lowest priority fee.)
    pub fn validate_fee(&self, fee_paid: u64) -> (bool, u64) {
        // add some margin
        let lowest_w_margin = (self.lowest as f64 * 0.90) as u64;
        debug!("validate_fee: lowest {lowest_w_margin}");
        let valid = fee_paid >= lowest_w_margin;
        (valid, lowest_w_margin)
    }

    /// Map the priority to a fee to pay, based on
    /// current state of the spend queue.
    pub fn map_to_fee(&self, priority: SpendPriority) -> u64 {
        match priority {
            SpendPriority::Highest => self.highest,
            SpendPriority::High => self.high,
            SpendPriority::MediumHigh => self.medium_high,
            SpendPriority::Normal => self.avg,
            SpendPriority::MediumLow => self.medium_low,
            SpendPriority::Low => self.low,
            SpendPriority::Lowest => self.lowest,
        }
    }
}

impl SpendQSnapshot {
    /// Create a new snapshot of the spend queue.
    pub fn new(queue: Vec<u64>) -> Self {
        let default_val = 4;

        let high = queue.first().copied().unwrap_or(2 * default_val);
        let low = queue.last().copied().unwrap_or(default_val / 2);
        let (avg, std_dev, len) = calc_stats(&queue);

        let medium_high = (high + avg) / 2;
        let medium_low = (low + avg) / 2;
        let highest = high + std_dev;

        use std::cmp::Ordering::*;
        let lowest = match std_dev.cmp(&low) {
            Less => low - std_dev,
            _ => u64::min(low, (2 * std_dev) / 3),
        };

        debug!(
            "stats: highest {highest}, high {high}, medium_high {medium_high}, avg {avg}, medium_low {medium_low}, low {low}, lowest {lowest}, std_dev {std_dev}, len {len}, queue {:?}",
            queue
        );

        Self {
            #[cfg(test)]
            queue,
            stats: SpendQStats {
                highest,
                high,
                medium_high,
                avg,
                medium_low,
                low,
                lowest,
                len,
                std_dev,
            },
        }
    }

    /// This is not meant to return 100% consistent state,
    /// but an approximation which is good enough.
    ///
    /// If the queue becomes empty, the last three values popped will be used
    /// for these calcs.
    /// Before any item has been pushed to spend queue, the value with which
    /// the spend queue was instantiated will be used.
    pub fn stats(&self) -> SpendQStats {
        self.stats
    }
}

impl<T: Eq + Hash> SpendQ<T> {
    /// Create a new instance of the spend queue, with an initial value
    /// that will populate the stats, by which fee can be derived.
    /// The snapshot will have non zero values, replaced when values are pushed onto the spend queue.
    pub fn with_fee(current_fee: u64) -> Self {
        debug!("Starting fee: current_fee {current_fee}.");
        let default_val = u64::max(4, current_fee);
        Self {
            queue: PriorityQueue::new(),
            snapshot: SpendQSnapshot::new(vec![2 * default_val, default_val, default_val / 2]),
            last_pop: Instant::now(),
        }
    }

    /// We set a limit on how many tx will be processed per s.
    /// As the network grows, the total capacity will grow as well.
    ///
    /// Example:
    /// 1 tps for a group of 4 gives
    /// -> 25600 tps with 102400 nodes
    /// -> 76800 tps with 307200 nodes
    /// -> etc..
    /// ..and for a group of 8 gives
    /// -> 12800 tps with 102400 nodes
    /// -> 38400 tps with 307200 nodes
    /// -> etc..
    pub fn elapsed(&self) -> bool {
        Instant::now() - self.last_pop > Duration::from_secs(1)
    }

    /// Return a snapshot of the fees in the queue, with preserved order.
    pub fn snapshot(&self) -> SpendQSnapshot {
        self.snapshot.clone()
    }

    /// This requires all validation of the fee to have already been made.
    /// There is no validation here!
    pub fn push(&mut self, item: T, priority: u64) {
        let _ = self.queue.push(item, priority);
        // We update our snapshot after every change to the queue.
        self.set_snapshot();
    }

    /// Pop the highest priority item from the queue.
    pub fn pop(&mut self) -> Option<(T, u64)> {
        let item = self.queue.pop();

        // Preserve last 3 items in snapshot, as to preserve the current fee.
        if self.queue.len() > 3 {
            // We update our snapshot after every change to the queue, except when only 3 left.
            self.set_snapshot();
        }
        // To achieve our desired tsp, we set the `last_pop` time to now.
        self.last_pop = Instant::now();

        item
    }

    // Populate the snapshot with the fees only, and
    // sort them, so that stats are properly calculated over them.
    // Makes sure at least 3 items are in the snapshot.
    fn set_snapshot(&mut self) {
        let mut queue: Vec<_> = self.queue.iter().map(|(_, fee)| *fee).collect();

        queue.sort();
        queue.reverse(); // highest first

        let queue = if self.queue.is_empty() {
            // this should be unreachable
            let default_val = 4;
            vec![2 * default_val, default_val, default_val / 2]
        } else if queue.len() == 1 {
            // with one value, the spread is large
            vec![2 * queue[0], queue[0], queue[0] / 2]
        } else if self.queue.len() == 2 {
            // with two values, the spread is lower
            vec![queue[0], (queue[0] + queue[1]) / 2, queue[1]]
        } else {
            queue
        };

        self.snapshot = SpendQSnapshot::new(queue);
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.queue.len()
    }
}

/// Calculate the avg value of the set.
fn avg(data: &Vec<u64>) -> Option<u64> {
    let sum: u64 = data.iter().sum();
    let count = data.len();
    if count > 0 {
        Some((sum as f64 / count as f64).round() as u64)
    } else {
        None
    }
}

/// Calculates the standard deviation of the set of fees.
/// If the set is empty the provided `fallback_value` is used
/// as `avg`, and zero as `std_dev`.
fn calc_stats(data: &Vec<u64>) -> (u64, u64, usize) {
    match (avg(data), data.len()) {
        (Some(avg), count) if count > 0 => {
            let variance = data
                .iter()
                .map(|value| {
                    let diff = avg as i64 - *value as i64;
                    (diff * diff) as f64
                })
                .sum::<f64>()
                / count as f64;

            let std_dev = variance.sqrt().round() as u64;
            (avg, std_dev, count)
        }
        _ => (0, 0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::super::Result;
    use super::*;

    #[test]
    #[allow(clippy::result_large_err)]
    fn spendq_snapshot_is_sorted_highest_first() -> Result<()> {
        let mut spendq = SpendQ::<usize>::with_fee(0);

        for i in 1..11 {
            let rand_item: usize = rand::random();
            spendq.push(rand_item, i);
        }

        let snapshot = spendq.snapshot();

        let non_sorted_vec: Vec<_> = spendq.queue.iter().map(|(_, fee)| *fee).collect();
        let sorted_vec = snapshot.queue;

        // This shall show that we are required to sort the vec,
        // it's not enough to just collect the prios, because they
        // come ordered by the hash of the item they carry in the queue.
        assert_ne!(non_sorted_vec, sorted_vec);

        // this is the expected outcome, largest value first
        let expected_vec = vec![10, 9, 8, 7, 6, 5, 4, 3, 2, 1];
        assert_eq!(sorted_vec, expected_vec);

        Ok(())
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn zero_init_spendq_default_stats_values() -> Result<()> {
        let spendq = SpendQ::<usize>::with_fee(0);

        let snapshot = spendq.snapshot();
        let stats = snapshot.stats();

        assert_eq!(stats.high, 8);
        assert_eq!(stats.low, 2);
        assert_eq!(stats.avg, 5);
        assert_eq!(stats.std_dev, 3);

        assert_eq!(snapshot.queue.len(), 3);
        assert!(spendq.is_empty());

        Ok(())
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn spendq_instantiated_with_fee_gives_initial_value_in_stats() -> Result<()> {
        // We can pass in an initial value to spend queue, which will reflect in stats
        // while we haven't had any values pushed onto the spend queue.
        let initial_value = 42;
        let spendq = SpendQ::<usize>::with_fee(initial_value);

        let snapshot = spendq.snapshot();
        let stats = snapshot.stats();

        assert_eq!(stats.high, 2 * initial_value);
        assert_eq!(stats.low, initial_value / 2);
        assert_eq!(stats.avg, 49);
        assert_eq!(stats.std_dev, 26);

        assert_eq!(snapshot.queue.len(), 3);
        assert!(spendq.is_empty());

        Ok(())
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn pushing_an_item_sets_stats() -> Result<()> {
        // We use an initial value.
        let initial_value = 42;
        let mut spendq = SpendQ::<usize>::with_fee(initial_value);
        let snapshot = spendq.snapshot();

        let stats = snapshot.stats();

        // A single value gives high spread stats.
        assert_eq!(stats.high, 2 * initial_value);
        assert_eq!(stats.low, initial_value / 2);
        assert_eq!(stats.avg, 49);
        assert_eq!(stats.std_dev, 26);

        // The initial value will be what we have in the snapshot queue.
        assert_eq!(snapshot.queue.len(), 3);
        assert!(spendq.queue.is_empty());

        // We push an item, and take a new snapshot..
        let rand_item: usize = rand::random();
        let actual_value = 24;
        spendq.push(rand_item, actual_value);

        let snapshot = spendq.snapshot();

        let stats = snapshot.stats();

        // It's the items in the spend queue that are counted towards the stats.
        assert_eq!(stats.high, 2 * actual_value);
        assert_eq!(stats.low, actual_value / 2);
        assert_eq!(stats.avg, 28);
        assert_eq!(stats.std_dev, 15);

        assert_eq!(snapshot.queue.len(), 3);
        assert!(!spendq.is_empty());

        Ok(())
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn when_pushed_to_empty_popped_item_is_preserved_in_stats_until_new_item_is_pushed(
    ) -> Result<()> {
        let mut spendq = SpendQ::<usize>::with_fee(0);

        // We push an item
        let rand_item: usize = rand::random();
        let value_42 = 42;
        spendq.push(rand_item, value_42);

        let snapshot = spendq.snapshot();
        let stats_value_42 = snapshot.stats();

        let popped_item_42 = spendq.pop();

        assert_eq!(popped_item_42, Some((rand_item, value_42)));

        // Take a new snapshot..
        let snapshot = spendq.snapshot();

        // Last item is still in the snapshot queue, to preserve the value (the last price)
        // for stats queries. But the actual spend queue is empty.
        assert_eq!(snapshot.queue.len(), 3);
        assert!(spendq.is_empty());

        assert_eq!(snapshot.queue, vec![2 * value_42, value_42, value_42 / 2]);

        let stats_value_42_preserved = snapshot.stats();

        // The stats will be the same as it was when the popped item was still in the queue.
        assert_eq!(stats_value_42, stats_value_42_preserved);

        // ------------------------------------------------
        // The spend queue is now empty. Let's try again.
        // ------------------------------------------------

        // We push a new item.
        let other_rand_item: usize = rand::random();
        let value_66 = 66;
        spendq.push(other_rand_item, value_66);

        // Take a new snapshot..
        let snapshot = spendq.snapshot();

        assert_eq!(snapshot.queue.len(), 3);
        assert_eq!(spendq.len(), 1);

        let stats_value_66 = snapshot.stats();

        // The stats will now be updated by the new item in the queue,
        // i.e. `stats_value_66` is not equal to `stats_preserved_value_42`.
        assert_ne!(stats_value_66, stats_value_42_preserved);

        // The previously preserved value has been replaced.
        assert_eq!(snapshot.queue, vec![2 * value_66, value_66, value_66 / 2]);

        let popped_item_66 = spendq.pop();
        // The last item in the queue now, is the `value_66`.
        assert_eq!(popped_item_66, Some((other_rand_item, value_66)));

        // And same as the previous time, when we take a new snapshot..
        let snapshot = spendq.snapshot();

        // ..last item is still in the snapshot queue.
        // But the actual spend queue is empty.
        assert_eq!(snapshot.queue.len(), 3);
        assert!(spendq.is_empty());

        assert_eq!(snapshot.queue, vec![2 * value_66, value_66, value_66 / 2]);

        let stats_value_66_preserved = snapshot.stats();

        // The stats will be the same as it was when the popped item was still in the queue.
        assert_eq!(stats_value_66, stats_value_66_preserved);

        Ok(())
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn last_three_item_are_preserved_in_stats_until_new_item_is_pushed() -> Result<()> {
        let mut spendq = SpendQ::<usize>::with_fee(0);

        // We push three items
        let item_1 = rand::random();
        let item_2 = rand::random();
        let item_3 = rand::random();
        let value_41 = 41;
        let value_42 = 42;
        let value_43 = 43;
        spendq.push(item_1, value_41);
        spendq.push(item_2, value_42);
        spendq.push(item_3, value_43);

        let snapshot = spendq.snapshot();
        let stats_3_values = snapshot.stats();

        let popped_item_43 = spendq.pop();

        assert_eq!(popped_item_43, Some((item_3, value_43)));

        // Take a new snapshot..
        let snapshot = spendq.snapshot();

        // Last three items are still in the snapshot queue, to preserve the value (the last price)
        // for stats queries. But the actual spend queue has one item less.
        assert_eq!(snapshot.queue, vec![43, 42, 41]);
        assert_eq!(spendq.len(), 2);

        let stats_2_values = snapshot.stats();

        // The stats will be the same as it was when the popped item was still in the queue.
        assert_eq!(stats_3_values, stats_2_values);

        // ------------------------------------------------
        // Pop the last two items as well.
        // ------------------------------------------------

        let popped_item_42 = spendq.pop();
        assert_eq!(popped_item_42, Some((item_2, value_42)));
        let popped_item_41 = spendq.pop();
        assert_eq!(popped_item_41, Some((item_1, value_41)));

        // Take a new snapshot..
        let snapshot = spendq.snapshot();

        // Last three items are still in the snapshot queue, to preserve the value (the last price)
        // for stats queries. But the actual spend queue is empty.
        assert_eq!(snapshot.queue, vec![43, 42, 41]);
        assert!(spendq.is_empty());

        let stats_0_values = snapshot.stats();

        // The stats will be the same as it was when the three last popped item were still in the queue.
        assert_eq!(stats_3_values, stats_0_values);

        // ------------------------------------------------
        // The spend queue is now empty. Let's try again.
        // ------------------------------------------------

        // We push a new item.
        let other_rand_item: usize = rand::random();
        let value_66 = 66;
        spendq.push(other_rand_item, value_66);

        // Take a new snapshot..
        let snapshot = spendq.snapshot();

        // The old values are evicted from snapshot.
        assert_eq!(snapshot.queue.len(), 3);
        assert_eq!(spendq.len(), 1);

        let stats_value_66 = snapshot.stats();

        // The stats will now be updated by the new item in the queue,
        // i.e. `stats_value_66` is not equal to `stats_0_values`.
        assert_ne!(stats_value_66, stats_0_values);

        // The previously preserved value has been replaced.
        assert_eq!(snapshot.queue, vec![2 * value_66, value_66, value_66 / 2]);

        let popped_item_66 = spendq.pop();
        // The last item in the queue now, is the `value_66`.
        assert_eq!(popped_item_66, Some((other_rand_item, value_66)));

        // And same as the previous time, when we take a new snapshot..
        let snapshot = spendq.snapshot();

        // ..last item is still in the snapshot queue.
        // But the actual spend queue is empty.
        assert_eq!(snapshot.queue.len(), 3);
        assert!(spendq.is_empty());

        assert_eq!(snapshot.queue, vec![2 * value_66, value_66, value_66 / 2]);

        let stats_value_66_preserved = snapshot.stats();

        // The stats will be the same as it was when the popped item was still in the queue.
        assert_eq!(stats_value_66, stats_value_66_preserved);

        Ok(())
    }
}
