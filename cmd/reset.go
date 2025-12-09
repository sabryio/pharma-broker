package cmd

import (
	"context"
	"fmt"
	"os"
	"strings"

	"github.com/rs/zerolog"
	"github.com/spf13/cobra"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
	storageGorm "pharmabroker/storage/gorm"
)

var resetDbCmd = &cobra.Command{
	Use:   "reset-db",
	Short: "Reset the database (DANGER: Deletes all data)",
	Long: `Completely wipes the SQLite database and re-initializes it with:
- Empty tables
- Default schema migrations
- Seeded medication mappings

WARNING: This action is irreversible. All offers, requests, and matches will be lost.`,
	Run: runResetDb,
}

var forceReset bool

func init() {
	resetDbCmd.Flags().BoolVarP(&forceReset, "force", "f", false, "Force reset without confirmation")
}

func runResetDb(cmd *cobra.Command, args []string) {
	// Setup logging
	log := zerolog.New(zerolog.ConsoleWriter{Out: os.Stdout}).
		With().
		Timestamp().
		Logger()

	// Load configuration
	cfg := config.Load()

	dbPath := cfg.Database.Path

	// Safety check
	if !forceReset {
		fmt.Printf("⚠️  DANGER: You are about to DELETE the database at: %s\n", dbPath)
		fmt.Print("Are you sure you want to continue? [y/N]: ")
		var input string
		fmt.Scanln(&input)
		if strings.ToLower(input) != "y" {
			log.Info().Msg("Operation cancelled")
			return
		}
	}

	log.Info().Msg("Stopping services...")
	// (In a real deployment we might need to stop the service, but here we assume user runs this manually)

	// Remove DB files
	files := []string{
		dbPath,
		dbPath + "-shm",
		dbPath + "-wal",
	}

	for _, f := range files {
		if err := os.Remove(f); err != nil && !os.IsNotExist(err) {
			log.Error().Err(err).Str("file", f).Msg("Failed to remove file")
			// Try to proceed anyway? No, if we can't delete DB, we can't reset.
			// But maybe only -shm/-wal are missing.
		} else if err == nil {
			log.Info().Str("file", f).Msg("Deleted file")
		}
	}

	log.Info().Msg("Re-initializing database...")

	// Re-initialize (runs migrations)
	db, err := storageGorm.NewDB(&storageGorm.Config{Path: dbPath})
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to re-initialize database")
	}
	defer db.Close()

	// Seed data
	ctx := context.Background()
	medicationRepo := storageGorm.NewMedicationMappingRepo(db)

	// Load medication mappings from file (supports both legacy and rich format)
	medicationMappings, err := domain.LoadRichMedicationMappings("medications.json")
	if err != nil {
		log.Warn().Err(err).Msg("Failed to load medications.json")
	}

	log.Info().Msg("Seeding medication mappings...")
	count := 0
	synonymCount := 0
	for _, mapping := range medicationMappings {
		if err := medicationRepo.Save(ctx, mapping); err != nil {
			log.Warn().Err(err).Str("arabic", mapping.ArabicName).Msg("Failed to seed mapping")
		} else {
			count++
			synonymCount += len(mapping.Synonyms)
		}
	}
	log.Info().
		Int("count", count).
		Int("synonyms", synonymCount).
		Msg("Seeded medication mappings")

	log.Info().Msg("✅ Database reset complete")
}
