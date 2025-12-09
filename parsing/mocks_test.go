package parsing

import (
	"context"
	"pharmabroker/domain/entity"
	"time"
)

// MockRawMessageRepo
type MockRawMessageRepo struct {
	OnSave                   func(ctx context.Context, msg *entity.RawMessage) error
	OnGetByID                func(ctx context.Context, id string) (*entity.RawMessage, error)
	OnGetUnprocessed         func(ctx context.Context, limit int) ([]*entity.RawMessage, error)
	OnMarkProcessed          func(ctx context.Context, id string, err error) error
	OnGetLastMessageBySender func(ctx context.Context, groupJID, senderJID string) (*entity.RawMessage, error)
	OnArchiveOldMessages     func(ctx context.Context, archivePath string, cutoff time.Time) (int64, error)
}

func (m *MockRawMessageRepo) Save(ctx context.Context, msg *entity.RawMessage) error {
	if m.OnSave != nil {
		return m.OnSave(ctx, msg)
	}
	return nil
}
func (m *MockRawMessageRepo) GetByID(ctx context.Context, id string) (*entity.RawMessage, error) {
	if m.OnGetByID != nil {
		return m.OnGetByID(ctx, id)
	}
	return nil, nil
}
func (m *MockRawMessageRepo) GetUnprocessed(ctx context.Context, limit int) ([]*entity.RawMessage, error) {
	if m.OnGetUnprocessed != nil {
		return m.OnGetUnprocessed(ctx, limit)
	}
	return nil, nil
}
func (m *MockRawMessageRepo) MarkProcessed(ctx context.Context, id string, err error) error {
	if m.OnMarkProcessed != nil {
		return m.OnMarkProcessed(ctx, id, err)
	}
	return nil
}
func (m *MockRawMessageRepo) GetLastMessageBySender(ctx context.Context, groupJID, senderJID string) (*entity.RawMessage, error) {
	if m.OnGetLastMessageBySender != nil {
		return m.OnGetLastMessageBySender(ctx, groupJID, senderJID)
	}
	return nil, nil // Default: no previous message found
}
func (m *MockRawMessageRepo) ArchiveOldMessages(ctx context.Context, archivePath string, cutoff time.Time) (int64, error) {
	if m.OnArchiveOldMessages != nil {
		return m.OnArchiveOldMessages(ctx, archivePath, cutoff)
	}
	return 0, nil
}

// MockOfferRepo
type MockOfferRepo struct {
	OnSave        func(ctx context.Context, offer *entity.Offer) error
	OnSearch      func(ctx context.Context, query string, limit, offset int) ([]*entity.Offer, error)
	OnCountActive func(ctx context.Context) (int64, error)
	// Add others as needed to satisfy interface
	OnGetByID      func(ctx context.Context, id string) (*entity.Offer, error)
	OnGetActive    func(ctx context.Context, limit, offset int) ([]*entity.Offer, error)
	OnUpdateStatus func(ctx context.Context, id string, status entity.ItemStatus) error
}

func (m *MockOfferRepo) Save(ctx context.Context, offer *entity.Offer) error {
	if m.OnSave != nil {
		return m.OnSave(ctx, offer)
	}
	return nil
}
func (m *MockOfferRepo) Search(ctx context.Context, query string, limit, offset int) ([]*entity.Offer, error) {
	if m.OnSearch != nil {
		return m.OnSearch(ctx, query, limit, offset)
	}
	return []*entity.Offer{}, nil
}
func (m *MockOfferRepo) CountActive(ctx context.Context) (int64, error) {
	if m.OnCountActive != nil {
		return m.OnCountActive(ctx)
	}
	return 0, nil
}
func (m *MockOfferRepo) GetByID(ctx context.Context, id string) (*entity.Offer, error) {
	return nil, nil
}
func (m *MockOfferRepo) GetActive(ctx context.Context, limit, offset int) ([]*entity.Offer, error) {
	return nil, nil
}
func (m *MockOfferRepo) UpdateStatus(ctx context.Context, id string, status entity.ItemStatus) error {
	return nil
}

// MockRequestRepo
type MockRequestRepo struct {
	OnSave   func(ctx context.Context, req *entity.Request) error
	OnSearch func(ctx context.Context, query string, limit, offset int) ([]*entity.Request, error)
	// stub others
	OnGetByID      func(ctx context.Context, id string) (*entity.Request, error)
	OnGetActive    func(ctx context.Context, limit, offset int) ([]*entity.Request, error)
	OnUpdateStatus func(ctx context.Context, id string, status entity.ItemStatus) error
	OnCountActive  func(ctx context.Context) (int64, error)
}

func (m *MockRequestRepo) Save(ctx context.Context, req *entity.Request) error {
	if m.OnSave != nil {
		return m.OnSave(ctx, req)
	}
	return nil
}
func (m *MockRequestRepo) Search(ctx context.Context, query string, limit, offset int) ([]*entity.Request, error) {
	if m.OnSearch != nil {
		return m.OnSearch(ctx, query, limit, offset)
	}
	return []*entity.Request{}, nil
}
func (m *MockRequestRepo) GetByID(ctx context.Context, id string) (*entity.Request, error) {
	return nil, nil
}
func (m *MockRequestRepo) GetActive(ctx context.Context, limit, offset int) ([]*entity.Request, error) {
	return nil, nil
}
func (m *MockRequestRepo) UpdateStatus(ctx context.Context, id string, status entity.ItemStatus) error {
	return nil
}
func (m *MockRequestRepo) CountActive(ctx context.Context) (int64, error) { return 0, nil }

// MockMatchRepo
type MockMatchRepo struct {
	OnSave func(ctx context.Context, match *entity.Match) error
	// stub others
	OnGetByID             func(ctx context.Context, id string) (*entity.Match, error)
	OnGetPending          func(ctx context.Context, limit, offset int) ([]*entity.MatchWithDetails, error)
	OnGetByOfferID        func(ctx context.Context, id string) ([]*entity.Match, error)
	OnGetByRequestID      func(ctx context.Context, id string) ([]*entity.Match, error)
	OnUpdateStatus        func(ctx context.Context, id string, status entity.MatchStatus, matchedBy string) error
	OnCountPending        func(ctx context.Context) (int64, error)
	OnCountConfirmedToday func(ctx context.Context) (int64, error)
}

func (m *MockMatchRepo) Save(ctx context.Context, match *entity.Match) error {
	if m.OnSave != nil {
		return m.OnSave(ctx, match)
	}
	return nil
}
func (m *MockMatchRepo) GetByID(ctx context.Context, id string) (*entity.Match, error) {
	return nil, nil
}
func (m *MockMatchRepo) GetPending(ctx context.Context, limit, offset int) ([]*entity.MatchWithDetails, error) {
	return nil, nil
}
func (m *MockMatchRepo) GetByOfferID(ctx context.Context, id string) ([]*entity.Match, error) {
	return nil, nil
}
func (m *MockMatchRepo) GetByRequestID(ctx context.Context, id string) ([]*entity.Match, error) {
	return nil, nil
}
func (m *MockMatchRepo) UpdateStatus(ctx context.Context, id string, status entity.MatchStatus, matchedBy string) error {
	return nil
}
func (m *MockMatchRepo) CountPending(ctx context.Context) (int64, error)        { return 0, nil }
func (m *MockMatchRepo) CountConfirmedToday(ctx context.Context) (int64, error) { return 0, nil }

// MockMedicationRepo
type MockMedicationRepo struct {
	OnGetAll          func(ctx context.Context) ([]*entity.MedicationMapping, error)
	OnSave            func(ctx context.Context, mapping *entity.MedicationMapping) error
	OnGetByArabicName func(ctx context.Context, arabicName string) (*entity.MedicationMapping, error)
	OnCount           func(ctx context.Context) (int, error)
	OnSearch          func(ctx context.Context, query string) ([]*entity.MedicationMapping, error)
}

func (m *MockMedicationRepo) GetAll(ctx context.Context) ([]*entity.MedicationMapping, error) {
	if m.OnGetAll != nil {
		return m.OnGetAll(ctx)
	}
	return []*entity.MedicationMapping{}, nil
}
func (m *MockMedicationRepo) Search(ctx context.Context, query string) ([]*entity.MedicationMapping, error) {
	if m.OnSearch != nil {
		return m.OnSearch(ctx, query)
	}
	return []*entity.MedicationMapping{}, nil
}
func (m *MockMedicationRepo) Save(ctx context.Context, mapping *entity.MedicationMapping) error {
	return nil
}
func (m *MockMedicationRepo) GetByArabicName(ctx context.Context, arabicName string) (*entity.MedicationMapping, error) {
	return nil, nil
}
func (m *MockMedicationRepo) Count(ctx context.Context) (int, error) { return 0, nil }

// MockMatchQueueRepo
type MockMatchQueueRepo struct {
	OnEnqueue      func(ctx context.Context, item *entity.MatchQueueItem) error
	OnDequeueBatch func(ctx context.Context, limit int) ([]*entity.MatchQueueItem, error)
	OnDelete       func(ctx context.Context, id string) error
	OnCount        func(ctx context.Context) (int, error)
}

func (m *MockMatchQueueRepo) Enqueue(ctx context.Context, item *entity.MatchQueueItem) error {
	if m.OnEnqueue != nil {
		return m.OnEnqueue(ctx, item)
	}
	return nil
}
func (m *MockMatchQueueRepo) DequeueBatch(ctx context.Context, limit int) ([]*entity.MatchQueueItem, error) {
	if m.OnDequeueBatch != nil {
		return m.OnDequeueBatch(ctx, limit)
	}
	return nil, nil
}
func (m *MockMatchQueueRepo) Delete(ctx context.Context, id string) error {
	if m.OnDelete != nil {
		return m.OnDelete(ctx, id)
	}
	return nil
}
func (m *MockMatchQueueRepo) Count(ctx context.Context) (int, error) {
	if m.OnCount != nil {
		return m.OnCount(ctx)
	}
	return 0, nil
}
