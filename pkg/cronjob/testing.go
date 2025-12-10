package cronjob

import (
	"sync"
)

// NoopLogger is a logger that does nothing.
type NoopLogger struct{}

func (NoopLogger) Info(msg string, keyvals ...any)             {}
func (NoopLogger) Error(msg string, err error, keyvals ...any) {}
func (NoopLogger) Debug(msg string, keyvals ...any)            {}

// NoopMetrics is a metrics collector that does nothing.
type NoopMetrics struct{}

func (NoopMetrics) Increment(name string, labels map[string]string)                        {}
func (NoopMetrics) ObserveDuration(name string, labels map[string]string, seconds float64) {}

// RecordingLogger captures log entries for testing.
type RecordingLogger struct {
	mu      sync.Mutex
	Entries []LogEntry
}

// LogEntry represents a single log entry.
type LogEntry struct {
	Level   string
	Message string
	Keyvals []any
	Err     error
}

func NewRecordingLogger() *RecordingLogger {
	return &RecordingLogger{Entries: make([]LogEntry, 0)}
}

func (l *RecordingLogger) Info(msg string, keyvals ...any) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.Entries = append(l.Entries, LogEntry{Level: "info", Message: msg, Keyvals: keyvals})
}

func (l *RecordingLogger) Error(msg string, err error, keyvals ...any) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.Entries = append(l.Entries, LogEntry{Level: "error", Message: msg, Err: err, Keyvals: keyvals})
}

func (l *RecordingLogger) Debug(msg string, keyvals ...any) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.Entries = append(l.Entries, LogEntry{Level: "debug", Message: msg, Keyvals: keyvals})
}

func (l *RecordingLogger) GetEntries() []LogEntry {
	l.mu.Lock()
	defer l.mu.Unlock()
	return append([]LogEntry{}, l.Entries...)
}

func (l *RecordingLogger) Clear() {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.Entries = l.Entries[:0]
}

// RecordingMetrics captures metrics for testing.
type RecordingMetrics struct {
	mu        sync.Mutex
	Counters  map[string]int
	Durations map[string][]float64
}

func NewRecordingMetrics() *RecordingMetrics {
	return &RecordingMetrics{
		Counters:  make(map[string]int),
		Durations: make(map[string][]float64),
	}
}

func (m *RecordingMetrics) Increment(name string, labels map[string]string) {
	m.mu.Lock()
	defer m.mu.Unlock()
	key := m.buildKey(name, labels)
	m.Counters[key]++
}

func (m *RecordingMetrics) ObserveDuration(name string, labels map[string]string, seconds float64) {
	m.mu.Lock()
	defer m.mu.Unlock()
	key := m.buildKey(name, labels)
	m.Durations[key] = append(m.Durations[key], seconds)
}

func (m *RecordingMetrics) buildKey(name string, labels map[string]string) string {
	key := name
	for k, v := range labels {
		key += ":" + k + "=" + v
	}
	return key
}

func (m *RecordingMetrics) GetCounter(name string, labels map[string]string) int {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.Counters[m.buildKey(name, labels)]
}

func (m *RecordingMetrics) GetDurations(name string, labels map[string]string) []float64 {
	m.mu.Lock()
	defer m.mu.Unlock()
	return append([]float64{}, m.Durations[m.buildKey(name, labels)]...)
}
