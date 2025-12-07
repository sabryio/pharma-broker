// Comprehensive Local Workflow Playground
//
// Tests the FULL PharmaBroker pipeline using ACTUAL package implementations:
// 1. Initialize database and all repositories
// 2. Create AI provider and Parser (actual implementation)
// 3. Feed hardcoded messages through Parser.ProcessMessage
// 4. Let Parser process batch (AI parsing + matching)
// 5. Generate reports using actual report generator
// 6. Send notifications via actual notification service
//
// Usage:
//   go run cmd/playground/workflow/main.go
//   go run cmd/playground/workflow/main.go -skip-notify
//   go run cmd/playground/workflow/main.go -provider gemini

package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/google/uuid"
	"github.com/joho/godotenv"
	"github.com/rs/zerolog"

	"pharmabroker/internal/ai"
	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
	"pharmabroker/internal/notify"
	"pharmabroker/internal/reports"
	"pharmabroker/internal/storage"
)

// Phase tracking
type Phase struct {
	Name    string
	Status  string
	Start   time.Time
	End     time.Time
	Details string
}

type Workflow struct {
	phases  []Phase
	current int
	log     zerolog.Logger
}

func NewWorkflow(log zerolog.Logger) *Workflow {
	return &Workflow{phases: make([]Phase, 0), log: log}
}

func (w *Workflow) StartPhase(name string) {
	w.phases = append(w.phases, Phase{Name: name, Start: time.Now()})
	w.current = len(w.phases) - 1
	w.log.Info().Str("phase", name).Msg("🚀 Starting phase")
}

func (w *Workflow) EndPhase(status, details string) {
	if w.current >= 0 && w.current < len(w.phases) {
		w.phases[w.current].End = time.Now()
		w.phases[w.current].Status = status
		w.phases[w.current].Details = details
		elapsed := w.phases[w.current].End.Sub(w.phases[w.current].Start)
		w.log.Info().Str("phase", w.phases[w.current].Name).Str("status", status).Dur("elapsed", elapsed).Msg("✅ Phase completed")
	}
}

func (w *Workflow) PrintSummary() {
	fmt.Println("\n" + strings.Repeat("=", 70))
	fmt.Println("📊 WORKFLOW SUMMARY")
	fmt.Println(strings.Repeat("=", 70))
	var total time.Duration
	for i, p := range w.phases {
		d := p.End.Sub(p.Start)
		total += d
		icon := "✅"
		if p.Status != "success" {
			icon = "❌"
		}
		fmt.Printf("%d. %s %-22s %-10s %v\n", i+1, icon, p.Name, p.Status, d.Round(time.Millisecond))
		if p.Details != "" {
			fmt.Printf("       %s\n", p.Details)
		}
	}
	fmt.Println(strings.Repeat("-", 70))
	fmt.Printf("   Total: %v\n", total.Round(time.Millisecond))
}

// NoOpBroadcaster implements SSEBroadcaster for testing
type NoOpBroadcaster struct{}

func (b *NoOpBroadcaster) BroadcastNewOffer(offerID, medication string)     {}
func (b *NoOpBroadcaster) BroadcastNewRequest(requestID, medication string) {}
func (b *NoOpBroadcaster) BroadcastNewMatch(matchID string, score float64)  {}

// NoOpErrorNotifier implements ErrorNotifier for testing
type NoOpErrorNotifier struct {
	log zerolog.Logger
}

func (n *NoOpErrorNotifier) NotifyError(err error) {
	n.log.Error().Err(err).Msg("Error notified")
}

// Hardcoded test messages (Arabic + English)
func getTestMessages() []*domain.RawMessage {
	return []*domain.RawMessage{
		{
			ID:         uuid.New().String(),
			ExternalID: uuid.New().String(),
			GroupJID:   "test-group-1@g.us",
			GroupName:  "مجموعة الأدوية",
			SenderName: "Ahmed",
			Content:    "للبيع: Augmentin 1g - 50 علبة بـ 150 جنيه للعلبة",
			Timestamp:  time.Now(),
		},
		{
			ID:         uuid.New().String(),
			ExternalID: uuid.New().String(),
			GroupJID:   "test-group-1@g.us",
			GroupName:  "مجموعة الأدوية",
			SenderName: "Dr. Mohamed",
			Content:    "مطلوب بشكل عاجل: أوجمنتين 1 جرام - 20 علبة - أفضل سعر",
			Timestamp:  time.Now(),
		},
		{
			ID:         uuid.New().String(),
			ExternalID: uuid.New().String(),
			GroupJID:   "test-group-2@g.us",
			GroupName:  "صيادلة القاهرة",
			SenderName: "Pharmacy Cairo",
			Content:    "متوفر للبيع: Panadol Extra 500 علبة @ 35 LE",
			Timestamp:  time.Now(),
		},
		{
			ID:         uuid.New().String(),
			ExternalID: uuid.New().String(),
			GroupJID:   "test-group-2@g.us",
			GroupName:  "صيادلة القاهرة",
			SenderName: "Pharmacy Alex",
			Content:    "محتاج بانادول اكسترا 100 علبة ضروري",
			Timestamp:  time.Now(),
		},
		{
			ID:         uuid.New().String(),
			ExternalID: uuid.New().String(),
			GroupJID:   "test-group-1@g.us",
			GroupName:  "مجموعة الأدوية",
			SenderName: "Supplier",
			Content:    "عرض اليوم: Cataflam 50mg - 200 شريط بسعر 75 جنيه",
			Timestamp:  time.Now(),
		},
		{
			ID:         uuid.New().String(),
			ExternalID: uuid.New().String(),
			GroupJID:   "test-group-3@g.us",
			GroupName:  "موردين الأدوية",
			SenderName: "Ali",
			Content:    "أحتاج كتافلام 50 - 100 شريط - عاجل جداً",
			Timestamp:  time.Now(),
		},
	}
}

func main() {
	skipNotify := flag.Bool("skip-notify", false, "Skip notification phase")
	provider := flag.String("provider", "docker", "AI provider: docker or gemini")
	flag.Parse()

	_ = godotenv.Load()

	log := zerolog.New(zerolog.ConsoleWriter{Out: os.Stdout, TimeFormat: "15:04:05"}).With().Timestamp().Logger()
	workflow := NewWorkflow(log)

	fmt.Println("\n" + strings.Repeat("=", 70))
	fmt.Println("🔬 PHARMABROKER COMPREHENSIVE WORKFLOW TEST")
	fmt.Println("   Using ACTUAL package implementations")
	fmt.Println(strings.Repeat("=", 70))
	fmt.Printf("Time: %s | Provider: %s | Skip Notify: %v\n", time.Now().Format("15:04:05"), *provider, *skipNotify)
	fmt.Println(strings.Repeat("=", 70))

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Minute)
	defer cancel()

	cfg := config.Load()
	cfg.AI.Provider = *provider
	cfg.Database.Path = "./data/workflow_test.db"
	cfg.Database.EnableWAL = true

	// ============================================================
	// PHASE 1: Initialize Database & Repositories
	// ============================================================
	workflow.StartPhase("Database Init")

	db, err := storage.New(&cfg.Database)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to init database")
	}
	defer db.Close()

	// Create ALL repositories (matching actual serve.go)
	rawMsgRepo := storage.NewRawMessageRepo(db)
	offerRepo := storage.NewOfferRepo(db)
	requestRepo := storage.NewRequestRepo(db)
	matchRepo := storage.NewMatchRepo(db)
	matchQueueRepo := storage.NewMatchQueueRepo(db)
	medicationRepo := storage.NewMedicationMappingRepo(db)
	configRepo := storage.NewConfigRepo(db)
	reportRepo := storage.NewReportRepo(db)

	workflow.EndPhase("success", fmt.Sprintf("8 repos initialized, DB: %s", cfg.Database.Path))

	// ============================================================
	// PHASE 2: Create AI Provider
	// ============================================================
	workflow.StartPhase("AI Provider Init")

	aiProvider, err := ai.NewAIProvider(ctx, cfg, log)
	if err != nil {
		log.Error().Err(err).Msg("Failed to create AI provider")
		workflow.EndPhase("failed", err.Error())
		workflow.PrintSummary()
		os.Exit(1)
	}
	workflow.EndPhase("success", fmt.Sprintf("Provider: %s, Model: %s", cfg.AI.Provider, cfg.DockerModel.Model))

	// ============================================================
	// PHASE 3: Create Parser (Actual Implementation)
	// ============================================================
	workflow.StartPhase("Parser Init")

	broadcaster := &NoOpBroadcaster{}
	errorNotifier := &NoOpErrorNotifier{log: log}

	parser := ai.NewParser(
		rawMsgRepo,
		aiProvider,
		offerRepo,
		requestRepo,
		matchRepo, // Added dependency
		medicationRepo,
		matchQueueRepo,
		configRepo,
		errorNotifier,
		broadcaster,
		log,
	)

	// Parser is fully configured via constructor - no additional setup needed
	// matchRepo is passed via constructor, parserCfg is set via SetParserConfig if available
	_ = matchRepo // matchRepo used in parser internally via matchQueueRepo flow

	workflow.EndPhase("success", "Parser created with all dependencies")

	// ============================================================
	// PHASE 4: Feed Messages Through Parser
	// ============================================================
	workflow.StartPhase("Message Processing")

	testMessages := getTestMessages()
	log.Info().Int("count", len(testMessages)).Msg("Test messages created")

	// Start parser (spawns worker goroutines)
	parser.Start(ctx)
	defer parser.Stop()

	// Feed messages through the ACTUAL ProcessMessage method
	for _, msg := range testMessages {
		// IMPORTANT: Save message to DB first to satisfy Foreign Key constraints
		if err := rawMsgRepo.Save(ctx, msg); err != nil {
			log.Error().Err(err).Str("id", msg.ID).Msg("Failed to save raw message")
			continue
		}

		parser.ProcessMessage(ctx, msg)
		log.Debug().Str("id", msg.ID).Str("sender", msg.SenderName).Msg("Message queued")
	}

	// Wait for processing (batch interval + matching)
	log.Info().Msg("Waiting for Parser to process batch...")
	time.Sleep(10 * time.Second) // Allow batch processing + matching

	// Check results
	offers, _ := offerRepo.GetActive(ctx, 100, 0)
	requests, _ := requestRepo.GetActive(ctx, 100, 0)
	pendingMatches, _ := matchRepo.GetPending(ctx, 100, 0)

	workflow.EndPhase("success", fmt.Sprintf("Offers: %d, Requests: %d, Matches: %d", len(offers), len(requests), len(pendingMatches)))

	// ============================================================
	// PHASE 5: Generate Report (Actual Implementation)
	// ============================================================
	workflow.StartPhase("Report Generation")

	generator := reports.NewGenerator(reportRepo, log)
	report, err := generator.GenerateHourlyReport(ctx, reports.ReportConfig{
		PeriodHours: 24,
		MinScore:    0.0,
	})
	if err != nil {
		log.Error().Err(err).Msg("Report generation failed")
		workflow.EndPhase("failed", err.Error())
	} else {
		summaryText := generator.GenerateSummaryText(report)
		fmt.Println("\n" + strings.Repeat("-", 50))
		fmt.Println("📊 REPORT SUMMARY")
		fmt.Println(strings.Repeat("-", 50))
		fmt.Println(summaryText)

		// Save CSV
		csvData, _ := generator.ExportToCSV(report)
		csvPath := "./data/workflow_report.csv"
		os.WriteFile(csvPath, csvData, 0644)

		workflow.EndPhase("success", fmt.Sprintf("Report saved to %s", csvPath))
	}

	// ============================================================
	// PHASE 6: Send Notifications (Actual Implementation)
	// ============================================================
	if !*skipNotify && report != nil {
		workflow.StartPhase("Notifications")

		// Create notification service with config from Reports settings
		notifyService := notify.NewNotificationService(
			notify.TelegramConfig{
				Enabled:  cfg.Reports.Telegram.Enabled,
				BotToken: cfg.Reports.Telegram.BotToken,
				ChatIDs:  cfg.Reports.Telegram.ChatIDs,
			},
			notify.EmailConfig{
				Enabled:    cfg.Reports.Email.Enabled,
				SMTPHost:   cfg.Reports.Email.SMTPHost,
				SMTPPort:   cfg.Reports.Email.SMTPPort,
				Username:   cfg.Reports.Email.Username,
				Password:   cfg.Reports.Email.Password,
				FromName:   cfg.Reports.Email.FromName,
				FromEmail:  cfg.Reports.Email.FromEmail,
				Recipients: cfg.Reports.Email.Recipients,
			},
			log,
		)

		summaryText := generator.GenerateSummaryText(report)
		htmlReport := generator.GenerateHTMLReport(report)
		csvData, _ := generator.ExportToCSV(report)

		err := notifyService.SendReport(ctx, summaryText, htmlReport, csvData, "workflow_report.csv")
		if err != nil {
			log.Error().Err(err).Msg("Notification failed")
			workflow.EndPhase("failed", err.Error())
		} else {
			workflow.EndPhase("success", "Telegram + Email sent")
		}
	} else {
		log.Info().Msg("Skipping notifications")
	}

	// ============================================================
	// SUMMARY & VERIFICATION
	// ============================================================
	workflow.PrintSummary()

	fmt.Println("\n📁 Generated Files:")
	fmt.Println("   - ./data/workflow_test.db (SQLite)")
	fmt.Println("   - ./data/workflow_report.csv")

	fmt.Println("\n📊 Database Contents:")
	fmt.Printf("   - Offers: %d\n", len(offers))
	fmt.Printf("   - Requests: %d\n", len(requests))
	fmt.Printf("   - Pending Matches: %d\n", len(pendingMatches))

	if len(offers) > 0 || len(requests) > 0 {
		fmt.Println("\n✅ Workflow completed successfully!")
	} else {
		fmt.Println("\n⚠️ No offers/requests created - check AI provider connection")
	}
}
