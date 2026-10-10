//! Bounded queue primitives for media pipelines.
//!
//! The queue never grows beyond its configured capacity. Select the overflow
//! policy according to the payload: audio usually prefers back-pressure or
//! explicit failure, while preview video may prefer dropping the oldest frame.

use std::collections::VecDeque;

/// What to do when pushing into a full queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// Keep queued data and reject the incoming item.
    DropNewest,
    /// Accept the incoming item and remove the oldest queued item.
    DropOldest,
}

/// Result of attempting to enqueue an item.
#[derive(Debug, PartialEq, Eq)]
pub enum PushResult<T> {
    Enqueued,
    DroppedNewest(T),
    EnqueuedDroppingOldest(T),
}

/// Construction error for a queue that cannot have zero capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueCapacityError;

/// A bounded FIFO. It is not internally synchronized; wrap it in a mutex when
/// sharing it between producer and consumer threads.
#[derive(Debug, Clone)]
pub struct BoundedQueue<T> {
    items: VecDeque<T>,
    capacity: usize,
    policy: OverflowPolicy,
    dropped_items: u64,
}

impl<T> BoundedQueue<T> {
    pub fn new(capacity: usize, policy: OverflowPolicy) -> Result<Self, QueueCapacityError> {
        if capacity == 0 {
            return Err(QueueCapacityError);
        }
        Ok(Self {
            items: VecDeque::with_capacity(capacity),
            capacity,
            policy,
            dropped_items: 0,
        })
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.items.len() == self.capacity
    }

    pub fn dropped_items(&self) -> u64 {
        self.dropped_items
    }

    pub fn push(&mut self, item: T) -> PushResult<T> {
        if !self.is_full() {
            self.items.push_back(item);
            return PushResult::Enqueued;
        }

        self.dropped_items = self.dropped_items.saturating_add(1);
        match self.policy {
            OverflowPolicy::DropNewest => PushResult::DroppedNewest(item),
            OverflowPolicy::DropOldest => {
                let dropped = self.items.pop_front().expect("full queue has an item");
                self.items.push_back(item);
                PushResult::EnqueuedDroppingOldest(dropped)
            }
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_capacity() {
        assert!(BoundedQueue::<u8>::new(0, OverflowPolicy::DropNewest).is_err());
    }

    #[test]
    fn drop_newest_preserves_existing_queue_contents() {
        let mut queue = BoundedQueue::new(2, OverflowPolicy::DropNewest).unwrap();
        assert_eq!(queue.push(1), PushResult::Enqueued);
        assert_eq!(queue.push(2), PushResult::Enqueued);
        assert_eq!(queue.push(3), PushResult::DroppedNewest(3));
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.dropped_items(), 1);
    }

    #[test]
    fn drop_oldest_keeps_the_newest_frames_and_stays_bounded() {
        let mut queue = BoundedQueue::new(2, OverflowPolicy::DropOldest).unwrap();
        assert_eq!(queue.push(1), PushResult::Enqueued);
        assert_eq!(queue.push(2), PushResult::Enqueued);
        assert_eq!(queue.push(3), PushResult::EnqueuedDroppingOldest(1));
        assert_eq!(queue.len(), 2);
        assert!(queue.is_full());
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.dropped_items(), 1);
    }

    #[test]
    fn clear_removes_items_without_resetting_drop_diagnostics() {
        let mut queue = BoundedQueue::new(1, OverflowPolicy::DropNewest).unwrap();
        queue.push(1);
        assert_eq!(queue.push(2), PushResult::DroppedNewest(2));
        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(queue.dropped_items(), 1);
    }
}
