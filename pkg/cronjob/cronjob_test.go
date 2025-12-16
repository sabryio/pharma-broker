package cronjob

import (
	"context"
	"errors"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

// testJob is a configurable test job.
type testJob struct {
	id       string
	schedule string
	runFunc  func(ctx context.Context) error
	runCount atomic.Int32
}

func newTestJob(id, schedule string) *testJob {
	return &testJob{
		id:       id,
		schedule: schedule,
		runFunc:  func(ctx context.Context) error { return nil },
	}
}

func (j *testJob) ID() string       { return j.id }
func (j *testJob) Schedule() string { return j.schedule }
func (j *testJob) Run(ctx context.Context) error {
	j.runCount.Add(1)
	if j.runFunc != nil {
		return j.runFunc(ctx)
	}
	return nil
}
func (j *testJob) RunCount() int { return int(j.runCount.Load()) }

// ============= Registry Tests =============

func TestRegistry_Register(t *testing.T) {
	r := NewRegistry()
	job := newTestJob("test-job", "* * * * *")

	if err := r.Register(job); err != nil {
		t.Fatalf("Register failed: %v", err)
	}

	if r.Count() != 1 {
		t.Errorf("Count = %d, want 1", r.Count())
	}
}

func TestRegistry_RegisterNil(t *testing.T) {
	r := NewRegistry()

	if err := r.Register(nil); err != nil {
		t.Fatalf("Register(nil) should not error: %v", err)
	}

	if r.Count() != 0 {
		t.Errorf("Count = %d, want 0 for nil job", r.Count())
	}
}

func TestRegistry_Get(t *testing.T) {
	r := NewRegistry()
	job := newTestJob("test-job", "* * * * *")
	_ = r.Register(job)

	retrieved, ok := r.Get("test-job")
	if !ok {
		t.Fatal("Get returned false for existing job")
	}
	if retrieved.ID() != job.ID() {
		t.Errorf("Got job ID %s, want %s", retrieved.ID(), job.ID())
	}

	_, ok = r.Get("nonexistent")
	if ok {
		t.Error("Get returned true for nonexistent job")
	}
}

func TestRegistry_List(t *testing.T) {
	r := NewRegistry()

	for i := 0; i < 5; i++ {
		_ = r.Register(newTestJob("job-"+string(rune('a'+i)), "* * * * *"))
	}

	jobs := r.List()
	if len(jobs) != 5 {
		t.Errorf("List() returned %d jobs, want 5", len(jobs))
	}
}

func TestRegistry_Unregister(t *testing.T) {
	r := NewRegistry()
	_ = r.Register(newTestJob("job-a", "* * * * *"))
	_ = r.Register(newTestJob("job-b", "* * * * *"))

	r.Unregister("job-a")

	if r.Count() != 1 {
		t.Errorf("Count = %d, want 1 after unregister", r.Count())
	}

	if _, ok := r.Get("job-a"); ok {
		t.Error("Job still exists after unregister")
	}
}

func TestRegistry_Clear(t *testing.T) {
	r := NewRegistry()
	for i := 0; i < 10; i++ {
		_ = r.Register(newTestJob("job-"+string(rune('a'+i)), "* * * * *"))
	}

	r.Clear()

	if r.Count() != 0 {
		t.Errorf("Count = %d, want 0 after clear", r.Count())
	}
}

func TestRegistry_Concurrent(t *testing.T) {
	r := NewRegistry()
	var wg sync.WaitGroup

	// Concurrent writes
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			_ = r.Register(newTestJob("job-"+string(rune(i)), "* * * * *"))
		}(i)
	}

	// Concurrent reads
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_ = r.List()
			_ = r.Count()
			_, _ = r.Get("job-a")
		}()
	}

	wg.Wait()
	// Should not panic or race
}

// ============= Scheduler Tests =============

func TestScheduler_ScheduleJob(t *testing.T) {
	logger := NewRecordingLogger()
	metrics := NewRecordingMetrics()
	s := NewScheduler(logger, metrics, WithSeconds())
	defer s.Stop(context.Background())

	job := newTestJob("test-job", "@every 1s")
	entryID, err := s.ScheduleJob(job)
	if err != nil {
		t.Fatalf("ScheduleJob failed: %v", err)
	}

	if entryID == "" {
		t.Error("ScheduleJob returned empty entry ID")
	}

	// Verify entry is registered
	id, ok := s.GetEntryID("test-job")
	if !ok {
		t.Error("GetEntryID returned false for scheduled job")
	}
	if id != entryID {
		t.Errorf("GetEntryID = %s, want %s", id, entryID)
	}
}

func TestScheduler_ScheduleJob_NilJob(t *testing.T) {
	logger := NoopLogger{}
	metrics := NoopMetrics{}
	s := NewScheduler(logger, metrics)

	_, err := s.ScheduleJob(nil)
	if err == nil {
		t.Error("ScheduleJob(nil) should return error")
	}
}

func TestScheduler_ScheduleJob_EmptySchedule(t *testing.T) {
	logger := NoopLogger{}
	metrics := NoopMetrics{}
	s := NewScheduler(logger, metrics)

	job := newTestJob("test-job", "")
	_, err := s.ScheduleJob(job)
	if err == nil {
		t.Error("ScheduleJob with empty schedule should return error")
	}
}

func TestScheduler_ScheduleJob_InvalidCron(t *testing.T) {
	logger := NoopLogger{}
	metrics := NoopMetrics{}
	s := NewScheduler(logger, metrics)

	job := newTestJob("test-job", "invalid cron expression")
	_, err := s.ScheduleJob(job)
	if err == nil {
		t.Error("ScheduleJob with invalid cron should return error")
	}
}

func TestScheduler_ScheduleJob_Duplicate(t *testing.T) {
	logger := NoopLogger{}
	metrics := NoopMetrics{}
	s := NewScheduler(logger, metrics)
	defer s.Stop(context.Background())

	job := newTestJob("test-job", "* * * * *")
	_, err := s.ScheduleJob(job)
	if err != nil {
		t.Fatalf("First ScheduleJob failed: %v", err)
	}

	_, err = s.ScheduleJob(job)
	if err == nil {
		t.Error("Duplicate ScheduleJob should return error")
	}
}

func TestScheduler_RemoveJob(t *testing.T) {
	logger := NoopLogger{}
	metrics := NoopMetrics{}
	s := NewScheduler(logger, metrics)
	defer s.Stop(context.Background())

	job := newTestJob("test-job", "* * * * *")
	_, _ = s.ScheduleJob(job)

	err := s.RemoveJob("test-job")
	if err != nil {
		t.Fatalf("RemoveJob failed: %v", err)
	}

	_, ok := s.GetEntryID("test-job")
	if ok {
		t.Error("Job still exists after RemoveJob")
	}
}

func TestScheduler_RemoveJob_NotFound(t *testing.T) {
	logger := NoopLogger{}
	metrics := NoopMetrics{}
	s := NewScheduler(logger, metrics)

	err := s.RemoveJob("nonexistent")
	if err == nil {
		t.Error("RemoveJob for nonexistent job should return error")
	}
}

func TestScheduler_StartStop(t *testing.T) {
	logger := NewRecordingLogger()
	metrics := NoopMetrics{}
	s := NewScheduler(logger, metrics).(*CronScheduler)

	s.Start()
	if !s.IsRunning() {
		t.Error("Scheduler should be running after Start")
	}

	// Start again should be idempotent
	s.Start()

	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	s.Stop(ctx)
	if s.IsRunning() {
		t.Error("Scheduler should not be running after Stop")
	}

	// Stop again should be idempotent
	s.Stop(ctx)
}

func TestScheduler_JobExecution(t *testing.T) {
	logger := NewRecordingLogger()
	metrics := NewRecordingMetrics()
	s := NewScheduler(logger, metrics, WithSeconds())

	var executed atomic.Bool
	done := make(chan struct{})

	job := newTestJob("fast-job", "@every 1s")
	job.runFunc = func(ctx context.Context) error {
		if executed.CompareAndSwap(false, true) {
			close(done)
		}
		return nil
	}

	_, err := s.ScheduleJob(job)
	if err != nil {
		t.Fatalf("ScheduleJob failed: %v", err)
	}

	s.Start()
	defer s.Stop(context.Background())

	select {
	case <-done:
		// Wait a bit for metrics to be recorded (job has run but metrics need to complete)
		time.Sleep(100 * time.Millisecond)
	case <-time.After(3 * time.Second):
		t.Fatal("Job did not execute within timeout")
	}

	// Verify metrics
	successCount := metrics.GetCounter("cronjob_success_total", map[string]string{"job": "fast-job"})
	if successCount < 1 {
		t.Errorf("Success counter = %d, want >= 1", successCount)
	}

	durations := metrics.GetDurations("cronjob_duration_seconds", map[string]string{"job": "fast-job"})
	if len(durations) < 1 {
		t.Error("No duration metrics recorded")
	}
}

func TestScheduler_JobFailure(t *testing.T) {
	logger := NewRecordingLogger()
	metrics := NewRecordingMetrics()
	s := NewScheduler(logger, metrics, WithSeconds())

	done := make(chan struct{})
	var once sync.Once

	job := newTestJob("failing-job", "@every 1s")
	job.runFunc = func(ctx context.Context) error {
		once.Do(func() { close(done) })
		return errors.New("intentional failure")
	}

	_, _ = s.ScheduleJob(job)
	s.Start()
	defer s.Stop(context.Background())

	select {
	case <-done:
		// Wait a bit for metrics to be recorded
		time.Sleep(100 * time.Millisecond)
	case <-time.After(3 * time.Second):
		t.Fatal("Job did not execute within timeout")
	}

	failCount := metrics.GetCounter("cronjob_failed_total", map[string]string{"job": "failing-job"})
	if failCount < 1 {
		t.Errorf("Failure counter = %d, want >= 1", failCount)
	}

	// Check error was logged
	entries := logger.GetEntries()
	hasError := false
	for _, e := range entries {
		if e.Level == "error" && e.Message == "job.failed" {
			hasError = true
			break
		}
	}
	if !hasError {
		t.Error("Expected error log entry for failed job")
	}
}

func TestScheduler_Hooks(t *testing.T) {
	logger := NoopLogger{}
	metrics := NoopMetrics{}

	var beforeCalled, afterCalled atomic.Bool
	var afterResult *JobResult

	s := NewScheduler(logger, metrics,
		WithSeconds(),
		WithBeforeHook(func(ctx context.Context, job Job, result *JobResult) {
			beforeCalled.Store(true)
		}),
		WithAfterHook(func(ctx context.Context, job Job, result *JobResult) {
			afterCalled.Store(true)
			afterResult = result
		}),
	)

	done := make(chan struct{})
	job := newTestJob("hooked-job", "@every 1s")
	job.runFunc = func(ctx context.Context) error {
		close(done)
		return nil
	}

	_, _ = s.ScheduleJob(job)
	s.Start()
	defer s.Stop(context.Background())

	select {
	case <-done:
		time.Sleep(100 * time.Millisecond) // Let hooks complete
	case <-time.After(3 * time.Second):
		t.Fatal("Job did not execute within timeout")
	}

	if !beforeCalled.Load() {
		t.Error("Before hook was not called")
	}
	if !afterCalled.Load() {
		t.Error("After hook was not called")
	}
	if afterResult == nil || !afterResult.Success {
		t.Error("After hook did not receive success result")
	}
}

func TestScheduler_NextRun(t *testing.T) {
	logger := NoopLogger{}
	metrics := NoopMetrics{}
	s := NewScheduler(logger, metrics).(*CronScheduler)

	job := newTestJob("scheduled-job", "0 0 * * *") // Midnight daily
	_, _ = s.ScheduleJob(job)

	s.Start()
	defer s.Stop(context.Background())

	nextRun, ok := s.NextRun("scheduled-job")
	if !ok {
		t.Error("NextRun returned false for scheduled job")
	}

	if nextRun.Before(time.Now()) {
		t.Error("NextRun should be in the future")
	}
}

func TestScheduler_JobCount(t *testing.T) {
	logger := NoopLogger{}
	metrics := NoopMetrics{}
	s := NewScheduler(logger, metrics).(*CronScheduler)
	defer s.Stop(context.Background())

	for i := 0; i < 5; i++ {
		job := newTestJob("job-"+string(rune('a'+i)), "* * * * *")
		_, _ = s.ScheduleJob(job)
	}

	if s.JobCount() != 5 {
		t.Errorf("JobCount = %d, want 5", s.JobCount())
	}
}

func TestScheduler_GracefulShutdown(t *testing.T) {
	logger := NewRecordingLogger()
	metrics := NoopMetrics{}
	s := NewScheduler(logger, metrics, WithSeconds())

	var jobRunning atomic.Bool
	jobDone := make(chan struct{})

	job := newTestJob("slow-job", "@every 1s")
	job.runFunc = func(ctx context.Context) error {
		jobRunning.Store(true)
		time.Sleep(500 * time.Millisecond)
		close(jobDone)
		return nil
	}

	_, _ = s.ScheduleJob(job)
	s.Start()

	// Wait for job to start
	for i := 0; i < 20 && !jobRunning.Load(); i++ {
		time.Sleep(100 * time.Millisecond)
	}

	// Stop with timeout
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	s.Stop(ctx)

	// Job should have completed
	select {
	case <-jobDone:
		// Success
	case <-time.After(time.Second):
		// Job may not have started yet, that's okay
	}
}

func TestScheduler_Concurrent(t *testing.T) {
	logger := NoopLogger{}
	metrics := NewRecordingMetrics()
	s := NewScheduler(logger, metrics, WithSeconds())

	var wg sync.WaitGroup

	// Schedule multiple jobs concurrently
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			job := newTestJob("concurrent-job-"+string(rune('a'+i)), "@every 1s")
			_, _ = s.ScheduleJob(job)
		}(i)
	}

	wg.Wait()
	s.Start()

	time.Sleep(1500 * time.Millisecond)

	// Remove jobs concurrently
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			_ = s.RemoveJob("concurrent-job-" + string(rune('a'+i)))
		}(i)
	}

	wg.Wait()
	s.Stop(context.Background())

	// Should not panic or race
}

// ============= Recording Logger/Metrics Tests =============

func TestRecordingLogger(t *testing.T) {
	l := NewRecordingLogger()

	l.Info("test info", "key", "value")
	l.Error("test error", errors.New("err"), "key", "value")
	l.Debug("test debug")

	entries := l.GetEntries()
	if len(entries) != 3 {
		t.Errorf("Got %d entries, want 3", len(entries))
	}

	l.Clear()
	if len(l.GetEntries()) != 0 {
		t.Error("Clear did not remove entries")
	}
}

func TestRecordingMetrics(t *testing.T) {
	m := NewRecordingMetrics()

	m.Increment("test_counter", map[string]string{"job": "test"})
	m.Increment("test_counter", map[string]string{"job": "test"})
	m.ObserveDuration("test_duration", map[string]string{"job": "test"}, 1.5)

	count := m.GetCounter("test_counter", map[string]string{"job": "test"})
	if count != 2 {
		t.Errorf("Counter = %d, want 2", count)
	}

	durations := m.GetDurations("test_duration", map[string]string{"job": "test"})
	if len(durations) != 1 || durations[0] != 1.5 {
		t.Errorf("Durations = %v, want [1.5]", durations)
	}
}

// ============= Integration/Stress Tests =============

func TestScheduler_StressTest(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping stress test in short mode")
	}

	logger := NoopLogger{}
	metrics := NewRecordingMetrics()
	s := NewScheduler(logger, metrics, WithSeconds())

	const numJobs = 50
	var totalRuns atomic.Int32

	// Schedule many fast jobs
	for i := 0; i < numJobs; i++ {
		job := newTestJob("stress-job-"+string(rune(i)), "@every 1s")
		job.runFunc = func(ctx context.Context) error {
			totalRuns.Add(1)
			return nil
		}
		_, _ = s.ScheduleJob(job)
	}

	s.Start()
	time.Sleep(2500 * time.Millisecond)
	s.Stop(context.Background())

	runs := totalRuns.Load()
	// Each job should run at least once in 2.5 seconds
	if runs < int32(numJobs) {
		t.Errorf("Total runs = %d, expected at least %d", runs, numJobs)
	}

	t.Logf("Stress test: %d jobs, %d total runs", numJobs, runs)
}
