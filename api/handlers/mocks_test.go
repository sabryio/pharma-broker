package handlers

import (
	"context"
	"time"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// Mock repositories for testing

type mockOfferRepo struct {
	offers []*entity.Offer
}

func (m *mockOfferRepo) Save(ctx context.Context, o *entity.Offer) error { return nil }
func (m *mockOfferRepo) GetByID(ctx context.Context, id string) (*entity.Offer, error) {
	for _, o := range m.offers {
		if o.ID == id {
			return o, nil
		}
	}
	return nil, nil
}
func (m *mockOfferRepo) GetActive(ctx context.Context, limit, offset int) ([]*entity.Offer, error) {
	return m.offers, nil
}
func (m *mockOfferRepo) Search(ctx context.Context, query string, limit, offset int) ([]*entity.Offer, error) {
	return m.offers, nil
}
func (m *mockOfferRepo) CountActive(ctx context.Context) (int64, error) {
	return int64(len(m.offers)), nil
}
func (m *mockOfferRepo) UpdateStatus(ctx context.Context, id string, status entity.ItemStatus) error {
	return nil
}
func (m *mockOfferRepo) FindRecentDuplicate(ctx context.Context, senderPhone, medication string, within time.Duration) (*entity.Offer, error) {
	return nil, nil
}

// Verify mockOfferRepo implements repository.OfferRepository
var _ repository.OfferRepository = (*mockOfferRepo)(nil)

type mockRequestRepo struct {
	requests []*entity.Request
}

func (m *mockRequestRepo) Save(ctx context.Context, r *entity.Request) error { return nil }
func (m *mockRequestRepo) GetByID(ctx context.Context, id string) (*entity.Request, error) {
	for _, r := range m.requests {
		if r.ID == id {
			return r, nil
		}
	}
	return nil, nil
}
func (m *mockRequestRepo) GetActive(ctx context.Context, limit, offset int) ([]*entity.Request, error) {
	return m.requests, nil
}
func (m *mockRequestRepo) Search(ctx context.Context, query string, limit, offset int) ([]*entity.Request, error) {
	return m.requests, nil
}
func (m *mockRequestRepo) CountActive(ctx context.Context) (int64, error) {
	return int64(len(m.requests)), nil
}
func (m *mockRequestRepo) UpdateStatus(ctx context.Context, id string, status entity.ItemStatus) error {
	return nil
}

var _ repository.RequestRepository = (*mockRequestRepo)(nil)

type mockMatchRepo struct {
	matches []*entity.MatchWithDetails
}

func (m *mockMatchRepo) Save(ctx context.Context, match *entity.Match) error { return nil }
func (m *mockMatchRepo) GetByID(ctx context.Context, id string) (*entity.Match, error) {
	for _, match := range m.matches {
		if match.ID == id {
			return &entity.Match{
				ID:        match.ID,
				OfferID:   match.OfferID,
				RequestID: match.RequestID,
				Score:     match.Score,
				Status:    match.Status,
			}, nil
		}
	}
	return nil, nil
}
func (m *mockMatchRepo) GetPending(ctx context.Context, limit, offset int) ([]*entity.MatchWithDetails, error) {
	return m.matches, nil
}
func (m *mockMatchRepo) CountPending(ctx context.Context) (int64, error) {
	return int64(len(m.matches)), nil
}
func (m *mockMatchRepo) UpdateStatus(ctx context.Context, id string, status entity.MatchStatus, matchedBy string, notes string) error {
	return nil
}
func (m *mockMatchRepo) GetByOfferID(ctx context.Context, offerID string) ([]*entity.Match, error) {
	return nil, nil
}
func (m *mockMatchRepo) GetByRequestID(ctx context.Context, requestID string) ([]*entity.Match, error) {
	return nil, nil
}
func (m *mockMatchRepo) CountConfirmedToday(ctx context.Context) (int64, error) {
	return 0, nil
}
func (m *mockMatchRepo) GetStaleMatches(ctx context.Context, statuses []entity.MatchStatus, maxAge time.Duration, limit int) ([]*entity.Match, error) {
	return nil, nil
}

var _ repository.MatchRepository = (*mockMatchRepo)(nil)

type mockGroupRepo struct {
	groups []*entity.Group
}

func (m *mockGroupRepo) Save(ctx context.Context, g *entity.Group) error {
	return nil
}
func (m *mockGroupRepo) GetAll(ctx context.Context) ([]*entity.Group, error) {
	return m.groups, nil
}
func (m *mockGroupRepo) GetMonitored(ctx context.Context) ([]*entity.Group, error) {
	var result []*entity.Group
	for _, g := range m.groups {
		if g.Monitored {
			result = append(result, g)
		}
	}
	return result, nil
}
func (m *mockGroupRepo) SetMonitored(ctx context.Context, jid string, monitored bool) error {
	return nil
}
func (m *mockGroupRepo) UpdateLastMessage(ctx context.Context, jid string) error {
	return nil
}
func (m *mockGroupRepo) IncrementMessageCount(ctx context.Context, jid string) error {
	return nil
}
func (m *mockGroupRepo) SaveFromSync(ctx context.Context, jid, name, desc string) error {
	return nil
}
func (m *mockGroupRepo) EnableFromConfig(ctx context.Context, jids []string) (int, error) {
	return 0, nil
}

var _ repository.GroupRepository = (*mockGroupRepo)(nil)

type mockStatsRepo struct{}

func (m *mockStatsRepo) GetStats(ctx context.Context) (*entity.Stats, error) {
	return &entity.Stats{
		ActiveOffers:   10,
		ActiveRequests: 5,
		PendingMatches: 3,
	}, nil
}
func (m *mockStatsRepo) GetProcessedToday(ctx context.Context) (int64, error) {
	return 25, nil
}

var _ repository.StatsRepository = (*mockStatsRepo)(nil)

type mockConfigRepo struct{}

func (m *mockConfigRepo) GetAll(ctx context.Context) (*entity.AppConfig, error) {
	return &entity.AppConfig{AutoParseEnabled: true, SkipOwnMessages: true}, nil
}
func (m *mockConfigRepo) UpdateFromMap(ctx context.Context, updates map[string]interface{}) error {
	return nil
}

var _ repository.ConfigRepository = (*mockConfigRepo)(nil)

type mockAuditRepo struct {
	logs []*entity.AuditLog
}

func (m *mockAuditRepo) Log(ctx context.Context, action entity.AuditAction, entityID, details string) error {
	m.logs = append(m.logs, &entity.AuditLog{
		ID:        "test-id",
		Action:    action,
		EntityID:  entityID,
		Details:   details,
		CreatedAt: time.Now(),
	})
	return nil
}
func (m *mockAuditRepo) LogWithValues(ctx context.Context, action entity.AuditAction, entityID, oldVal, newVal, details string) error {
	return nil
}
func (m *mockAuditRepo) GetRecent(ctx context.Context, limit int) ([]*entity.AuditLog, error) {
	return m.logs, nil
}
func (m *mockAuditRepo) GetByAction(ctx context.Context, action entity.AuditAction, limit int) ([]*entity.AuditLog, error) {
	return m.logs, nil
}
func (m *mockAuditRepo) DeleteOlderThan(ctx context.Context, cutoff time.Time) (int64, error) {
	return 0, nil
}

var _ repository.AuditRepository = (*mockAuditRepo)(nil)

type mockFeedbackRepo struct{}

func (m *mockFeedbackRepo) RecordFeedback(ctx context.Context, feedback *entity.MatchFeedback) error {
	return nil
}
func (m *mockFeedbackRepo) GetFeedbackByMatch(ctx context.Context, matchID string) ([]*entity.MatchFeedback, error) {
	return nil, nil
}
func (m *mockFeedbackRepo) AnalyzeFeedback(ctx context.Context, days int) (*entity.FeedbackAnalysis, error) {
	return &entity.FeedbackAnalysis{}, nil
}
func (m *mockFeedbackRepo) GetRecentFeedback(ctx context.Context, limit int) ([]*entity.MatchFeedback, error) {
	return nil, nil
}

var _ repository.FeedbackRepository = (*mockFeedbackRepo)(nil)

type mockLeaderboardRepo struct{}

func (m *mockLeaderboardRepo) GetTopDemand(ctx context.Context, limit int) ([]*entity.DemandStats, error) {
	return nil, nil
}
func (m *mockLeaderboardRepo) GetDemandForMedication(ctx context.Context, medication string) (*entity.DemandStats, error) {
	return nil, nil
}
func (m *mockLeaderboardRepo) RefreshLeaderboard(ctx context.Context) error {
	return nil
}

var _ repository.LeaderboardRepository = (*mockLeaderboardRepo)(nil)

type mockReviewRepo struct {
	items []*entity.ReviewQueueItem
}

func (m *mockReviewRepo) Save(ctx context.Context, item *entity.ReviewQueueItem) error {
	return nil
}
func (m *mockReviewRepo) GetByID(ctx context.Context, id string) (*entity.ReviewQueueItem, error) {
	for _, item := range m.items {
		if item.ID == id {
			return item, nil
		}
	}
	return nil, nil
}
func (m *mockReviewRepo) GetPending(ctx context.Context, limit, offset int) ([]*entity.ReviewQueueItem, error) {
	return m.items, nil
}
func (m *mockReviewRepo) CountPending(ctx context.Context) (int64, error) {
	return int64(len(m.items)), nil
}
func (m *mockReviewRepo) Approve(ctx context.Context, id string, reviewedBy string, correctedItems []entity.ParsedItem, note string) error {
	return nil
}
func (m *mockReviewRepo) Reject(ctx context.Context, id string, reviewedBy string, reason string) error {
	return nil
}
func (m *mockReviewRepo) GetByRawMessageID(ctx context.Context, rawMessageID string) (*entity.ReviewQueueItem, error) {
	return nil, nil
}

var _ repository.ReviewQueueRepository = (*mockReviewRepo)(nil)
