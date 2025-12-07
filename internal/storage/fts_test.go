package storage

import (
	"context"
	"os"
	"testing"
	"time"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"

	"github.com/google/uuid"
)

// TestFTS_Matching verifies that the FTS search logic works as expected
// specifically testing the "Augmentin 1g" vs "Augmentin" case.
func TestFTS_Matching(t *testing.T) {
	// Temporarily create a file DB to make debugging easier if needed, or stick to memory
	dbPath := "test_fts.db"
	_ = os.Remove(dbPath)
	defer os.Remove(dbPath)

	dbConf := &config.DatabaseConfig{
		Path: dbPath,
	}

	db, err := New(dbConf)
	if err != nil {
		t.Fatalf("Failed to init DB: %v", err)
	}
	defer db.Close()

	// Initialize Repos
	rawMsgRepo := NewRawMessageRepo(db)
	offerRepo := NewOfferRepo(db)
	requestRepo := NewRequestRepo(db)

	ctx := context.Background()

	// Helper to create raw message
	createRawMsg := func(id string) {
		msg := &domain.RawMessage{
			ID:         id,
			ExternalID: uuid.New().String(),
			Content:    "Test message",
			Timestamp:  time.Now(),
		}
		if err := rawMsgRepo.Save(ctx, msg); err != nil {
			t.Fatalf("Failed to save raw message: %v", err)
		}
	}

	// 1. Insert an Offer for "Augmentin 1g"
	offerID := uuid.New().String()
	rawMsgID_A := uuid.New().String()
	createRawMsg(rawMsgID_A)

	offer := &domain.Offer{
		ID:           offerID,
		Medication:   "Augmentin 1g",
		Status:       domain.StatusActive,
		RawMessageID: rawMsgID_A,
	}
	if err := offerRepo.Save(ctx, offer); err != nil {
		t.Fatalf("Failed to save offer: %v", err)
	}

	// 2. Insert a Request for "Augmentin"
	reqID := uuid.New().String()
	rawMsgID_B := uuid.New().String()
	createRawMsg(rawMsgID_B)

	req := &domain.Request{
		ID:           reqID,
		Medication:   "Augmentin",
		Status:       domain.StatusActive,
		RawMessageID: rawMsgID_B,
	}
	if err := requestRepo.Save(ctx, req); err != nil {
		t.Fatalf("Failed to save request: %v", err)
	}

	// 3. Test Search Logic
	// Logic from parser.go: sanitizeForFTS and then Search

	// Helper to mimic parser.go logic (simplified)
	sanitizeForFTS := func(s string) string {
		return "Augmentin OR 1g"
	}

	// Case A: Offer "Augmentin 1g" searching for Requests using sanitize("Augmentin 1g")
	// Expected: Should find Request "Augmentin"
	queryA := sanitizeForFTS("Augmentin 1g")

	requests, err := requestRepo.Search(ctx, queryA, 10, 0)
	if err != nil {
		t.Fatalf("Search A failed: %v", err)
	}
	found := false
	for _, r := range requests {
		if r.ID == reqID {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("Case A Failed: Search with query '%s' did not find Request 'Augmentin'", queryA)
	}

	// Case B: Request "Augmentin" searching for Offers using sanitize("Augmentin")
	// Expected: Should find Offer "Augmentin 1g"
	queryB := "Augmentin"
	offers, err := offerRepo.Search(ctx, queryB, 10, 0)
	if err != nil {
		t.Fatalf("Search B failed: %v", err)
	}
	found = false
	for _, o := range offers {
		if o.ID == offerID {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("Case B Failed: Search with query '%s' did not find Offer 'Augmentin 1g'", queryB)
	}

	// Case C: Exact phrase match attempt (old logic simulation)
	queryC := "Augmentin 1g"
	requestsC, _ := requestRepo.Search(ctx, queryC, 10, 0)
	foundC := false
	for _, r := range requestsC {
		if r.ID == reqID {
			foundC = true
		}
	}
	if foundC {
		t.Logf("Interesting: 'Augmentin 1g' (AND) matched 'Augmentin'?")
	} else {
		t.Logf("Confirmed: 'Augmentin 1g' (AND) does not match 'Augmentin'. OR is required.")
	}
}
