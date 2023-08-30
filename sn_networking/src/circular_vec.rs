// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::error::Error;

/// Based on https://users.rust-lang.org/t/the-best-ring-buffer-library/58489/7

/// A circular buffer implemented with a VecDeque.
#[derive(Debug)]
pub struct CircularVec<T> {
    inner: std::collections::VecDeque<T>,
}

impl<T> CircularVec<T> {
    /// Creates a new CircularVec with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: std::collections::VecDeque::with_capacity(capacity),
        }
    }

    /// Pushes an item into the CircularVec. If the CircularVec is full, the oldest item is removed.
    #[allow(clippy::result_large_err)]
    pub fn push(&mut self, item: T) -> Result<(), Error> {
        if self.inner.len() == self.inner.capacity() {
            self.inner
                .pop_front()
                .ok_or(Error::CircularVecPopFrontError)?;
        }
        self.inner.push_back(item);
        Ok(())
    }

    /// Checks if the CircularVec contains the given item.
    pub fn contains(&self, item: &T) -> bool
    where
        T: PartialEq,
    {
        self.inner.contains(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_contains() {
        let mut cv = CircularVec::new(2);
        assert!(cv.push(1).is_ok());
        assert!(cv.push(2).is_ok());
        assert!(cv.contains(&1));
        assert!(cv.contains(&2));

        assert!(cv.push(3).is_ok());
        assert!(!cv.contains(&1));
        assert!(cv.contains(&2));
        assert!(cv.contains(&3));
    }
}
