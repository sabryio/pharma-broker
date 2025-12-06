package storage_test

import (
	"context"
	"database/sql"
	"path/filepath"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	_ "modernc.org/sqlite" // Pure Go SQLite driver

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage"
)

func TestArchiveOldMessages(t *testing.T) {
	tempDir := t.TempDir()
	mainPath := filepath.Join(tempDir, "test_main.db")

	// Initialize Repo with our testing DB wrapper
	cfg := config.DatabaseConfig{
		Path:      mainPath,
		EnableWAL: false,
	}

	// Use New to get proper struct
	realDB, err := storage.New(&cfg)
	require.NoError(t, err)
	defer realDB.Close()

	repo := storage.NewRawMessageRepo(realDB)

	ctx := context.Background()

	// Seed Data
	now := time.Now()
	oldTime := now.AddDate(0, 0, -60) // 60 days old
	newTime := now.AddDate(0, 0, -1)  // 1 day old

	// Old Message (Should be archived)
	err = repo.Save(ctx, &domain.RawMessage{
		ID:         "old-1",
		ExternalID: "ext-old-1",
		Content:    "Old Message",
		Timestamp:  oldTime,
	})
	require.NoError(t, err)

	// New Message (Should stay)
	err = repo.Save(ctx, &domain.RawMessage{
		ID:         "new-1",
		ExternalID: "ext-new-1",
		Content:    "New Message",
		Timestamp:  newTime,
	})
	require.NoError(t, err)

	// Define Archive Path
	archivePath := filepath.Join(filepath.Dir(mainPath), "test_archive.db")

	// Check before archive
	msg, err := repo.GetByID(ctx, "old-1")
	require.NoError(t, err)
	assert.NotNil(t, msg)

	// Perform Archive (Retain 30 days)
	cutoff := now.AddDate(0, 0, -30)
	count, err := repo.ArchiveOldMessages(ctx, archivePath, cutoff)
	require.NoError(t, err)
	assert.Equal(t, int64(1), count)

	// Check Main DB (Old gone, New stays)
	_, err = repo.GetByID(ctx, "old-1")
	assert.Error(t, err, "Old message should be gone from main DB")

	msgNew, err := repo.GetByID(ctx, "new-1")
	assert.NoError(t, err)
	assert.NotNil(t, msgNew)

	// Check Archive DB
	// We need to attach/open archive DB. Since it is a separate file, we can open it.
	// We must ensure ArchiveOldMessages detached it.

	archiveDB, err := sql.Open("sqlite", archivePath)
	require.NoError(t, err)
	defer archiveDB.Close()

	var content string
	err = archiveDB.QueryRow("SELECT content FROM raw_messages WHERE id = ?", "old-1").Scan(&content)
	assert.NoError(t, err, "Old message should exist in archive DB")
	assert.Equal(t, "Old Message", content)
}
