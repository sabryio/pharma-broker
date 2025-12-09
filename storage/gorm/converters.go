// Package gorm provides converters between domain entities and GORM models.
package gorm

import (
	"encoding/binary"
	"encoding/json"
	"math"

	"pharmabroker/domain/entity"
)

// ========================================
// Domain -> GORM Model Converters
// ========================================

// ToRawMessageModel converts entity.RawMessage to gorm RawMessage
func ToRawMessageModel(d *entity.RawMessage) *RawMessage {
	return &RawMessage{
		ID:             d.ID,
		ExternalID:     nilIfEmpty(d.ExternalID),
		GroupJID:       d.GroupJID,
		GroupName:      d.GroupName,
		SenderJID:      d.SenderJID,
		SenderPhone:    d.SenderPhone,
		SenderName:     nilIfEmpty(d.SenderName),
		Content:        d.Content,
		Timestamp:      d.Timestamp,
		ProcessedAt:    d.ProcessedAt,
		Error:          nilIfEmpty(d.Error),
		ReplyToID:      nilIfEmpty(d.ReplyToID),
		ReplyToContent: nilIfEmpty(d.ReplyToContent),
		ReplyToSender:  nilIfEmpty(d.ReplyToSender),
	}
}

// ToOfferModel converts entity.Offer to gorm Offer
func ToOfferModel(d *entity.Offer) *Offer {
	return &Offer{
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

// ToRequestModel converts entity.Request to gorm Request
func ToRequestModel(d *entity.Request) *Request {
	return &Request{
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

// ToMatchModel converts entity.Match to gorm Match
func ToMatchModel(d *entity.Match) *Match {
	return &Match{
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

// ToGroupModel converts entity.Group to gorm Group
func ToGroupModel(d *entity.Group) *Group {
	return &Group{
		JID:          d.JID,
		Name:         d.Name,
		Description:  nilIfEmpty(d.Description),
		Monitored:    d.Monitored,
		AddedAt:      d.AddedAt,
		LastMessage:  d.LastMessage,
		MessageCount: d.MessageCount,
	}
}

// ========================================
// GORM Model -> Domain Converters
// ========================================

// ToRawMessageEntity converts gorm RawMessage to entity.RawMessage
func ToRawMessageEntity(m *RawMessage) *entity.RawMessage {
	return &entity.RawMessage{
		ID:             m.ID,
		ExternalID:     deref(m.ExternalID),
		GroupJID:       m.GroupJID,
		GroupName:      m.GroupName,
		SenderJID:      m.SenderJID,
		SenderPhone:    m.SenderPhone,
		SenderName:     deref(m.SenderName),
		Content:        m.Content,
		Timestamp:      m.Timestamp,
		ProcessedAt:    m.ProcessedAt,
		Error:          deref(m.Error),
		ReplyToID:      deref(m.ReplyToID),
		ReplyToContent: deref(m.ReplyToContent),
		ReplyToSender:  deref(m.ReplyToSender),
	}
}

// ToOfferEntity converts gorm Offer to entity.Offer
func ToOfferEntity(m *Offer) *entity.Offer {
	return &entity.Offer{
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
		Status:        entity.ItemStatus(m.Status),
		CreatedAt:     m.CreatedAt,
		UpdatedAt:     m.UpdatedAt,
	}
}

// ToRequestEntity converts gorm Request to entity.Request
func ToRequestEntity(m *Request) *entity.Request {
	return &entity.Request{
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
		Status:        entity.ItemStatus(m.Status),
		CreatedAt:     m.CreatedAt,
		UpdatedAt:     m.UpdatedAt,
	}
}

// ToMatchEntity converts gorm Match to entity.Match
func ToMatchEntity(m *Match) *entity.Match {
	return &entity.Match{
		ID:          m.ID,
		OfferID:     m.OfferID,
		RequestID:   m.RequestID,
		Score:       m.Score,
		Reasoning:   deref(m.Reasoning),
		MatchedBy:   deref(m.MatchedBy),
		Status:      entity.MatchStatus(m.Status),
		CreatedAt:   m.CreatedAt,
		ConfirmedAt: m.ConfirmedAt,
		Notes:       deref(m.Notes),
	}
}

// ToGroupEntity converts gorm Group to entity.Group
func ToGroupEntity(m *Group) *entity.Group {
	return &entity.Group{
		JID:          m.JID,
		Name:         m.Name,
		Description:  deref(m.Description),
		Monitored:    m.Monitored,
		AddedAt:      m.AddedAt,
		LastMessage:  m.LastMessage,
		MessageCount: m.MessageCount,
	}
}

// ========================================
// Batch Converters
// ========================================

// ToOffersEntity converts a slice of gorm Offers to entity Offers
func ToOffersEntity(models []Offer) []*entity.Offer {
	result := make([]*entity.Offer, len(models))
	for i := range models {
		result[i] = ToOfferEntity(&models[i])
	}
	return result
}

// ToRequestsEntity converts a slice of gorm Requests to entity Requests
func ToRequestsEntity(models []Request) []*entity.Request {
	result := make([]*entity.Request, len(models))
	for i := range models {
		result[i] = ToRequestEntity(&models[i])
	}
	return result
}

// ToMatchesEntity converts a slice of gorm Matches to entity Matches
func ToMatchesEntity(models []Match) []*entity.Match {
	result := make([]*entity.Match, len(models))
	for i := range models {
		result[i] = ToMatchEntity(&models[i])
	}
	return result
}

// ToMatchesWithDetailsEntity converts a slice of gorm Matches (with preloaded Offer/Request) to MatchWithDetails
func ToMatchesWithDetailsEntity(models []Match) []*entity.MatchWithDetails {
	result := make([]*entity.MatchWithDetails, len(models))
	for i := range models {
		mwd := &entity.MatchWithDetails{
			Match: *ToMatchEntity(&models[i]),
		}
		if models[i].Offer != nil {
			mwd.Offer = ToOfferEntity(models[i].Offer)
		}
		if models[i].Request != nil {
			mwd.Request = ToRequestEntity(models[i].Request)
		}
		result[i] = mwd
	}
	return result
}

// ToRawMessagesEntity converts a slice of gorm RawMessages to entity RawMessages
func ToRawMessagesEntity(models []RawMessage) []*entity.RawMessage {
	result := make([]*entity.RawMessage, len(models))
	for i := range models {
		result[i] = ToRawMessageEntity(&models[i])
	}
	return result
}

// ToGroupsEntity converts a slice of gorm Groups to entity Groups
func ToGroupsEntity(models []Group) []*entity.Group {
	result := make([]*entity.Group, len(models))
	for i := range models {
		result[i] = ToGroupEntity(&models[i])
	}
	return result
}

// ========================================
// MedicationMapping Converters
// ========================================

// ToMedicationMappingModel converts entity.MedicationMapping to gorm MedicationMapping
func ToMedicationMappingModel(d *entity.MedicationMapping) *MedicationMapping {
	return &MedicationMapping{
		ID:          d.ID,
		ArabicName:  d.ArabicName,
		EnglishName: d.EnglishName,
		Synonyms:    serializeSynonyms(d.Synonyms),
		CreatedAt:   d.CreatedAt,
		UpdatedAt:   d.UpdatedAt,
	}
}

// ToMedicationMappingEntity converts gorm MedicationMapping to entity.MedicationMapping
func ToMedicationMappingEntity(m *MedicationMapping) *entity.MedicationMapping {
	return &entity.MedicationMapping{
		ID:          m.ID,
		ArabicName:  m.ArabicName,
		EnglishName: m.EnglishName,
		Synonyms:    deserializeSynonyms(m.Synonyms),
		CreatedAt:   m.CreatedAt,
		UpdatedAt:   m.UpdatedAt,
	}
}

// ToMedicationMappingsEntity converts a slice of gorm MedicationMappings to entity MedicationMappings
func ToMedicationMappingsEntity(models []MedicationMapping) []*entity.MedicationMapping {
	result := make([]*entity.MedicationMapping, len(models))
	for i := range models {
		result[i] = ToMedicationMappingEntity(&models[i])
	}
	return result
}

// ========================================
// MatchQueueItem Converters
// ========================================

// ToMatchQueueModel converts entity.MatchQueueItem to gorm MatchQueue
func ToMatchQueueModel(d *entity.MatchQueueItem) *MatchQueue {
	return &MatchQueue{
		ID:         d.ID,
		SourceType: d.SourceType,
		SourceID:   d.SourceID,
		CreatedAt:  d.CreatedAt,
	}
}

// ToMatchQueueItemEntity converts gorm MatchQueue to entity.MatchQueueItem
func ToMatchQueueItemEntity(m *MatchQueue) *entity.MatchQueueItem {
	return &entity.MatchQueueItem{
		ID:         m.ID,
		SourceType: m.SourceType,
		SourceID:   m.SourceID,
		CreatedAt:  m.CreatedAt,
	}
}

// ToMatchQueueItemsEntity converts a slice of gorm MatchQueues to entity MatchQueueItems
func ToMatchQueueItemsEntity(models []MatchQueue) []*entity.MatchQueueItem {
	result := make([]*entity.MatchQueueItem, len(models))
	for i := range models {
		result[i] = ToMatchQueueItemEntity(&models[i])
	}
	return result
}

// ========================================
// ReviewQueueItem Converters
// ========================================

// ToReviewQueueModel converts entity.ReviewQueueItem to gorm ReviewQueue
func ToReviewQueueModel(d *entity.ReviewQueueItem) *ReviewQueue {
	var partialJSON, correctedJSON string
	if len(d.PartialItems) > 0 {
		data, _ := json.Marshal(d.PartialItems)
		partialJSON = string(data)
	}
	if len(d.CorrectedItems) > 0 {
		data, _ := json.Marshal(d.CorrectedItems)
		correctedJSON = string(data)
	}

	return &ReviewQueue{
		ID:             d.ID,
		RawMessageID:   d.RawMessageID,
		GroupName:      d.GroupName,
		SenderName:     d.SenderName,
		Content:        d.Content,
		ReplyContext:   nilIfEmpty(d.ReplyContext),
		PartialItems:   partialJSON,
		ParsePass:      d.ParsePass,
		AvgConfidence:  d.AvgConfidence,
		FailureReason:  nilIfEmpty(d.FailureReason),
		Status:         string(d.Status),
		ReviewedBy:     nilIfEmpty(d.ReviewedBy),
		ReviewedAt:     d.ReviewedAt,
		ReviewNote:     nilIfEmpty(d.ReviewNote),
		CorrectedItems: nilIfEmpty(correctedJSON),
		CreatedAt:      d.CreatedAt,
		UpdatedAt:      d.UpdatedAt,
	}
}

// ToReviewQueueItemEntity converts gorm ReviewQueue to entity.ReviewQueueItem
func ToReviewQueueItemEntity(m *ReviewQueue) *entity.ReviewQueueItem {
	item := &entity.ReviewQueueItem{
		ID:            m.ID,
		RawMessageID:  m.RawMessageID,
		GroupName:     m.GroupName,
		SenderName:    m.SenderName,
		Content:       m.Content,
		ReplyContext:  deref(m.ReplyContext),
		ParsePass:     m.ParsePass,
		AvgConfidence: m.AvgConfidence,
		FailureReason: deref(m.FailureReason),
		Status:        entity.ReviewStatus(m.Status),
		ReviewedBy:    deref(m.ReviewedBy),
		ReviewedAt:    m.ReviewedAt,
		ReviewNote:    deref(m.ReviewNote),
		CreatedAt:     m.CreatedAt,
		UpdatedAt:     m.UpdatedAt,
	}

	// Parse JSON fields
	if m.PartialItems != "" {
		_ = json.Unmarshal([]byte(m.PartialItems), &item.PartialItems)
	}
	if m.CorrectedItems != nil && *m.CorrectedItems != "" {
		_ = json.Unmarshal([]byte(*m.CorrectedItems), &item.CorrectedItems)
	}

	return item
}

// ToReviewQueueItemsEntity converts a slice of gorm ReviewQueues to entity ReviewQueueItems
func ToReviewQueueItemsEntity(models []ReviewQueue) []*entity.ReviewQueueItem {
	result := make([]*entity.ReviewQueueItem, len(models))
	for i := range models {
		result[i] = ToReviewQueueItemEntity(&models[i])
	}
	return result
}

// ========================================
// Helper Functions
// ========================================

func nilIfEmpty(s string) *string {
	if s == "" {
		return nil
	}
	return &s
}

func nilIfZero(f float64) *float64 {
	if f == 0 {
		return nil
	}
	return &f
}

func deref(s *string) string {
	if s == nil {
		return ""
	}
	return *s
}

func derefFloat(f *float64) float64 {
	if f == nil {
		return 0
	}
	return *f
}

// serializeSynonyms converts a slice of strings to JSON
func serializeSynonyms(synonyms []string) string {
	if len(synonyms) == 0 {
		return "[]"
	}
	data, _ := json.Marshal(synonyms)
	return string(data)
}

// deserializeSynonyms converts JSON to a slice of strings
func deserializeSynonyms(s string) []string {
	if s == "" || s == "[]" {
		return nil
	}
	var result []string
	json.Unmarshal([]byte(s), &result)
	return result
}

// float32SliceToBytes converts []float32 to []byte for storage
func float32SliceToBytes(floats []float32) []byte {
	if len(floats) == 0 {
		return nil
	}
	buf := make([]byte, len(floats)*4)
	for i, f := range floats {
		binary.LittleEndian.PutUint32(buf[i*4:], math.Float32bits(f))
	}
	return buf
}

// bytesToFloat32Slice converts []byte to []float32
func bytesToFloat32Slice(data []byte) []float32 {
	if len(data) == 0 {
		return nil
	}
	floats := make([]float32, len(data)/4)
	for i := range floats {
		bits := binary.LittleEndian.Uint32(data[i*4:])
		floats[i] = math.Float32frombits(bits)
	}
	return floats
}

// ========================================
// AuditLog Converters
// ========================================

// ToAuditLogModel converts entity.AuditLog to gorm AuditLog
func ToAuditLogModel(d *entity.AuditLog) *AuditLog {
	return &AuditLog{
		ID:        d.ID,
		Action:    string(d.Action),
		EntityID:  nilIfEmpty(d.EntityID),
		OldValue:  nilIfEmpty(d.OldValue),
		NewValue:  nilIfEmpty(d.NewValue),
		Details:   nilIfEmpty(d.Details),
		IPAddress: nilIfEmpty(d.IPAddress),
		CreatedAt: d.CreatedAt,
	}
}

// ToAuditLogEntity converts gorm AuditLog to entity.AuditLog
func ToAuditLogEntity(m *AuditLog) *entity.AuditLog {
	return &entity.AuditLog{
		ID:        m.ID,
		Action:    entity.AuditAction(m.Action),
		EntityID:  deref(m.EntityID),
		OldValue:  deref(m.OldValue),
		NewValue:  deref(m.NewValue),
		Details:   deref(m.Details),
		IPAddress: deref(m.IPAddress),
		CreatedAt: m.CreatedAt,
	}
}

// ToAuditLogsEntity converts a slice of gorm AuditLogs to entity AuditLogs
func ToAuditLogsEntity(models []AuditLog) []*entity.AuditLog {
	result := make([]*entity.AuditLog, len(models))
	for i := range models {
		result[i] = ToAuditLogEntity(&models[i])
	}
	return result
}

// ========================================
// WeightHistory Converters
// ========================================

// ToWeightHistoryModel converts entity.WeightHistory to gorm WeightHistory
func ToWeightHistoryModel(d *entity.WeightHistory) *WeightHistory {
	return &WeightHistory{
		ID:        d.ID,
		Weights:   weightsToJSON(d.MedicationWeight, d.DosageWeight, d.QuantityWeight, d.PriceWeight, d.RecencyWeight),
		Source:    string(d.Source),
		AppliedAt: d.AppliedAt,
	}
}

// ToWeightHistoryEntity converts gorm WeightHistory to entity.WeightHistory
func ToWeightHistoryEntity(m *WeightHistory) *entity.WeightHistory {
	medW, dosW, qtyW, prcW, recW := jsonToWeights(m.Weights)
	return &entity.WeightHistory{
		ID:               m.ID,
		MedicationWeight: medW,
		DosageWeight:     dosW,
		QuantityWeight:   qtyW,
		PriceWeight:      prcW,
		RecencyWeight:    recW,
		Source:           entity.WeightSource(m.Source),
		AppliedAt:        m.AppliedAt,
	}
}

// ToWeightHistoriesEntity converts a slice of gorm WeightHistory to entity WeightHistory
func ToWeightHistoriesEntity(models []WeightHistory) []*entity.WeightHistory {
	result := make([]*entity.WeightHistory, len(models))
	for i := range models {
		result[i] = ToWeightHistoryEntity(&models[i])
	}
	return result
}

// weightsToJSON serializes weights to JSON
func weightsToJSON(med, dos, qty, prc, rec float64) string {
	type weights struct {
		Medication float64 `json:"medication"`
		Dosage     float64 `json:"dosage"`
		Quantity   float64 `json:"quantity"`
		Price      float64 `json:"price"`
		Recency    float64 `json:"recency"`
	}
	data, _ := json.Marshal(weights{med, dos, qty, prc, rec})
	return string(data)
}

// jsonToWeights deserializes weights from JSON
func jsonToWeights(s string) (med, dos, qty, prc, rec float64) {
	if s == "" {
		return
	}
	type weights struct {
		Medication float64 `json:"medication"`
		Dosage     float64 `json:"dosage"`
		Quantity   float64 `json:"quantity"`
		Price      float64 `json:"price"`
		Recency    float64 `json:"recency"`
	}
	var w weights
	if err := json.Unmarshal([]byte(s), &w); err == nil {
		return w.Medication, w.Dosage, w.Quantity, w.Price, w.Recency
	}
	return
}

// ========================================
// MatchFeedback Converters
// ========================================

// ToMatchFeedbackModel converts entity.MatchFeedback to gorm MatchFeedback
func ToMatchFeedbackModel(d *entity.MatchFeedback) *MatchFeedback {
	return &MatchFeedback{
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

// ToMatchFeedbackEntity converts gorm MatchFeedback to entity.MatchFeedback
func ToMatchFeedbackEntity(m *MatchFeedback) *entity.MatchFeedback {
	return &entity.MatchFeedback{
		ID:                 m.ID,
		MatchID:            m.MatchID,
		OperatorID:         deref(m.OperatorID),
		Decision:           entity.FeedbackDecision(m.Decision),
		Reason:             deref(m.Reason),
		OriginalScore:      m.OriginalScore,
		OriginalConfidence: deref(m.OriginalConfidence),
		CreatedAt:          m.CreatedAt,
	}
}

// ToMatchFeedbacksEntity converts a slice of gorm MatchFeedback to entity MatchFeedback
func ToMatchFeedbacksEntity(models []MatchFeedback) []*entity.MatchFeedback {
	result := make([]*entity.MatchFeedback, len(models))
	for i := range models {
		result[i] = ToMatchFeedbackEntity(&models[i])
	}
	return result
}

// ========================================
// FeedbackRecord Converters
// ========================================

// ToFeedbackRecordModel converts entity.FeedbackRecord to gorm FeedbackRecord
func ToFeedbackRecordModel(d *entity.FeedbackRecord) *FeedbackRecord {
	return &FeedbackRecord{
		ID:              d.ID,
		MatchID:         d.MatchID,
		OfferID:         d.OfferID,
		RequestID:       d.RequestID,
		Action:          string(d.Action),
		MedicationScore: d.MedicationScore,
		DosageScore:     d.DosageScore,
		QuantityScore:   d.QuantityScore,
		PriceScore:      d.PriceScore,
		RecencyScore:    d.RecencyScore,
		TotalScore:      d.TotalScore,
		FeedbackAt:      d.FeedbackAt,
		UserID:          nilIfEmpty(d.UserID),
		CreatedAt:       d.CreatedAt,
	}
}

// ToFeedbackRecordEntity converts gorm FeedbackRecord to entity.FeedbackRecord
func ToFeedbackRecordEntity(m *FeedbackRecord) *entity.FeedbackRecord {
	return &entity.FeedbackRecord{
		ID:              m.ID,
		MatchID:         m.MatchID,
		OfferID:         m.OfferID,
		RequestID:       m.RequestID,
		Action:          entity.FeedbackAction(m.Action),
		MedicationScore: m.MedicationScore,
		DosageScore:     m.DosageScore,
		QuantityScore:   m.QuantityScore,
		PriceScore:      m.PriceScore,
		RecencyScore:    m.RecencyScore,
		TotalScore:      m.TotalScore,
		FeedbackAt:      m.FeedbackAt,
		UserID:          deref(m.UserID),
		CreatedAt:       m.CreatedAt,
	}
}

// ToFeedbackRecordsEntity converts slice of gorm FeedbackRecord to entity
func ToFeedbackRecordsEntity(models []FeedbackRecord) []*entity.FeedbackRecord {
	result := make([]*entity.FeedbackRecord, len(models))
	for i := range models {
		result[i] = ToFeedbackRecordEntity(&models[i])
	}
	return result
}
