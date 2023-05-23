#[derive(Debug)]
pub(super) struct CircularVec<T> {
    inner: std::collections::VecDeque<T>,
}

impl<T> CircularVec<T> {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            inner: std::collections::VecDeque::with_capacity(capacity),
        }
    }

    pub(crate) fn push(&mut self, item: T) {
        if self.inner.len() == self.inner.capacity() {
            let _ = self.inner.pop_front();
        }
        self.inner.push_back(item);
    }

    pub(crate) fn contains(&self, item: &T) -> bool
    where
        T: PartialEq,
    {
        self.inner.contains(item)
    }
}
