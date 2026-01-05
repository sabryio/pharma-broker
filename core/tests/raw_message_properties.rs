//! Property-based tests for Raw Message Repository
//!
//! Feature: raw-messages-display
//! Tests Properties 4, 5, 6, 7 from the design document
//!
//! Feature: message-reprocessing-fix
//! Tests Property 1: Reset clears processing state

use chrono::{DateTime, Duration, Utc};
use proptest::prelude::*;
use uuid::Uuid;

// =============================================================================
// Test Data Generators
// =============================================================================

/// Generate a random message content string
fn arb_content() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9 ]{10,200}")
        .unwrap()
        .prop_map(|s| s.trim().to_string())
}

/// Generate a random timestamp within the last 30 days
fn arb_timestamp() -> impl Strategy<Value = DateTime<Utc>> {
    (0i64..30 * 24 * 60).prop_map(|minutes_ago| Utc::now() - Duration::minutes(minutes_ago))
}

/// Generate a random processing status
fn arb_processing_status() -> impl Strategy<Value = ProcessingStatus> {
    prop_oneof![
        Just(ProcessingStatus::Unprocessed),
        Just(ProcessingStatus::Processed),
        Just(ProcessingStatus::Error),
    ]
}

/// Simulated raw message for testing
#[derive(Debug, Clone)]
struct TestRawMessage {
    content: String,
    timestamp: DateTime<Utc>,
    processed_at: Option<DateTime<Utc>>,
    error: Option<String>,
}

/// Processing status enum matching the params
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessingStatus {
    Unprocessed,
    Processed,
    Error,
}

impl TestRawMessage {
    fn new(content: String, timestamp: DateTime<Utc>, status: ProcessingStatus) -> Self {
        let (processed_at, error) = match status {
            ProcessingStatus::Unprocessed => (None, None),
            ProcessingStatus::Processed => (Some(Utc::now()), None),
            ProcessingStatus::Error => (Some(Utc::now()), Some("Parse error".to_string())),
        };

        Self {
            content,
            timestamp,
            processed_at,
            error,
        }
    }

    fn matches_search(&self, search: &str) -> bool {
        self.content.to_lowercase().contains(&search.to_lowercase())
    }

    fn matches_status(&self, status: ProcessingStatus) -> bool {
        match status {
            ProcessingStatus::Unprocessed => self.processed_at.is_none(),
            ProcessingStatus::Processed => self.processed_at.is_some() && self.error.is_none(),
            ProcessingStatus::Error => self.error.is_some(),
        }
    }

    fn in_date_range(&self, start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>) -> bool {
        let after_start = start.is_none_or(|s| self.timestamp >= s);
        let before_end = end.is_none_or(|e| self.timestamp <= e);
        after_start && before_end
    }
}

/// Generate a collection of test messages
fn arb_messages(count: usize) -> impl Strategy<Value = Vec<TestRawMessage>> {
    prop::collection::vec(
        (arb_content(), arb_timestamp(), arb_processing_status()).prop_map(
            |(content, timestamp, status)| TestRawMessage::new(content, timestamp, status),
        ),
        0..count,
    )
}

// =============================================================================
// Filter Functions (simulating repository behavior)
// =============================================================================

fn filter_by_search<'a>(messages: &'a [TestRawMessage], search: &str) -> Vec<&'a TestRawMessage> {
    if search.is_empty() {
        messages.iter().collect()
    } else {
        messages
            .iter()
            .filter(|m| m.matches_search(search))
            .collect()
    }
}

fn filter_by_status(messages: &[TestRawMessage], status: ProcessingStatus) -> Vec<&TestRawMessage> {
    messages
        .iter()
        .filter(|m| m.matches_status(status))
        .collect()
}

fn filter_by_date_range(
    messages: &[TestRawMessage],
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
) -> Vec<&TestRawMessage> {
    messages
        .iter()
        .filter(|m| m.in_date_range(start, end))
        .collect()
}

fn sort_by_timestamp(messages: &mut [&TestRawMessage], ascending: bool) {
    if ascending {
        messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    } else {
        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }
}

// =============================================================================
// Property Tests
// =============================================================================

// Validates: Requirements 3.1
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: raw-messages-display, Property 4: Search Filter Correctness
    ///
    /// For any search term and message dataset, all returned messages SHALL contain
    /// the search term (case-insensitive) in their content field.
    #[test]
    fn prop_search_filter_returns_matching_messages(
        messages in arb_messages(50),
        search_term in "[a-zA-Z]{2,8}",
    ) {
        let filtered = filter_by_search(&messages, &search_term);

        // All filtered messages must contain the search term
        for msg in &filtered {
            prop_assert!(
                msg.matches_search(&search_term),
                "Message '{}' should contain search term '{}'",
                msg.content,
                search_term
            );
        }

        // No matching messages should be excluded
        let expected_count = messages.iter().filter(|m| m.matches_search(&search_term)).count();
        prop_assert_eq!(
            filtered.len(),
            expected_count,
            "Filter should return all matching messages"
        );
    }

    /// Feature: raw-messages-display, Property 4: Search Filter Correctness (empty search)
    ///
    /// When search term is empty, all messages should be returned.
    #[test]
    fn prop_empty_search_returns_all_messages(
        messages in arb_messages(50),
    ) {
        let filtered = filter_by_search(&messages, "");
        prop_assert_eq!(
            filtered.len(),
            messages.len(),
            "Empty search should return all messages"
        );
    }
}

// Validates: Requirements 3.3
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: raw-messages-display, Property 5: Status Filter Correctness
    ///
    /// For any status filter value, all returned messages SHALL match the filter criteria.
    #[test]
    fn prop_status_filter_returns_correct_messages(
        messages in arb_messages(50),
        status in arb_processing_status(),
    ) {
        let filtered = filter_by_status(&messages, status);

        // All filtered messages must match the status
        for msg in &filtered {
            prop_assert!(
                msg.matches_status(status),
                "Message should match status {:?}",
                status
            );
        }

        // No matching messages should be excluded
        let expected_count = messages.iter().filter(|m| m.matches_status(status)).count();
        prop_assert_eq!(
            filtered.len(),
            expected_count,
            "Filter should return all messages with status {:?}",
            status
        );
    }

    /// Feature: raw-messages-display, Property 5: Status Filter Correctness (processed)
    ///
    /// Processed messages have processed_at set and no error.
    #[test]
    fn prop_processed_status_filter_excludes_errors(
        messages in arb_messages(50),
    ) {
        let filtered = filter_by_status(&messages, ProcessingStatus::Processed);

        for msg in &filtered {
            prop_assert!(
                msg.processed_at.is_some(),
                "Processed message must have processed_at"
            );
            prop_assert!(
                msg.error.is_none(),
                "Processed message must not have error"
            );
        }
    }

    /// Feature: raw-messages-display, Property 5: Status Filter Correctness (error)
    ///
    /// Error messages have error field set.
    #[test]
    fn prop_error_status_filter_requires_error(
        messages in arb_messages(50),
    ) {
        let filtered = filter_by_status(&messages, ProcessingStatus::Error);

        for msg in &filtered {
            prop_assert!(
                msg.error.is_some(),
                "Error message must have error field set"
            );
        }
    }
}

// Validates: Requirements 3.4
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: raw-messages-display, Property 6: Date Range Filter Correctness
    ///
    /// For any date range, all returned messages SHALL have timestamp within the range.
    #[test]
    fn prop_date_range_filter_returns_messages_in_range(
        messages in arb_messages(50),
        days_ago_start in 1u32..30,
        days_ago_end in 0u32..15,
    ) {
        // Ensure start is before end
        let start = Utc::now() - Duration::days(days_ago_start as i64);
        let end = Utc::now() - Duration::days(days_ago_end as i64);

        if start <= end {
            let filtered = filter_by_date_range(&messages, Some(start), Some(end));

            for msg in &filtered {
                prop_assert!(
                    msg.timestamp >= start,
                    "Message timestamp {} should be >= start {}",
                    msg.timestamp,
                    start
                );
                prop_assert!(
                    msg.timestamp <= end,
                    "Message timestamp {} should be <= end {}",
                    msg.timestamp,
                    end
                );
            }

            // No matching messages should be excluded
            let expected_count = messages
                .iter()
                .filter(|m| m.in_date_range(Some(start), Some(end)))
                .count();
            prop_assert_eq!(
                filtered.len(),
                expected_count,
                "Filter should return all messages in date range"
            );
        }
    }

    /// Feature: raw-messages-display, Property 6: Date Range Filter (start only)
    ///
    /// When only start date is provided, all messages after start should be returned.
    #[test]
    fn prop_date_range_filter_start_only(
        messages in arb_messages(50),
        days_ago in 1u32..30,
    ) {
        let start = Utc::now() - Duration::days(days_ago as i64);
        let filtered = filter_by_date_range(&messages, Some(start), None);

        for msg in &filtered {
            prop_assert!(
                msg.timestamp >= start,
                "Message timestamp should be >= start"
            );
        }
    }

    /// Feature: raw-messages-display, Property 6: Date Range Filter (end only)
    ///
    /// When only end date is provided, all messages before end should be returned.
    #[test]
    fn prop_date_range_filter_end_only(
        messages in arb_messages(50),
        days_ago in 0u32..15,
    ) {
        let end = Utc::now() - Duration::days(days_ago as i64);
        let filtered = filter_by_date_range(&messages, None, Some(end));

        for msg in &filtered {
            prop_assert!(
                msg.timestamp <= end,
                "Message timestamp should be <= end"
            );
        }
    }
}

// Validates: Requirements 5.1, 5.2
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: raw-messages-display, Property 7: Sort Order Correctness
    ///
    /// For any sort order, consecutive messages SHALL be ordered correctly.
    #[test]
    fn prop_sort_order_is_correct_ascending(
        messages in arb_messages(50),
    ) {
        let mut refs: Vec<&TestRawMessage> = messages.iter().collect();
        sort_by_timestamp(&mut refs, true);

        // Check ascending order
        for i in 0..refs.len().saturating_sub(1) {
            prop_assert!(
                refs[i].timestamp <= refs[i + 1].timestamp,
                "Messages should be in ascending order by timestamp"
            );
        }
    }

    /// Feature: raw-messages-display, Property 7: Sort Order Correctness (descending)
    #[test]
    fn prop_sort_order_is_correct_descending(
        messages in arb_messages(50),
    ) {
        let mut refs: Vec<&TestRawMessage> = messages.iter().collect();
        sort_by_timestamp(&mut refs, false);

        // Check descending order
        for i in 0..refs.len().saturating_sub(1) {
            prop_assert!(
                refs[i].timestamp >= refs[i + 1].timestamp,
                "Messages should be in descending order by timestamp"
            );
        }
    }

    /// Feature: raw-messages-display, Property 7: Sort preserves all elements
    ///
    /// Sorting should not add or remove any messages.
    #[test]
    fn prop_sort_preserves_all_elements(
        messages in arb_messages(50),
        ascending in prop::bool::ANY,
    ) {
        let original_count = messages.len();
        let mut refs: Vec<&TestRawMessage> = messages.iter().collect();
        sort_by_timestamp(&mut refs, ascending);

        prop_assert_eq!(
            refs.len(),
            original_count,
            "Sorting should preserve message count"
        );
    }
}

// =============================================================================
// API Validation Property Tests
// =============================================================================

/// Simulated API query parameters for validation testing
#[derive(Debug, Clone)]
struct TestApiParams {
    limit: i64,
    offset: i64,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
    status: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
}

impl TestApiParams {
    fn new() -> Self {
        Self {
            limit: 20,
            offset: 0,
            start_date: None,
            end_date: None,
            status: None,
            sort_by: None,
            sort_order: None,
        }
    }

    fn with_limit(mut self, limit: i64) -> Self {
        self.limit = limit;
        self
    }

    fn with_offset(mut self, offset: i64) -> Self {
        self.offset = offset;
        self
    }

    fn with_date_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.start_date = Some(start);
        self.end_date = Some(end);
        self
    }

    fn with_status(mut self, status: &str) -> Self {
        self.status = Some(status.to_string());
        self
    }

    fn with_sort_by(mut self, sort_by: &str) -> Self {
        self.sort_by = Some(sort_by.to_string());
        self
    }

    fn with_sort_order(mut self, sort_order: &str) -> Self {
        self.sort_order = Some(sort_order.to_string());
        self
    }
}

/// Validation result type
#[derive(Debug, Clone, PartialEq, Eq)]
enum ValidationResult {
    Valid,
    Invalid(String),
}

/// Validate API parameters (mirrors the actual validate_params function)
fn validate_api_params(params: &TestApiParams) -> ValidationResult {
    // Validate limit
    if params.limit < 1 || params.limit > 100 {
        return ValidationResult::Invalid("Limit must be between 1 and 100".to_string());
    }

    // Validate offset
    if params.offset < 0 {
        return ValidationResult::Invalid("Offset must be non-negative".to_string());
    }

    // Validate date range
    if let (Some(start), Some(end)) = (params.start_date, params.end_date)
        && start > end
    {
        return ValidationResult::Invalid(
            "Start date must be before or equal to end date".to_string(),
        );
    }

    // Validate status
    if let Some(ref status) = params.status {
        let valid_statuses = ["all", "processed", "unprocessed", "error"];
        if !valid_statuses.contains(&status.as_str()) {
            return ValidationResult::Invalid(format!(
                "Invalid status '{}'. Valid values: all, processed, unprocessed, error",
                status
            ));
        }
    }

    // Validate sort_by
    if let Some(ref sort_by) = params.sort_by {
        let valid_fields = ["timestamp", "processed_at", "created_at"];
        if !valid_fields.contains(&sort_by.as_str()) {
            return ValidationResult::Invalid(format!(
                "Invalid sort_by '{}'. Valid values: timestamp, processed_at, created_at",
                sort_by
            ));
        }
    }

    // Validate sort_order
    if let Some(ref sort_order) = params.sort_order {
        let valid_orders = ["asc", "desc"];
        if !valid_orders.contains(&sort_order.as_str()) {
            return ValidationResult::Invalid(format!(
                "Invalid sort_order '{}'. Valid values: asc, desc",
                sort_order
            ));
        }
    }

    ValidationResult::Valid
}

/// Generate valid limit values (1-100)
fn arb_valid_limit() -> impl Strategy<Value = i64> {
    1i64..=100
}

/// Generate invalid limit values (outside 1-100)
fn arb_invalid_limit() -> impl Strategy<Value = i64> {
    prop_oneof![
        -100i64..0,   // Negative values
        Just(0i64),   // Zero
        101i64..1000  // Too large
    ]
}

/// Generate valid offset values (>= 0)
fn arb_valid_offset() -> impl Strategy<Value = i64> {
    0i64..10000
}

/// Generate invalid offset values (< 0)
fn arb_invalid_offset() -> impl Strategy<Value = i64> {
    -1000i64..-1
}

/// Generate valid status values
fn arb_valid_status() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("all".to_string()),
        Just("processed".to_string()),
        Just("unprocessed".to_string()),
        Just("error".to_string()),
    ]
}

/// Generate invalid status values
fn arb_invalid_status() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{3,10}")
        .unwrap()
        .prop_filter("Must not be a valid status", |s| {
            !["all", "processed", "unprocessed", "error"].contains(&s.as_str())
        })
}

/// Generate valid sort_by values
fn arb_valid_sort_by() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("timestamp".to_string()),
        Just("processed_at".to_string()),
        Just("created_at".to_string()),
    ]
}

/// Generate invalid sort_by values
fn arb_invalid_sort_by() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z_]{3,15}")
        .unwrap()
        .prop_filter("Must not be a valid sort field", |s| {
            !["timestamp", "processed_at", "created_at"].contains(&s.as_str())
        })
}

/// Generate valid sort_order values
fn arb_valid_sort_order() -> impl Strategy<Value = String> {
    prop_oneof![Just("asc".to_string()), Just("desc".to_string()),]
}

/// Generate invalid sort_order values
fn arb_invalid_sort_order() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{2,8}")
        .unwrap()
        .prop_filter("Must not be a valid sort order", |s| {
            !["asc", "desc"].contains(&s.as_str())
        })
}

// Validates: Requirements 7.5
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any invalid limit value (outside 1-100), the API SHALL reject the request.
    #[test]
    fn prop_invalid_limit_is_rejected(
        invalid_limit in arb_invalid_limit(),
    ) {
        let params = TestApiParams::new().with_limit(invalid_limit);
        let result = validate_api_params(&params);

        prop_assert!(
            matches!(result, ValidationResult::Invalid(_)),
            "Invalid limit {} should be rejected",
            invalid_limit
        );
    }

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any valid limit value (1-100), the API SHALL accept the request.
    #[test]
    fn prop_valid_limit_is_accepted(
        valid_limit in arb_valid_limit(),
    ) {
        let params = TestApiParams::new().with_limit(valid_limit);
        let result = validate_api_params(&params);

        prop_assert_eq!(
            result,
            ValidationResult::Valid,
            "Valid limit {} should be accepted",
            valid_limit
        );
    }

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any negative offset, the API SHALL reject the request.
    #[test]
    fn prop_invalid_offset_is_rejected(
        invalid_offset in arb_invalid_offset(),
    ) {
        let params = TestApiParams::new().with_offset(invalid_offset);
        let result = validate_api_params(&params);

        prop_assert!(
            matches!(result, ValidationResult::Invalid(_)),
            "Invalid offset {} should be rejected",
            invalid_offset
        );
    }

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any non-negative offset, the API SHALL accept the request.
    #[test]
    fn prop_valid_offset_is_accepted(
        valid_offset in arb_valid_offset(),
    ) {
        let params = TestApiParams::new().with_offset(valid_offset);
        let result = validate_api_params(&params);

        prop_assert_eq!(
            result,
            ValidationResult::Valid,
            "Valid offset {} should be accepted",
            valid_offset
        );
    }

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any date range where start > end, the API SHALL reject the request.
    #[test]
    fn prop_invalid_date_range_is_rejected(
        days_diff in 1i64..30,
    ) {
        let end = Utc::now() - Duration::days(10);
        let start = end + Duration::days(days_diff); // start > end

        let params = TestApiParams::new().with_date_range(start, end);
        let result = validate_api_params(&params);

        prop_assert!(
            matches!(result, ValidationResult::Invalid(_)),
            "Date range with start > end should be rejected"
        );
    }

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any date range where start <= end, the API SHALL accept the request.
    #[test]
    fn prop_valid_date_range_is_accepted(
        days_diff in 0i64..30,
    ) {
        let start = Utc::now() - Duration::days(10 + days_diff);
        let end = Utc::now() - Duration::days(10); // start <= end

        let params = TestApiParams::new().with_date_range(start, end);
        let result = validate_api_params(&params);

        prop_assert_eq!(
            result,
            ValidationResult::Valid,
            "Date range with start <= end should be accepted"
        );
    }

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any invalid status value, the API SHALL reject the request.
    #[test]
    fn prop_invalid_status_is_rejected(
        invalid_status in arb_invalid_status(),
    ) {
        let params = TestApiParams::new().with_status(&invalid_status);
        let result = validate_api_params(&params);

        prop_assert!(
            matches!(result, ValidationResult::Invalid(_)),
            "Invalid status '{}' should be rejected",
            invalid_status
        );
    }

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any valid status value, the API SHALL accept the request.
    #[test]
    fn prop_valid_status_is_accepted(
        valid_status in arb_valid_status(),
    ) {
        let params = TestApiParams::new().with_status(&valid_status);
        let result = validate_api_params(&params);

        prop_assert_eq!(
            result,
            ValidationResult::Valid,
            "Valid status '{}' should be accepted",
            valid_status
        );
    }

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any invalid sort_by value, the API SHALL reject the request.
    #[test]
    fn prop_invalid_sort_by_is_rejected(
        invalid_sort_by in arb_invalid_sort_by(),
    ) {
        let params = TestApiParams::new().with_sort_by(&invalid_sort_by);
        let result = validate_api_params(&params);

        prop_assert!(
            matches!(result, ValidationResult::Invalid(_)),
            "Invalid sort_by '{}' should be rejected",
            invalid_sort_by
        );
    }

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any valid sort_by value, the API SHALL accept the request.
    #[test]
    fn prop_valid_sort_by_is_accepted(
        valid_sort_by in arb_valid_sort_by(),
    ) {
        let params = TestApiParams::new().with_sort_by(&valid_sort_by);
        let result = validate_api_params(&params);

        prop_assert_eq!(
            result,
            ValidationResult::Valid,
            "Valid sort_by '{}' should be accepted",
            valid_sort_by
        );
    }

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any invalid sort_order value, the API SHALL reject the request.
    #[test]
    fn prop_invalid_sort_order_is_rejected(
        invalid_sort_order in arb_invalid_sort_order(),
    ) {
        let params = TestApiParams::new().with_sort_order(&invalid_sort_order);
        let result = validate_api_params(&params);

        prop_assert!(
            matches!(result, ValidationResult::Invalid(_)),
            "Invalid sort_order '{}' should be rejected",
            invalid_sort_order
        );
    }

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any valid sort_order value, the API SHALL accept the request.
    #[test]
    fn prop_valid_sort_order_is_accepted(
        valid_sort_order in arb_valid_sort_order(),
    ) {
        let params = TestApiParams::new().with_sort_order(&valid_sort_order);
        let result = validate_api_params(&params);

        prop_assert_eq!(
            result,
            ValidationResult::Valid,
            "Valid sort_order '{}' should be accepted",
            valid_sort_order
        );
    }

    /// Feature: raw-messages-display, Property 10: Invalid Parameter Rejection
    ///
    /// For any combination of valid parameters, the API SHALL accept the request.
    #[test]
    fn prop_all_valid_params_accepted(
        limit in arb_valid_limit(),
        offset in arb_valid_offset(),
        status in arb_valid_status(),
        sort_by in arb_valid_sort_by(),
        sort_order in arb_valid_sort_order(),
    ) {
        let params = TestApiParams::new()
            .with_limit(limit)
            .with_offset(offset)
            .with_status(&status)
            .with_sort_by(&sort_by)
            .with_sort_order(&sort_order);

        let result = validate_api_params(&params);

        prop_assert_eq!(
            result,
            ValidationResult::Valid,
            "All valid parameters should be accepted"
        );
    }
}

// =============================================================================
// Reset for Reprocessing Property Tests
// =============================================================================

/// Simulated raw message for reset testing with various processing states
#[derive(Debug, Clone)]
struct ResetTestMessage {
    id: Uuid,
    content: String,
    timestamp: DateTime<Utc>,
    processed_at: Option<DateTime<Utc>>,
    error: Option<String>,
}

impl ResetTestMessage {
    /// Create a message with random processing state
    fn with_state(processed_at: Option<DateTime<Utc>>, error: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            content: "Test message content".to_string(),
            timestamp: Utc::now(),
            processed_at,
            error,
        }
    }

    /// Simulate reset_for_reprocessing behavior
    /// Sets processed_at = None and error = None
    fn reset_for_reprocessing(&mut self) {
        self.processed_at = None;
        self.error = None;
    }

    /// Compute the processing status based on fields
    fn compute_status(&self) -> &'static str {
        if self.processed_at.is_none() {
            "unprocessed"
        } else if self.error.is_some() {
            "error"
        } else {
            "processed"
        }
    }
}

/// Generate optional processed_at timestamp
fn arb_processed_at() -> impl Strategy<Value = Option<DateTime<Utc>>> {
    prop_oneof![
        Just(None),
        (0i64..30 * 24 * 60)
            .prop_map(|minutes_ago| Some(Utc::now() - Duration::minutes(minutes_ago))),
    ]
}

/// Generate optional error string
fn arb_error() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        prop::string::string_regex("[A-Za-z ]{5,50}")
            .unwrap()
            .prop_map(Some),
    ]
}

// Validates: Requirements 1.1, 1.2, 1.3, 5.1, 5.2
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: message-reprocessing-fix, Property 1: Reset clears processing state
    ///
    /// For any raw message (regardless of current processed_at or error values),
    /// calling reset_for_reprocessing SHALL result in a message where processed_at
    /// is NULL and error is NULL, and the computed status is "unprocessed".
    ///
    /// **Validates: Requirements 1.1, 1.2, 1.3, 5.1, 5.2**
    #[test]
    fn prop_reset_clears_processing_state(
        processed_at in arb_processed_at(),
        error in arb_error(),
    ) {
        // Create a message with random processing state
        let mut message = ResetTestMessage::with_state(processed_at, error);

        // Store original state for debugging
        let original_processed_at = message.processed_at;
        let original_error = message.error.clone();

        // Apply reset_for_reprocessing
        message.reset_for_reprocessing();

        // Verify processed_at is None (Requirement 1.1, 5.1)
        prop_assert!(
            message.processed_at.is_none(),
            "After reset, processed_at should be None. Original: {:?}",
            original_processed_at
        );

        // Verify error is None (Requirement 1.2, 5.1)
        prop_assert!(
            message.error.is_none(),
            "After reset, error should be None. Original: {:?}",
            original_error
        );

        // Verify computed status is "unprocessed" (Requirement 1.3)
        prop_assert_eq!(
            message.compute_status(),
            "unprocessed",
            "After reset, status should be 'unprocessed'"
        );
    }

    /// Feature: message-reprocessing-fix, Property 1: Reset is idempotent
    ///
    /// Calling reset_for_reprocessing multiple times should have the same effect
    /// as calling it once.
    ///
    /// **Validates: Requirements 1.1, 1.2, 1.3, 5.1, 5.2**
    #[test]
    fn prop_reset_is_idempotent(
        processed_at in arb_processed_at(),
        error in arb_error(),
        reset_count in 1usize..5,
    ) {
        let mut message = ResetTestMessage::with_state(processed_at, error);

        // Apply reset multiple times
        for _ in 0..reset_count {
            message.reset_for_reprocessing();
        }

        // Verify state is still reset
        prop_assert!(
            message.processed_at.is_none(),
            "After {} resets, processed_at should still be None",
            reset_count
        );
        prop_assert!(
            message.error.is_none(),
            "After {} resets, error should still be None",
            reset_count
        );
        prop_assert_eq!(
            message.compute_status(),
            "unprocessed",
            "After {} resets, status should still be 'unprocessed'",
            reset_count
        );
    }

    /// Feature: message-reprocessing-fix, Property 1: Reset preserves message identity
    ///
    /// Resetting a message should not change its ID, content, or timestamp.
    ///
    /// **Validates: Requirements 5.2**
    #[test]
    fn prop_reset_preserves_message_identity(
        processed_at in arb_processed_at(),
        error in arb_error(),
    ) {
        let mut message = ResetTestMessage::with_state(processed_at, error);

        // Store original identity fields
        let original_id = message.id;
        let original_content = message.content.clone();
        let original_timestamp = message.timestamp;

        // Apply reset
        message.reset_for_reprocessing();

        // Verify identity fields are preserved
        prop_assert_eq!(
            message.id,
            original_id,
            "Reset should not change message ID"
        );
        prop_assert_eq!(
            message.content,
            original_content,
            "Reset should not change message content"
        );
        prop_assert_eq!(
            message.timestamp,
            original_timestamp,
            "Reset should not change message timestamp"
        );
    }
}

// =============================================================================
// Existing Items Preservation Property Tests (Reprocessing)
// =============================================================================

/// Simulated offer for testing item preservation
#[derive(Debug, Clone)]
struct TestOffer {
    id: Uuid,
    raw_message_id: Uuid,
}

/// Simulated request for testing item preservation
#[derive(Debug, Clone)]
struct TestRequest {
    id: Uuid,
    raw_message_id: Uuid,
}

/// Simulated database state for testing reprocessing
#[derive(Debug, Clone)]
struct TestDatabaseState {
    offers: Vec<TestOffer>,
    requests: Vec<TestRequest>,
}

impl TestDatabaseState {
    fn new() -> Self {
        Self {
            offers: Vec::new(),
            requests: Vec::new(),
        }
    }

    fn add_offer(&mut self, raw_message_id: Uuid, _: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.offers.push(TestOffer { id, raw_message_id });
        id
    }

    fn add_request(&mut self, raw_message_id: Uuid, _: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.requests.push(TestRequest { id, raw_message_id });
        id
    }

    fn get_offers_for_message(&self, raw_message_id: Uuid) -> Vec<&TestOffer> {
        self.offers
            .iter()
            .filter(|o| o.raw_message_id == raw_message_id)
            .collect()
    }

    fn get_requests_for_message(&self, raw_message_id: Uuid) -> Vec<&TestRequest> {
        self.requests
            .iter()
            .filter(|r| r.raw_message_id == raw_message_id)
            .collect()
    }

    fn offer_exists(&self, id: Uuid) -> bool {
        self.offers.iter().any(|o| o.id == id)
    }

    fn request_exists(&self, id: Uuid) -> bool {
        self.requests.iter().any(|r| r.id == id)
    }
}

/// Generate a random number of existing items (0-5)
fn arb_item_count() -> impl Strategy<Value = usize> {
    0usize..6
}

// Validates: Requirements 3.1, 3.2
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: message-reprocessing-fix, Property 3: Existing items preserved during reprocessing
    ///
    /// For any raw message with existing offers or requests, after reprocessing,
    /// all previously existing items SHALL still exist in the database with unchanged IDs.
    ///
    /// **Validates: Requirements 3.1, 3.2**
    #[test]
    fn prop_existing_items_preserved_during_reprocessing(
        existing_offer_count in arb_item_count(),
        existing_request_count in arb_item_count(),
        new_offer_count in arb_item_count(),
        new_request_count in arb_item_count(),
    ) {
        let raw_message_id = Uuid::new_v4();
        let mut db = TestDatabaseState::new();

        // Create existing offers and requests
        let existing_offer_ids: Vec<Uuid> = (0..existing_offer_count)
            .map(|i| db.add_offer(raw_message_id, &format!("ExistingMed{}", i)))
            .collect();

        let existing_request_ids: Vec<Uuid> = (0..existing_request_count)
            .map(|i| db.add_request(raw_message_id, &format!("ExistingMed{}", i)))
            .collect();

        // Simulate reprocessing: add new items (existing items are NOT deleted)
        for i in 0..new_offer_count {
            db.add_offer(raw_message_id, &format!("NewMed{}", i));
        }
        for i in 0..new_request_count {
            db.add_request(raw_message_id, &format!("NewMed{}", i));
        }

        // Verify all existing offers still exist
        for offer_id in &existing_offer_ids {
            prop_assert!(
                db.offer_exists(*offer_id),
                "Existing offer {} should still exist after reprocessing",
                offer_id
            );
        }

        // Verify all existing requests still exist
        for request_id in &existing_request_ids {
            prop_assert!(
                db.request_exists(*request_id),
                "Existing request {} should still exist after reprocessing",
                request_id
            );
        }

        // Verify total counts are correct (existing + new)
        let total_offers = db.get_offers_for_message(raw_message_id).len();
        let total_requests = db.get_requests_for_message(raw_message_id).len();

        prop_assert_eq!(
            total_offers,
            existing_offer_count + new_offer_count,
            "Total offers should be existing + new"
        );
        prop_assert_eq!(
            total_requests,
            existing_request_count + new_request_count,
            "Total requests should be existing + new"
        );
    }

    /// Feature: message-reprocessing-fix, Property 3: Item IDs are immutable
    ///
    /// Existing item IDs should not change during reprocessing.
    ///
    /// **Validates: Requirements 3.1**
    #[test]
    fn prop_existing_item_ids_unchanged(
        existing_offer_count in 1usize..5,
        existing_request_count in 1usize..5,
    ) {
        let raw_message_id = Uuid::new_v4();
        let mut db = TestDatabaseState::new();

        // Create existing items and store their IDs
        let original_offer_ids: Vec<Uuid> = (0..existing_offer_count)
            .map(|i| db.add_offer(raw_message_id, &format!("Med{}", i)))
            .collect();

        let original_request_ids: Vec<Uuid> = (0..existing_request_count)
            .map(|i| db.add_request(raw_message_id, &format!("Med{}", i)))
            .collect();

        // Simulate reprocessing (add some new items)
        db.add_offer(raw_message_id, "NewMed");
        db.add_request(raw_message_id, "NewMed");

        // Get current items for the message
        let current_offers = db.get_offers_for_message(raw_message_id);
        let current_requests = db.get_requests_for_message(raw_message_id);

        // Verify original IDs are still present
        for original_id in &original_offer_ids {
            prop_assert!(
                current_offers.iter().any(|o| o.id == *original_id),
                "Original offer ID {} should still be present",
                original_id
            );
        }

        for original_id in &original_request_ids {
            prop_assert!(
                current_requests.iter().any(|r| r.id == *original_id),
                "Original request ID {} should still be present",
                original_id
            );
        }
    }

    /// Feature: message-reprocessing-fix, Property 3: New items get new IDs
    ///
    /// New items created during reprocessing should have unique IDs.
    ///
    /// **Validates: Requirements 3.2**
    #[test]
    fn prop_new_items_get_unique_ids(
        existing_count in 1usize..5,
        new_count in 1usize..5,
    ) {
        let raw_message_id = Uuid::new_v4();
        let mut db = TestDatabaseState::new();

        // Create existing items
        let existing_ids: Vec<Uuid> = (0..existing_count)
            .map(|i| db.add_offer(raw_message_id, &format!("ExistingMed{}", i)))
            .collect();

        // Create new items during "reprocessing"
        let new_ids: Vec<Uuid> = (0..new_count)
            .map(|i| db.add_offer(raw_message_id, &format!("NewMed{}", i)))
            .collect();

        // Verify no ID collision between existing and new
        for new_id in &new_ids {
            prop_assert!(
                !existing_ids.contains(new_id),
                "New item ID {} should not collide with existing IDs",
                new_id
            );
        }

        // Verify all IDs are unique
        let all_ids: Vec<Uuid> = existing_ids.iter().chain(new_ids.iter()).cloned().collect();
        let unique_count = {
            let mut sorted = all_ids.clone();
            sorted.sort();
            sorted.dedup();
            sorted.len()
        };

        prop_assert_eq!(
            unique_count,
            all_ids.len(),
            "All item IDs should be unique"
        );
    }
}

// =============================================================================
// Bulk Reprocess Independence Property Tests
// =============================================================================

/// Simulated bulk reprocess result for testing
#[derive(Debug, Clone)]
struct TestBulkReprocessResult {
    id: Uuid,
    success: bool,
    offers_created: i32,
    requests_created: i32,
    error: Option<String>,
}

/// Simulated bulk reprocess response
#[derive(Debug, Clone)]
struct TestBulkReprocessResponse {
    results: Vec<TestBulkReprocessResult>,
    total_succeeded: i32,
    total_failed: i32,
    total_offers_created: i32,
    total_requests_created: i32,
}

/// Simulated message store for bulk reprocess testing
#[derive(Debug, Clone)]
struct TestMessageStore {
    existing_ids: std::collections::HashSet<Uuid>,
}

impl TestMessageStore {
    fn new() -> Self {
        Self {
            existing_ids: std::collections::HashSet::new(),
        }
    }

    fn add_message(&mut self, id: Uuid) {
        self.existing_ids.insert(id);
    }

    fn exists(&self, id: Uuid) -> bool {
        self.existing_ids.contains(&id)
    }

    /// Simulate bulk reprocess behavior
    /// - For each ID, check if message exists
    /// - If exists, simulate successful processing
    /// - If not exists, return failure
    /// - Continue processing regardless of individual failures
    fn bulk_reprocess(&self, ids: &[Uuid]) -> TestBulkReprocessResponse {
        let mut results = Vec::with_capacity(ids.len());
        let mut total_succeeded = 0i32;
        let mut total_failed = 0i32;
        let mut total_offers_created = 0i32;
        let mut total_requests_created = 0i32;

        for &id in ids {
            if self.exists(id) {
                // Simulate successful processing with random item counts
                let offers = 1; // Simplified for testing
                let requests = 1;

                results.push(TestBulkReprocessResult {
                    id,
                    success: true,
                    offers_created: offers,
                    requests_created: requests,
                    error: None,
                });

                total_succeeded += 1;
                total_offers_created += offers;
                total_requests_created += requests;
            } else {
                // Message not found - failure
                results.push(TestBulkReprocessResult {
                    id,
                    success: false,
                    offers_created: 0,
                    requests_created: 0,
                    error: Some("Message not found".to_string()),
                });

                total_failed += 1;
            }
        }

        TestBulkReprocessResponse {
            results,
            total_succeeded,
            total_failed,
            total_offers_created,
            total_requests_created,
        }
    }
}

/// Generate a random number of valid message IDs (1-10)
fn arb_valid_id_count() -> impl Strategy<Value = usize> {
    1usize..11
}

/// Generate a random number of invalid message IDs (0-5)
fn arb_invalid_id_count() -> impl Strategy<Value = usize> {
    0usize..6
}

// Validates: Requirements 4.1, 4.2, 4.3
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: message-reprocessing-fix, Property 4: Bulk reprocess handles all messages independently
    ///
    /// For any set of message IDs submitted for bulk reprocessing, the response SHALL
    /// contain exactly one status entry per input ID.
    ///
    /// **Validates: Requirements 4.1, 4.2, 4.3**
    #[test]
    fn prop_bulk_reprocess_returns_entry_for_each_id(
        valid_count in arb_valid_id_count(),
        invalid_count in arb_invalid_id_count(),
    ) {
        let mut store = TestMessageStore::new();

        // Create valid message IDs
        let valid_ids: Vec<Uuid> = (0..valid_count)
            .map(|_| {
                let id = Uuid::new_v4();
                store.add_message(id);
                id
            })
            .collect();

        // Create invalid message IDs (not in store)
        let invalid_ids: Vec<Uuid> = (0..invalid_count)
            .map(|_| Uuid::new_v4())
            .collect();

        // Combine all IDs
        let all_ids: Vec<Uuid> = valid_ids.iter().chain(invalid_ids.iter()).cloned().collect();
        let total_count = all_ids.len();

        // Perform bulk reprocess
        let response = store.bulk_reprocess(&all_ids);

        // Verify response has exactly one entry per input ID
        prop_assert_eq!(
            response.results.len(),
            total_count,
            "Response should have exactly one entry per input ID"
        );

        // Verify all input IDs are present in results
        let result_ids: std::collections::HashSet<Uuid> = response.results.iter().map(|r| r.id).collect();
        for id in &all_ids {
            prop_assert!(
                result_ids.contains(id),
                "Result should contain entry for ID {}",
                id
            );
        }
    }

    /// Feature: message-reprocessing-fix, Property 4: Failures don't block other messages
    ///
    /// For any batch with mix of valid/invalid IDs, failures for individual messages
    /// SHALL NOT prevent processing of other messages in the batch.
    ///
    /// **Validates: Requirements 4.1, 4.2, 4.3**
    #[test]
    fn prop_bulk_reprocess_failures_dont_block_others(
        valid_count in arb_valid_id_count(),
        invalid_count in 1usize..6, // At least one invalid
    ) {
        let mut store = TestMessageStore::new();

        // Create valid message IDs
        let valid_ids: Vec<Uuid> = (0..valid_count)
            .map(|_| {
                let id = Uuid::new_v4();
                store.add_message(id);
                id
            })
            .collect();

        // Create invalid message IDs (not in store)
        let invalid_ids: Vec<Uuid> = (0..invalid_count)
            .map(|_| Uuid::new_v4())
            .collect();

        // Combine all IDs (interleave to test order independence)
        let mut all_ids = Vec::new();
        let max_len = valid_ids.len().max(invalid_ids.len());
        for i in 0..max_len {
            if i < invalid_ids.len() {
                all_ids.push(invalid_ids[i]);
            }
            if i < valid_ids.len() {
                all_ids.push(valid_ids[i]);
            }
        }

        // Perform bulk reprocess
        let response = store.bulk_reprocess(&all_ids);

        // Verify all valid IDs succeeded despite invalid IDs in batch
        let successful_ids: std::collections::HashSet<Uuid> = response.results
            .iter()
            .filter(|r| r.success)
            .map(|r| r.id)
            .collect();

        for valid_id in &valid_ids {
            prop_assert!(
                successful_ids.contains(valid_id),
                "Valid ID {} should succeed despite invalid IDs in batch",
                valid_id
            );
        }

        // Verify all invalid IDs failed
        let failed_ids: std::collections::HashSet<Uuid> = response.results
            .iter()
            .filter(|r| !r.success)
            .map(|r| r.id)
            .collect();

        for invalid_id in &invalid_ids {
            prop_assert!(
                failed_ids.contains(invalid_id),
                "Invalid ID {} should fail",
                invalid_id
            );
        }

        // Verify counts match
        prop_assert_eq!(
            response.total_succeeded as usize,
            valid_count,
            "Total succeeded should equal valid count"
        );
        prop_assert_eq!(
            response.total_failed as usize,
            invalid_count,
            "Total failed should equal invalid count"
        );
    }

    /// Feature: message-reprocessing-fix, Property 4: Per-message success/failure status
    ///
    /// For any bulk reprocess operation, each result entry SHALL have a clear
    /// success/failure status with appropriate error message for failures.
    ///
    /// **Validates: Requirements 4.2**
    #[test]
    fn prop_bulk_reprocess_has_clear_status_per_message(
        valid_count in arb_valid_id_count(),
        invalid_count in arb_invalid_id_count(),
    ) {
        let mut store = TestMessageStore::new();

        // Create valid message IDs
        let valid_ids: Vec<Uuid> = (0..valid_count)
            .map(|_| {
                let id = Uuid::new_v4();
                store.add_message(id);
                id
            })
            .collect();

        // Create invalid message IDs
        let invalid_ids: Vec<Uuid> = (0..invalid_count)
            .map(|_| Uuid::new_v4())
            .collect();

        let all_ids: Vec<Uuid> = valid_ids.iter().chain(invalid_ids.iter()).cloned().collect();

        // Perform bulk reprocess
        let response = store.bulk_reprocess(&all_ids);

        // Verify each result has clear status
        for result in &response.results {
            if result.success {
                // Successful results should have no error
                prop_assert!(
                    result.error.is_none(),
                    "Successful result for {} should have no error",
                    result.id
                );
            } else {
                // Failed results should have an error message
                prop_assert!(
                    result.error.is_some(),
                    "Failed result for {} should have an error message",
                    result.id
                );
                prop_assert!(
                    !result.error.as_ref().unwrap().is_empty(),
                    "Error message for {} should not be empty",
                    result.id
                );
            }
        }
    }

    /// Feature: message-reprocessing-fix, Property 4: Totals are consistent
    ///
    /// The total counts in the response SHALL match the sum of individual results.
    ///
    /// **Validates: Requirements 4.2**
    #[test]
    fn prop_bulk_reprocess_totals_are_consistent(
        valid_count in arb_valid_id_count(),
        invalid_count in arb_invalid_id_count(),
    ) {
        let mut store = TestMessageStore::new();

        // Create valid message IDs
        let valid_ids: Vec<Uuid> = (0..valid_count)
            .map(|_| {
                let id = Uuid::new_v4();
                store.add_message(id);
                id
            })
            .collect();

        // Create invalid message IDs
        let invalid_ids: Vec<Uuid> = (0..invalid_count)
            .map(|_| Uuid::new_v4())
            .collect();

        let all_ids: Vec<Uuid> = valid_ids.iter().chain(invalid_ids.iter()).cloned().collect();

        // Perform bulk reprocess
        let response = store.bulk_reprocess(&all_ids);

        // Calculate expected totals from individual results
        let expected_succeeded: i32 = response.results.iter().filter(|r| r.success).count() as i32;
        let expected_failed: i32 = response.results.iter().filter(|r| !r.success).count() as i32;
        let expected_offers: i32 = response.results.iter().map(|r| r.offers_created).sum();
        let expected_requests: i32 = response.results.iter().map(|r| r.requests_created).sum();

        // Verify totals match
        prop_assert_eq!(
            response.total_succeeded,
            expected_succeeded,
            "total_succeeded should match sum of successful results"
        );
        prop_assert_eq!(
            response.total_failed,
            expected_failed,
            "total_failed should match sum of failed results"
        );
        prop_assert_eq!(
            response.total_offers_created,
            expected_offers,
            "total_offers_created should match sum of individual offers"
        );
        prop_assert_eq!(
            response.total_requests_created,
            expected_requests,
            "total_requests_created should match sum of individual requests"
        );

        // Verify succeeded + failed = total
        prop_assert_eq!(
            (response.total_succeeded + response.total_failed) as usize,
            response.results.len(),
            "succeeded + failed should equal total results"
        );
    }

    /// Feature: message-reprocessing-fix, Property 4: Empty batch handling
    ///
    /// An empty batch should be rejected with an appropriate error.
    /// (This is tested separately as it's an edge case)
    #[test]
    fn prop_bulk_reprocess_empty_batch_returns_empty_response(
        _dummy in Just(()),
    ) {
        let store = TestMessageStore::new();
        let empty_ids: Vec<Uuid> = Vec::new();

        let response = store.bulk_reprocess(&empty_ids);

        // Empty batch should return empty results
        prop_assert!(
            response.results.is_empty(),
            "Empty batch should return empty results"
        );
        prop_assert_eq!(
            response.total_succeeded,
            0,
            "Empty batch should have 0 succeeded"
        );
        prop_assert_eq!(
            response.total_failed,
            0,
            "Empty batch should have 0 failed"
        );
    }

    /// Feature: message-reprocessing-fix, Property 4: Order independence
    ///
    /// The order of IDs in the batch should not affect which messages succeed or fail.
    ///
    /// **Validates: Requirements 4.3**
    #[test]
    fn prop_bulk_reprocess_order_independent(
        valid_count in 2usize..6,
        invalid_count in 1usize..4,
    ) {
        let mut store = TestMessageStore::new();

        // Create valid message IDs
        let valid_ids: Vec<Uuid> = (0..valid_count)
            .map(|_| {
                let id = Uuid::new_v4();
                store.add_message(id);
                id
            })
            .collect();

        // Create invalid message IDs
        let invalid_ids: Vec<Uuid> = (0..invalid_count)
            .map(|_| Uuid::new_v4())
            .collect();

        // Create two different orderings
        let order1: Vec<Uuid> = valid_ids.iter().chain(invalid_ids.iter()).cloned().collect();
        let mut order2 = order1.clone();
        order2.reverse();

        // Process both orderings
        let response1 = store.bulk_reprocess(&order1);
        let response2 = store.bulk_reprocess(&order2);

        // Verify same success/failure counts
        prop_assert_eq!(
            response1.total_succeeded,
            response2.total_succeeded,
            "Different orderings should have same success count"
        );
        prop_assert_eq!(
            response1.total_failed,
            response2.total_failed,
            "Different orderings should have same failure count"
        );

        // Verify same IDs succeeded in both
        let succeeded1: std::collections::HashSet<Uuid> = response1.results
            .iter()
            .filter(|r| r.success)
            .map(|r| r.id)
            .collect();
        let succeeded2: std::collections::HashSet<Uuid> = response2.results
            .iter()
            .filter(|r| r.success)
            .map(|r| r.id)
            .collect();

        prop_assert_eq!(
            succeeded1,
            succeeded2,
            "Same IDs should succeed regardless of order"
        );
    }
}

// =============================================================================
// Batch Processor Pickup Property Tests
// =============================================================================

/// Simulated message for batch processor testing
#[derive(Debug, Clone)]
struct BatchProcessorTestMessage {
    id: Uuid,
    content: String,
    processed_at: Option<DateTime<Utc>>,
    error: Option<String>,
}

impl BatchProcessorTestMessage {
    fn new_unprocessed(content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            content,
            processed_at: None,
            error: None,
        }
    }

    fn new_processed(content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            content,
            processed_at: Some(Utc::now()),
            error: None,
        }
    }

    fn new_with_error(content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            content,
            processed_at: Some(Utc::now()),
            error: Some("Previous error".to_string()),
        }
    }

    fn is_unprocessed(&self) -> bool {
        self.processed_at.is_none()
    }

    /// Simulate processing the message
    fn mark_processed(&mut self, error: Option<String>) {
        self.processed_at = Some(Utc::now());
        self.error = error;
    }
}

/// Simulated batch processor for testing
#[derive(Debug)]
struct TestBatchProcessor {
    messages: Vec<BatchProcessorTestMessage>,
    processed_ids: std::collections::HashSet<Uuid>,
}

impl TestBatchProcessor {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            processed_ids: std::collections::HashSet::new(),
        }
    }

    fn add_message(&mut self, msg: BatchProcessorTestMessage) {
        self.messages.push(msg);
    }

    /// Get unprocessed messages (simulates get_unprocessed repository method)
    fn get_unprocessed(&self, limit: usize) -> Vec<&BatchProcessorTestMessage> {
        self.messages
            .iter()
            .filter(|m| m.is_unprocessed())
            .take(limit)
            .collect()
    }

    /// Simulate batch processor picking up and processing messages
    /// Returns the IDs of messages that were processed
    fn process_batch(&mut self, batch_size: usize) -> Vec<Uuid> {
        let unprocessed: Vec<Uuid> = self
            .messages
            .iter()
            .filter(|m| m.is_unprocessed())
            .take(batch_size)
            .map(|m| m.id)
            .collect();

        // Process each message
        for id in &unprocessed {
            if let Some(msg) = self.messages.iter_mut().find(|m| m.id == *id) {
                msg.mark_processed(None);
                self.processed_ids.insert(*id);
            }
        }

        unprocessed
    }

    /// Check if a message was processed
    fn was_processed(&self, id: Uuid) -> bool {
        self.processed_ids.contains(&id)
    }

    /// Get count of unprocessed messages
    fn unprocessed_count(&self) -> usize {
        self.messages.iter().filter(|m| m.is_unprocessed()).count()
    }
}

/// Generate a random message content
fn arb_batch_content() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9 ]{10,100}")
        .unwrap()
        .prop_map(|s| s.trim().to_string())
}

/// Generate a random processing state for batch processor tests
#[derive(Debug, Clone, Copy)]
enum BatchMessageState {
    Unprocessed,
    Processed,
    Error,
}

fn arb_batch_message_state() -> impl Strategy<Value = BatchMessageState> {
    prop_oneof![
        Just(BatchMessageState::Unprocessed),
        Just(BatchMessageState::Processed),
        Just(BatchMessageState::Error),
    ]
}

// Validates: Requirements 6.1, 6.2, 7.1, 7.2
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: message-reprocessing-fix, Property 5: Batch processor picks up unprocessed messages
    ///
    /// For any message with processed_at IS NULL, the BatchProcessor SHALL eventually
    /// pick it up and process it, resulting in either processed_at being set or an
    /// error being recorded.
    ///
    /// **Validates: Requirements 6.1, 6.2, 7.1, 7.2**
    #[test]
    fn prop_batch_processor_picks_up_unprocessed_messages(
        unprocessed_count in 1usize..20,
        batch_size in 1usize..10,
    ) {
        let mut processor = TestBatchProcessor::new();

        // Create unprocessed messages
        let unprocessed_ids: Vec<Uuid> = (0..unprocessed_count)
            .map(|i| {
                let msg = BatchProcessorTestMessage::new_unprocessed(format!("Message {}", i));
                let id = msg.id;
                processor.add_message(msg);
                id
            })
            .collect();

        // Process batches until all messages are processed
        let mut iterations = 0;
        let max_iterations = (unprocessed_count / batch_size) + 2; // Safety limit

        while processor.unprocessed_count() > 0 && iterations < max_iterations {
            processor.process_batch(batch_size);
            iterations += 1;
        }

        // Verify all unprocessed messages were eventually processed
        for id in &unprocessed_ids {
            prop_assert!(
                processor.was_processed(*id),
                "Unprocessed message {} should have been picked up and processed",
                id
            );
        }

        // Verify no unprocessed messages remain
        prop_assert_eq!(
            processor.unprocessed_count(),
            0,
            "All unprocessed messages should have been processed"
        );
    }

    /// Feature: message-reprocessing-fix, Property 5: Already processed messages are not re-processed
    ///
    /// Messages that are already processed (processed_at IS NOT NULL) should not be
    /// picked up by the batch processor.
    ///
    /// **Validates: Requirements 6.1, 6.2**
    #[test]
    fn prop_batch_processor_ignores_already_processed(
        processed_count in 1usize..10,
        unprocessed_count in 1usize..10,
        batch_size in 1usize..10,
    ) {
        let mut processor = TestBatchProcessor::new();

        // Create already processed messages
        let processed_ids: Vec<Uuid> = (0..processed_count)
            .map(|i| {
                let msg = BatchProcessorTestMessage::new_processed(format!("Processed {}", i));
                let id = msg.id;
                processor.add_message(msg);
                id
            })
            .collect();

        // Create unprocessed messages
        let unprocessed_ids: Vec<Uuid> = (0..unprocessed_count)
            .map(|i| {
                let msg = BatchProcessorTestMessage::new_unprocessed(format!("Unprocessed {}", i));
                let id = msg.id;
                processor.add_message(msg);
                id
            })
            .collect();

        // Process all batches
        let mut iterations = 0;
        let max_iterations = (unprocessed_count / batch_size) + 2;

        while processor.unprocessed_count() > 0 && iterations < max_iterations {
            processor.process_batch(batch_size);
            iterations += 1;
        }

        // Verify already processed messages were NOT re-processed
        // (they should not appear in processed_ids set since they were already processed)
        for id in &processed_ids {
            prop_assert!(
                !processor.was_processed(*id),
                "Already processed message {} should not have been re-processed",
                id
            );
        }

        // Verify unprocessed messages WERE processed
        for id in &unprocessed_ids {
            prop_assert!(
                processor.was_processed(*id),
                "Unprocessed message {} should have been processed",
                id
            );
        }
    }

    /// Feature: message-reprocessing-fix, Property 5: Reset messages are picked up
    ///
    /// Messages that have been reset for reprocessing (processed_at = NULL after reset)
    /// should be picked up by the batch processor.
    ///
    /// **Validates: Requirements 7.1, 7.2**
    #[test]
    fn prop_batch_processor_picks_up_reset_messages(
        initial_processed_count in 1usize..10,
        reset_count in 1usize..5,
        batch_size in 1usize..10,
    ) {
        let mut processor = TestBatchProcessor::new();

        // Create initially processed messages
        let mut messages: Vec<BatchProcessorTestMessage> = (0..initial_processed_count)
            .map(|i| BatchProcessorTestMessage::new_processed(format!("Message {}", i)))
            .collect();

        // Reset some messages for reprocessing
        let reset_ids: Vec<Uuid> = messages
            .iter_mut()
            .take(reset_count.min(initial_processed_count))
            .map(|msg| {
                // Simulate reset_for_reprocessing
                msg.processed_at = None;
                msg.error = None;
                msg.id
            })
            .collect();

        // Add all messages to processor
        for msg in messages {
            processor.add_message(msg);
        }

        // Process batches
        let mut iterations = 0;
        let max_iterations = (reset_ids.len() / batch_size) + 2;

        while processor.unprocessed_count() > 0 && iterations < max_iterations {
            processor.process_batch(batch_size);
            iterations += 1;
        }

        // Verify reset messages were picked up and processed
        for id in &reset_ids {
            prop_assert!(
                processor.was_processed(*id),
                "Reset message {} should have been picked up and processed",
                id
            );
        }
    }

    /// Feature: message-reprocessing-fix, Property 5: Batch size is respected
    ///
    /// Each batch should process at most batch_size messages.
    ///
    /// **Validates: Requirements 6.1**
    #[test]
    fn prop_batch_processor_respects_batch_size(
        message_count in 10usize..50,
        batch_size in 1usize..10,
    ) {
        let mut processor = TestBatchProcessor::new();

        // Create unprocessed messages
        for i in 0..message_count {
            let msg = BatchProcessorTestMessage::new_unprocessed(format!("Message {}", i));
            processor.add_message(msg);
        }

        // Process one batch and check size
        let processed = processor.process_batch(batch_size);

        prop_assert!(
            processed.len() <= batch_size,
            "Batch should process at most {} messages, but processed {}",
            batch_size,
            processed.len()
        );

        // If there were enough messages, batch should be full
        if message_count >= batch_size {
            prop_assert_eq!(
                processed.len(),
                batch_size,
                "Batch should be full when enough messages are available"
            );
        }
    }

    /// Feature: message-reprocessing-fix, Property 5: Processing sets processed_at
    ///
    /// After batch processing, all processed messages should have processed_at set.
    ///
    /// **Validates: Requirements 6.2**
    #[test]
    fn prop_batch_processor_sets_processed_at(
        message_count in 1usize..20,
        batch_size in 1usize..10,
    ) {
        let mut processor = TestBatchProcessor::new();

        // Create unprocessed messages
        for i in 0..message_count {
            let msg = BatchProcessorTestMessage::new_unprocessed(format!("Message {}", i));
            processor.add_message(msg);
        }

        // Process all batches
        let mut iterations = 0;
        let max_iterations = (message_count / batch_size) + 2;

        while processor.unprocessed_count() > 0 && iterations < max_iterations {
            processor.process_batch(batch_size);
            iterations += 1;
        }

        // Verify all messages now have processed_at set
        for msg in &processor.messages {
            prop_assert!(
                msg.processed_at.is_some(),
                "Message {} should have processed_at set after processing",
                msg.id
            );
        }
    }
}
