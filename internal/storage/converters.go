package storage

import (
	"encoding/json"
	"math"
	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"
)

// ========================================
// Domain -> GORM Model Converters
// ========================================

// ToRawMessageModel converts domain.RawMessage to models.RawMessage
func ToRawMessageModel(d *domain.RawMessage) *models.RawMessage {
	return &models.RawMessage{
		ID:          d.ID,
		ExternalID:  nilIfEmpty(d.ExternalID),
		GroupJID:    d.GroupJID,
		GroupName:   d.GroupName,
		SenderJID:   d.SenderJID,
		SenderPhone: d.SenderPhone,
		SenderName:  nilIfEmpty(d.SenderName),
		Content:     d.Content,
		Timestamp:   d.Timestamp,
		ProcessedAt: d.ProcessedAt,
		Error:       nilIfEmpty(d.Error),
	}
}

// ToOfferModel converts domain.Offer to models.Offer
func ToOfferModel(d *domain.Offer) *models.Offer {
	return &models.Offer{
		ID:            d.ID,
		RawMessageID:  nilIfEmpty(d.RawMessageID),
		SourcePhone:   d.SourcePhone,
		SourceName:    nilIfEmpty(d.SourceName),
		SourceGroup:   d.SourceGroup,
		GroupName:     nilIfEmpty(d.GroupName),
		Medication:    d.Medication,
		MedicationRaw: d.MedicationRaw,
		Quantity:      d.Quantity,
		Unit:          d.Unit,
		Price:         nilIfZero(d.Price),
		Currency:      d.Currency,
		ExpiryDate:    d.ExpiryDate,
		BatchNumber:   nilIfEmpty(d.BatchNumber),
		Notes:         nilIfEmpty(d.Notes),
		RawMessage:    d.RawMessage,
		Status:        string(d.Status),
		CreatedAt:     d.CreatedAt,
		UpdatedAt:     d.UpdatedAt,
	}
}

// ToRequestModel converts domain.Request to models.Request
func ToRequestModel(d *domain.Request) *models.Request {
	return &models.Request{
		ID:            d.ID,
		RawMessageID:  nilIfEmpty(d.RawMessageID),
		SourcePhone:   d.SourcePhone,
		SourceName:    nilIfEmpty(d.SourceName),
		SourceGroup:   d.SourceGroup,
		GroupName:     nilIfEmpty(d.GroupName),
		Medication:    d.Medication,
		MedicationRaw: d.MedicationRaw,
		Quantity:      d.Quantity,
		Unit:          d.Unit,
		MaxPrice:      nilIfZero(d.MaxPrice),
		Currency:      d.Currency,
		Urgent:        d.Urgent,
		Notes:         nilIfEmpty(d.Notes),
		RawMessage:    d.RawMessage,
		Status:        string(d.Status),
		CreatedAt:     d.CreatedAt,
		UpdatedAt:     d.UpdatedAt,
	}
}

// ToMatchModel converts domain.Match to models.Match
func ToMatchModel(d *domain.Match) *models.Match {
	return &models.Match{
		ID:          d.ID,
		OfferID:     d.OfferID,
		RequestID:   d.RequestID,
		Score:       d.Score,
		Reasoning:   nilIfEmpty(d.Reasoning),
		MatchedBy:   nilIfEmpty(d.MatchedBy),
		Status:      string(d.Status),
		CreatedAt:   d.CreatedAt,
		ConfirmedAt: d.ConfirmedAt,
		Notes:       nilIfEmpty(d.Notes),
	}
}

// ToMatchQueueModel converts domain.MatchQueueItem to models.MatchQueue
func ToMatchQueueModel(d *domain.MatchQueueItem) *models.MatchQueue {
	return &models.MatchQueue{
		ID:         d.ID,
		SourceType: d.SourceType,
		SourceID:   d.SourceID,
		CreatedAt:  d.CreatedAt,
	}
}

// ToGroupModel converts domain.Group to models.Group
func ToGroupModel(d *domain.Group) *models.Group {
	return &models.Group{
		JID:          d.JID,
		Name:         d.Name,
		Description:  nilIfEmpty(d.Description),
		Monitored:    d.Monitored,
		AddedAt:      d.AddedAt,
		LastMessage:  d.LastMessage,
		MessageCount: d.MessageCount,
	}
}

// ToMedicationMappingModel converts domain.MedicationMapping to models.MedicationMapping
func ToMedicationMappingModel(d *domain.MedicationMapping) *models.MedicationMapping {
	return &models.MedicationMapping{
		ID:          d.ID,
		ArabicName:  d.ArabicName,
		EnglishName: d.EnglishName,
		Synonyms:    serializeSynonyms(d.Synonyms),
		Embedding:   float32SliceToBytes(d.Embedding),
		CreatedAt:   d.CreatedAt,
		UpdatedAt:   d.UpdatedAt,
	}
}

// ToMatchFeedbackModel converts domain.MatchFeedback to models.MatchFeedback
func ToMatchFeedbackModel(d *domain.MatchFeedback) *models.MatchFeedback {
	return &models.MatchFeedback{
		ID:                 d.ID,
		MatchID:            d.MatchID,
		OperatorID:         nilIfEmpty(d.OperatorID),
		Decision:           string(d.Decision),
		Reason:             nilIfEmpty(d.Reason),
		OriginalScore:      d.OriginalScore,
		OriginalConfidence: nilIfEmpty(d.OriginalConfidence),
		CreatedAt:          d.CreatedAt,
	}
}

// ========================================
// GORM Model -> Domain Converters
// ========================================

// ToRawMessageDomain converts models.RawMessage to domain.RawMessage
func ToRawMessageDomain(m *models.RawMessage) *domain.RawMessage {
	return &domain.RawMessage{
		ID:          m.ID,
		ExternalID:  deref(m.ExternalID),
		GroupJID:    m.GroupJID,
		GroupName:   m.GroupName,
		SenderJID:   m.SenderJID,
		SenderPhone: m.SenderPhone,
		SenderName:  deref(m.SenderName),
		Content:     m.Content,
		Timestamp:   m.Timestamp,
		ProcessedAt: m.ProcessedAt,
		Error:       deref(m.Error),
	}
}

// ToOfferDomain converts models.Offer to domain.Offer
func ToOfferDomain(m *models.Offer) *domain.Offer {
	return &domain.Offer{
		ID:            m.ID,
		RawMessageID:  deref(m.RawMessageID),
		SourcePhone:   m.SourcePhone,
		SourceName:    deref(m.SourceName),
		SourceGroup:   m.SourceGroup,
		GroupName:     deref(m.GroupName),
		Medication:    m.Medication,
		MedicationRaw: m.MedicationRaw,
		Quantity:      m.Quantity,
		Unit:          m.Unit,
		Price:         derefFloat(m.Price),
		Currency:      m.Currency,
		ExpiryDate:    m.ExpiryDate,
		BatchNumber:   deref(m.BatchNumber),
		Notes:         deref(m.Notes),
		RawMessage:    m.RawMessage,
		Status:        domain.ItemStatus(m.Status),
		CreatedAt:     m.CreatedAt,
		UpdatedAt:     m.UpdatedAt,
	}
}

// ToRequestDomain converts models.Request to domain.Request
func ToRequestDomain(m *models.Request) *domain.Request {
	return &domain.Request{
		ID:            m.ID,
		RawMessageID:  deref(m.RawMessageID),
		SourcePhone:   m.SourcePhone,
		SourceName:    deref(m.SourceName),
		SourceGroup:   m.SourceGroup,
		GroupName:     deref(m.GroupName),
		Medication:    m.Medication,
		MedicationRaw: m.MedicationRaw,
		Quantity:      m.Quantity,
		Unit:          m.Unit,
		MaxPrice:      derefFloat(m.MaxPrice),
		Currency:      m.Currency,
		Urgent:        m.Urgent,
		Notes:         deref(m.Notes),
		RawMessage:    m.RawMessage,
		Status:        domain.ItemStatus(m.Status),
		CreatedAt:     m.CreatedAt,
		UpdatedAt:     m.UpdatedAt,
	}
}

// ToMatchDomain converts models.Match to domain.Match
func ToMatchDomain(m *models.Match) *domain.Match {
	return &domain.Match{
		ID:          m.ID,
		OfferID:     m.OfferID,
		RequestID:   m.RequestID,
		Score:       m.Score,
		Reasoning:   deref(m.Reasoning),
		MatchedBy:   deref(m.MatchedBy),
		Status:      domain.MatchStatus(m.Status),
		CreatedAt:   m.CreatedAt,
		ConfirmedAt: m.ConfirmedAt,
		Notes:       deref(m.Notes),
	}
}

// ToMatchWithDetailsDomain converts models.Match (with preloaded Offer/Request) to domain.MatchWithDetails
func ToMatchWithDetailsDomain(m *models.Match) *domain.MatchWithDetails {
	mwd := &domain.MatchWithDetails{
		Match: *ToMatchDomain(m),
	}
	if m.Offer != nil {
		mwd.Offer = ToOfferDomain(m.Offer)
	}
	if m.Request != nil {
		mwd.Request = ToRequestDomain(m.Request)
	}
	return mwd
}

// ToMatchQueueItemDomain converts models.MatchQueue to domain.MatchQueueItem
func ToMatchQueueItemDomain(m *models.MatchQueue) *domain.MatchQueueItem {
	return &domain.MatchQueueItem{
		ID:         m.ID,
		SourceType: m.SourceType,
		SourceID:   m.SourceID,
		CreatedAt:  m.CreatedAt,
	}
}

// ToGroupDomain converts models.Group to domain.Group
func ToGroupDomain(m *models.Group) *domain.Group {
	return &domain.Group{
		JID:          m.JID,
		Name:         m.Name,
		Description:  deref(m.Description),
		Monitored:    m.Monitored,
		AddedAt:      m.AddedAt,
		LastMessage:  m.LastMessage,
		MessageCount: m.MessageCount,
	}
}

// ToMedicationMappingDomain converts models.MedicationMapping to domain.MedicationMapping
func ToMedicationMappingDomain(m *models.MedicationMapping) *domain.MedicationMapping {
	return &domain.MedicationMapping{
		ID:          m.ID,
		ArabicName:  m.ArabicName,
		EnglishName: m.EnglishName,
		Synonyms:    deserializeSynonyms(m.Synonyms),
		Embedding:   bytesToFloat32Slice(m.Embedding),
		CreatedAt:   m.CreatedAt,
		UpdatedAt:   m.UpdatedAt,
	}
}

// ToMatchFeedbackDomain converts models.MatchFeedback to domain.MatchFeedback
func ToMatchFeedbackDomain(m *models.MatchFeedback) *domain.MatchFeedback {
	return &domain.MatchFeedback{
		ID:                 m.ID,
		MatchID:            m.MatchID,
		OperatorID:         deref(m.OperatorID),
		Decision:           domain.FeedbackDecision(m.Decision),
		Reason:             deref(m.Reason),
		OriginalScore:      m.OriginalScore,
		OriginalConfidence: deref(m.OriginalConfidence),
		CreatedAt:          m.CreatedAt,
	}
}

// ========================================
// Batch Converters
// ========================================

// ToOffersDomain converts a slice of models.Offer to domain.Offer
func ToOffersDomain(ms []models.Offer) []*domain.Offer {
	result := make([]*domain.Offer, len(ms))
	for i := range ms {
		result[i] = ToOfferDomain(&ms[i])
	}
	return result
}

// ToRequestsDomain converts a slice of models.Request to domain.Request
func ToRequestsDomain(ms []models.Request) []*domain.Request {
	result := make([]*domain.Request, len(ms))
	for i := range ms {
		result[i] = ToRequestDomain(&ms[i])
	}
	return result
}

// ToMatchesDomain converts a slice of models.Match to domain.Match
func ToMatchesDomain(ms []models.Match) []*domain.Match {
	result := make([]*domain.Match, len(ms))
	for i := range ms {
		result[i] = ToMatchDomain(&ms[i])
	}
	return result
}

// ToMatchesWithDetailsDomain converts a slice of models.Match to domain.MatchWithDetails
func ToMatchesWithDetailsDomain(ms []models.Match) []*domain.MatchWithDetails {
	result := make([]*domain.MatchWithDetails, len(ms))
	for i := range ms {
		result[i] = ToMatchWithDetailsDomain(&ms[i])
	}
	return result
}

// ========================================
// Helper Functions
// ========================================

// nilIfEmpty returns nil if s is empty, otherwise a pointer to s
func nilIfEmpty(s string) *string {
	if s == "" {
		return nil
	}
	return &s
}

// nilIfZero returns nil if f is zero, otherwise a pointer to f
func nilIfZero(f float64) *float64 {
	if f == 0 {
		return nil
	}
	return &f
}

// deref returns the value of a string pointer, or empty string if nil
func deref(s *string) string {
	if s == nil {
		return ""
	}
	return *s
}

// derefFloat returns the value of a float64 pointer, or 0 if nil
func derefFloat(f *float64) float64 {
	if f == nil {
		return 0
	}
	return *f
}

// float32SliceToBytes converts []float32 to []byte for storage
func float32SliceToBytes(floats []float32) []byte {
	if len(floats) == 0 {
		return nil
	}
	bytes := make([]byte, len(floats)*4)
	for i, f := range floats {
		bits := math.Float32bits(f)
		bytes[i*4] = byte(bits)
		bytes[i*4+1] = byte(bits >> 8)
		bytes[i*4+2] = byte(bits >> 16)
		bytes[i*4+3] = byte(bits >> 24)
	}
	return bytes
}

// bytesToFloat32Slice converts []byte back to []float32
func bytesToFloat32Slice(bytes []byte) []float32 {
	if len(bytes) == 0 || len(bytes)%4 != 0 {
		return nil
	}
	floats := make([]float32, len(bytes)/4)
	for i := range floats {
		bits := uint32(bytes[i*4]) |
			uint32(bytes[i*4+1])<<8 |
			uint32(bytes[i*4+2])<<16 |
			uint32(bytes[i*4+3])<<24
		floats[i] = math.Float32frombits(bits)
	}
	return floats
}

// serializeSynonyms converts []string to JSON string for storage
func serializeSynonyms(synonyms []string) string {
	if len(synonyms) == 0 {
		return "[]"
	}
	data, err := json.Marshal(synonyms)
	if err != nil {
		return "[]"
	}
	return string(data)
}

// deserializeSynonyms converts JSON string back to []string
func deserializeSynonyms(s string) []string {
	if s == "" || s == "[]" {
		return nil
	}
	var synonyms []string
	if err := json.Unmarshal([]byte(s), &synonyms); err != nil {
		return nil
	}
	return synonyms
}
