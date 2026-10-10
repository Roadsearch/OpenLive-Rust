//! Thread-safe shared wrapper around the bounded media queue.
//!
//! Locking is intentionally brief: no callbacks or media processing run while
//! the queue mutex is held. This wrapper is suitable for simple producer /
//! consumer hand-off, but it is not a waitable blocking channel.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::bounded_queue::{BoundedQueue, OverflowPolicy, PushResult, QueueCapacityError};

/// A snapshot of queue state, useful for diagnostics and overload monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedQueueStats {
    pub capacity: usize,
    pub len: usize,
    pub dropped_items: u64,
}

/// Cloneable, thread-safe access to one bounded FIFO.
#[derive(Debug)]
pub struct SharedBoundedQueue<T> {
    inner: Arc<Mutex<BoundedQueue<T>>>,
}

impl<T> Clone for SharedBoundedQueue<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> SharedBoundedQueue<T> {
    /// Creates a queue shared by all clones. Capacity must be non-zero.
    pub fn new(
        capacity: usize,
        policy: OverflowPolicy,
    ) -> Result<Self, QueueCapacityError> {
        Ok(Self {
            inner: Arc::new(Mutex::new(BoundedQueue::new(capacity, policy)?)),
        })
    }

    fn lock(&self) -> MutexGuard<'_, BoundedQueue<T>> {
        // Recover the queue after a panic while locked; keep the queue usable.
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Attempts to enqueue without exceeding the configured capacity.
    pub fn push(&self, item: T) -> PushResult<T> {
        self.lock().push(item)
    }

    /// Removes the oldest item, or returns no item if the queue is empty.
    pub fn pop(&self) -> Option<T> {
        self.lock().pop()
    }

    /// Removes all queued items. Drop diagnostics are preserved.
    pub fn clear(&self) {
        self.lock().clear();
    }

    /// Returns a consistent snapshot of queue capacity, length, and drops.
    pub fn stats(&self) -> SharedQueueStats {
        let queue = self.lock();
        SharedQueueStats {
            capacity: queue.capacity(),
            len: queue.len(),
            dropped_items: queue.dropped_items(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.lock().is_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn clones_share_the_same_queue_and_diagnostics() {
        let producer = SharedBoundedQueue::new(2, OverflowPolicy::DropOldest).unwrap();
        let consumer = producer.clone();

        assert_eq!(producer.push(10), PushResult::Enqueued);
        assert_eq!(producer.push(20), PushResult::Enqueued);
        assert_eq!(consumer.push(30), PushResult::EnqueuedDroppingOldest(10));
        assert_eq!(consumer.pop(), Some(20));
        assert_eq!(producer.pop(), Some(30));
        assert_eq!(
            producer.stats(),
            SharedQueueStats {
                capacity: 2,
                len: 0,
                dropped_items: 1,
            }
        );
    }

    #[test]
    fn supports_concurrent_producer_and_consumer_access() {
        let queue = SharedBoundedQueue::new(64, OverflowPolicy::DropNewest).unwrap();
        let producer_queue = queue.clone();
        let producer = thread::spawn(move || {
            for value in 0..10_000 {
                let _ = producer_queue.push(value);
            }
        });

        let observer = producer.clone();
        let consumer_queue = queue.clone();
        let consumer = thread::spawn(move || {
            let mut consumed = 0usize;
            while consumed < 10_000 {
                if consumer_queue.pop().is_some() {
                    consumed += 1;
                } else if observer.is_finished() {
                    break;
                } else {
                    thread::yield_now();
                }
            }
            consumed
        });

        let _ = producer.join();
        let _ = consumer.join();
        assert!(queue.stats().len <= 64);
    }

    #[test]
    fn rejects_zero_capacity() {
        assert!(SharedBoundedQueue::<u8>::new(0, OverflowPolicy::DropNewest).is_err());
    }
}
