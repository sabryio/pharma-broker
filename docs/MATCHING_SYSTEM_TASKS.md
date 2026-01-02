# Matching System Enhancement Tasks

> Comprehensive task list extracted from [MATCHING_SYSTEM_ENHANCEMENTS.md](./MATCHING_SYSTEM_ENHANCEMENTS.md)

---

## Priority Legend

| Priority | Timeline | Description                      |
| -------- | -------- | -------------------------------- |
| **P0**   | Week 1-2 | Critical for production          |
| **P1**   | Week 3-4 | High impact, should be done soon |
| **P2**   | Week 5-8 | Medium impact, nice to have      |
| **P3**   | Backlog  | Low priority, future enhancement |

---

## 1. Master Medication Database

> **Goal**: Create an authoritative medication reference database for deterministic matching.

### Tasks

- [x] **1.1** Create migration files for `master_medications`, `medication_aliases` tables
  - Priority: **P0** | Effort: 2h | Dependencies: None
  - Files: `core/crates/db/src/migration/m20251231_000013_create_medication_master.rs`, `m20251231_000014_create_medication_aliases.rs`
  - **STATUS: DONE** - Includes vector embeddings, trigram indexes, FTS indexes

- [x] **1.2** Create `MasterMedicationRepository` trait and SeaORM implementation
  - Priority: **P0** | Effort: 4h | Dependencies: 1.1
  - Files: `core/crates/db/src/traits.rs`, `core/crates/db/src/repo/master_medication_repo.rs`
  - **STATUS: DONE** - Includes fuzzy search, semantic search, CRUD operations

- [x] **1.3** Create `MedicationResolver` service with staged resolution (exact → alias → embedding)
  - Priority: **P0** | Effort: 6h | Dependencies: 1.2
  - File: `core/src/matching/medication_resolver.rs`
  - **STATUS: DONE** - 4-stage resolution: exact alias → exact name → fuzzy → semantic

- [x] **1.4** Seed initial medication data (Egyptian market focus)
  - Priority: **P1** | Effort: 8h | Dependencies: 1.2
  - Files: `data/master_medications_seed.json`, `core/src/bin/seed_medications.rs`
  - **STATUS: DONE** - Created seed file with 50+ common Egyptian pharmacy medications including antibiotics, cardiovascular, diabetes, GI, and respiratory medications. Created seeder binary with dry-run, clear, and skip-existing options.

---

## 2. Temperature & Sampling Controls

> **Goal**: Configure AI model parameters to reduce randomness and hallucination.

### Tasks

- [x] **2.1** Create `AIModelConfig` structure with context-specific presets
  - Priority: **P0** | Effort: 2h | Dependencies: None
  - File: `core/crates/ai-client/src/client.rs`
  - **STATUS: DONE** - Added `AIContext` enum with `Parsing`, `Comparison`, `General` presets. Each context has optimized temperature, top_p, and max_tokens.

- [x] **2.2** Update AI client to support context-specific configurations
  - Priority: **P0** | Effort: 3h | Dependencies: 2.1
  - File: `core/crates/ai-client/src/client.rs`
  - **STATUS: DONE** - Added `generate_object_with_context()` method that uses context-specific temperature and sampling parameters.

- [x] **2.3** Update `PharmaParser` and `AIReviewer` to use temperature configs
  - Priority: **P0** | Effort: 2h | Dependencies: 2.2
  - Files: `core/src/ai/pharma_parser.rs`, `core/src/matching/reviewer.rs`
  - **STATUS: DONE** - PharmaParser uses `AIContext::Parsing` (temp 0.2), AIReviewer uses `AIContext::Comparison` (temp 0.0).

---

## 3. Multi-Model Consensus Audit

> **Goal**: Use multiple AI models to audit matches and require agreement for high-confidence decisions.

### Tasks

- [x] **3.1** Create `ConsensusAuditor` with parallel multi-model execution
  - Priority: **P1** | Effort: 6h | Dependencies: 2.3
  - File: `core/src/matching/consensus_auditor.rs`
  - **STATUS: DONE** - Created `ConsensusAuditor` with parallel multi-model execution, configurable agreement threshold (default 67%), unanimous mode option, detailed consensus statistics, and model failure handling.

- [x] **3.2** Create `AuditorFactory` for multi-model configuration
  - Priority: **P1** | Effort: 3h | Dependencies: 3.1
  - File: `core/src/matching/auditor_factory.rs`
  - **STATUS: DONE** - Created `AuditorFactory` with `ModelConfig` builder, `AuditorType` enum (Single/Consensus/Hybrid), production/development presets, and `HybridAuditor` for score-based auditor selection.

---

## 4. Automated Alias Learning

> **Goal**: Automatically learn new medication aliases from operator feedback.

### Tasks

- [x] **4.1** Create `AliasLearner` service with confirmation tracking
  - Priority: **P1** | Effort: 6h | Dependencies: 1.2
  - File: `core/src/matching/alias_learner.rs`
  - **STATUS: DONE** - Created `AliasLearner` with configurable confirmation threshold, pending alias tracking, conflict detection, and statistics.

- [x] **4.2** Integrate alias learning into match confirmation/rejection flow
  - Priority: **P1** | Effort: 2h | Dependencies: 4.1
  - Files: `core/src/api/match_reviews.rs`, `core/src/api/routes.rs`, `core/src/main.rs`
  - **STATUS: DONE** - Integrated AliasLearner into AppState, calls learn_from_confirmation() on match confirmation and learn_from_rejection() on rejection. Persists learned aliases to database when confirmation threshold is met. 2. Call `alias_learner.learn_from_confirmation()` in `update_match_review_status` when confirmed 3. Call `alias_learner.learn_from_rejection()` when rejected 4. Persist learned aliases to database via `MedicationAliasRepository`

---

## 5. Hierarchical Matching Pipeline

> **Goal**: Replace single-pass matching with a staged approach that progressively narrows candidates.

### Tasks

- [x] **5.1** Create `HierarchicalMatcher` with 5-stage pipeline
  - Priority: **P0** | Effort: 8h | Dependencies: 1.3
  - File: `core/src/matching/hierarchical_matcher.rs`
  - **STATUS: DONE** - Created types, config, stats tracking for 5-stage pipeline (ExactMatch → AliasMatch → FTSMatch → EmbeddingMatch → FuzzyValidated). Full matcher implementation can be integrated with existing MatchingEngine.
  - Stages:
    1. Exact Match (master_medication_id)
    2. Alias Lookup (O(1))
    3. FTS + Trigram (O(log n))
    4. Embedding Similarity
    5. Fuzzy + Raw Validation

---

## 6. Confidence Calibration (Platt Scaling)

> **Goal**: Calibrate raw match scores to true probabilities using historical feedback.

### Tasks

- [x] **6.1** Create `PlattCalibrator` with gradient descent fitting
  - Priority: **P2** | Effort: 4h | Dependencies: None
  - File: `core/src/matching/calibration.rs`
  - **STATUS: DONE (HISTOGRAM BINNING)** - Implemented as `ConfidenceCalibrator` using histogram binning approach instead of Platt scaling. Features: ECE/MCE calculation, online learning, bin-based calibration, smoothing factor, statistics tracking.

---

## 7. Contrastive Validation

> **Goal**: Detect false positives by ensuring match score is significantly higher than random alternatives.

### Tasks

- [x] **7.1** Create `ContrastiveValidator` with negative sampling
  - Priority: **P2** | Effort: 4h | Dependencies: None
  - File: `core/src/matching/contrastive_validator.rs`
  - **STATUS: DONE** - Created `ContrastiveValidator` with configurable negative sampling (default 3), margin thresholds (15% vs avg, 10% vs max), embedding-based validation support, and detailed statistics tracking.

---

## 8. Uncertainty Quantification

> **Goal**: Estimate prediction uncertainty using Monte Carlo dropout or ensemble variance.

### Tasks

- [ ] **8.1** Create `UncertaintyEstimator` with weight perturbation
  - Priority: **P3** | Effort: 6h | Dependencies: None
  - File: `core/src/matching/uncertainty_estimator.rs`
  - Features: Mean score, std dev, confidence intervals

---

## 9. Circuit Breaker with Fallback

> **Goal**: Graceful degradation to deterministic matching when AI is unavailable.

### Tasks

- [x] **9.1** Enhance `CircuitBreaker` with `FallbackStrategy` enum
  - Priority: **P1** | Effort: 3h | Dependencies: None
  - File: `core/src/ai/circuit_breaker.rs`
  - **STATUS: DONE** - Added `FallbackStrategy` enum with 4 strategies: DeterministicOnly, CachedEmbeddings, QueueForLater, RejectAll. Added `should_match()`, `allow_ai_call()`, `fallback_strategy()` methods.

- [x] **9.2** Create `FallbackMatcher` for AI-free matching
  - Priority: **P1** | Effort: 4h | Dependencies: 9.1, 1.3
  - File: `core/src/matching/fallback_matcher.rs`
  - **STATUS: DONE** - Created `FallbackMatcher` with deterministic matching using MedicationResolver. Supports multiple strategies: SameMaster (both resolve to same master), ExactMatch, FuzzyMatch, CachedEmbedding. Integrates with FallbackStrategy from circuit breaker.

---

## 10. Audit Trail with Reproducibility

> **Goal**: Store complete snapshots of all inputs and parameters for debugging.

### Tasks

- [ ] **10.1** Create `match_audit_records` table migration
  - Priority: **P2** | Effort: 3h | Dependencies: None
  - File: `migrations/YYYYMMDD_create_match_audit_records.sql`
  - Fields: offer_snapshot, request_snapshot, weights_snapshot, score_breakdown, pipeline_version

- [ ] **10.2** Create `AuditRecorder` service with replay capability
  - Priority: **P2** | Effort: 4h | Dependencies: 10.1
  - File: `core/src/matching/audit_recorder.rs`

---

## 11. A/B Test Auto-Rollback

> **Goal**: Automatically rollback A/B tests that show degraded performance.

### Tasks

- [ ] **11.1** Enhance `ABTestManager` with auto-rollback and auto-promote logic
  - Priority: **P2** | Effort: 4h | Dependencies: None
  - File: `core/src/matching/abtest.rs`
  - **STATUS: PARTIAL** - A/B test manager exists with statistical significance testing. Missing: auto-rollback on degraded performance, auto-promote on success.
  - Config: `min_samples=50`, `max_rejection_rate_increase=1.2`, `significance_threshold=0.05`

---

## Implementation Schedule

### Week 1-2 (P0 - Foundation) - STATUS: COMPLETE ✅

```
├── ✅ 1.1 Master Medication DB Migration (DONE)
├── ✅ 1.2 Master Medication Repository (DONE)
├── ✅ 2.1 AI Config Structure (DONE - AIContext enum)
├── ✅ 2.2 Update AI Client (DONE - generate_object_with_context)
├── ✅ 2.3 Update Parser & Reviewer (DONE - using context-specific temps)
├── ✅ 1.3 Medication Resolver Service (DONE)
└── ✅ 5.1 Hierarchical Matching Pipeline (DONE - types, config, stats)
```

**Remaining Effort**: 0 hours - All P0 tasks complete!

### Week 3-4 (P1 - Core Enhancements) - STATUS: COMPLETE ✅

```
├── ✅ 3.1 Consensus Auditor (DONE)
├── ✅ 3.2 Multi-Model Factory (DONE)
├── ✅ 4.1 Alias Learner (DONE)
├── ✅ 4.2 Alias Learning Integration (DONE)
├── ✅ 9.1 Circuit Breaker Fallback (DONE - FallbackStrategy enum)
├── ✅ 9.2 Fallback Matcher (DONE)
└── ✅ 1.4 Seed Initial Data (DONE)
```

**Remaining Effort**: 0 hours - All P1 tasks complete!

### Week 5-8 (P2 - Optimization) - STATUS: IN PROGRESS

```
├── ✅ 6.1 Platt Scaling Calibrator (DONE - histogram binning)
├── ✅ 7.1 Contrastive Validator (DONE)
├── ⏳ 10.1 Audit Trail Schema (TODO)
├── ⏳ 10.2 Audit Recorder Service (TODO)
└── ⚠️ 11.1 A/B Auto-Rollback (PARTIAL - needs auto-rollback logic)
```

**Remaining Effort**: ~11 hours

### Backlog (P3 - Future)

```
└── ⏳ 8.1 Uncertainty Estimation (TODO)
```

**Total Effort**: ~6 hours

---

## Success Metrics

| Metric                   | Current | Target    | Measurement                      |
| ------------------------ | ------- | --------- | -------------------------------- |
| False Positive Rate      | ~5%     | <1%       | Rejected matches / Total matches |
| Hallucination Detection  | Manual  | Automated | Contrastive validation failures  |
| Deterministic Match Rate | ~10%    | >50%      | Exact + Alias matches / Total    |
| AI Dependency            | 100%    | <50%      | Matches requiring AI / Total     |
| Mean Time to Match       | ~500ms  | <100ms    | P95 latency                      |
| Operator Rejection Rate  | ~15%    | <5%       | Manual rejections / Suggested    |

---

## Environment Variables Reference

```bash
# Master Medication Database
MASTER_MED_EMBEDDING_THRESHOLD=0.85
MASTER_MED_MAX_CANDIDATES=10

# AI Configuration
AI_PARSING_TEMPERATURE=0.2
AI_COMPARISON_TEMPERATURE=0.0

# Consensus Audit
CONSENSUS_MIN_AGREEMENT=0.67
CONSENSUS_REQUIRE_UNANIMOUS=true

# Alias Learning
ALIAS_LEARNING_ENABLED=true
ALIAS_MIN_CONFIRMATIONS=2

# Hierarchical Matching
HIERARCHICAL_FTS_MIN_SCORE=0.3
HIERARCHICAL_EMBEDDING_MIN_SIM=0.7

# Circuit Breaker
CIRCUIT_FALLBACK_STRATEGY=deterministic_only

# A/B Testing
ABTEST_AUTO_ROLLBACK_ENABLED=true
```

---

**Total Tasks**: 21  
**Total Estimated Effort**: ~84 hours (~10-11 days)
