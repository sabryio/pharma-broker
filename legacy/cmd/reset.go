package cmd

import (
	"context"
	"fmt"
	"os"
	"strings"

	"github.com/rs/zerolog"
	"github.com/spf13/cobra"

	"pharmabroker/domain/entity"
	"pharmabroker/pkg/config"
	storageGorm "pharmabroker/storage/gorm"
)

var resetDbCmd = &cobra.Command{
	Use:   "reset-db",
	Short: "Reset the database (DANGER: Deletes all data)",
	Long: `Completely wipes the PostgreSQL database and re-initializes it with:
- Empty tables (using TRUNCATE CASCADE)
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

	dsn := cfg.Database.DSN

	// Safety check
	if !forceReset {
		fmt.Printf("⚠️  DANGER: You are about to TRUNCATE ALL TABLES in database: %s\n", maskDSNForReset(dsn))
		fmt.Print("Are you sure you want to continue? [y/N]: ")
		var input string
		fmt.Scanln(&input)
		if strings.ToLower(input) != "y" {
			log.Info().Msg("Operation cancelled")
			return
		}
	}

	log.Info().Msg("Connecting to database...")

	// Connect to database
	db, err := storageGorm.NewDB(&storageGorm.Config{DSN: dsn})
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to connect to database")
	}
	defer db.Close()

	log.Info().Msg("Truncating all tables...")

	// Truncate all tables in order (respecting FK constraints)
	tables := []string{
		"feedback_records",
		"weight_history",
		"review_queue",
		"unmapped_medications",
		"audit_logs",
		"demand_leaderboard",
		"match_feedback",
		"failed_messages",
		"match_queue",
		"matches",
		"offers",
		"requests",
		"raw_messages",
		"medication_mappings",
		"groups",
		"config",
		"bot_users",
	}

	for _, table := range tables {
		result := db.GORM().Exec("TRUNCATE TABLE " + table + " CASCADE")
		if result.Error != nil {
			log.Warn().Err(result.Error).Str("table", table).Msg("Failed to truncate table (may not exist yet)")
		} else {
			log.Debug().Str("table", table).Msg("Truncated table")
		}
	}

	log.Info().Msg("Tables truncated successfully")

	// Seed data
	ctx := context.Background()
	medicationRepo := storageGorm.NewMedicationMappingRepo(db)

	// Load medication mappings from file (supports both legacy and rich format)
	medicationMappings, err := entity.LoadRichMedicationMappings("medications.json")
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

// maskDSNForReset masks the password in DSN for display
func maskDSNForReset(dsn string) string {
	// Simple masking - find password between : and @
	start := strings.Index(dsn, "://")
	if start == -1 {
		return dsn
	}
	userStart := start + 3
	atPos := strings.Index(dsn[userStart:], "@")
	if atPos == -1 {
		return dsn
	}
	colonPos := strings.Index(dsn[userStart:userStart+atPos], ":")
	if colonPos == -1 {
		return dsn
	}
	return dsn[:userStart+colonPos+1] + "***" + dsn[userStart+atPos:]
}
