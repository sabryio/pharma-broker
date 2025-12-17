package matching

import (
	"hash/fnv"
	"math"
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"
)

// =============================================================================
// A/B Test Configuration
// =============================================================================

// ABTestConfig defines an A/B test experiment.
type ABTestConfig struct {
	TestID      string    // Unique identifier for the test
	Name        string    // Human-readable name
	Description string    // What this test is evaluating
	ControlPct  float64   // Percentage of users in control group (0.0-1.0)
	TestWeights Weights   // Weights for test group
	StartTime   time.Time // When the test starts
	EndTime     time.Time // When the test ends
	MinSamples  int       // Minimum samples before results are valid
	Active      bool      // Whether the test is currently active
}

// ABTestResult holds the results of an A/B test.
type ABTestResult struct {
	TestID                   string
	ControlSamples           int64
	ControlConfirmed         int64
	ControlRejected          int64
	ControlAvgScore          float64
	TestSamples              int64
	TestConfirmed            int64
	TestRejected             int64
	TestAvgScore             float64
	StartTime                time.Time
	LastUpdated              time.Time
	StatisticallySignificant bool
	PValue                   float64
	Uplift                   float64 // Percentage improvement of test over control
}

// ABTestStats tracks atomic statistics for an A/B test.
type ABTestStats struct {
	ControlSamples   atomic.Int64
	ControlConfirmed atomic.Int64
	ControlScoreSum  atomic.Int64 // Stored as int64 (score * 10000)
	TestSamples      atomic.Int64
	TestConfirmed    atomic.Int64
	TestScoreSum     atomic.Int64 // Stored as int64 (score * 10000)
}

// =============================================================================
// A/B Test Manager
// =============================================================================

// ABTestManager manages A/B testing for weight optimization.
type ABTestManager struct {
	tests       map[string]*ABTestConfig
	stats       map[string]*ABTestStats
	baseWeights Weights
	log         zerolog.Logger
	mu          sync.RWMutex
}

// NewABTestManager creates a new A/B test manager.
func NewABTestManager(baseWeights Weights, log zerolog.Logger) *ABTestManager {
	return &ABTestManager{
		tests:       make(map[string]*ABTestConfig),
		stats:       make(map[string]*ABTestStats),
		baseWeights: baseWeights,
		log:         log.With().Str("component", "ab-test").Logger(),
	}
}

// CreateTest creates a new A/B test.
func (m *ABTestManager) CreateTest(cfg ABTestConfig) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	if cfg.ControlPct <= 0 || cfg.ControlPct >= 1 {
		cfg.ControlPct = 0.5 // Default 50/50 split
	}
	if cfg.MinSamples <= 0 {
		cfg.MinSamples = 100
	}

	cfg.Active = true
	m.tests[cfg.TestID] = &cfg
	m.stats[cfg.TestID] = &ABTestStats{}

	m.log.Info().
		Str("test_id", cfg.TestID).
		Str("name", cfg.Name).
		Float64("control_pct", cfg.ControlPct).
		Time("start", cfg.StartTime).
		Time("end", cfg.EndTime).
		Msg("🧪 Created A/B test")

	return nil
}

// GetWeightsForUser returns the appropriate weights for a user based on active tests.
func (m *ABTestManager) GetWeightsForUser(userID string) (Weights, string) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	now := time.Now()

	for testID, test := range m.tests {
		if !test.Active {
			continue
		}
		if now.Before(test.StartTime) || now.After(test.EndTime) {
			continue
		}

		// Deterministic assignment based on user ID + test ID
		bucket := m.getUserBucket(userID, testID)

		if bucket >= test.ControlPct {
			// Test group
			return test.TestWeights, testID + ":test"
		}
		// Control group
		return m.baseWeights, testID + ":control"
	}

	// No active test, use base weights
	return m.baseWeights, ""
}

// getUserBucket returns a deterministic bucket (0.0-1.0) for a user in a test.
func (m *ABTestManager) getUserBucket(userID, testID string) float64 {
	h := fnv.New32a()
	h.Write([]byte(userID + ":" + testID))
	return float64(h.Sum32()) / float64(math.MaxUint32)
}

// RecordFeedback records feedback for an A/B test.
func (m *ABTestManager) RecordFeedback(userID string, confirmed bool, score float64) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	now := time.Now()

	for testID, test := range m.tests {
		if !test.Active {
			continue
		}
		if now.Before(test.StartTime) || now.After(test.EndTime) {
			continue
		}

		stats := m.stats[testID]
		bucket := m.getUserBucket(userID, testID)
		scoreInt := int64(score * 10000)

		if bucket >= test.ControlPct {
			// Test group
			stats.TestSamples.Add(1)
			stats.TestScoreSum.Add(scoreInt)
			if confirmed {
				stats.TestConfirmed.Add(1)
			}
		} else {
			// Control group
			stats.ControlSamples.Add(1)
			stats.ControlScoreSum.Add(scoreInt)
			if confirmed {
				stats.ControlConfirmed.Add(1)
			}
		}
	}
}

// GetTestResult returns the current results for a test.
func (m *ABTestManager) GetTestResult(testID string) *ABTestResult {
	m.mu.RLock()
	defer m.mu.RUnlock()

	test, ok := m.tests[testID]
	if !ok {
		return nil
	}

	stats, ok := m.stats[testID]
	if !ok {
		return nil
	}

	controlSamples := stats.ControlSamples.Load()
	testSamples := stats.TestSamples.Load()

	result := &ABTestResult{
		TestID:           testID,
		ControlSamples:   controlSamples,
		ControlConfirmed: stats.ControlConfirmed.Load(),
		ControlRejected:  controlSamples - stats.ControlConfirmed.Load(),
		TestSamples:      testSamples,
		TestConfirmed:    stats.TestConfirmed.Load(),
		TestRejected:     testSamples - stats.TestConfirmed.Load(),
		StartTime:        test.StartTime,
		LastUpdated:      time.Now(),
	}

	// Calculate average scores
	if controlSamples > 0 {
		result.ControlAvgScore = float64(stats.ControlScoreSum.Load()) / float64(controlSamples) / 10000
	}
	if testSamples > 0 {
		result.TestAvgScore = float64(stats.TestScoreSum.Load()) / float64(testSamples) / 10000
	}

	// Calculate uplift
	if result.ControlAvgScore > 0 {
		result.Uplift = (result.TestAvgScore - result.ControlAvgScore) / result.ControlAvgScore * 100
	}

	// Calculate statistical significance (simplified chi-square test)
	result.StatisticallySignificant, result.PValue = m.calculateSignificance(result, test.MinSamples)

	return result
}

// calculateSignificance performs a simplified significance test.
func (m *ABTestManager) calculateSignificance(result *ABTestResult, minSamples int) (bool, float64) {
	if result.ControlSamples < int64(minSamples) || result.TestSamples < int64(minSamples) {
		return false, 1.0 // Not enough samples
	}

	// Calculate confirmation rates
	controlRate := float64(result.ControlConfirmed) / float64(result.ControlSamples)
	testRate := float64(result.TestConfirmed) / float64(result.TestSamples)

	// Pooled rate
	totalConfirmed := result.ControlConfirmed + result.TestConfirmed
	totalSamples := result.ControlSamples + result.TestSamples
	pooledRate := float64(totalConfirmed) / float64(totalSamples)

	// Standard error
	se := math.Sqrt(pooledRate * (1 - pooledRate) * (1/float64(result.ControlSamples) + 1/float64(result.TestSamples)))

	if se == 0 {
		return false, 1.0
	}

	// Z-score
	z := math.Abs(testRate-controlRate) / se

	// Approximate p-value (two-tailed)
	pValue := 2 * (1 - normalCDF(z))

	return pValue < 0.05, pValue
}

// normalCDF approximates the cumulative distribution function of standard normal.
func normalCDF(z float64) float64 {
	// Approximation using error function
	return 0.5 * (1 + math.Erf(z/math.Sqrt2))
}

// EndTest ends an A/B test and returns final results.
func (m *ABTestManager) EndTest(testID string) *ABTestResult {
	m.mu.Lock()
	test, ok := m.tests[testID]
	if !ok {
		m.mu.Unlock()
		return nil
	}
	test.Active = false
	m.mu.Unlock()

	// Get result without holding lock
	result := m.GetTestResult(testID)
	if result == nil {
		return nil
	}

	m.log.Info().
		Str("test_id", testID).
		Int64("control_samples", result.ControlSamples).
		Int64("test_samples", result.TestSamples).
		Float64("uplift", result.Uplift).
		Bool("significant", result.StatisticallySignificant).
		Msg("🏁 A/B test ended")

	return result
}

// GetActiveTests returns all active tests.
func (m *ABTestManager) GetActiveTests() []ABTestConfig {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var active []ABTestConfig
	now := time.Now()

	for _, test := range m.tests {
		if test.Active && now.After(test.StartTime) && now.Before(test.EndTime) {
			active = append(active, *test)
		}
	}

	return active
}

// DeleteTest removes a test.
func (m *ABTestManager) DeleteTest(testID string) {
	m.mu.Lock()
	defer m.mu.Unlock()

	delete(m.tests, testID)
	delete(m.stats, testID)
}

// SetBaseWeights updates the base weights (control group weights).
func (m *ABTestManager) SetBaseWeights(weights Weights) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.baseWeights = weights
}
