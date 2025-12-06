package storage

import (
	"context"
	"database/sql"
	"time"

	"pharmabroker/internal/domain"

	"github.com/google/uuid"
)

// MedicationMappingRepo implements domain.MedicationMappingRepository
type MedicationMappingRepo struct {
	db *DB
}

// NewMedicationMappingRepo creates a new repository
func NewMedicationMappingRepo(db *DB) *MedicationMappingRepo {
	return &MedicationMappingRepo{db: db}
}

// Save saves a medication mapping
func (r *MedicationMappingRepo) Save(ctx context.Context, m *domain.MedicationMapping) error {
	query := `
		INSERT INTO medication_mappings (id, arabic_name, english_name, created_at, updated_at)
		VALUES (?, ?, ?, ?, ?)
		ON CONFLICT(arabic_name) DO UPDATE SET
			english_name = excluded.english_name,
			updated_at = excluded.updated_at
	`

	if m.ID == "" {
		m.ID = uuid.New().String()
	}
	if m.CreatedAt.IsZero() {
		m.CreatedAt = time.Now()
	}
	m.UpdatedAt = time.Now()

	_, err := r.db.conn.ExecContext(ctx, query,
		m.ID,
		m.ArabicName,
		m.EnglishName,
		m.CreatedAt,
		m.UpdatedAt,
	)
	return err
}

// GetByArabicName returns a mapping by Arabic name
func (r *MedicationMappingRepo) GetByArabicName(ctx context.Context, arabicName string) (*domain.MedicationMapping, error) {
	query := `SELECT id, arabic_name, english_name, created_at, updated_at FROM medication_mappings WHERE arabic_name = ?`

	row := r.db.conn.QueryRowContext(ctx, query, arabicName)
	var m domain.MedicationMapping
	if err := row.Scan(&m.ID, &m.ArabicName, &m.EnglishName, &m.CreatedAt, &m.UpdatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	return &m, nil
}

// GetAll returns all mappings
func (r *MedicationMappingRepo) GetAll(ctx context.Context) ([]*domain.MedicationMapping, error) {
	query := `SELECT id, arabic_name, english_name, created_at, updated_at FROM medication_mappings`

	rows, err := r.db.conn.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var mappings []*domain.MedicationMapping
	for rows.Next() {
		var m domain.MedicationMapping
		if err := rows.Scan(&m.ID, &m.ArabicName, &m.EnglishName, &m.CreatedAt, &m.UpdatedAt); err != nil {
			return nil, err
		}
		mappings = append(mappings, &m)
	}
	return mappings, nil
}

// Count returns the number of mappings
func (r *MedicationMappingRepo) Count(ctx context.Context) (int, error) {
	var count int
	err := r.db.conn.QueryRowContext(ctx, "SELECT COUNT(*) FROM medication_mappings").Scan(&count)
	return count, err
}
