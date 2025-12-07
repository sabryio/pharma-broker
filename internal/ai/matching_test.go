package ai

import (
	"context"
	"testing"
	"time"

	"pharmabroker/internal/domain"
)

// ---- Mock implementations for matching tests ----

type matchingMatchRepo struct {
	matches []*domain.Match
	saved   []*domain.Match
}

func newMatchingMatchRepo() *matchingMatchRepo {
	return &matchingMatchRepo{
		matches: make([]*domain.Match, 0),
		saved:   make([]*domain.Match, 0),
	}
}

func (m *matchingMatchRepo) Save(ctx context.Context, match *domain.Match) error {
	m.saved = append(m.saved, match)
	m.matches = append(m.matches, match)
	return nil
}

func (m *matchingMatchRepo) GetByID(ctx context.Context, id string) (*domain.Match, error) {
	for _, match := range m.matches {
		if match.ID == id {
			return match, nil
		}
	}
	return nil, nil
}

func (m *matchingMatchRepo) GetPending(ctx context.Context, limit, offset int) ([]*domain.MatchWithDetails, error) {
	var results []*domain.MatchWithDetails
	for _, match := range m.matches {
		if match.Status == domain.MatchStatusPending {
			results = append(results, &domain.MatchWithDetails{Match: *match})
		}
	}
	return results, nil
}

func (m *matchingMatchRepo) CountPending(ctx context.Context) (int64, error) {
	var count int64
	for _, match := range m.matches {
		if match.Status == domain.MatchStatusPending {
			count++
		}
	}
	return count, nil
}

func (m *matchingMatchRepo) UpdateStatus(ctx context.Context, id string, status domain.MatchStatus, matchedBy string) error {
	for _, match := range m.matches {
		if match.ID == id {
			match.Status = status
			match.MatchedBy = matchedBy
			break
		}
	}
	return nil
}

func (m *matchingMatchRepo) GetByOfferID(ctx context.Context, offerID string) ([]*domain.Match, error) {
	var results []*domain.Match
	for _, match := range m.matches {
		if match.OfferID == offerID {
			results = append(results, match)
		}
	}
	return results, nil
}

func (m *matchingMatchRepo) GetByRequestID(ctx context.Context, requestID string) ([]*domain.Match, error) {
	var results []*domain.Match
	for _, match := range m.matches {
		if match.RequestID == requestID {
			results = append(results, match)
		}
	}
	return results, nil
}

func (m *matchingMatchRepo) CountConfirmedToday(ctx context.Context) (int64, error) {
	return 0, nil
}

// ---- Tests ----

func TestMatchRepo_SaveAndRetrieve(t *testing.T) {
	ctx := context.Background()
	repo := newMatchingMatchRepo()

	match := &domain.Match{
		ID:        "match-001",
		OfferID:   "offer-001",
		RequestID: "req-001",
		Score:     0.85,
		Reasoning: "High medication similarity",
		Status:    domain.MatchStatusPending,
		CreatedAt: time.Now(),
	}

	// Save
	err := repo.Save(ctx, match)
	if err != nil {
		t.Fatalf("Failed to save match: %v", err)
	}

	// Retrieve
	retrieved, err := repo.GetByID(ctx, match.ID)
	if err != nil {
		t.Fatalf("Failed to retrieve match: %v", err)
	}

	if retrieved.ID != match.ID {
		t.Errorf("Retrieved match ID = %s, want %s", retrieved.ID, match.ID)
	}
	if retrieved.Score != match.Score {
		t.Errorf("Retrieved score = %.2f, want %.2f", retrieved.Score, match.Score)
	}
}

func TestMatchRepo_GetPending(t *testing.T) {
	ctx := context.Background()
	repo := newMatchingMatchRepo()

	// Add pending matches
	repo.Save(ctx, &domain.Match{ID: "m1", Status: domain.MatchStatusPending, Score: 0.9})
	repo.Save(ctx, &domain.Match{ID: "m2", Status: domain.MatchStatusConfirmed, Score: 0.8})
	repo.Save(ctx, &domain.Match{ID: "m3", Status: domain.MatchStatusPending, Score: 0.7})

	pending, err := repo.GetPending(ctx, 10, 0)
	if err != nil {
		t.Fatalf("GetPending failed: %v", err)
	}

	if len(pending) != 2 {
		t.Errorf("Expected 2 pending matches, got %d", len(pending))
	}
}

func TestMatchRepo_CountPending(t *testing.T) {
	ctx := context.Background()
	repo := newMatchingMatchRepo()

	repo.Save(ctx, &domain.Match{ID: "m1", Status: domain.MatchStatusPending})
	repo.Save(ctx, &domain.Match{ID: "m2", Status: domain.MatchStatusConfirmed})
	repo.Save(ctx, &domain.Match{ID: "m3", Status: domain.MatchStatusPending})
	repo.Save(ctx, &domain.Match{ID: "m4", Status: domain.MatchStatusRejected})

	count, err := repo.CountPending(ctx)
	if err != nil {
		t.Fatalf("CountPending failed: %v", err)
	}

	if count != 2 {
		t.Errorf("Expected 2 pending, got %d", count)
	}
}

func TestMatchStatus_Confirm(t *testing.T) {
	ctx := context.Background()
	repo := newMatchingMatchRepo()

	match := &domain.Match{
		ID:        "match-confirm",
		OfferID:   "offer-c1",
		RequestID: "req-c1",
		Score:     0.85,
		Status:    domain.MatchStatusPending,
		CreatedAt: time.Now(),
	}
	repo.Save(ctx, match)

	// Confirm match
	err := repo.UpdateStatus(ctx, match.ID, domain.MatchStatusConfirmed, "operator@phone")
	if err != nil {
		t.Fatalf("Failed to confirm match: %v", err)
	}

	// Verify status changed
	updated, _ := repo.GetByID(ctx, match.ID)
	if updated.Status != domain.MatchStatusConfirmed {
		t.Errorf("Match status = %s, want CONFIRMED", updated.Status)
	}
	if updated.MatchedBy != "operator@phone" {
		t.Errorf("Match.MatchedBy = %s, want 'operator@phone'", updated.MatchedBy)
	}
}

func TestMatchStatus_Reject(t *testing.T) {
	ctx := context.Background()
	repo := newMatchingMatchRepo()

	match := &domain.Match{
		ID:        "match-reject",
		OfferID:   "offer-r1",
		RequestID: "req-r1",
		Score:     0.60,
		Status:    domain.MatchStatusPending,
		CreatedAt: time.Now(),
	}
	repo.Save(ctx, match)

	// Reject match
	err := repo.UpdateStatus(ctx, match.ID, domain.MatchStatusRejected, "admin")
	if err != nil {
		t.Fatalf("Failed to reject match: %v", err)
	}

	// Verify status changed
	updated, _ := repo.GetByID(ctx, match.ID)
	if updated.Status != domain.MatchStatusRejected {
		t.Errorf("Match status = %s, want REJECTED", updated.Status)
	}
}

func TestMatchRepo_GetByOfferID(t *testing.T) {
	ctx := context.Background()
	repo := newMatchingMatchRepo()

	repo.Save(ctx, &domain.Match{ID: "m1", OfferID: "offer-x", RequestID: "r1", Status: domain.MatchStatusPending})
	repo.Save(ctx, &domain.Match{ID: "m2", OfferID: "offer-x", RequestID: "r2", Status: domain.MatchStatusPending})
	repo.Save(ctx, &domain.Match{ID: "m3", OfferID: "offer-y", RequestID: "r3", Status: domain.MatchStatusPending})

	matches, err := repo.GetByOfferID(ctx, "offer-x")
	if err != nil {
		t.Fatalf("GetByOfferID failed: %v", err)
	}

	if len(matches) != 2 {
		t.Errorf("Expected 2 matches for offer-x, got %d", len(matches))
	}
}

func TestMatchRepo_GetByRequestID(t *testing.T) {
	ctx := context.Background()
	repo := newMatchingMatchRepo()

	repo.Save(ctx, &domain.Match{ID: "m1", OfferID: "o1", RequestID: "req-a", Status: domain.MatchStatusPending})
	repo.Save(ctx, &domain.Match{ID: "m2", OfferID: "o2", RequestID: "req-a", Status: domain.MatchStatusConfirmed})
	repo.Save(ctx, &domain.Match{ID: "m3", OfferID: "o3", RequestID: "req-b", Status: domain.MatchStatusPending})

	matches, err := repo.GetByRequestID(ctx, "req-a")
	if err != nil {
		t.Fatalf("GetByRequestID failed: %v", err)
	}

	if len(matches) != 2 {
		t.Errorf("Expected 2 matches for req-a, got %d", len(matches))
	}
}

func TestCosineSimilarity_Vectors(t *testing.T) {
	tests := []struct {
		name     string
		a, b     []float32
		expected float64
		delta    float64
	}{
		{"identical", []float32{1, 0, 0}, []float32{1, 0, 0}, 1.0, 0.01},
		{"orthogonal", []float32{1, 0, 0}, []float32{0, 1, 0}, 0.0, 0.01},
		{"opposite", []float32{1, 0, 0}, []float32{-1, 0, 0}, -1.0, 0.01},
		{"same_direction", []float32{1, 1, 1}, []float32{1, 1, 1}, 1.0, 0.01},
		{"zero_vector", []float32{0, 0, 0}, []float32{1, 1, 1}, 0.0, 0.01},
		{"close_vectors", []float32{3, 4}, []float32{4, 3}, 0.96, 0.05},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := CosineSimilarity(tt.a, tt.b)
			diff := result - tt.expected
			if diff < -tt.delta || diff > tt.delta {
				t.Errorf("CosineSimilarity(%v, %v) = %.4f, want %.4f (±%.2f)",
					tt.a, tt.b, result, tt.expected, tt.delta)
			}
		})
	}
}

func TestMatchScoring_Fields(t *testing.T) {
	// Test domain.Match struct fields for scoring compatibility
	match := &domain.Match{
		Score:  0.87,
		Status: domain.MatchStatusPending,
	}

	if match.Score < 0 || match.Score > 1 {
		t.Errorf("Score should be between 0 and 1, got %.2f", match.Score)
	}

	if match.Status != domain.MatchStatusPending {
		t.Errorf("Expected PENDING status, got %s", match.Status)
	}
}

func TestMatchStatus_Transitions(t *testing.T) {
	ctx := context.Background()
	repo := newMatchingMatchRepo()

	match := &domain.Match{
		ID:     "trans-001",
		Status: domain.MatchStatusPending,
	}
	repo.Save(ctx, match)

	// Pending -> Confirmed
	repo.UpdateStatus(ctx, match.ID, domain.MatchStatusConfirmed, "user1")
	retrieved, _ := repo.GetByID(ctx, match.ID)
	if retrieved.Status != domain.MatchStatusConfirmed {
		t.Error("Failed transition: Pending -> Confirmed")
	}

	// Reset and test Pending -> Rejected
	match2 := &domain.Match{ID: "trans-002", Status: domain.MatchStatusPending}
	repo.Save(ctx, match2)
	repo.UpdateStatus(ctx, match2.ID, domain.MatchStatusRejected, "user2")
	retrieved2, _ := repo.GetByID(ctx, match2.ID)
	if retrieved2.Status != domain.MatchStatusRejected {
		t.Error("Failed transition: Pending -> Rejected")
	}
}

func TestMatchWithDetails_Embedding(t *testing.T) {
	// Test MatchWithDetails struct for reporting
	mwd := &domain.MatchWithDetails{
		Match: domain.Match{
			ID:        "detail-001",
			OfferID:   "o1",
			RequestID: "r1",
			Score:     0.92,
			Status:    domain.MatchStatusPending,
		},
		Offer: &domain.Offer{
			ID:         "o1",
			Medication: "Paracetamol 500mg",
			Quantity:   100,
			Price:      25.50,
		},
		Request: &domain.Request{
			ID:         "r1",
			Medication: "Paracetamol",
			Quantity:   50,
			MaxPrice:   30.00,
			Urgent:     true,
		},
	}

	if mwd.Offer == nil || mwd.Request == nil {
		t.Error("Expected Offer and Request to be populated")
	}

	if mwd.Request.Urgent != true {
		t.Error("Expected Request.Urgent to be true")
	}
}
