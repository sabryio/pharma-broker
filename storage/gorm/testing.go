package gorm

import (
	"context"
	"reflect"
	"testing"
	"time"

	"github.com/google/uuid"

	"pharmabroker/internal/domain"
)

// =============================================================================
// Test Infrastructure for GORM Repositories
// =============================================================================

// TestDB wraps DB for testing with cleanup helpers
type TestDB struct {
	*DB
	t *testing.T
}

// SetupTestDB creates an in-memory GORM database for testing
func SetupTestDB(t *testing.T) *TestDB {
	t.Helper()

	cfg := Config{
		Path: ":memory:",
	}

	db, err := NewDB(&cfg)
	if err != nil {
		t.Fatalf("Failed to create test database: %v", err)
	}

	// Run migrations for all models
	err = db.Conn.AutoMigrate(
		&RawMessage{},
		&Offer{},
		&Request{},
		&Match{},
		&MatchQueue{},
		&AppConfig{},
		&Group{},
		&MedicationMapping{},
		&FailedMessage{},
		&MatchFeedback{},
		&DemandLeaderboard{},
		&AuditLog{},
		&UnmappedMedication{},
		&ReviewQueue{},
		&FeedbackRecord{},
		&WeightHistory{},
	)
	if err != nil {
		t.Fatalf("Failed to migrate test database: %v", err)
	}

	// Create FTS virtual table for requests
	err = db.Conn.Exec(`
		CREATE VIRTUAL TABLE IF NOT EXISTS requests_fts USING fts5(
			medication,
			notes,
			raw_message,
			medication_raw,
			content='requests',
			content_rowid='rowid'
		);
		CREATE TRIGGER IF NOT EXISTS requests_ai AFTER INSERT ON requests BEGIN
			INSERT INTO requests_fts(rowid, medication, notes, raw_message, medication_raw)
			VALUES (new.rowid, new.medication, new.notes, new.raw_message, new.medication_raw);
		END;
		CREATE TRIGGER IF NOT EXISTS requests_ad AFTER DELETE ON requests BEGIN
			INSERT INTO requests_fts(requests_fts, rowid, medication, notes, raw_message, medication_raw)
			VALUES('delete', old.rowid, old.medication, old.notes, old.raw_message, old.medication_raw);
		END;
		CREATE TRIGGER IF NOT EXISTS requests_au AFTER UPDATE ON requests BEGIN
			INSERT INTO requests_fts(requests_fts, rowid, medication, notes, raw_message, medication_raw)
			VALUES('delete', old.rowid, old.medication, old.notes, old.raw_message, old.medication_raw);
			INSERT INTO requests_fts(rowid, medication, notes, raw_message, medication_raw)
			VALUES (new.rowid, new.medication, new.notes, new.raw_message, new.medication_raw);
		END;
	`).Error
	if err != nil {
		t.Fatalf("Failed to create requests_fts table: %v", err)
	}

	return &TestDB{DB: db, t: t}
}

// Close cleans up the test database
func (tdb *TestDB) Close() {
	if err := tdb.DB.Close(); err != nil {
		tdb.t.Errorf("Failed to close test database: %v", err)
	}
}

// =============================================================================
// Test Data Factories
// =============================================================================

// NewTestRawMessage creates a domain.RawMessage with test data
func NewTestRawMessage(opts ...func(*domain.RawMessage)) *domain.RawMessage {
	msg := &domain.RawMessage{
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

// NewTestOffer creates a domain.Offer with test data
// NOTE: RawMessageID is set to empty string - use NewTestOfferWithRawMessage for FK compliance
func NewTestOffer(opts ...func(*domain.Offer)) *domain.Offer {
	offer := &domain.Offer{
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
		Status:        domain.StatusActive,
		CreatedAt:     time.Now(),
		UpdatedAt:     time.Now(),
	}
	for _, opt := range opts {
		opt(offer)
	}
	return offer
}

// NewTestRequest creates a domain.Request with test data
// NOTE: RawMessageID is set to empty string - use NewTestRequestWithRawMessage for FK compliance
func NewTestRequest(opts ...func(*domain.Request)) *domain.Request {
	req := &domain.Request{
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
		Status:        domain.StatusActive,
		CreatedAt:     time.Now(),
		UpdatedAt:     time.Now(),
	}
	for _, opt := range opts {
		opt(req)
	}
	return req
}

// CreateTestOfferWithRawMessage creates a raw message first, then an offer referencing it
func CreateTestOfferWithRawMessage(t *testing.T, db *TestDB, opts ...func(*domain.Offer)) *domain.Offer {
	t.Helper()
	ctx := testCtx()
	rawMsgRepo := NewRawMessageRepo(db.DB)

	// Create raw message first
	rawMsg := NewTestRawMessage()
	if err := rawMsgRepo.Save(ctx, rawMsg); err != nil {
		t.Fatalf("Failed to create raw message: %v", err)
	}

	// Create offer with valid raw_message_id
	offer := NewTestOffer(func(o *domain.Offer) {
		o.RawMessageID = rawMsg.ID
	})
	for _, opt := range opts {
		opt(offer)
	}
	return offer
}

// CreateTestRequestWithRawMessage creates a raw message first, then a request referencing it
func CreateTestRequestWithRawMessage(t *testing.T, db *TestDB, opts ...func(*domain.Request)) *domain.Request {
	t.Helper()
	ctx := testCtx()
	rawMsgRepo := NewRawMessageRepo(db.DB)

	// Create raw message first
	rawMsg := NewTestRawMessage()
	if err := rawMsgRepo.Save(ctx, rawMsg); err != nil {
		t.Fatalf("Failed to create raw message: %v", err)
	}

	// Create request with valid raw_message_id
	req := NewTestRequest(func(r *domain.Request) {
		r.RawMessageID = rawMsg.ID
	})
	for _, opt := range opts {
		opt(req)
	}
	return req
}

// NewTestMatch creates a domain.Match with test data
func NewTestMatch(offerID, requestID string, opts ...func(*domain.Match)) *domain.Match {
	match := &domain.Match{
		ID:        uuid.New().String(),
		OfferID:   offerID,
		RequestID: requestID,
		Score:     0.85,
		Reasoning: "Strong medication match",
		MatchedBy: "AUTO",
		Status:    domain.MatchStatusPending,
		CreatedAt: time.Now(),
	}
	for _, opt := range opts {
		opt(match)
	}
	return match
}

// NewTestGroup creates a domain.Group with test data
func NewTestGroup(opts ...func(*domain.Group)) *domain.Group {
	group := &domain.Group{
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

// NewTestMedicationMapping creates a domain.MedicationMapping with test data
func NewTestMedicationMapping(opts ...func(*domain.MedicationMapping)) *domain.MedicationMapping {
	mapping := &domain.MedicationMapping{
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

// NewTestMatchFeedback creates a domain.MatchFeedback with test data
func NewTestMatchFeedback(matchID string, opts ...func(*domain.MatchFeedback)) *domain.MatchFeedback {
	fb := &domain.MatchFeedback{
		ID:                 uuid.New().String(),
		MatchID:            matchID,
		OperatorID:         "operator-1",
		Decision:           domain.FeedbackConfirmed,
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
func testCtx() context.Context {
	ctx, _ := context.WithTimeout(context.Background(), 10*time.Second)
	return ctx
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
