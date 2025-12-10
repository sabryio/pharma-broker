package cronjob

import "sync"

// Registry keeps track of registered jobs. Thread-safe and extensible.
type Registry struct {
	mu   sync.RWMutex
	jobs map[string]Job
}

// NewRegistry creates a new job registry.
func NewRegistry() *Registry {
	return &Registry{jobs: make(map[string]Job)}
}

// Register adds a job to the registry.
func (r *Registry) Register(job Job) error {
	if job == nil {
		return nil
	}
	r.mu.Lock()
	defer r.mu.Unlock()
	r.jobs[job.ID()] = job
	return nil
}

// Unregister removes a job from the registry.
func (r *Registry) Unregister(id string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	delete(r.jobs, id)
}

// Get retrieves a job by ID.
func (r *Registry) Get(id string) (Job, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	j, ok := r.jobs[id]
	return j, ok
}

// List returns all registered jobs.
func (r *Registry) List() []Job {
	r.mu.RLock()
	defer r.mu.RUnlock()
	out := make([]Job, 0, len(r.jobs))
	for _, j := range r.jobs {
		out = append(out, j)
	}
	return out
}

// Count returns the number of registered jobs.
func (r *Registry) Count() int {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return len(r.jobs)
}

// Clear removes all jobs from the registry.
func (r *Registry) Clear() {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.jobs = make(map[string]Job)
}

// IDs returns all registered job IDs.
func (r *Registry) IDs() []string {
	r.mu.RLock()
	defer r.mu.RUnlock()
	ids := make([]string, 0, len(r.jobs))
	for id := range r.jobs {
		ids = append(ids, id)
	}
	return ids
}
