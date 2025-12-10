package core

import (
	"strings"

	"github.com/go-telegram/bot"
	"github.com/mattn/go-runewidth"
)

// EscapeMarkdownV2 escapes special characters for Telegram MarkdownV2.
// Uses the go-telegram/bot library's built-in helper.
func EscapeMarkdownV2(s string) string {
	return bot.EscapeMarkdown(s)
}

// Bold wraps text in MarkdownV2 bold syntax.
func Bold(s string) string {
	return "*" + EscapeMarkdownV2(s) + "*"
}

// Italic wraps text in MarkdownV2 italic syntax.
func Italic(s string) string {
	return "_" + EscapeMarkdownV2(s) + "_"
}

// Code wraps text in MarkdownV2 inline code syntax.
func Code(s string) string {
	return "`" + strings.ReplaceAll(strings.ReplaceAll(s, "`", "\\`"), "\\", "\\\\") + "`"
}

// InlineButton represents an inline keyboard button.
type InlineButton struct {
	Text         string
	CallbackData string
	URL          string
}

// InlineKeyboard represents a row of inline buttons.
type InlineKeyboard [][]InlineButton

func Separator(text string) string {
	width := runewidth.StringWidth(text)
	sepChar := "━"
	sepWidth := runewidth.StringWidth(sepChar)

	// number of separator characters needed
	count := width / sepWidth

	return strings.Repeat(sepChar, count)
}
