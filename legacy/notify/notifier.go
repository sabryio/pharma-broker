package notify

import (
	"bytes"
	"context"
	"encoding/base64"
	"errors"
	"fmt"
	"io"
	"mime/multipart"
	"net/http"
	"net/smtp"
	"strings"
	"time"

	"github.com/rs/zerolog"
	"golang.org/x/sync/errgroup"
)

const (
	telegramAPIBaseURL = "https://api.telegram.org/bot%s"
	defaultHTTPTimeout = 30 * time.Second
)

// Notifier defines the interface for sending notifications
type Notifier interface {
	SendMessage(ctx context.Context, message string) error
	SendDocument(ctx context.Context, filename string, data []byte, caption string) error
}

// ReportNotifier defines the interface for sending reports
type ReportNotifier interface {
	SendReport(ctx context.Context, subject, htmlBody string, csvData []byte, csvFilename string) error
}

// TelegramConfig holds Telegram bot settings
type TelegramConfig struct {
	Enabled  bool     `yaml:"enabled"`
	BotToken string   `yaml:"bot_token"`
	ChatIDs  []string `yaml:"chat_ids"` // Can be group IDs (negative) or user IDs
}

// TelegramNotifier sends notifications via Telegram Bot API
type TelegramNotifier struct {
	config TelegramConfig
	client *http.Client
	log    zerolog.Logger
}

// NewTelegramNotifier creates a Telegram notifier
func NewTelegramNotifier(config TelegramConfig, log zerolog.Logger) *TelegramNotifier {
	return &TelegramNotifier{
		config: config,
		client: &http.Client{Timeout: defaultHTTPTimeout},
		log:    log.With().Str("notifier", "telegram").Logger(),
	}
}

// Ensure TelegramNotifier implements Notifier interface
var _ Notifier = (*TelegramNotifier)(nil)

// SendMessage sends a text message to all configured chats
func (t *TelegramNotifier) SendMessage(ctx context.Context, message string) error {
	if !t.config.Enabled || t.config.BotToken == "" {
		return nil
	}

	var errs []error
	for _, chatID := range t.config.ChatIDs {
		if err := t.sendToChat(ctx, chatID, message); err != nil {
			t.log.Error().Err(err).Str("chat_id", chatID).Msg("Failed to send message")
			errs = append(errs, fmt.Errorf("chat %s: %w", chatID, err))
		}
	}

	return errors.Join(errs...)
}

// SendDocument sends a file to all configured chats
func (t *TelegramNotifier) SendDocument(ctx context.Context, filename string, data []byte, caption string) error {
	if !t.config.Enabled || t.config.BotToken == "" {
		return nil
	}

	var errs []error
	for _, chatID := range t.config.ChatIDs {
		if err := t.sendDocumentToChat(ctx, chatID, filename, data, caption); err != nil {
			t.log.Error().Err(err).Str("chat_id", chatID).Msg("Failed to send document")
			errs = append(errs, fmt.Errorf("chat %s: %w", chatID, err))
		}
	}

	return errors.Join(errs...)
}

func (t *TelegramNotifier) sendToChat(ctx context.Context, chatID, message string) error {
	url := fmt.Sprintf(telegramAPIBaseURL+"/sendMessage", t.config.BotToken)

	body := fmt.Sprintf(`{"chat_id":"%s","text":%q,"parse_mode":"HTML"}`, chatID, message)
	req, err := http.NewRequestWithContext(ctx, "POST", url, strings.NewReader(body))
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := t.client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if err := t.checkResponse(resp); err != nil {
		return err
	}

	t.log.Debug().Str("chat_id", chatID).Msg("Message sent successfully")
	return nil
}

// checkResponse validates the HTTP response from Telegram API
func (t *TelegramNotifier) checkResponse(resp *http.Response) error {
	if resp.StatusCode != http.StatusOK {
		respBody, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("telegram API error (status %d): %s", resp.StatusCode, string(respBody))
	}
	return nil
}

func (t *TelegramNotifier) sendDocumentToChat(ctx context.Context, chatID, filename string, data []byte, caption string) error {
	url := fmt.Sprintf(telegramAPIBaseURL+"/sendDocument", t.config.BotToken)

	var buf bytes.Buffer
	writer := multipart.NewWriter(&buf)

	// Add chat_id
	if err := writer.WriteField("chat_id", chatID); err != nil {
		return fmt.Errorf("writing chat_id field: %w", err)
	}

	// Add caption
	if caption != "" {
		if err := writer.WriteField("caption", caption); err != nil {
			return fmt.Errorf("writing caption field: %w", err)
		}
	}

	// Add document
	part, err := writer.CreateFormFile("document", filename)
	if err != nil {
		return fmt.Errorf("creating form file: %w", err)
	}
	if _, err := part.Write(data); err != nil {
		return fmt.Errorf("writing document data: %w", err)
	}

	if err := writer.Close(); err != nil {
		return fmt.Errorf("closing multipart writer: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, "POST", url, &buf)
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", writer.FormDataContentType())

	resp, err := t.client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if err := t.checkResponse(resp); err != nil {
		return err
	}

	t.log.Debug().Str("chat_id", chatID).Str("filename", filename).Msg("Document sent successfully")
	return nil
}

// EmailConfig holds email settings
type EmailConfig struct {
	Enabled    bool     `yaml:"enabled"`
	SMTPHost   string   `yaml:"smtp_host"`
	SMTPPort   int      `yaml:"smtp_port"`
	Username   string   `yaml:"username"`
	Password   string   `yaml:"password"`
	FromName   string   `yaml:"from_name"`
	FromEmail  string   `yaml:"from_email"`
	Recipients []string `yaml:"recipients"`
}

// EmailNotifier sends notifications via SMTP
type EmailNotifier struct {
	config EmailConfig
	log    zerolog.Logger
}

// NewEmailNotifier creates an email notifier
func NewEmailNotifier(config EmailConfig, log zerolog.Logger) *EmailNotifier {
	return &EmailNotifier{
		config: config,
		log:    log.With().Str("notifier", "email").Logger(),
	}
}

// Ensure EmailNotifier implements ReportNotifier interface
var _ ReportNotifier = (*EmailNotifier)(nil)

// SendReport sends an HTML report with CSV attachment.
// Note: smtp.SendMail is blocking and does not support context cancellation.
// The ctx parameter is accepted for interface consistency but cancellation
// will not interrupt an in-progress SMTP send.
func (e *EmailNotifier) SendReport(ctx context.Context, subject, htmlBody string, csvData []byte, csvFilename string) error {
	if !e.config.Enabled {
		return nil
	}

	// Check context before starting
	if err := ctx.Err(); err != nil {
		return fmt.Errorf("context cancelled before sending email: %w", err)
	}

	boundary := "----=_Part_0_1234567890"

	var msg bytes.Buffer

	// Headers
	msg.WriteString(fmt.Sprintf("From: %s <%s>\r\n", e.config.FromName, e.config.FromEmail))
	msg.WriteString(fmt.Sprintf("To: %s\r\n", strings.Join(e.config.Recipients, ", ")))
	msg.WriteString(fmt.Sprintf("Subject: %s\r\n", subject))
	msg.WriteString("MIME-Version: 1.0\r\n")
	msg.WriteString(fmt.Sprintf("Content-Type: multipart/mixed; boundary=\"%s\"\r\n", boundary))
	msg.WriteString("\r\n")

	// HTML body part
	msg.WriteString(fmt.Sprintf("--%s\r\n", boundary))
	msg.WriteString("Content-Type: text/html; charset=\"UTF-8\"\r\n")
	msg.WriteString("\r\n")
	msg.WriteString(htmlBody)
	msg.WriteString("\r\n")

	// CSV attachment part with proper base64 encoding
	if len(csvData) > 0 {
		msg.WriteString(fmt.Sprintf("--%s\r\n", boundary))
		msg.WriteString(fmt.Sprintf("Content-Type: text/csv; name=\"%s\"\r\n", csvFilename))
		msg.WriteString("Content-Transfer-Encoding: base64\r\n")
		msg.WriteString(fmt.Sprintf("Content-Disposition: attachment; filename=\"%s\"\r\n", csvFilename))
		msg.WriteString("\r\n")
		msg.WriteString(base64.StdEncoding.EncodeToString(csvData))
		msg.WriteString("\r\n")
	}

	msg.WriteString(fmt.Sprintf("--%s--\r\n", boundary))

	// Send via SMTP
	addr := fmt.Sprintf("%s:%d", e.config.SMTPHost, e.config.SMTPPort)
	auth := smtp.PlainAuth("", e.config.Username, e.config.Password, e.config.SMTPHost)

	err := smtp.SendMail(addr, auth, e.config.FromEmail, e.config.Recipients, msg.Bytes())
	if err != nil {
		e.log.Error().Err(err).Msg("Failed to send email")
		return fmt.Errorf("sending email: %w", err)
	}

	e.log.Info().
		Strs("recipients", e.config.Recipients).
		Str("subject", subject).
		Msg("Email sent successfully")

	return nil
}

// NotificationService coordinates all notification channels
type NotificationService struct {
	telegram *TelegramNotifier
	email    *EmailNotifier
	log      zerolog.Logger
}

// NewNotificationService creates a notification service
func NewNotificationService(telegramConfig TelegramConfig, emailConfig EmailConfig, log zerolog.Logger) *NotificationService {
	return &NotificationService{
		telegram: NewTelegramNotifier(telegramConfig, log),
		email:    NewEmailNotifier(emailConfig, log),
		log:      log.With().Str("service", "notifications").Logger(),
	}
}

// SendReport sends the report via all enabled channels concurrently
func (n *NotificationService) SendReport(ctx context.Context, summaryText, htmlReport string, csvData []byte, csvFilename string) error {
	g, ctx := errgroup.WithContext(ctx)

	// Send to Telegram (parallel goroutines)
	if n.telegram.config.Enabled {
		// Send summary message
		g.Go(func() error {
			if err := n.telegram.SendMessage(ctx, summaryText); err != nil {
				n.log.Error().Err(err).Msg("Failed to send Telegram message")
				return fmt.Errorf("telegram message: %w", err)
			}
			return nil
		})

		// Send CSV file
		if len(csvData) > 0 {
			g.Go(func() error {
				if err := n.telegram.SendDocument(ctx, csvFilename, csvData, "📊 Full Match Report"); err != nil {
					n.log.Error().Err(err).Msg("Failed to send Telegram document")
					return fmt.Errorf("telegram document: %w", err)
				}
				return nil
			})
		}
	}

	// Send to Email
	if n.email.config.Enabled {
		g.Go(func() error {
			subject := fmt.Sprintf("PharmaBroker Report - %s", time.Now().Format("Jan 2, 15:04"))
			if err := n.email.SendReport(ctx, subject, htmlReport, csvData, csvFilename); err != nil {
				n.log.Error().Err(err).Msg("Failed to send email")
				return fmt.Errorf("email: %w", err)
			}
			return nil
		})
	}

	if err := g.Wait(); err != nil {
		return fmt.Errorf("notification error: %w", err)
	}

	n.log.Info().Msg("All notifications sent successfully")
	return nil
}
