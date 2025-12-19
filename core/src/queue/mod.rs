//! Message Queue Module
//!
//! Provides a bounded, async message queue with backpressure handling.
//! Prevents memory exhaustion under high load by limiting concurrent processing.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, Semaphore};
use tokio::time::timeout;

/// Configuration for the message queue
#[derive(Debug, Clone)]
pub struct QueueConfig {
    /// Maximum number of messages that can be queued
    pub max_size: usize,
    /// Maximum number of concurrent workers processing messages
    pub max_workers: usize,
    /// Timeout for processing a single message
    pub process_timeout: Duration,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_size: 1000,
            max_workers: 5,
            process_timeout: Duration::from_secs(30),
        }
    }
}

/// Error type for queue operations
#[derive(Debug, Clone, PartialEq)]
pub enum QueueError {
    /// Queue is full, cannot accept more messages
    QueueFull,
    /// Queue has been closed
    QueueClosed,
    /// Processing timed out
    ProcessTimeout,
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueError::QueueFull => write!(f, "Queue is full"),
            QueueError::QueueClosed => write!(f, "Queue is closed"),
            QueueError::ProcessTimeout => write!(f, "Processing timed out"),
        }
    }
}

impl std::error::Error for QueueError {}

/// A message that can be queued
pub trait QueueMessage: Send + Sync + Clone + 'static {
    /// Unique identifier for the message
    fn id(&self) -> &str;
}

/// Statistics about the queue
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    pub queued: u64,
    pub processed: u64,
    pub failed: u64,
    pub rejected: u64,
    pub current_size: usize,
    pub in_flight: usize,
}

/// Bounded message queue with backpressure
pub struct MessageQueue<T: QueueMessage> {
    config: QueueConfig,
    buffer: Arc<Mutex<VecDeque<T>>>,
    semaphore: Arc<Semaphore>,
    stats: Arc<Mutex<QueueStats>>,
    closed: Arc<std::sync::atomic::AtomicBool>,
}

impl<T: QueueMessage> MessageQueue<T> {
    /// Create a new message queue with the given configuration
    pub fn new(config: QueueConfig) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(config.max_workers)),
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(config.max_size))),
            stats: Arc::new(Mutex::new(QueueStats::default())),
            closed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            config,
        }
    }

    /// Try to enqueue a message (non-blocking)
    /// Returns QueueError::QueueFull if the queue is at capacity
    pub async fn try_enqueue(&self, msg: T) -> Result<(), QueueError> {
        if self.closed.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(QueueError::QueueClosed);
        }

        let mut buffer = self.buffer.lock().await;
        if buffer.len() >= self.config.max_size {
            let mut stats = self.stats.lock().await;
            stats.rejected += 1;
            return Err(QueueError::QueueFull);
        }

        buffer.push_back(msg);
        let mut stats = self.stats.lock().await;
        stats.queued += 1;
        stats.current_size = buffer.len();
        Ok(())
    }

    /// Get the next message from the queue (blocks until available or closed)
    pub async fn dequeue(&self) -> Option<T> {
        loop {
            if self.closed.load(std::sync::atomic::Ordering::SeqCst) {
                return None;
            }

            {
                let mut buffer = self.buffer.lock().await;
                if let Some(msg) = buffer.pop_front() {
                    let mut stats = self.stats.lock().await;
                    stats.current_size = buffer.len();
                    stats.in_flight += 1;
                    return Some(msg);
                }
            }

            // Wait a bit before checking again
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Process a message with the given handler (respects concurrency limit)
    pub async fn process_with_handler<F, Fut>(&self, msg: T, handler: F) -> Result<(), QueueError>
    where
        F: FnOnce(T) -> Fut + Send,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>
            + Send,
    {
        // Acquire semaphore permit (limits concurrent workers)
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| QueueError::QueueClosed)?;

        let result = timeout(self.config.process_timeout, handler(msg)).await;

        let mut stats = self.stats.lock().await;
        stats.in_flight = stats.in_flight.saturating_sub(1);

        match result {
            Ok(Ok(())) => {
                stats.processed += 1;
                Ok(())
            }
            Ok(Err(_)) => {
                stats.failed += 1;
                Ok(())
            }
            Err(_) => {
                stats.failed += 1;
                Err(QueueError::ProcessTimeout)
            }
        }
    }

    /// Get current queue statistics
    pub async fn stats(&self) -> QueueStats {
        let stats = self.stats.lock().await;
        stats.clone()
    }

    /// Get current queue size
    pub async fn len(&self) -> usize {
        self.buffer.lock().await.len()
    }

    /// Check if queue is empty
    pub async fn is_empty(&self) -> bool {
        self.buffer.lock().await.is_empty()
    }

    /// Close the queue (no more messages will be accepted)
    pub fn close(&self) {
        self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Check if queue is closed
    pub fn is_closed(&self) -> bool {
        self.closed.load(std::sync::atomic::Ordering::SeqCst)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestMessage {
        id: String,
        content: String,
    }

    impl QueueMessage for TestMessage {
        fn id(&self) -> &str {
            &self.id
        }
    }

    #[tokio::test]
    async fn test_queue_enqueue_dequeue() {
        let queue = MessageQueue::new(QueueConfig::default());

        let msg = TestMessage {
            id: "1".to_string(),
            content: "test".to_string(),
        };

        queue.try_enqueue(msg.clone()).await.unwrap();
        assert_eq!(queue.len().await, 1);

        let dequeued = queue.dequeue().await.unwrap();
        assert_eq!(dequeued.id, "1");
        assert!(queue.is_empty().await);
    }

    #[tokio::test]
    async fn test_queue_backpressure() {
        let config = QueueConfig {
            max_size: 3,
            max_workers: 1,
            process_timeout: Duration::from_secs(1),
        };
        let queue = MessageQueue::new(config);

        // Fill the queue
        for i in 0..3 {
            let msg = TestMessage {
                id: i.to_string(),
                content: format!("msg{}", i),
            };
            queue.try_enqueue(msg).await.unwrap();
        }

        // Queue should now be full
        let msg = TestMessage {
            id: "overflow".to_string(),
            content: "should fail".to_string(),
        };

        let result = queue.try_enqueue(msg).await;
        assert_eq!(result, Err(QueueError::QueueFull));

        // Stats should show rejection
        let stats = queue.stats().await;
        assert_eq!(stats.queued, 3);
        assert_eq!(stats.rejected, 1);
    }

    #[tokio::test]
    async fn test_queue_concurrent_workers() {
        let config = QueueConfig {
            max_size: 10,
            max_workers: 2,
            process_timeout: Duration::from_secs(5),
        };
        let queue = Arc::new(MessageQueue::new(config));

        // Enqueue 5 messages
        for i in 0..5 {
            let msg = TestMessage {
                id: i.to_string(),
                content: format!("msg{}", i),
            };
            queue.try_enqueue(msg).await.unwrap();
        }

        let processed = Arc::new(std::sync::atomic::AtomicU32::new(0));

        // Process all messages with workers
        let mut handles = vec![];
        for _ in 0..5 {
            if let Some(msg) = queue.dequeue().await {
                let q = Arc::clone(&queue);
                let p = Arc::clone(&processed);
                let handle = tokio::spawn(async move {
                    q.process_with_handler(msg, |_m| async move {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        p.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        Ok(())
                    })
                    .await
                    .ok();
                });
                handles.push(handle);
            }
        }

        for handle in handles {
            handle.await.unwrap();
        }

        assert_eq!(processed.load(std::sync::atomic::Ordering::SeqCst), 5);

        let stats = queue.stats().await;
        assert_eq!(stats.processed, 5);
    }

    #[tokio::test]
    async fn test_queue_closed() {
        let queue = MessageQueue::new(QueueConfig::default());

        queue.close();

        let msg = TestMessage {
            id: "1".to_string(),
            content: "test".to_string(),
        };

        let result = queue.try_enqueue(msg).await;
        assert_eq!(result, Err(QueueError::QueueClosed));
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let queue = MessageQueue::new(QueueConfig::default());

        for i in 0..3 {
            let msg = TestMessage {
                id: i.to_string(),
                content: format!("msg{}", i),
            };
            queue.try_enqueue(msg).await.unwrap();
        }

        let stats = queue.stats().await;
        assert_eq!(stats.queued, 3);
        assert_eq!(stats.current_size, 3);
        assert_eq!(stats.processed, 0);
        assert_eq!(stats.failed, 0);
    }
}
