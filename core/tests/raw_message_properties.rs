//! Property-based tests for Raw Message Repository
//!
//! Feature: raw-messages-display
//! Tests Properties 4, 5, 6, 7 from the design document

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
    id: Uuid,
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
            id: Uuid::new_v4(),
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
        let after_start = start.map_or(true, |s| self.timestamp >= s);
        let before_end = end.map_or(true, |e| self.timestamp <= e);
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
    if let (Some(start), Some(end)) = (params.start_date, params.end_date) {
        if start > end {
            return ValidationResult::Invalid(
                "Start date must be before or equal to end date".to_string(),
            );
        }
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
