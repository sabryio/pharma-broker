//! Phase 2: Reliability Integration Tests
//!
//! Tests for queue processing and retry logic.
//! See: docs/phases/02-reliability.md

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use pharma_core::queue::{MessageQueue, QueueConfig, QueueMessage};
use pharma_core::retry::{RetryConfig, with_retry};

/// Test message type for queue tests
#[derive(Debug, Clone)]
struct TestMessage {
    id: String,
}

impl QueueMessage for TestMessage {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Test message queue basic operations
#[tokio::test]
async fn test_message_queue_enqueue_dequeue() {
    let queue: MessageQueue<TestMessage> = MessageQueue::new(QueueConfig::default());

    // Enqueue items
    queue
        .try_enqueue(TestMessage {
            id: "1".to_string(),
        })
        .await
        .unwrap();
    queue
        .try_enqueue(TestMessage {
            id: "2".to_string(),
        })
        .await
        .unwrap();
    queue
        .try_enqueue(TestMessage {
            id: "3".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(queue.len().await, 3);

    // Dequeue item
    let item = queue.dequeue().await;
    assert!(item.is_some());
    assert_eq!(item.unwrap().id, "1");
    assert_eq!(queue.len().await, 2);
}

/// Test queue backpressure
#[tokio::test]
async fn test_queue_backpressure() {
    let config = QueueConfig {
        max_size: 3,
        max_workers: 1,
        process_timeout: Duration::from_secs(1),
    };
    let queue: MessageQueue<TestMessage> = MessageQueue::new(config);

    // Fill the queue
    for i in 0..3 {
        queue
            .try_enqueue(TestMessage { id: i.to_string() })
            .await
            .unwrap();
    }

    // Queue should now be full
    let result = queue
        .try_enqueue(TestMessage {
            id: "overflow".to_string(),
        })
        .await;
    assert!(result.is_err());
}

/// Test retry with successful operation
#[tokio::test]
async fn test_retry_success_first_attempt() {
    let config = RetryConfig::default();

    let result = with_retry(config, || async { Ok::<_, &str>("success") }, |_| true).await;

    assert!(result.result.is_ok());
    assert_eq!(result.attempts, 1);
    assert_eq!(result.result.unwrap(), "success");
}

/// Test retry with eventual success
#[tokio::test]
async fn test_retry_eventual_success() {
    let config = RetryConfig {
        max_attempts: 5,
        initial_delay: Duration::from_millis(10),
        max_delay: Duration::from_millis(100),
        multiplier: 2.0,
        jitter: false,
    };

    let attempt_count = Arc::new(AtomicU32::new(0));
    let counter = attempt_count.clone();

    let result = with_retry(
        config,
        move || {
            let count = counter.clone();
            async move {
                let attempts = count.fetch_add(1, Ordering::SeqCst) + 1;
                if attempts < 3 {
                    Err("retry me")
                } else {
                    Ok("finally success")
                }
            }
        },
        |_| true,
    )
    .await;

    assert!(result.result.is_ok());
    assert_eq!(result.result.unwrap(), "finally success");
    assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
}

/// Test retry exhausts attempts
#[tokio::test]
async fn test_retry_exhausted() {
    let config = RetryConfig {
        max_attempts: 3,
        initial_delay: Duration::from_millis(5),
        max_delay: Duration::from_millis(20),
        multiplier: 2.0,
        jitter: false,
    };

    let attempt_count = Arc::new(AtomicU32::new(0));
    let counter = attempt_count.clone();

    let result = with_retry(
        config,
        move || {
            let count = counter.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>("always fail")
            }
        },
        |_| true,
    )
    .await;

    assert!(result.result.is_err());
    assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
}
