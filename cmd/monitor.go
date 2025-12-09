package cmd

import (
	"context"
	"fmt"
	"os"
	"strings"

	"github.com/charmbracelet/bubbles/list"
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
	Short: "Interactive group selection for monitoring",
	Long: `Opens an interactive UI to select which WhatsApp groups to monitor.
	
This connects to WhatsApp, fetches all available groups, and lets you
toggle monitoring on/off for each group using a nice terminal UI.

Controls:
  ↑/↓    Navigate groups
  Space  Toggle monitoring for selected group
  Enter  Save and exit
  q      Quit without saving`,
	Run: runMonitor,
}

// Styles
var (
	titleStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(lipgloss.Color("170")).
			MarginLeft(2)

	infoStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("241")).
			MarginLeft(2)

	checkStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("42"))

	uncheckStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("241"))
)

// groupItem represents a WhatsApp group in the list
type groupItem struct {
	jid       string
	name      string
	monitored bool
}

func (g groupItem) Title() string {
	check := uncheckStyle.Render("[ ]")
	if g.monitored {
		check = checkStyle.Render("[✓]")
	}
	return fmt.Sprintf("%s %s", check, g.name)
}

func (g groupItem) Description() string {
	return g.jid
}

func (g groupItem) FilterValue() string {
	return g.name
}

// model is the Bubble Tea model for group selection
type model struct {
	list      list.Model
	groups    []groupItem
	quitting  bool
	saved     bool
	err       error
	groupRepo *storageGorm.GroupRepo
	ctx       context.Context
}

func (m model) Init() tea.Cmd {
	return nil
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "q", "ctrl+c":
			m.quitting = true
			return m, tea.Quit

		case " ": // Toggle monitoring
			if i, ok := m.list.SelectedItem().(groupItem); ok {
				// Toggle the item
				for idx := range m.groups {
					if m.groups[idx].jid == i.jid {
						m.groups[idx].monitored = !m.groups[idx].monitored
						break
					}
				}
				// Update list items
				items := make([]list.Item, len(m.groups))
				for idx, g := range m.groups {
					items[idx] = g
				}
				m.list.SetItems(items)
			}
			return m, nil

		case "enter": // Save and exit
			m.saved = true
			// Save to database
			for _, g := range m.groups {
				if err := m.groupRepo.SetMonitored(m.ctx, g.jid, g.monitored); err != nil {
					m.err = err
					break
				}
			}
			m.quitting = true
			return m, tea.Quit
		}

	case tea.WindowSizeMsg:
		m.list.SetWidth(msg.Width)
		m.list.SetHeight(msg.Height - 6)
		return m, nil
	}

	var cmd tea.Cmd
	m.list, cmd = m.list.Update(msg)
	return m, cmd
}

func (m model) View() string {
	if m.quitting {
		if m.saved {
			if m.err != nil {
				return fmt.Sprintf("\n  ❌ Error saving: %v\n\n", m.err)
			}
			// Count monitored
			count := 0
			for _, g := range m.groups {
				if g.monitored {
					count++
				}
			}
			return fmt.Sprintf("\n  ✅ Saved! Monitoring %d groups.\n\n", count)
		}
		return "\n  Cancelled.\n\n"
	}

	var b strings.Builder
	b.WriteString("\n")
	b.WriteString(titleStyle.Render("📱 PharmaBroker Group Monitor"))
	b.WriteString("\n")
	b.WriteString(infoStyle.Render("Space: toggle • Enter: save • q: quit"))
	b.WriteString("\n\n")
	b.WriteString(m.list.View())
	return b.String()
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
	db, err := storageGorm.NewDB(&storageGorm.Config{Path: cfg.Database.Path})
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to initialize database: %v\n", err)
		os.Exit(1)
	}
	defer db.Close()

	groupRepo := storageGorm.NewGroupRepo(db)

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

	if len(groups) == 0 {
		fmt.Println("No groups found. Make sure you're in some WhatsApp groups.")
		return
	}

	// Convert to list items
	items := make([]list.Item, len(groups))
	groupItems := make([]groupItem, len(groups))
	for i, g := range groups {
		groupItems[i] = groupItem{
			jid:       g.JID,
			name:      g.Name,
			monitored: g.Monitored,
		}
		items[i] = groupItems[i]
	}

	// Create list
	delegate := list.NewDefaultDelegate()
	delegate.ShowDescription = true
	l := list.New(items, delegate, 80, 20)
	l.Title = ""
	l.SetShowTitle(false)
	l.SetShowStatusBar(false)
	l.SetFilteringEnabled(true)
	l.SetShowFilter(true)

	// Create model
	m := model{
		list:      l,
		groups:    groupItems,
		groupRepo: groupRepo,
		ctx:       ctx,
	}

	// Run Bubble Tea
	p := tea.NewProgram(m, tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		fmt.Fprintf(os.Stderr, "Error running UI: %v\n", err)
		os.Exit(1)
	}
}
