package matching

import (
	"testing"
	"time"

	"github.com/rs/zerolog"
)

func TestNewABTestManager(t *testing.T) {
	log := zerolog.Nop()
	baseWeights := DefaultWeights()
	manager := NewABTestManager(baseWeights, log)

	if manager == nil {
		t.Fatal("NewABTestManager returned nil")
	}
}

func TestABTestManager_CreateTest(t *testing.T) {
	log := zerolog.Nop()
	manager := NewABTestManager(DefaultWeights(), log)

	cfg := ABTestConfig{
		TestID:      "test-1",
		Name:        "Weight Optimization Test",
		ControlPct:  0.5,
		TestWeights: Weights{Medication: 0.4, Dosage: 0.3, Quantity: 0.1, Price: 0.1, Recency: 0.1},
		StartTime:   time.Now().Add(-time.Hour),
		EndTime:     time.Now().Add(24 * time.Hour),
		MinSamples:  50,
	}

	err := manager.CreateTest(cfg)
	if err != nil {
		t.Errorf("CreateTest() error = %v", err)
	}

	active := manager.GetActiveTests()
	if len(active) != 1 {
		t.Errorf("GetActiveTests() = %d, want 1", len(active))
	}
}

func TestABTestManager_GetWeightsForUser_NoActiveTest(t *testing.T) {
	log := zerolog.Nop()
	baseWeights := Weights{Medication: 0.35, Dosage: 0.25, Quantity: 0.15, Price: 0.15, Recency: 0.10}
	manager := NewABTestManager(baseWeights, log)

	weights, testGroup := manager.GetWeightsForUser("user-123")

	if testGroup != "" {
		t.Errorf("testGroup = %s, want empty", testGroup)
	}
	if weights.Medication != baseWeights.Medication {
		t.Errorf("weights.Medication = %f, want %f", weights.Medication, baseWeights.Medication)
	}
}

func TestABTestManager_GetWeightsForUser_WithActiveTest(t *testing.T) {
	log := zerolog.Nop()
	baseWeights := Weights{Medication: 0.35, Dosage: 0.25, Quantity: 0.15, Price: 0.15, Recency: 0.10}
	testWeights := Weights{Medication: 0.50, Dosage: 0.20, Quantity: 0.10, Price: 0.10, Recency: 0.10}
	manager := NewABTestManager(baseWeights, log)

	cfg := ABTestConfig{
		TestID:      "test-1",
		ControlPct:  0.5,
		TestWeights: testWeights,
		StartTime:   time.Now().Add(-time.Hour),
		EndTime:     time.Now().Add(24 * time.Hour),
	}
	_ = manager.CreateTest(cfg)

	// Test multiple users - should get deterministic assignment
	controlCount := 0
	testCount := 0
	for i := range 100 {
		userID := string(rune('A'+i%26)) + string(rune('0'+i/26))
		_, group := manager.GetWeightsForUser(userID)
		if group == "test-1:control" {
			controlCount++
		} else if group == "test-1:test" {
			testCount++
		}
	}

	// With 50/50 split, expect roughly equal distribution
	if controlCount == 0 || testCount == 0 {
		t.Errorf("Expected both groups to have users: control=%d, test=%d", controlCount, testCount)
	}
}

func TestABTestManager_GetWeightsForUser_Deterministic(t *testing.T) {
	log := zerolog.Nop()
	manager := NewABTestManager(DefaultWeights(), log)

	cfg := ABTestConfig{
		TestID:      "test-1",
		ControlPct:  0.5,
		TestWeights: Weights{Medication: 0.5, Dosage: 0.2, Quantity: 0.1, Price: 0.1, Recency: 0.1},
		StartTime:   time.Now().Add(-time.Hour),
		EndTime:     time.Now().Add(24 * time.Hour),
	}
	_ = manager.CreateTest(cfg)

	// Same user should always get same assignment
	_, group1 := manager.GetWeightsForUser("user-123")
	_, group2 := manager.GetWeightsForUser("user-123")
	_, group3 := manager.GetWeightsForUser("user-123")

	if group1 != group2 || group2 != group3 {
		t.Errorf("User assignment not deterministic: %s, %s, %s", group1, group2, group3)
	}
}

func TestABTestManager_RecordFeedback(t *testing.T) {
	log := zerolog.Nop()
	manager := NewABTestManager(DefaultWeights(), log)

	cfg := ABTestConfig{
		TestID:      "test-1",
		ControlPct:  0.5,
		TestWeights: Weights{Medication: 0.5, Dosage: 0.2, Quantity: 0.1, Price: 0.1, Recency: 0.1},
		StartTime:   time.Now().Add(-time.Hour),
		EndTime:     time.Now().Add(24 * time.Hour),
		MinSamples:  10,
	}
	_ = manager.CreateTest(cfg)

	// Record feedback for multiple users
	for i := range 50 {
		userID := string(rune('A' + i%26))
		manager.RecordFeedback(userID, i%3 == 0, 0.75)
	}

	result := manager.GetTestResult("test-1")
	if result == nil {
		t.Fatal("GetTestResult returned nil")
	}

	totalSamples := result.ControlSamples + result.TestSamples
	if totalSamples != 50 {
		t.Errorf("Total samples = %d, want 50", totalSamples)
	}
}

func TestABTestManager_GetTestResult(t *testing.T) {
	log := zerolog.Nop()
	manager := NewABTestManager(DefaultWeights(), log)

	cfg := ABTestConfig{
		TestID:      "test-1",
		ControlPct:  0.5,
		TestWeights: Weights{Medication: 0.5, Dosage: 0.2, Quantity: 0.1, Price: 0.1, Recency: 0.1},
		StartTime:   time.Now().Add(-time.Hour),
		EndTime:     time.Now().Add(24 * time.Hour),
		MinSamples:  5,
	}
	_ = manager.CreateTest(cfg)

	// Record enough feedback
	for i := range 20 {
		userID := string(rune('A' + i))
		manager.RecordFeedback(userID, true, 0.8)
	}

	result := manager.GetTestResult("test-1")
	if result == nil {
		t.Fatal("GetTestResult returned nil")
	}

	if result.TestID != "test-1" {
		t.Errorf("TestID = %s, want test-1", result.TestID)
	}
}

func TestABTestManager_EndTest(t *testing.T) {
	log := zerolog.Nop()
	manager := NewABTestManager(DefaultWeights(), log)

	cfg := ABTestConfig{
		TestID:      "test-1",
		ControlPct:  0.5,
		TestWeights: Weights{Medication: 0.5, Dosage: 0.2, Quantity: 0.1, Price: 0.1, Recency: 0.1},
		StartTime:   time.Now().Add(-time.Hour),
		EndTime:     time.Now().Add(24 * time.Hour),
	}
	_ = manager.CreateTest(cfg)

	result := manager.EndTest("test-1")
	if result == nil {
		t.Fatal("EndTest returned nil")
	}

	// Test should no longer be active
	active := manager.GetActiveTests()
	if len(active) != 0 {
		t.Errorf("GetActiveTests() after end = %d, want 0", len(active))
	}
}

func TestABTestManager_DeleteTest(t *testing.T) {
	log := zerolog.Nop()
	manager := NewABTestManager(DefaultWeights(), log)

	cfg := ABTestConfig{
		TestID:      "test-1",
		ControlPct:  0.5,
		TestWeights: Weights{Medication: 0.5, Dosage: 0.2, Quantity: 0.1, Price: 0.1, Recency: 0.1},
		StartTime:   time.Now().Add(-time.Hour),
		EndTime:     time.Now().Add(24 * time.Hour),
	}
	_ = manager.CreateTest(cfg)

	manager.DeleteTest("test-1")

	result := manager.GetTestResult("test-1")
	if result != nil {
		t.Error("GetTestResult should return nil after delete")
	}
}

func TestABTestManager_SetBaseWeights(t *testing.T) {
	log := zerolog.Nop()
	manager := NewABTestManager(DefaultWeights(), log)

	newWeights := Weights{Medication: 0.5, Dosage: 0.2, Quantity: 0.1, Price: 0.1, Recency: 0.1}
	manager.SetBaseWeights(newWeights)

	weights, _ := manager.GetWeightsForUser("user-123")
	if weights.Medication != newWeights.Medication {
		t.Errorf("Base weights not updated: got %f, want %f", weights.Medication, newWeights.Medication)
	}
}

func TestNormalCDF(t *testing.T) {
	// Test known values
	tests := []struct {
		z        float64
		expected float64
		epsilon  float64
	}{
		{0, 0.5, 0.01},
		{1.96, 0.975, 0.01},
		{-1.96, 0.025, 0.01},
	}

	for _, tt := range tests {
		result := normalCDF(tt.z)
		if result < tt.expected-tt.epsilon || result > tt.expected+tt.epsilon {
			t.Errorf("normalCDF(%f) = %f, want ~%f", tt.z, result, tt.expected)
		}
	}
}
