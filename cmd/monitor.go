package cmd

import (
	"context"
	"fmt"
	"os"
	"strings"
	"unicode"

	"github.com/abadojack/whatlanggo"
	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/rs/zerolog"
	"github.com/spf13/cobra"

	"pharmabroker/messaging/whatsapp"
	"pharmabroker/pkg/config"
	storageGorm "pharmabroker/storage/gorm"
)

var monitorCmd = &cobra.Command{
	Use:   "monitor",
	Short: "Interactive app configuration and group monitoring",
	Long: `Opens an interactive TUI to configure PharmaBroker settings.

Features:
  📱 Groups  - Select which WhatsApp groups to monitor
  ⚙️  Config  - Toggle app settings (skip own messages, auto-parse)
  👥 Admins  - Manage authorized bot users

Controls:
  Tab/1/2/3  Switch between tabs
  ↑/↓        Navigate items
  Space      Toggle selection
  Enter      Save and exit
  q          Quit without saving`,
	Run: runMonitor,
}

// Tab represents active tab
type Tab int

const (
	TabGroups Tab = iota
	TabConfig
	TabAdmins
)

// Modern color palette
var (
	// Base colors - Dark theme
	colorBg        = lipgloss.Color("#0F172A") // Deep navy
	colorSurface   = lipgloss.Color("#1E293B") // Card bg
	colorBorder    = lipgloss.Color("#334155") // Border
	colorText      = lipgloss.Color("#F1F5F9") // Primary text
	colorMuted     = lipgloss.Color("#64748B") // Muted text
	colorHighlight = lipgloss.Color("#475569") // Selection bg

	// Accent colors
	colorPrimary   = lipgloss.Color("#A855F7") // Purple
	colorSecondary = lipgloss.Color("#22D3EE") // Cyan
	colorSuccess   = lipgloss.Color("#22C55E") // Green
	colorWarning   = lipgloss.Color("#F59E0B") // Amber
)

// Styles - base styles without fixed widths (applied dynamically)
var (
	// App container
	appStyle = lipgloss.NewStyle().
			Padding(1, 2)

	// Header with gradient effect (purple to cyan text)
	headerBaseStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(colorPrimary).
			Background(colorSurface).
			Padding(1, 3).
			Align(lipgloss.Center).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(colorPrimary)

	// Tab bar
	tabBarBaseStyle = lipgloss.NewStyle().
			Background(colorSurface).
			Padding(0, 1).
			Border(lipgloss.NormalBorder(), false, false, true, false).
			BorderForeground(colorBorder)

	activeTabStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(colorBg).
			Background(colorPrimary).
			Padding(0, 2).
			MarginRight(1)

	inactiveTabStyle = lipgloss.NewStyle().
				Foreground(colorMuted).
				Padding(0, 2).
				MarginRight(1)

	// Content panel
	panelBaseStyle = lipgloss.NewStyle().
			Background(colorSurface).
			Padding(1, 2).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(colorBorder)

	// Panel title
	panelTitleStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(colorSecondary).
			MarginBottom(1)

	// List items
	selectedItemStyle = lipgloss.NewStyle().
				Foreground(colorPrimary).
				Bold(true)

	normalItemStyle = lipgloss.NewStyle().
			Foreground(colorText)

	descriptionStyle = lipgloss.NewStyle().
				Foreground(colorMuted).
				Italic(true).
				MarginLeft(4)

	// Check indicators
	checkOnStyle = lipgloss.NewStyle().
			Foreground(colorSuccess).
			Bold(true)

	checkOffStyle = lipgloss.NewStyle().
			Foreground(colorMuted)

	// Status bar
	statusBarBaseStyle = lipgloss.NewStyle().
				Background(colorSurface).
				Foreground(colorMuted).
				Padding(0, 2).
				Border(lipgloss.RoundedBorder()).
				BorderForeground(colorBorder)

	// Count badge
	countStyle = lipgloss.NewStyle().
			Foreground(colorSecondary).
			Bold(true)

	// Cursor
	cursorStyle = lipgloss.NewStyle().
			Foreground(colorPrimary).
			Bold(true)

	// Empty state
	emptyStyle = lipgloss.NewStyle().
			Foreground(colorMuted).
			Italic(true).
			Align(lipgloss.Center).
			Width(56)
)

// groupItem represents a WhatsApp group
type groupItem struct {
	jid       string
	name      string
	monitored bool
}

// configItem represents a config setting
type configItem struct {
	key         string
	label       string
	description string
	enabled     bool
	isText      bool
	textValue   string
}

// adminItem represents an admin user
type adminItem struct {
	id       string
	name     string
	platform string
	isAdmin  bool
}

// model is the Bubble Tea model
type model struct {
	// State
	activeTab Tab
	cursor    int
	quitting  bool
	saved     bool
	err       error
	width     int
	height    int

	// Data
	groups  []groupItem
	configs []configItem
	admins  []adminItem

	// Repos
	groupRepo   *storageGorm.GroupRepo
	configRepo  *storageGorm.ConfigRepo
	botUserRepo *storageGorm.BotUserRepo
	ctx         context.Context

	// Input
	textInput textinput.Model
	editing   bool
}

func (m model) Init() tea.Cmd {
	return textinput.Blink
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		// Handle text input mode
		if m.editing {
			switch msg.String() {
			case "enter":
				m.editing = false
				if m.activeTab == TabConfig && m.cursor < len(m.configs) {
					m.configs[m.cursor].textValue = m.textInput.Value()
				}
				return m, nil
			case "esc":
				m.editing = false
				return m, nil
			}
			var cmd tea.Cmd
			m.textInput, cmd = m.textInput.Update(msg)
			return m, cmd
		}

		switch msg.String() {
		case "q", "ctrl+c":
			m.quitting = true
			return m, tea.Quit

		case "tab", "right":
			m.activeTab = (m.activeTab + 1) % 3
			m.cursor = 0
			return m, nil

		case "shift+tab", "left":
			m.activeTab = (m.activeTab + 2) % 3
			m.cursor = 0
			return m, nil

		case "1":
			m.activeTab = TabGroups
			m.cursor = 0
			return m, nil
		case "2":
			m.activeTab = TabConfig
			m.cursor = 0
			return m, nil
		case "3":
			m.activeTab = TabAdmins
			m.cursor = 0
			return m, nil

		case "up", "k":
			if m.cursor > 0 {
				m.cursor--
			}
			return m, nil

		case "down", "j":
			maxItems := m.getMaxItems()
			if m.cursor < maxItems-1 {
				m.cursor++
			}
			return m, nil

		case " ": // Toggle
			m.toggleCurrentItem()
			return m, nil

		case "enter":
			// Check if editing text field
			if m.activeTab == TabConfig && m.cursor < len(m.configs) {
				if m.configs[m.cursor].isText {
					m.editing = true
					m.textInput.SetValue(m.configs[m.cursor].textValue)
					m.textInput.Focus()
					return m, textinput.Blink
				}
			}
			// Otherwise save and exit
			m.saveAll()
			m.saved = true
			m.quitting = true
			return m, tea.Quit
		}

	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return m, nil
	}

	return m, nil
}

func (m *model) getMaxItems() int {
	switch m.activeTab {
	case TabGroups:
		return len(m.groups)
	case TabConfig:
		return len(m.configs)
	case TabAdmins:
		return len(m.admins)
	}
	return 0
}

func (m *model) toggleCurrentItem() {
	switch m.activeTab {
	case TabGroups:
		if m.cursor < len(m.groups) {
			m.groups[m.cursor].monitored = !m.groups[m.cursor].monitored
		}
	case TabConfig:
		if m.cursor < len(m.configs) && !m.configs[m.cursor].isText {
			m.configs[m.cursor].enabled = !m.configs[m.cursor].enabled
		}
	case TabAdmins:
		if m.cursor < len(m.admins) {
			m.admins[m.cursor].isAdmin = !m.admins[m.cursor].isAdmin
		}
	}
}

func (m *model) saveAll() {
	// Save groups
	for _, g := range m.groups {
		if err := m.groupRepo.SetMonitored(m.ctx, g.jid, g.monitored); err != nil {
			m.err = err
			return
		}
	}

	// Save config
	for _, c := range m.configs {
		var value string
		if c.isText {
			value = c.textValue
		} else if c.enabled {
			value = "true"
		} else {
			value = "false"
		}
		if err := m.configRepo.Set(m.ctx, c.key, value); err != nil {
			m.err = err
			return
		}
	}

	// Save bot users authorization
	if m.botUserRepo != nil {
		for _, a := range m.admins {
			if a.isAdmin {
				if err := m.botUserRepo.Authorize(m.ctx, a.id, "admin", "monitor-tui"); err != nil {
					m.err = err
					return
				}
			} else {
				if err := m.botUserRepo.Deauthorize(m.ctx, a.id); err != nil {
					m.err = err
					return
				}
			}
		}
	}
}

func (m model) View() string {
	if m.quitting {
		if m.saved {
			if m.err != nil {
				return fmt.Sprintf("\n  ❌ Error saving: %v\n\n", m.err)
			}
			count := 0
			for _, g := range m.groups {
				if g.monitored {
					count++
				}
			}
			return fmt.Sprintf("\n  ✅ Settings saved! Monitoring %d groups.\n\n", count)
		}
		return "\n  Cancelled.\n\n"
	}

	// Calculate width (use terminal width minus padding)
	width := m.width - 4
	if width < 60 {
		width = 60
	}
	panelHeight := m.height - 12
	if panelHeight < 10 {
		panelHeight = 10
	}

	// Apply dynamic widths to styles
	headerStyle := headerBaseStyle.Width(width)
	tabBarStyle := tabBarBaseStyle.Width(width)
	panelStyle := panelBaseStyle.Width(width).Height(panelHeight)
	statusBarStyle := statusBarBaseStyle.Width(width)

	var sections []string

	// Header
	header := headerStyle.Render("💊 PharmaBroker Monitor")
	sections = append(sections, header)

	// Tab bar
	tabs := m.renderTabs()
	sections = append(sections, tabBarStyle.Render(tabs))

	// Content panel
	var content string
	var title string
	switch m.activeTab {
	case TabGroups:
		monitored := m.countMonitored()
		title = fmt.Sprintf("📱 WhatsApp Groups  %s", countStyle.Render(fmt.Sprintf("(%d/%d active)", monitored, len(m.groups))))
		content = m.renderGroups()
	case TabConfig:
		title = "⚙️  Application Settings"
		content = m.renderConfig()
	case TabAdmins:
		adminCount := m.countAdmins()
		title = fmt.Sprintf("👥 Bot Users  %s", countStyle.Render(fmt.Sprintf("(%d authorized)", adminCount)))
		content = m.renderAdmins()
	}

	panel := panelTitleStyle.Render(title) + "\n" + content
	sections = append(sections, panelStyle.Render(panel))

	// Status bar
	footer := m.renderStatusBar()
	sections = append(sections, statusBarStyle.Render(footer))

	return appStyle.Render(lipgloss.JoinVertical(lipgloss.Left, sections...))
}

func (m model) renderTabs() string {
	tabs := []struct {
		num  string
		icon string
		name string
	}{
		{"1", "📱", "Groups"},
		{"2", "⚙️", "Config"},
		{"3", "👥", "Admins"},
	}

	var rendered []string
	for i, t := range tabs {
		label := fmt.Sprintf("%s %s %s", t.num, t.icon, t.name)
		if Tab(i) == m.activeTab {
			rendered = append(rendered, activeTabStyle.Render(label))
		} else {
			rendered = append(rendered, inactiveTabStyle.Render(label))
		}
	}
	return lipgloss.JoinHorizontal(lipgloss.Top, rendered...)
}

func (m model) countMonitored() int {
	count := 0
	for _, g := range m.groups {
		if g.monitored {
			count++
		}
	}
	return count
}

func (m model) countAdmins() int {
	count := 0
	for _, a := range m.admins {
		if a.isAdmin {
			count++
		}
	}
	return count
}

func (m model) renderGroups() string {
	if len(m.groups) == 0 {
		return emptyStyle.Render("No groups found.\nMake sure you're in WhatsApp groups.")
	}

	var lines []string
	for i, g := range m.groups {
		// Indicator
		check := checkOffStyle.Render("○")
		if g.monitored {
			check = checkOnStyle.Render("●")
		}

		// Format name for proper RTL display
		displayName := formatForTerminal(g.name)

		// Name with cursor
		name := normalItemStyle.Render(displayName)
		cursor := "  "
		if i == m.cursor {
			cursor = cursorStyle.Render("▸ ")
			name = selectedItemStyle.Render(displayName)
		}

		lines = append(lines, fmt.Sprintf("%s%s %s", cursor, check, name))
	}

	return strings.Join(lines, "\n")
}

// formatForTerminal formats text for proper terminal display.
// Reverses Arabic text segments for correct RTL rendering in LTR terminals.
func formatForTerminal(text string) string {
	// Detect language
	info := whatlanggo.Detect(text)

	// If Arabic with high confidence, reverse the text
	if info.Lang == whatlanggo.Arb && info.Confidence > 0.5 {
		return reverseArabicText(text)
	}

	// Check if text contains Arabic characters (mixed text)
	if containsArabic(text) {
		return reverseArabicText(text)
	}

	return text
}

// containsArabic checks if text contains Arabic characters.
func containsArabic(text string) bool {
	for _, r := range text {
		if unicode.Is(unicode.Arabic, r) {
			return true
		}
	}
	return false
}

// reverseArabicText reverses Arabic text segments while preserving numbers and punctuation.
func reverseArabicText(text string) string {
	runes := []rune(text)
	n := len(runes)

	// Reverse the entire string
	reversed := make([]rune, n)
	for i, r := range runes {
		reversed[n-1-i] = r
	}

	return string(reversed)
}

func (m model) renderConfig() string {
	var lines []string

	for i, c := range m.configs {
		cursor := "  "
		if i == m.cursor {
			cursor = cursorStyle.Render("▸ ")
		}

		if c.isText {
			// Text input field
			label := normalItemStyle.Render(c.label)
			if i == m.cursor {
				label = selectedItemStyle.Render(c.label)
			}

			value := c.textValue
			if value == "" {
				value = "(not set)"
			}

			if m.editing && i == m.cursor {
				lines = append(lines, fmt.Sprintf("%s📝 %s:", cursor, label))
				lines = append(lines, fmt.Sprintf("     %s", m.textInput.View()))
			} else {
				lines = append(lines, fmt.Sprintf("%s📝 %s: %s", cursor, label, countStyle.Render(value)))
			}
		} else {
			// Toggle field
			check := checkOffStyle.Render("○")
			if c.enabled {
				check = checkOnStyle.Render("●")
			}

			label := normalItemStyle.Render(c.label)
			if i == m.cursor {
				label = selectedItemStyle.Render(c.label)
			}

			lines = append(lines, fmt.Sprintf("%s%s %s", cursor, check, label))
		}

		// Description (skip if editing)
		if !m.editing || i != m.cursor {
			lines = append(lines, descriptionStyle.Render(c.description))
		}
		lines = append(lines, "") // Spacing
	}

	return strings.Join(lines, "\n")
}

func (m model) renderAdmins() string {
	if len(m.admins) == 0 {
		return emptyStyle.Render("No users found.\nUsers appear after sending /start to the bot.")
	}

	var lines []string
	for i, a := range m.admins {
		// Indicator
		check := checkOffStyle.Render("○")
		if a.isAdmin {
			check = checkOnStyle.Render("●")
		}

		// Platform badge
		platformIcon := "💬"
		if a.platform == "telegram" {
			platformIcon = "📱"
		}

		// Format name for proper RTL display
		displayName := formatForTerminal(a.name)

		// Name with cursor
		name := normalItemStyle.Render(displayName)
		cursor := "  "
		if i == m.cursor {
			cursor = cursorStyle.Render("▸ ")
			name = selectedItemStyle.Render(displayName)
		}

		platform := descriptionStyle.Render(fmt.Sprintf("%s %s", platformIcon, a.platform))
		lines = append(lines, fmt.Sprintf("%s%s %s  %s", cursor, check, name, platform))
	}

	return strings.Join(lines, "\n")
}

func (m model) renderStatusBar() string {
	if m.editing {
		return "💡 Enter: confirm │ Esc: cancel"
	}
	return "Tab: switch │ ↑↓: navigate │ Space: toggle │ Enter: save │ q: quit"
}

func runMonitor(cmd *cobra.Command, args []string) {
	// Load configuration
	cfg := config.Load()

	// Setup quiet logging
	log := zerolog.New(zerolog.ConsoleWriter{Out: os.Stderr}).
		With().
		Timestamp().
		Logger().
		Level(zerolog.WarnLevel)

	// Create context
	ctx := context.Background()

	// Initialize database
	db, err := storageGorm.NewDB(&storageGorm.Config{DSN: cfg.Database.DSN})
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to initialize database: %v\n", err)
		os.Exit(1)
	}
	defer db.Close()

	groupRepo := storageGorm.NewGroupRepo(db)
	configRepo := storageGorm.NewConfigRepo(db)

	// Initialize WhatsApp manager
	waManager, err := whatsapp.NewManager(ctx, &cfg.WhatsApp, log)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to initialize WhatsApp: %v\n", err)
		os.Exit(1)
	}
	defer waManager.Disconnect()

	fmt.Println("🔌 Connecting to WhatsApp...")

	// Connect to WhatsApp
	if err := waManager.Connect(ctx); err != nil {
		fmt.Fprintf(os.Stderr, "Failed to connect to WhatsApp: %v\n", err)
		os.Exit(1)
	}

	fmt.Println("📋 Fetching groups...")

	// Sync groups from WhatsApp to database
	if err := waManager.SyncGroups(ctx, func(jid, name, desc string) error {
		return groupRepo.SaveFromSync(ctx, jid, name, desc)
	}); err != nil {
		fmt.Fprintf(os.Stderr, "Failed to sync groups: %v\n", err)
		os.Exit(1)
	}

	// Get all groups from database
	groups, err := groupRepo.GetAll(ctx)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to get groups: %v\n", err)
		os.Exit(1)
	}

	// Convert to group items
	groupItems := make([]groupItem, len(groups))
	for i, g := range groups {
		groupItems[i] = groupItem{
			jid:       g.JID,
			name:      g.Name,
			monitored: g.Monitored,
		}
	}

	// Load config from database
	appConfig, _ := configRepo.GetAll(ctx)

	// Seed admin phone from config.yaml if not set in database
	adminPhone := appConfig.AdminPhone
	if adminPhone == "" && cfg.Monitor.AdminPhone != "" {
		adminPhone = cfg.Monitor.AdminPhone
	}

	configItems := []configItem{
		{
			key:         "skip_own_messages",
			label:       "Skip Own Messages",
			description: "Don't process messages sent by this WhatsApp account",
			enabled:     appConfig.SkipOwnMessages,
		},
		{
			key:         "auto_parse_enabled",
			label:       "Auto Parse Enabled",
			description: "Automatically parse incoming messages with AI",
			enabled:     appConfig.AutoParseEnabled,
		},
		{
			key:         "admin_phone",
			label:       "Admin Phone",
			description: "Primary admin phone number (E.164 format)",
			isText:      true,
			textValue:   adminPhone,
		},
	}

	// Create text input with styled appearance
	ti := textinput.New()
	ti.Placeholder = "+20xxxxxxxxxx"
	ti.CharLimit = 20
	ti.Width = 30

	// Load bot users
	botUserRepo := storageGorm.NewBotUserRepo(db)
	var adminItems []adminItem
	if botUsers, err := botUserRepo.GetAll(ctx, 100, 0); err == nil {
		for _, u := range botUsers {
			platform := u.Platform
			if u.HasTelegram() {
				platform = "telegram"
			} else if u.HasWhatsApp() {
				platform = "whatsapp"
			}
			adminItems = append(adminItems, adminItem{
				id:       u.ID,
				name:     u.DisplayName,
				platform: platform,
				isAdmin:  u.IsAuthorized,
			})
		}
	}

	// Create model
	m := model{
		groups:      groupItems,
		configs:     configItems,
		admins:      adminItems,
		groupRepo:   groupRepo,
		configRepo:  configRepo,
		botUserRepo: botUserRepo,
		ctx:         ctx,
		textInput:   ti,
	}

	// Run Bubble Tea
	p := tea.NewProgram(m, tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		fmt.Fprintf(os.Stderr, "Error running UI: %v\n", err)
		os.Exit(1)
	}
}
