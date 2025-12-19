package core

import "strings"

// CommandPrefix is the standard prefix for bot commands.
const CommandPrefix = "/"

// IsCommand checks if a message text is a bot command.
func IsCommand(text string) bool {
	return strings.HasPrefix(strings.TrimSpace(text), CommandPrefix)
}

// ParseCommand parses command text into a Command struct.
func ParseCommand(text string) *Command {
	text = strings.TrimSpace(text)
	if !strings.HasPrefix(text, CommandPrefix) {
		return nil
	}

	parts := strings.Fields(text)
	if len(parts) == 0 {
		return nil
	}

	cmdName := strings.TrimPrefix(parts[0], CommandPrefix)
	cmdName = strings.ToLower(cmdName)

	if cmdName == "" {
		return nil
	}

	var args []string
	if len(parts) > 1 {
		args = parts[1:]
	}

	return &Command{
		Name:    cmdName,
		Args:    args,
		RawText: text,
	}
}

// NormalizePhone removes common phone number formatting.
func NormalizePhone(phone string) string {
	phone = strings.ReplaceAll(phone, "+", "")
	phone = strings.ReplaceAll(phone, " ", "")
	phone = strings.ReplaceAll(phone, "-", "")
	phone = strings.ReplaceAll(phone, "(", "")
	phone = strings.ReplaceAll(phone, ")", "")
	return phone
}
