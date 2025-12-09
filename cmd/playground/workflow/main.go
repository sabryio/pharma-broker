// Comprehensive Local Workflow Playground
//
// Tests the FULL PharmaBroker pipeline using ACTUAL package implementations,
// simulating REAL WhatsApp message behavior exactly like serve.go:
//
// 1. Initialize database and all repositories
// 2. Create Listener (actual implementation) with groups pre-monitored
// 3. Create AI provider and Parser (actual implementation)
// 4. Wire Listener → msgChannel → Parser (like serve.go)
// 5. Simulate WhatsApp messages via Listener.HandleMessage()
// 6. Let Parser process batch (AI parsing + matching)
// 7. Generate reports using actual report generator
//
// Usage:
//   task workflow:test
//   task workflow:test:skip-notify
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

	"pharmabroker/ai"
	"pharmabroker/domain/entity"
	"pharmabroker/internal/config"
	"pharmabroker/internal/notify"
	"pharmabroker/internal/reports"
	"pharmabroker/internal/whatsapp"
	"pharmabroker/parsing"
	storageGorm "pharmabroker/storage/gorm"
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

// ============================================================
// REALISTIC MOCK WHATSAPP MESSAGES
// These simulate actual messages from Egyptian pharmacy groups
// ============================================================
func getMockWhatsAppMessages() []*whatsapp.IncomingMessage {
	baseTime := time.Now()

	return []*whatsapp.IncomingMessage{
		// ========== Group 1: مجموعة صيادلة القاهرة ==========
		{
			ID:          uuid.New().String(),
			GroupJID:    "120363012345678901@g.us",
			GroupName:   "مجموعة صيادلة القاهرة",
			SenderJID:   "201012345678@s.whatsapp.net",
			SenderPhone: "201012345678",
			SenderName:  "د. أحمد صيدلي",
			Content:     "السلام عليكم\nعندي للبيع:\n*اوجمنتين 1 جم* - 50 علبة بـ 150 جنيه\n*فلاجيل 500* - 30 علبة بـ 45 جنيه\nالتواصل واتس فقط",
			Timestamp:   baseTime.Add(-5 * time.Second),
			IsFromMe:    false,
		},
		{
			ID:          uuid.New().String(),
			GroupJID:    "120363012345678901@g.us",
			GroupName:   "مجموعة صيادلة القاهرة",
			SenderJID:   "201098765432@s.whatsapp.net",
			SenderPhone: "201098765432",
			SenderName:  "صيدلية النور",
			Content:     "محتاج ضروري جداً:\n*أوجمنتين 1 جرام* 20 علبة\nأي سعر مناسب",
			Timestamp:   baseTime.Add(-3 * time.Second),
			IsFromMe:    false,
		},
		// Reply to previous message
		{
			ID:             uuid.New().String(),
			GroupJID:       "120363012345678901@g.us",
			GroupName:      "مجموعة صيادلة القاهرة",
			SenderJID:      "201012345678@s.whatsapp.net",
			SenderPhone:    "201012345678",
			SenderName:     "د. أحمد صيدلي",
			Content:        "متوفر عندي، نفس السعر",
			Timestamp:      baseTime.Add(-1 * time.Second),
			IsFromMe:       false,
			ReplyToID:      "previous-msg-id",
			ReplyToContent: "محتاج ضروري جداً: أوجمنتين 1 جرام 20 علبة",
			ReplyToSender:  "201098765432@s.whatsapp.net",
		},

		// ========== Group 2: موردين الأدوية ==========
		{
			ID:          uuid.New().String(),
			GroupJID:    "120363098765432109@g.us",
			GroupName:   "موردين الأدوية",
			SenderJID:   "201155555555@s.whatsapp.net",
			SenderPhone: "201155555555",
			SenderName:  "مورد أدوية",
			Content:     "عرض اليوم 🔥\n*كتافلام 50 مجم* - 200 شريط @ 75 جنيه\n*بروفين 400* - 100 علبة @ 55 جنيه\n*بانادول اكسترا* - 50 علبة @ 40 جنيه\nالكميات محدودة!",
			Timestamp:   baseTime.Add(-10 * time.Second),
			IsFromMe:    false,
		},
		{
			ID:          uuid.New().String(),
			GroupJID:    "120363098765432109@g.us",
			GroupName:   "موردين الأدوية",
			SenderJID:   "201166666666@s.whatsapp.net",
			SenderPhone: "201166666666",
			SenderName:  "صيدلية الأمل",
			Content:     "مطلوب عاجل:\n- كتافلام 50 - 100 شريط\n- بانادول اكسترا - 30 علبة",
			Timestamp:   baseTime.Add(-8 * time.Second),
			IsFromMe:    false,
		},

		// ========== Group 3: صيادلة الإسكندرية ==========
		{
			ID:          uuid.New().String(),
			GroupJID:    "120363055555555555@g.us",
			GroupName:   "صيادلة الإسكندرية",
			SenderJID:   "201177777777@s.whatsapp.net",
			SenderPhone: "201177777777",
			SenderName:  "Pharmacy Alex",
			Content:     "Available for sale:\nConcor 5mg - 40 boxes @ 85 LE\nZoloft 50mg - 25 boxes @ 120 LE",
			Timestamp:   baseTime.Add(-15 * time.Second),
			IsFromMe:    false,
		},
		{
			ID:          uuid.New().String(),
			GroupJID:    "120363055555555555@g.us",
			GroupName:   "صيادلة الإسكندرية",
			SenderJID:   "201188888888@s.whatsapp.net",
			SenderPhone: "201188888888",
			SenderName:  "Dr. Sara",
			Content:     "محتاجين كونكور 5 - 20 علبة\nالحد الأقصى للسعر 90 جنيه",
			Timestamp:   baseTime.Add(-12 * time.Second),
			IsFromMe:    false,
		},

		// ========== Multi-concentration message ==========
		{
			ID:          uuid.New().String(),
			GroupJID:    "120363012345678901@g.us",
			GroupName:   "مجموعة صيادلة القاهرة",
			SenderJID:   "201199999999@s.whatsapp.net",
			SenderPhone: "201199999999",
			SenderName:  "صيدلية الشفاء",
			Content:     "مطلوب:\n*اوزمبك واحد ونص وربع*\n*زولادكس 3.6*",
			Timestamp:   baseTime.Add(-2 * time.Second),
			IsFromMe:    false,
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
	fmt.Println("🔬 PHARMABROKER REALISTIC WORKFLOW TEST")
	fmt.Println("   Simulating REAL WhatsApp message flow (exactly like serve.go)")
	fmt.Println(strings.Repeat("=", 70))
	fmt.Printf("Time: %s | Provider: %s | Skip Notify: %v\n", time.Now().Format("15:04:05"), *provider, *skipNotify)
	fmt.Println(strings.Repeat("=", 70))

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Minute)
	defer cancel()

	cfg := config.Load()
	cfg.AI.Provider = *provider
	cfg.Database.Path = "./data/workflow_test.db"
	cfg.Database.EnableWAL = true
	cfg.DockerModel.BaseURL = "http://localhost:12434/engines/llama.cpp/v1"

	// ============================================================
	// PHASE 1: Initialize Database & Repositories
	// ============================================================
	workflow.StartPhase("Database Init")

	// Delete old test database for clean run
	os.Remove(cfg.Database.Path)
	os.Remove(cfg.Database.Path + "-shm")
	os.Remove(cfg.Database.Path + "-wal")

	// Initialize storage/gorm layer
	newDB, err := storageGorm.NewDB(&storageGorm.Config{Path: cfg.Database.Path})
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to init storage/gorm layer")
	}
	defer newDB.Close()

	// Create ALL repositories (using new storageGorm implementations)
	rawMsgRepo := storageGorm.NewRawMessageRepo(newDB)
	offerRepo := storageGorm.NewOfferRepo(newDB)
	requestRepo := storageGorm.NewRequestRepo(newDB)
	matchRepo := storageGorm.NewMatchRepo(newDB)
	matchQueueRepo := storageGorm.NewMatchQueueRepo(newDB)
	groupRepo := storageGorm.NewGroupRepo(newDB)
	medicationRepo := storageGorm.NewMedicationMappingRepo(newDB)
	configRepo := storageGorm.NewConfigRepo(newDB)
	reportRepo := storageGorm.NewReportRepo(newDB)

	workflow.EndPhase("success", fmt.Sprintf("9 repos initialized, DB: %s", cfg.Database.Path))

	// ============================================================
	// PHASE 2: Setup Monitored Groups (like real WhatsApp sync)
	// ============================================================
	workflow.StartPhase("Group Setup")

	testGroups := []*entity.Group{
		{JID: "120363012345678901@g.us", Name: "مجموعة صيادلة القاهرة", Monitored: true, AddedAt: time.Now()},
		{JID: "120363098765432109@g.us", Name: "موردين الأدوية", Monitored: true, AddedAt: time.Now()},
		{JID: "120363055555555555@g.us", Name: "صيادلة الإسكندرية", Monitored: true, AddedAt: time.Now()},
	}

	for _, g := range testGroups {
		if err := groupRepo.Save(ctx, g); err != nil {
			log.Error().Err(err).Str("jid", g.JID).Msg("Failed to save group")
		}
	}

	workflow.EndPhase("success", fmt.Sprintf("%d groups set as monitored", len(testGroups)))

	// ============================================================
	// PHASE 3: Create AI Provider
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
	// PHASE 4: Create Listener & Parser (Exactly like serve.go)
	// ============================================================
	workflow.StartPhase("Listener & Parser Init")

	// Create the actual Listener (same as serve.go)
	listener := whatsapp.NewListener(log, rawMsgRepo, groupRepo)

	// Create Parser with all dependencies
	broadcaster := &NoOpBroadcaster{}
	errorNotifier := &NoOpErrorNotifier{log: log}

	parser := parsing.NewParser(
		rawMsgRepo,
		aiProvider,
		offerRepo,
		requestRepo,
		matchRepo,
		medicationRepo,
		matchQueueRepo,
		configRepo,
		errorNotifier,
		broadcaster,
		log,
	)

	// Wire Listener → msgChannel → Parser (EXACTLY like serve.go lines 256-262)
	go func() {
		msgChan := listener.MessageChannel()
		for msg := range msgChan {
			parser.ProcessMessage(ctx, msg)
		}
	}()

	// Start Parser (spawns worker goroutines)
	parser.Start(ctx)
	defer parser.Stop()

	workflow.EndPhase("success", "Listener created, Parser started, message loop wired")

	// ============================================================
	// PHASE 5: Simulate WhatsApp Messages (Like real incoming messages)
	// ============================================================
	workflow.StartPhase("Message Simulation")

	mockMessages := getMockWhatsAppMessages()
	log.Info().Int("count", len(mockMessages)).Msg("Simulating WhatsApp messages")

	fmt.Println("\n📱 Simulating incoming WhatsApp messages...")
	fmt.Println(strings.Repeat("-", 50))

	for i, msg := range mockMessages {
		fmt.Printf("%d. [%s] %s: %s\n", i+1, msg.GroupName, msg.SenderName, truncate(msg.Content, 60))

		// This is EXACTLY how real messages come in through WhatsApp
		// Listener.HandleMessage does:
		// 1. Check if group is monitored
		// 2. Deduplication check
		// 3. Save to RawMessages DB
		// 4. Update group stats
		// 5. Queue via msgChannel → Parser
		listener.HandleMessage(msg)

		// Small delay to simulate real message timing
		time.Sleep(100 * time.Millisecond)
	}

	fmt.Println(strings.Repeat("-", 50))

	workflow.EndPhase("success", fmt.Sprintf("%d messages simulated via Listener.HandleMessage", len(mockMessages)))

	// ============================================================
	// PHASE 6: Wait for Processing (AI + Matching)
	// ============================================================
	workflow.StartPhase("AI Processing")

	fmt.Println("\n⏳ Waiting for AI parsing and matching...")
	time.Sleep(25 * time.Second)

	// Check results
	offers, _ := offerRepo.GetActive(ctx, 100, 0)
	requests, _ := requestRepo.GetActive(ctx, 100, 0)
	pendingMatches, _ := matchRepo.GetPending(ctx, 100, 0)

	workflow.EndPhase("success", fmt.Sprintf("Offers: %d, Requests: %d, Matches: %d", len(offers), len(requests), len(pendingMatches)))

	// ============================================================
	// PHASE 7: Generate Report (Actual Implementation)
	// ============================================================
	workflow.StartPhase("Report Generation")

	generator := reports.NewGenerator(reportRepo, log)
	report, err := generator.GenerateHourlyReport(ctx, reports.ReportConfig{
		PeriodHours:      24,
		MinScore:         0.0,
		IncludePending:   true,
		IncludeConfirmed: true,
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

		// Save Excel
		xlsxData, err := generator.ExportToExcel(report)
		if err == nil {
			xlsxPath := "./data/workflow_report.xlsx"
			os.WriteFile(xlsxPath, xlsxData, 0644)
			workflow.EndPhase("success", fmt.Sprintf("Reports: %s, %s", csvPath, xlsxPath))
		} else {
			workflow.EndPhase("success", fmt.Sprintf("Report: %s (Excel failed)", csvPath))
		}
	}

	// ============================================================
	// PHASE 8: Send Notifications (Actual Implementation)
	// ============================================================
	if !*skipNotify && report != nil {
		workflow.StartPhase("Notifications")

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
			workflow.EndPhase("success", "Notifications sent")
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
	fmt.Println("   - ./data/workflow_report.xlsx")

	fmt.Println("\n📊 Database Contents:")
	fmt.Printf("   - Groups Monitored: %d\n", len(testGroups))
	fmt.Printf("   - Offers: %d\n", len(offers))
	fmt.Printf("   - Requests: %d\n", len(requests))
	fmt.Printf("   - Pending Matches: %d\n", len(pendingMatches))

	// Show extracted medications
	if len(offers) > 0 {
		fmt.Println("\n💊 Extracted Offers:")
		for i, o := range offers {
			if i >= 5 {
				fmt.Printf("   ... and %d more\n", len(offers)-5)
				break
			}
			fmt.Printf("   - %s (%.0f units @ %.0f EGP)\n", o.Medication, o.Quantity, o.Price)
		}
	}

	if len(requests) > 0 {
		fmt.Println("\n📋 Extracted Requests:")
		for i, r := range requests {
			if i >= 5 {
				fmt.Printf("   ... and %d more\n", len(requests)-5)
				break
			}
			urgent := ""
			if r.Urgent {
				urgent = " 🔥"
			}
			fmt.Printf("   - %s (%.0f units)%s\n", r.Medication, r.Quantity, urgent)
		}
	}

	if len(offers) > 0 || len(requests) > 0 {
		fmt.Println("\n✅ Workflow completed successfully - Real message flow simulated!")
	} else {
		fmt.Println("\n⚠️ No offers/requests created - check AI provider connection")
	}
}

func truncate(s string, maxLen int) string {
	s = strings.ReplaceAll(s, "\n", " ")
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}
