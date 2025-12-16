package gorm

import (
	"context"
	"pharmabroker/domain/entity"
	"reflect"
	"testing"
	"time"

	"github.com/google/uuid"
	"github.com/testcontainers/testcontainers-go"
	"github.com/testcontainers/testcontainers-go/modules/postgres"
	"github.com/testcontainers/testcontainers-go/wait"
)

// =============================================================================
// Test Infrastructure for GORM Repositories
// =============================================================================

// TestDB wraps DB for testing with cleanup helpers
type TestDB struct {
	*DB
	t         *testing.T
	container testcontainers.Container
}

// SetupTestDB creates a PostgreSQL database using testcontainers.
// Automatically starts a pgvector-enabled PostgreSQL container.
// No external dependencies required - Docker handles everything.
func SetupTestDB(t *testing.T) *TestDB {
	t.Helper()
	ctx := context.Background()

	// Start PostgreSQL container with pgvector support
	// Using Reuse to avoid spinning up new container for each test
	container, err := postgres.Run(ctx,
		"pgvector/pgvector:pg18-trixie",
		postgres.WithDatabase("pharmabroker_test"),
		postgres.WithUsername("postgres"),
		postgres.WithPassword("password"),
		testcontainers.WithWaitStrategy(
			wait.ForLog("database system is ready to accept connections").
				WithOccurrence(2).
				WithStartupTimeout(60*time.Second),
		),
		testcontainers.CustomizeRequest(testcontainers.GenericContainerRequest{
			ContainerRequest: testcontainers.ContainerRequest{
				Name: "pharmabroker-test-postgres",
			},
			Reuse: true,
		}),
	)
	if err != nil {
		t.Fatalf("Failed to start postgres container: %v", err)
	}

	// Get connection string
	dsn, err := container.ConnectionString(ctx, "sslmode=disable")
	if err != nil {
		container.Terminate(ctx)
		t.Fatalf("Failed to get connection string: %v", err)
	}

	cfg := Config{
		DSN:          dsn,
		MaxOpenConns: 5,
		MaxIdleConns: 2,
	}

	db, err := NewDB(&cfg)
	if err != nil {
		container.Terminate(ctx)
		t.Fatalf("Failed to create test database: %v", err)
	}

	// Clean all tables before test
	tables := []string{
		"feedback_records", "weight_history", "review_queue",
		"unmapped_medications", "audit_logs", "demand_leaderboard",
		"match_feedback", "failed_messages", "medication_mappings",
		"groups", "config", "match_queue", "matches",
		"offers", "requests", "raw_messages", "bot_users",
	}
	for _, table := range tables {
		db.Conn.Exec("TRUNCATE TABLE " + table + " CASCADE")
	}

	return &TestDB{DB: db, t: t, container: container}
}

// Close cleans up the test database connection.
// Note: Container is not terminated to allow reuse across tests.
func (tdb *TestDB) Close() {
	if err := tdb.DB.Close(); err != nil {
		tdb.t.Errorf("Failed to close test database: %v", err)
	}
	// Don't terminate container - it's reused across tests
}

// =============================================================================
// Test Data Factories
// =============================================================================

// NewTestRawMessage creates a entity.RawMessage with test data
func NewTestRawMessage(opts ...func(*entity.RawMessage)) *entity.RawMessage {
	msg := &entity.RawMessage{
		ID:          uuid.New().String(),
		ExternalID:  uuid.New().String(),
		GroupJID:    "test-group@g.us",
		GroupName:   "Test Group",
		SenderJID:   "sender@s.whatsapp.net",
		SenderPhone: "+201234567890",
		SenderName:  "Test Sender",
		Content:     "Test message content",
		Timestamp:   time.Now(),
	}
	for _, opt := range opts {
		opt(msg)
	}
	return msg
}

// NewTestOffer creates a entity.Offer with test data
// NOTE: RawMessageID is set to empty string - use NewTestOfferWithRawMessage for FK compliance
func NewTestOffer(opts ...func(*entity.Offer)) *entity.Offer {
	offer := &entity.Offer{
		ID:            uuid.New().String(),
		RawMessageID:  "", // Empty to avoid FK issues - tests should use helper
		SourcePhone:   "+201234567890",
		SourceName:    "Test Seller",
		SourceGroup:   "test-group@g.us",
		GroupName:     "Test Group",
		Medication:    "Augmentin 1g",
		MedicationRaw: "أوجمنتين 1 جم",
		Quantity:      50,
		Unit:          strPtr("boxes"),
		Price:         150.0,
		Currency:      "EGP",
		RawMessage:    "للبيع: Augmentin 1g - 50 علبة",
		Status:        entity.StatusActive,
		CreatedAt:     time.Now(),
		UpdatedAt:     time.Now(),
	}
	for _, opt := range opts {
		opt(offer)
	}
	return offer
}

// NewTestRequest creates a entity.Request with test data
// NOTE: RawMessageID is set to empty string - use NewTestRequestWithRawMessage for FK compliance
func NewTestRequest(opts ...func(*entity.Request)) *entity.Request {
	req := &entity.Request{
		ID:            uuid.New().String(),
		RawMessageID:  "", // Empty to avoid FK issues - tests should use helper
		SourcePhone:   "+201098765432",
		SourceName:    "Test Buyer",
		SourceGroup:   "test-group@g.us",
		GroupName:     "Test Group",
		Medication:    "Augmentin 1g",
		MedicationRaw: "أوجمنتين 1 جرام",
		Quantity:      20,
		Unit:          strPtr("boxes"),
		MaxPrice:      160.0,
		Currency:      "EGP",
		Urgent:        false,
		RawMessage:    "مطلوب: أوجمنتين 1 جرام - 20 علبة",
		Status:        entity.StatusActive,
		CreatedAt:     time.Now(),
		UpdatedAt:     time.Now(),
	}
	for _, opt := range opts {
		opt(req)
	}
	return req
}

// CreateTestOfferWithRawMessage creates a raw message first, then an offer referencing it
func CreateTestOfferWithRawMessage(t *testing.T, db *TestDB, opts ...func(*entity.Offer)) *entity.Offer {
	t.Helper()
	ctx, cancel := testCtx()
	defer cancel()
	rawMsgRepo := NewRawMessageRepo(db.DB)

	// Create raw message first
	rawMsg := NewTestRawMessage()
	if err := rawMsgRepo.Save(ctx, rawMsg); err != nil {
		t.Fatalf("Failed to create raw message: %v", err)
	}

	// Create offer with valid raw_message_id
	offer := NewTestOffer(func(o *entity.Offer) {
		o.RawMessageID = rawMsg.ID
	})
	for _, opt := range opts {
		opt(offer)
	}
	return offer
}

// CreateTestRequestWithRawMessage creates a raw message first, then a request referencing it
func CreateTestRequestWithRawMessage(t *testing.T, db *TestDB, opts ...func(*entity.Request)) *entity.Request {
	t.Helper()
	ctx, cancel := testCtx()
	defer cancel()
	rawMsgRepo := NewRawMessageRepo(db.DB)

	// Create raw message first
	rawMsg := NewTestRawMessage()
	if err := rawMsgRepo.Save(ctx, rawMsg); err != nil {
		t.Fatalf("Failed to create raw message: %v", err)
	}

	// Create request with valid raw_message_id
	req := NewTestRequest(func(r *entity.Request) {
		r.RawMessageID = rawMsg.ID
	})
	for _, opt := range opts {
		opt(req)
	}
	return req
}

// NewTestMatch creates a entity.Match with test data
func NewTestMatch(offerID, requestID string, opts ...func(*entity.Match)) *entity.Match {
	match := &entity.Match{
		ID:        uuid.New().String(),
		OfferID:   offerID,
		RequestID: requestID,
		Score:     0.85,
		Reasoning: "Strong medication match",
		MatchedBy: "AUTO",
		Status:    entity.MatchStatusPending,
		CreatedAt: time.Now(),
	}
	for _, opt := range opts {
		opt(match)
	}
	return match
}

// NewTestGroup creates a entity.Group with test data
func NewTestGroup(opts ...func(*entity.Group)) *entity.Group {
	group := &entity.Group{
		JID:         uuid.New().String() + "@g.us",
		Name:        "Test Group",
		Description: "Test group description",
		Monitored:   true,
		AddedAt:     time.Now(),
	}
	for _, opt := range opts {
		opt(group)
	}
	return group
}

// NewTestMedicationMapping creates a entity.MedicationMapping with test data
func NewTestMedicationMapping(opts ...func(*entity.MedicationMapping)) *entity.MedicationMapping {
	mapping := &entity.MedicationMapping{
		ID:          uuid.New().String(),
		ArabicName:  "أوجمنتين",
		EnglishName: "Augmentin",
		Synonyms:    []string{"Augmentin 1g", "أوجمنتين 1 جم"},
		CreatedAt:   time.Now(),
		UpdatedAt:   time.Now(),
	}
	for _, opt := range opts {
		opt(mapping)
	}
	return mapping
}

// NewTestMatchFeedback creates a entity.MatchFeedback with test data
func NewTestMatchFeedback(matchID string, opts ...func(*entity.MatchFeedback)) *entity.MatchFeedback {
	fb := &entity.MatchFeedback{
		ID:                 uuid.New().String(),
		MatchID:            matchID,
		OperatorID:         "operator-1",
		Decision:           entity.FeedbackConfirmed,
		OriginalScore:      0.85,
		OriginalConfidence: "HIGH",
		CreatedAt:          time.Now(),
	}
	for _, opt := range opts {
		opt(fb)
	}
	return fb
}

// =============================================================================
// Helper Functions
// =============================================================================

// strPtr returns a pointer to a string (for optional fields)
func strPtr(s string) *string {
	return &s
}

// testCtx returns a context with timeout for tests
// The caller should defer the cancel function
func testCtx() (context.Context, context.CancelFunc) {
	return context.WithTimeout(context.Background(), 10*time.Second)
}

// assertNoError fails the test if err is not nil
func assertNoError(t *testing.T, err error, msg string) {
	t.Helper()
	if err != nil {
		t.Fatalf("%s: %v", msg, err)
	}
}

// assertEqual fails the test if got != want
func assertEqual[T comparable](t *testing.T, got, want T, msg string) {
	t.Helper()
	if got != want {
		t.Errorf("%s: got %v, want %v", msg, got, want)
	}
}

// assertNotNil fails the test if v is nil
func assertNotNil(t *testing.T, v any, msg string) {
	t.Helper()
	if v == nil || isNilInterface(v) {
		t.Fatalf("%s: expected non-nil value", msg)
	}
}

// assertNil fails the test if v is not nil
func assertNil(t *testing.T, v any, msg string) {
	t.Helper()
	if v != nil && !isNilInterface(v) {
		t.Errorf("%s: expected nil, got %v", msg, v)
	}
}

// isNilInterface handles the interface nil comparison properly
// An interface can be non-nil but contain a nil pointer
func isNilInterface(v any) bool {
	if v == nil {
		return true
	}
	rv := reflect.ValueOf(v)
	switch rv.Kind() {
	case reflect.Pointer, reflect.Map, reflect.Slice, reflect.Chan, reflect.Func, reflect.Interface:
		return rv.IsNil()
	}
	return false
}
