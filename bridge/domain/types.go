// Package domain contains core domain models and strong types for the bridge.
package domain

import (
	"fmt"
	"strings"
)

// MessageID is a strongly-typed message identifier.
type MessageID string

func (id MessageID) String() string { return string(id) }

// Short returns the first 8 characters of the ID for logging.
func (id MessageID) Short() string {
	if len(id) <= 8 {
		return string(id)
	}
	return string(id[:8])
}

// JID represents a WhatsApp Jabber ID (user or group).
type JID string

func (j JID) String() string { return string(j) }

// IsGroup returns true if this JID represents a group.
func (j JID) IsGroup() bool {
	return strings.HasSuffix(string(j), "@g.us")
}

// IsUser returns true if this JID represents a user.
func (j JID) IsUser() bool {
	return strings.HasSuffix(string(j), "@s.whatsapp.net")
}

// Phone extracts the phone number from a user JID.
func (j JID) Phone() Phone {
	s := string(j)
	if idx := strings.Index(s, "@"); idx > 0 {
		return Phone(s[:idx])
	}
	return ""
}

// Phone represents a phone number.
type Phone string

func (p Phone) String() string { return string(p) }

// TraceID represents a request correlation ID.
type TraceID string

func (t TraceID) String() string { return string(t) }

// NewTraceID creates a new trace ID from a message ID and timestamp.
func NewTraceID(msgID MessageID, nanoSuffix int64) TraceID {
	return TraceID(fmt.Sprintf("%s-%d", msgID.Short(), nanoSuffix%1000000))
}

// UnixTimestamp represents a Unix timestamp in seconds.
type UnixTimestamp int64

func (t UnixTimestamp) Int64() int64 { return int64(t) }

// GroupInfo represents a WhatsApp group for syncing.
type GroupInfo struct {
	JID         JID
	Name        string
	Description string
	MemberCount int32 // Number of members in the group
}

// Version represents a semantic version string.
type Version string

const CurrentVersion Version = "0.5.0"

func (v Version) String() string { return string(v) }
