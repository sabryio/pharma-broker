package parsing

import (
	"context"
	"testing"

	"github.com/stretchr/testify/assert"

	"pharmabroker/domain/entity"
)

func TestParsePass_String(t *testing.T) {
	tests := []struct {
		pass     ParsePass
		expected string
	}{
		{ParsePassStrict, "STRICT"},
		{ParsePassRelaxed, "RELAXED"},
		{ParsePassReview, "REVIEW"},
		{ParsePass(99), "UNKNOWN"},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			assert.Equal(t, tt.expected, tt.pass.String())
		})
	}
}

func TestGetConfidenceLevelForScore(t *testing.T) {
	tests := []struct {
		name     string
		score    float64
		expected ParseConfidence
	}{
		{"High confidence 1.0", 1.0, ParseConfidenceHigh},
		{"High confidence 0.8", 0.8, ParseConfidenceHigh},
		{"Medium confidence 0.79", 0.79, ParseConfidenceMedium},
		{"Medium confidence 0.5", 0.5, ParseConfidenceMedium},
		{"Low confidence 0.49", 0.49, ParseConfidenceLow},
		{"Low confidence 0.1", 0.1, ParseConfidenceLow},
		{"Failed 0.0", 0.0, ParseConfidenceFailed},
		{"Failed negative", -0.1, ParseConfidenceFailed},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := GetConfidenceLevelForScore(tt.score)
			assert.Equal(t, tt.expected, result)
		})
	}
}

func TestDefaultMultiPassConfig(t *testing.T) {
	cfg := DefaultMultiPassConfig()

	assert.Equal(t, 0.7, cfg.StrictMinConfidence)
	assert.Equal(t, 0.4, cfg.RelaxedMinConfidence)
	assert.True(t, cfg.EnablePass2)
	assert.True(t, cfg.EnableReviewQueue)
}

func TestParser_CalculateAvgConfidence(t *testing.T) {
	p := &Parser{}

	tests := []struct {
		name     string
		items    []entity.ParsedItem
		expected float64
	}{
		{
			"Empty items",
			[]entity.ParsedItem{},
			0.0,
		},
		{
			"Single item",
			[]entity.ParsedItem{{AIConfidence: 0.9}},
			0.9,
		},
		{
			"Multiple items",
			[]entity.ParsedItem{
				{AIConfidence: 0.8},
				{AIConfidence: 0.6},
				{AIConfidence: 0.7},
			},
			0.7, // (0.8 + 0.6 + 0.7) / 3
		},
		{
			"All zeros",
			[]entity.ParsedItem{
				{AIConfidence: 0.0},
				{AIConfidence: 0.0},
			},
			0.0,
		},
		{
			"Mixed",
			[]entity.ParsedItem{
				{AIConfidence: 1.0},
				{AIConfidence: 0.0},
			},
			0.5,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := p.calculateAvgConfidence(tt.items)
			assert.InDelta(t, tt.expected, result, 0.001)
		})
	}
}

func TestParser_ShouldQueueForReview(t *testing.T) {
	// Parser without review queue repo - should never queue
	pNoRepo := &Parser{
		reviewQueueRepo: nil,
		multiPassConfig: DefaultMultiPassConfig(),
	}

	result := &entity.AIParseResult{
		Items: []entity.ParsedItem{},
	}
	assert.False(t, pNoRepo.shouldQueueForReview(result), "Should not queue when repo is nil")

	// Parser with review queue - mock
	pWithRepo := &Parser{
		reviewQueueRepo: &mockReviewQueueRepo{},
		multiPassConfig: DefaultMultiPassConfig(),
	}

	tests := []struct {
		name     string
		result   *entity.AIParseResult
		expected bool
	}{
		{
			"Empty items no error - should queue",
			&entity.AIParseResult{Items: []entity.ParsedItem{}, Error: ""},
			true,
		},
		{
			"Empty items with error - should not queue (error handled elsewhere)",
			&entity.AIParseResult{Items: []entity.ParsedItem{}, Error: "AI error"},
			false,
		},
		{
			"High confidence items - should not queue",
			&entity.AIParseResult{
				Items: []entity.ParsedItem{
					{AIConfidence: 0.9},
					{AIConfidence: 0.85},
				},
			},
			false,
		},
		{
			"Low confidence items - should queue",
			&entity.AIParseResult{
				Items: []entity.ParsedItem{
					{AIConfidence: 0.3},
					{AIConfidence: 0.2},
				},
			},
			true,
		},
		{
			"Just above threshold - should not queue",
			&entity.AIParseResult{
				Items: []entity.ParsedItem{
					{AIConfidence: 0.41},
				},
			},
			false,
		},
		{
			"Just below threshold - should queue",
			&entity.AIParseResult{
				Items: []entity.ParsedItem{
					{AIConfidence: 0.39},
				},
			},
			true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := pWithRepo.shouldQueueForReview(tt.result)
			assert.Equal(t, tt.expected, result)
		})
	}
}

// mockReviewQueueRepo is a minimal mock for testing
type mockReviewQueueRepo struct{}

func (m *mockReviewQueueRepo) Save(ctx context.Context, item *entity.ReviewQueueItem) error {
	return nil
}

func (m *mockReviewQueueRepo) GetByID(ctx context.Context, id string) (*entity.ReviewQueueItem, error) {
	return nil, nil
}

func (m *mockReviewQueueRepo) GetPending(ctx context.Context, limit, offset int) ([]*entity.ReviewQueueItem, error) {
	return nil, nil
}

func (m *mockReviewQueueRepo) CountPending(ctx context.Context) (int64, error) {
	return 0, nil
}

func (m *mockReviewQueueRepo) Approve(ctx context.Context, id string, reviewedBy string, correctedItems []entity.ParsedItem, note string) error {
	return nil
}

func (m *mockReviewQueueRepo) Reject(ctx context.Context, id string, reviewedBy string, reason string) error {
	return nil
}

func (m *mockReviewQueueRepo) GetByRawMessageID(ctx context.Context, rawMessageID string) (*entity.ReviewQueueItem, error) {
	return nil, nil
}
