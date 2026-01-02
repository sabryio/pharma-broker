# Matching System Technical Enhancements

## Production-Ready Implementation Roadmap

This document outlines comprehensive technical enhancements to improve matching accuracy, reduce AI hallucinations, and ensure production reliability.

---

## Table of Contents

1. [Master Medication Database](#1-master-medication-database)
2. [Temperature & Sampling Controls](#2-temperature--sampling-controls)
3. [Multi-Model Consensus Audit](#3-multi-model-consensus-audit)
4. [Automated Alias Learning](#4-automated-alias-learning)
5. [Hierarchical Matching Pipeline](#5-hierarchical-matching-pipeline)
6. [Confidence Calibration (Platt Scaling)](#6-confidence-calibration-platt-scaling)
7. [Contrastive Validation](#7-contrastive-validation)
8. [Uncertainty Quantification](#8-uncertainty-quantification)
9. [Circuit Breaker with Fallback](#9-circuit-breaker-with-fallback)
10. [Audit Trail with Reproducibility](#10-audit-trail-with-reproducibility)
11. [A/B Test Auto-Rollback](#11-ab-test-auto-rollback)
12. [Implementation Priority Matrix](#12-implementation-priority-matrix)

---

## 1. Master Medication Database

### Overview

Create an authoritative medication reference database that enables deterministic matching and eliminates AI dependency for known medications.

### Database Schema

```sql
-- Core master medication table
CREATE TABLE master_medications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    canonical_name VARCHAR(255) NOT NULL UNIQUE,
    canonical_name_ar VARCHAR(255),
    active_ingredient VARCHAR(255),
    active_ingredient_ar VARCHAR(255),
    atc_code VARCHAR(10),           -- WHO ATC classification
    therapeutic_class VARCHAR(100),
    manufacturer VARCHAR(255),
    country_of_origin VARCHAR(50),
    embedding VECTOR(384),          -- Pre-computed embedding
    is_verified BOOLEAN DEFAULT FALSE,
    verified_by UUID,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);


-- Medication aliases (brand names, generics, misspellings, transliterations)
CREATE TABLE medication_aliases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    master_id UUID NOT NULL REFERENCES master_medications(id) ON DELETE CASCADE,
    alias VARCHAR(255) NOT NULL,
    alias_normalized VARCHAR(255) NOT NULL, -- Lowercase, no diacritics
    alias_type VARCHAR(50) NOT NULL, -- 'brand', 'generic', 'misspelling', 'transliteration', 'learned'
    language VARCHAR(10) DEFAULT 'en', -- 'en', 'ar'
    confidence FLOAT DEFAULT 1.0, -- 1.0 for verified, lower for learned
    source VARCHAR(50) DEFAULT 'manual', -- 'manual', 'ai_learned', 'import'
    learned_from_match_id UUID, -- Reference to match that taught this alias
    usage_count INTEGER DEFAULT 0, -- How often this alias is matched
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(alias_normalized)
);

-- Indexes for fast lookup
CREATE INDEX idx_aliases_normalized ON medication_aliases(alias_normalized);
CREATE INDEX idx_aliases_master_id ON medication_aliases(master_id);
CREATE INDEX idx_master_meds_canonical ON master_medications(canonical_name);
CREATE INDEX idx_master_meds_embedding ON master_medications USING ivfflat (embedding vector_cosine_ops);

-- Dosage forms reference
CREATE TABLE dosage_forms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    master_id UUID NOT NULL REFERENCES master_medications(id) ON DELETE CASCADE,
    strength VARCHAR(50) NOT NULL, -- '500mg', '1g', '250mg/5ml'
    strength_normalized FLOAT, -- Normalized to base unit (mg)
    form VARCHAR(50), -- 'tablet', 'capsule', 'syrup', 'injection'
    package_size INTEGER, -- Number of units per package
    barcode VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

```

### Implementation Tasks

#### Task 1.1: Create Migration Files

- **File**: `migrations/YYYYMMDD_create_master_medications.sql`
- **Priority**: HIGH
- **Effort**: 2 hours

```rust
// core/crates/db/src/entity/master_medication.rs
use sea_orm::entity::prelude::*;
use pgvector::Vector;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "master_medications")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub canonical_name: String,
    pub canonical_name_ar: Option<String>,
    pub active_ingredient: Option<String>,
    pub atc_code: Option<String>,
    pub therapeutic_class: Option<String>,
    pub embedding: Option<Vector>,
    pub is_verified: bool,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}
```

#### Task 1.2: Create Repository Trait and Implementation

- **File**: `core/crates/db/src/repo/master_medication_repo.rs`
- **Priority**: HIGH
- **Effort**: 4 hours

```rust
// core/crates/db/src/traits.rs
#[async_trait]
pub trait MasterMedicationRepository: Send + Sync {
    /// Find by exact canonical name
    async fn find_by_name(&self, name: &str) -> Result<Option<MasterMedication>>;

    /// Find by alias (exact match on normalized alias)
    async fn find_by_alias(&self, alias: &str) -> Result<Option<MasterMedication>>;

    /// Find by embedding similarity (top-k)
    async fn find_by_embedding(&self, embedding: &[f32], limit: i64) -> Result<Vec<(MasterMedication, f64)>>;

    /// Add new master medication
    async fn save(&self, medication: &MasterMedication) -> Result<MasterMedication>;

    /// Add alias to existing medication
    async fn add_alias(&self, master_id: Uuid, alias: &str, alias_type: &str, confidence: f64) -> Result<MedicationAlias>;

    /// Increment alias usage count
    async fn increment_alias_usage(&self, alias_id: Uuid) -> Result<()>;

    /// Get all aliases for a medication
    async fn get_aliases(&self, master_id: Uuid) -> Result<Vec<MedicationAlias>>;

    /// Search medications (FTS + trigram)
    async fn search(&self, query: &str, limit: i64) -> Result<Vec<MasterMedication>>;
}
```

#### Task 1.3: Create Medication Resolver Service

- **File**: `core/src/matching/medication_resolver.rs` (enhance existing)
- **Priority**: HIGH
- **Effort**: 6 hours

```rust
// core/src/matching/medication_resolver.rs
pub struct MedicationResolver {
    master_repo: Arc<dyn MasterMedicationRepository>,
    embedding_cache: Arc<EmbeddingCache>,
    config: ResolverConfig,
}

#[derive(Debug, Clone)]
pub struct ResolverConfig {
    pub enable_exact_match: bool,
    pub enable_alias_lookup: bool,
    pub enable_embedding_search: bool,
    pub embedding_threshold: f64,
    pub max_candidates: i64,
}

#[derive(Debug, Clone)]
pub struct ResolutionResult {
    pub master_id: Option<Uuid>,
    pub canonical_name: Option<String>,
    pub method: ResolutionMethod,
    pub confidence: f64,
    pub alias_used: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResolutionMethod {
    ExactMatch,
    AliasMatch,
    EmbeddingMatch,
    NotFound,
}

impl MedicationResolver {
    /// Resolve medication name to master medication
    pub async fn resolve(&self, medication_name: &str) -> Result<ResolutionResult> {
        let normalized = normalize_for_matching(medication_name);

        // Stage 1: Exact canonical name match
        if self.config.enable_exact_match {
            if let Some(master) = self.master_repo.find_by_name(&normalized).await? {
                return Ok(ResolutionResult {
                    master_id: Some(master.id),
                    canonical_name: Some(master.canonical_name),
                    method: ResolutionMethod::ExactMatch,
                    confidence: 1.0,
                    alias_used: None,
                });
            }
        }

        // Stage 2: Alias lookup
        if self.config.enable_alias_lookup {
            if let Some(master) = self.master_repo.find_by_alias(&normalized).await? {
                return Ok(ResolutionResult {
                    master_id: Some(master.id),
                    canonical_name: Some(master.canonical_name),
                    method: ResolutionMethod::AliasMatch,
                    confidence: 0.95,
                    alias_used: Some(normalized.clone()),
                });
            }
        }

        // Stage 3: Embedding similarity search
        if self.config.enable_embedding_search {
            if let Some(embedding) = self.embedding_cache.get(&normalized).await {
                let candidates = self.master_repo
                    .find_by_embedding(&embedding, self.config.max_candidates)
                    .await?;

                if let Some((master, similarity)) = candidates.first() {
                    if *similarity >= self.config.embedding_threshold {
                        return Ok(ResolutionResult {
                            master_id: Some(master.id),
                            canonical_name: Some(master.canonical_name.clone()),
                            method: ResolutionMethod::EmbeddingMatch,
                            confidence: *similarity,
                            alias_used: None,
                        });
                    }
                }
            }
        }

        Ok(ResolutionResult {
            master_id: None,
            canonical_name: None,
            method: ResolutionMethod::NotFound,
            confidence: 0.0,
            alias_used: None,
        })
    }
}
```

#### Task 1.4: Seed Initial Data

- **File**: `data/master_medications_seed.json`
- **Priority**: MEDIUM
- **Effort**: 8 hours (data collection)

```json
{
  "medications": [
    {
      "canonical_name": "Augmentin",
      "canonical_name_ar": "أوجمنتين",
      "active_ingredient": "Amoxicillin/Clavulanic Acid",
      "atc_code": "J01CR02",
      "therapeutic_class": "Antibiotic",
      "aliases": [
        { "alias": "اوجمنتين", "type": "transliteration", "language": "ar" },
        { "alias": "Augmentin", "type": "brand", "language": "en" },
        { "alias": "Amoxiclav", "type": "generic", "language": "en" },
        { "alias": "اموكسيكلاف", "type": "generic", "language": "ar" }
      ],
      "dosage_forms": [
        { "strength": "1g", "form": "tablet" },
        { "strength": "625mg", "form": "tablet" },
        { "strength": "457mg/5ml", "form": "syrup" }
      ]
    }
  ]
}
```

---

## 2. Temperature & Sampling Controls

### Overview

Configure AI model parameters to reduce randomness and hallucination in different contexts.

### Implementation Tasks

#### Task 2.1: Create AI Config Structure

- **File**: `core/src/ai/config.rs`
- **Priority**: HIGH
- **Effort**: 2 hours

```rust
// core/src/ai/config.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIModelConfig {
    /// Model identifier
    pub model: String,

    /// Temperature (0.0 = deterministic, 1.0 = creative)
    pub temperature: f32,

    /// Top-p nucleus sampling (0.0-1.0)
    pub top_p: f32,

    /// Top-k sampling (number of tokens to consider)
    pub top_k: Option<u32>,

    /// Maximum tokens in response
    pub max_tokens: u32,

    /// Frequency penalty to reduce repetition
    pub frequency_penalty: f32,

    /// Presence penalty to encourage new topics
    pub presence_penalty: f32,

    /// Stop sequences
    pub stop_sequences: Vec<String>,
}

impl AIModelConfig {
    /// Config for parsing (low temperature, deterministic)
    pub fn for_parsing() -> Self {
        Self {
            model: "gpt-4o-mini".to_string(),
            temperature: 0.2,
            top_p: 0.9,
            top_k: Some(40),
            max_tokens: 2000,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            stop_sequences: vec![],
        }
    }

    /// Config for name comparison (zero temperature, exact)
    pub fn for_name_comparison() -> Self {
        Self {
            model: "gpt-4o-mini".to_string(),
            temperature: 0.0,
            top_p: 1.0,
            top_k: Some(1),
            max_tokens: 500,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            stop_sequences: vec![],
        }
    }

    /// Config for embedding generation
    pub fn for_embedding() -> Self {
        Self {
            model: "text-embedding-3-small".to_string(),
            temperature: 0.0,
            top_p: 1.0,
            top_k: None,
            max_tokens: 0,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            stop_sequences: vec![],
        }
    }

    /// Load from environment with context-specific defaults
    pub fn from_env(context: &str) -> Self {
        let base = match context {
            "parsing" => Self::for_parsing(),
            "comparison" => Self::for_name_comparison(),
            "embedding" => Self::for_embedding(),
            _ => Self::for_parsing(),
        };

        Self {
            temperature: std::env::var(format!("AI_{}_TEMPERATURE", context.to_uppercase()))
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(base.temperature),
            top_p: std::env::var(format!("AI_{}_TOP_P", context.to_uppercase()))
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(base.top_p),
            ..base
        }
    }
}
```

#### Task 2.2: Update AI Client to Use Context-Specific Config

- **File**: `core/crates/ai-client/src/lib.rs`
- **Priority**: HIGH
- **Effort**: 3 hours

```rust
// core/crates/ai-client/src/lib.rs
impl Client {
    /// Generate with context-specific configuration
    pub async fn generate_with_config<T: DeserializeOwned + JsonSchema>(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        config: &AIModelConfig,
    ) -> Result<T, Error> {
        let request = ChatCompletionRequest {
            model: config.model.clone(),
            messages: vec![
                Message::system(system_prompt),
                Message::user(user_prompt),
            ],
            temperature: Some(config.temperature),
            top_p: Some(config.top_p),
            max_tokens: Some(config.max_tokens),
            frequency_penalty: Some(config.frequency_penalty),
            presence_penalty: Some(config.presence_penalty),
            response_format: Some(ResponseFormat::JsonSchema {
                schema: schemars::schema_for!(T),
            }),
            ..Default::default()
        };

        self.execute_request(request).await
    }
}
```

#### Task 2.3: Update Parser and Reviewer to Use Configs

- **File**: `core/src/ai/pharma_parser.rs`, `core/src/matching/reviewer.rs`
- **Priority**: HIGH
- **Effort**: 2 hours

```rust
// core/src/ai/pharma_parser.rs
impl PharmaParser {
    pub async fn parse(&self, content: &str, ...) -> Result<Vec<ParsedItem>, Error> {
        let config = AIModelConfig::for_parsing();
        self.client.generate_with_config(
            &self.system_prompt,
            &user_prompt,
            &config,
        ).await
    }
}

// core/src/matching/reviewer.rs
impl AIReviewer {
    pub async fn audit_match(&self, ...) -> Result<ReviewResult, Error> {
        let config = AIModelConfig::for_name_comparison();
        self.client.generate_with_config(
            &system_prompt,
            &user_prompt,
            &config,
        ).await
    }
}
```

---

## 3. Multi-Model Consensus Audit

### Overview

Use multiple AI models to audit matches and require agreement for high-confidence decisions.

### Implementation Tasks

#### Task 3.1: Create Consensus Auditor

- **File**: `core/src/matching/consensus_auditor.rs`
- **Priority**: HIGH
- **Effort**: 6 hours

```rust
// core/src/matching/consensus_auditor.rs
use futures::future::join_all;
use std::sync::Arc;

pub struct ConsensusAuditor {
    auditors: Vec<Arc<AIReviewer>>,
    config: ConsensusConfig,
}

#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Minimum agreement ratio for approval (e.g., 0.67 = 2/3)
    pub min_agreement_ratio: f64,

    /// Require unanimous agreement for auto-confirm
    pub require_unanimous_for_auto: bool,

    /// Timeout for each auditor
    pub timeout_ms: u64,

    /// Enable parallel execution
    pub parallel: bool,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            min_agreement_ratio: 0.67,
            require_unanimous_for_auto: true,
            timeout_ms: 5000,
            parallel: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConsensusResult {
    pub final_status: ReviewStatus,
    pub final_confidence: f64,
    pub explanation: String,
    pub individual_results: Vec<ReviewResult>,
    pub agreement_ratio: f64,
    pub is_unanimous: bool,
}

impl ConsensusAuditor {
    pub fn new(auditors: Vec<Arc<AIReviewer>>, config: ConsensusConfig) -> Self {
        Self { auditors, config }
    }

    /// Audit match with multiple models
    pub async fn audit_match(
        &self,
        offer: &Offer,
        request: &Request,
        score: f64,
        reasoning: &str,
    ) -> Result<ConsensusResult, Error> {
        let results = if self.config.parallel {
            self.audit_parallel(offer, request, score, reasoning).await?
        } else {
            self.audit_sequential(offer, request, score, reasoning).await?
        };

        self.aggregate_results(results)
    }

    async fn audit_parallel(
        &self,
        offer: &Offer,
        request: &Request,
        score: f64,
        reasoning: &str,
    ) -> Result<Vec<ReviewResult>, Error> {
        let futures: Vec<_> = self.auditors.iter()
            .map(|auditor| {
                let offer = offer.clone();
                let request = request.clone();
                let reasoning = reasoning.to_string();
                let auditor = auditor.clone();
                async move {
                    tokio::time::timeout(
                        Duration::from_millis(self.config.timeout_ms),
                        auditor.audit_match(&offer, &request, score, &reasoning)
                    ).await
                }
            })
            .collect();

        let results = join_all(futures).await;

        results.into_iter()
            .filter_map(|r| r.ok().and_then(|r| r.ok()))
            .collect()
    }

    fn aggregate_results(&self, results: Vec<ReviewResult>) -> Result<ConsensusResult, Error> {
        if results.is_empty() {
            return Err(Error::NoAuditResults);
        }

        let total = results.len();
        let approved_count = results.iter()
            .filter(|r| r.status == ReviewStatus::Approved)
            .count();
        let rejected_count = results.iter()
            .filter(|r| r.status == ReviewStatus::Rejected)
            .count();

        let agreement_ratio = approved_count.max(rejected_count) as f64 / total as f64;
        let is_unanimous = approved_count == total || rejected_count == total;

        // Determine final status
        let final_status = if agreement_ratio >= self.config.min_agreement_ratio {
            if approved_count > rejected_count {
                ReviewStatus::Approved
            } else {
                ReviewStatus::Rejected
            }
        } else {
            ReviewStatus::Flagged  // No consensus
        };

        // Average confidence
        let final_confidence = results.iter()
            .map(|r| r.confidence as f64)
            .sum::<f64>() / total as f64;

        // Combine explanations
        let explanation = results.iter()
            .enumerate()
            .map(|(i, r)| format!("Model {}: {} - {}", i + 1, r.status, r.explanation))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ConsensusResult {
            final_status,
            final_confidence: final_confidence as f32,
            explanation,
            individual_results: results,
            agreement_ratio,
            is_unanimous,
        })
    }
}
```

#### Task 3.2: Create Multi-Model Factory

- **File**: `core/src/matching/auditor_factory.rs`
- **Priority**: MEDIUM
- **Effort**: 3 hours

```rust
// core/src/matching/auditor_factory.rs
pub struct AuditorFactory;

impl AuditorFactory {
    /// Create auditors for different models
    pub fn create_consensus_auditor(config: &AppConfig) -> ConsensusAuditor {
        let mut auditors = Vec::new();

        // Primary model (e.g., GPT-4o-mini)
        if let Some(primary_url) = &config.ai_primary_url {
            let client = AIClient::new(primary_url, &config.ai_primary_key);
            auditors.push(Arc::new(AIReviewer::new(Arc::new(client))));
        }

        // Secondary model (e.g., Claude)
        if let Some(secondary_url) = &config.ai_secondary_url {
            let client = AIClient::new(secondary_url, &config.ai_secondary_key);
            auditors.push(Arc::new(AIReviewer::new(Arc::new(client))));
        }

        // Local model (e.g., Ollama)
        if let Some(local_url) = &config.ai_local_url {
            let client = AIClient::new(local_url, "");
            auditors.push(Arc::new(AIReviewer::new(Arc::new(client))));
        }

        ConsensusAuditor::new(auditors, ConsensusConfig::default())
    }
}
```

---

## 4. Automated Alias Learning

### Overview

Automatically learn new medication aliases from operator feedback to improve future matching.

### Implementation Tasks

#### Task 4.1: Create Alias Learner Service

- **File**: `core/src/matching/alias_learner.rs`
- **Priority**: HIGH
- **Effort**: 6 hours

```rust
// core/src/matching/alias_learner.rs
pub struct AliasLearner {
    master_repo: Arc<dyn MasterMedicationRepository>,
    config: AliasLearnerConfig,
    stats: RwLock<AliasLearnerStats>,
}

#[derive(Debug, Clone)]
pub struct AliasLearnerConfig {
    /// Minimum match score to learn from
    pub min_score_threshold: f64,

    /// Minimum confidence for learned alias
    pub learned_alias_confidence: f64,

    /// Require N confirmations before learning
    pub min_confirmations: u32,

    /// Enable automatic learning
    pub enabled: bool,
}

impl Default for AliasLearnerConfig {
    fn default() -> Self {
        Self {
            min_score_threshold: 0.85,
            learned_alias_confidence: 0.90,
            min_confirmations: 2,
            enabled: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct AliasLearnerStats {
    pub aliases_learned: u64,
    pub aliases_rejected: u64,
    pub pending_confirmations: HashMap<String, u32>,
}

impl AliasLearner {
    /// Learn from confirmed match
    pub async fn learn_from_confirmation(
        &self,
        match_entity: &Match,
        offer: &Offer,
        request: &Request,
    ) -> Result<Option<MedicationAlias>, Error> {
        if !self.config.enabled {
            return Ok(None);
        }

        // Only learn from high-confidence confirmations
        if match_entity.score < self.config.min_score_threshold {
            return Ok(None);
        }

        // Check if offer medication is already known
        let offer_resolution = self.resolve_medication(&offer.medication).await?;
        let request_resolution = self.resolve_medication(&request.medication).await?;

        // Learn new alias if one side is known and other is not
        match (offer_resolution.master_id, request_resolution.master_id) {
            (Some(master_id), None) => {
                // Request medication is unknown, learn it as alias
                self.maybe_learn_alias(
                    master_id,
                    &request.medication,
                    &request.medication_raw,
                    match_entity.id,
                ).await
            }
            (None, Some(master_id)) => {
                // Offer medication is unknown, learn it as alias
                self.maybe_learn_alias(
                    master_id,
                    &offer.medication,
                    &offer.medication_raw,
                    match_entity.id,
                ).await
            }
            _ => Ok(None),
        }
    }

    async fn maybe_learn_alias(
        &self,
        master_id: Uuid,
        medication: &str,
        medication_raw: &str,
        match_id: Uuid,
    ) -> Result<Option<MedicationAlias>, Error> {
        let normalized = normalize_for_matching(medication);

        // Check pending confirmations
        let mut stats = self.stats.write().await;
        let count = stats.pending_confirmations
            .entry(normalized.clone())
            .or_insert(0);
        *count += 1;

        if *count < self.config.min_confirmations {
            tracing::info!(
                medication = %medication,
                confirmations = %count,
                required = %self.config.min_confirmations,
                "Alias pending more confirmations"
            );
            return Ok(None);
        }

        // Enough confirmations, learn the alias
        stats.pending_confirmations.remove(&normalized);
        stats.aliases_learned += 1;

        // Determine alias type
        let alias_type = if is_arabic(medication) {
            "transliteration"
        } else {
            "learned"
        };

        let alias = self.master_repo.add_alias(
            master_id,
            &normalized,
            alias_type,
            self.config.learned_alias_confidence,
        ).await?;

        tracing::info!(
            master_id = %master_id,
            alias = %normalized,
            alias_type = %alias_type,
            match_id = %match_id,
            "Learned new medication alias"
        );

        Ok(Some(alias))
    }

    /// Learn from rejection (negative example)
    pub async fn learn_from_rejection(
        &self,
        match_entity: &Match,
        offer: &Offer,
        request: &Request,
    ) -> Result<(), Error> {
        // If both medications resolved to same master, this is a false positive
        // We should NOT learn this as an alias
        let offer_resolution = self.resolve_medication(&offer.medication).await?;
        let request_resolution = self.resolve_medication(&request.medication).await?;

        if offer_resolution.master_id == request_resolution.master_id
            && offer_resolution.master_id.is_some()
        {
            tracing::warn!(
                offer_med = %offer.medication,
                request_med = %request.medication,
                master_id = ?offer_resolution.master_id,
                "Rejection of same-master match - possible data quality issue"
            );
        }

        // Clear any pending confirmations for these medications
        let mut stats = self.stats.write().await;
        stats.pending_confirmations.remove(&normalize_for_matching(&offer.medication));
        stats.pending_confirmations.remove(&normalize_for_matching(&request.medication));
        stats.aliases_rejected += 1;

        Ok(())
    }
}
```

#### Task 4.2: Integrate Alias Learning into Match Confirmation Flow

- **File**: `core/src/api/handlers.rs`
- **Priority**: HIGH
- **Effort**: 2 hours

```rust
// core/src/api/handlers.rs
pub async fn confirm_match<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
    Json(req): Json<ConfirmRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // ... existing confirmation logic ...

    // Learn from confirmation
    if let Err(e) = state.alias_learner
        .learn_from_confirmation(&match_entity, &offer, &request)
        .await
    {
        tracing::warn!(error = %e, "Failed to learn alias from confirmation");
    }

    // ... rest of handler ...
}
```

---

## 5. Hierarchical Matching Pipeline

### Overview

Replace single-pass matching with a staged approach that progressively narrows candidates.

### Implementation Tasks

#### Task 5.1: Create Hierarchical Matcher

- **File**: `core/src/matching/hierarchical_matcher.rs`
- **Priority**: HIGH
- **Effort**: 8 hours

```rust
// core/src/matching/hierarchical_matcher.rs
pub struct HierarchicalMatcher {
    master_repo: Arc<dyn MasterMedicationRepository>,
    offer_repo: Arc<dyn OfferRepository>,
    embedding_cache: Arc<EmbeddingCache>,
    scorer: Arc<Scorer>,
    config: HierarchicalConfig,
    stats: RwLock<HierarchicalStats>,
}

#[derive(Debug, Clone)]
pub struct HierarchicalConfig {
    /// Enable each stage
    pub enable_exact_match: bool,
    pub enable_alias_lookup: bool,
    pub enable_fts_search: bool,
    pub enable_embedding_search: bool,
    pub enable_fuzzy_validation: bool,

    /// Stage-specific thresholds
    pub fts_min_score: f64,
    pub embedding_min_similarity: f64,
    pub fuzzy_min_similarity: f64,

    /// Limits
    pub fts_max_candidates: i64,
    pub embedding_top_k: i64,
}

#[derive(Debug, Clone)]
pub struct MatchCandidate {
    pub offer: Offer,
    pub stage: MatchStage,
    pub stage_score: f64,
    pub final_score: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchStage {
    ExactMatch,
    AliasMatch,
    FTSMatch,
    EmbeddingMatch,
    FuzzyValidated,
}

#[derive(Debug, Default)]
pub struct HierarchicalStats {
    pub exact_matches: u64,
    pub alias_matches: u64,
    pub fts_candidates: u64,
    pub embedding_candidates: u64,
    pub fuzzy_validated: u64,
    pub total_requests: u64,
}

impl HierarchicalMatcher {
    /// Find matching offers for a request using hierarchical pipeline
    pub async fn find_matches(&self, request: &Request) -> Result<Vec<MatchCandidate>, Error> {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;
        drop(stats);

        // Stage 1: Exact Match (O(1))
        if self.config.enable_exact_match {
            if let Some(candidates) = self.stage_exact_match(request).await? {
                if !candidates.is_empty() {
                    let mut stats = self.stats.write().await;
                    stats.exact_matches += candidates.len() as u64;
                    return Ok(candidates);
                }
            }
        }

        // Stage 2: Alias Lookup (O(1))
        if self.config.enable_alias_lookup {
            if let Some(candidates) = self.stage_alias_lookup(request).await? {
                if !candidates.is_empty() {
                    let mut stats = self.stats.write().await;
                    stats.alias_matches += candidates.len() as u64;
                    return Ok(candidates);
                }
            }
        }

        // Stage 3: FTS + Trigram (O(log n))
        let mut candidates = Vec::new();
        if self.config.enable_fts_search {
            candidates = self.stage_fts_search(request).await?;
            let mut stats = self.stats.write().await;
            stats.fts_candidates += candidates.len() as u64;
        }

        // Stage 4: Embedding Similarity (on FTS candidates or all)
        if self.config.enable_embedding_search {
            candidates = self.stage_embedding_filter(request, candidates).await?;
            let mut stats = self.stats.write().await;
            stats.embedding_candidates += candidates.len() as u64;
        }

        // Stage 5: Fuzzy + Raw Validation (final scoring)
        if self.config.enable_fuzzy_validation {
            candidates = self.stage_fuzzy_validation(request, candidates).await?;
            let mut stats = self.stats.write().await;
            stats.fuzzy_validated += candidates.len() as u64;
        }

        // Sort by final score
        candidates.sort_by(|a, b| {
            b.final_score.unwrap_or(0.0)
                .partial_cmp(&a.final_score.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(candidates)
    }

    /// Stage 1: Exact match on master_medication_id
    async fn stage_exact_match(&self, request: &Request) -> Result<Option<Vec<MatchCandidate>>, Error> {
        if let Some(master_id) = &request.master_medication_id {
            let offers = self.offer_repo
                .find_by_master_medication_id(*master_id)
                .await?;

            let candidates = offers.into_iter()
                .map(|offer| MatchCandidate {
                    offer,
                    stage: MatchStage::ExactMatch,
                    stage_score: 1.0,
                    final_score: Some(1.0),
                })
                .collect();

            return Ok(Some(candidates));
        }
        Ok(None)
    }

    /// Stage 2: Alias lookup
    async fn stage_alias_lookup(&self, request: &Request) -> Result<Option<Vec<MatchCandidate>>, Error> {
        let normalized = normalize_for_matching(&request.medication);

        if let Some(master) = self.master_repo.find_by_alias(&normalized).await? {
            let offers = self.offer_repo
                .find_by_master_medication_id(master.id)
                .await?;

            let candidates = offers.into_iter()
                .map(|offer| MatchCandidate {
                    offer,
                    stage: MatchStage::AliasMatch,
                    stage_score: 0.95,
                    final_score: Some(0.95),
                })
                .collect();

            return Ok(Some(candidates));
        }
        Ok(None)
    }

    /// Stage 3: Full-text search with trigram
    async fn stage_fts_search(&self, request: &Request) -> Result<Vec<MatchCandidate>, Error> {
        let offers = self.offer_repo
            .search_by_medication(&request.medication, self.config.fts_max_candidates)
            .await?;

        offers.into_iter()
            .filter(|(_, score)| *score >= self.config.fts_min_score)
            .map(|(offer, score)| MatchCandidate {
                offer,
                stage: MatchStage::FTSMatch,
                stage_score: score,
                final_score: None,
            })
            .collect::<Vec<_>>()
            .pipe(Ok)
    }

    /// Stage 4: Embedding similarity filter
    async fn stage_embedding_filter(
        &self,
        request: &Request,
        candidates: Vec<MatchCandidate>,
    ) -> Result<Vec<MatchCandidate>, Error> {
        let request_embedding = match &request.content_embedding {
            Some(emb) => emb.as_slice(),
            None => return Ok(candidates),
        };

        let mut filtered = Vec::new();
        for mut candidate in candidates {
            if let Some(offer_emb) = &candidate.offer.content_embedding {
                let similarity = cosine_similarity(request_embedding, offer_emb.as_slice())?;
                if similarity >= self.config.embedding_min_similarity {
                    candidate.stage = MatchStage::EmbeddingMatch;
                    candidate.stage_score = similarity;
                    filtered.push(candidate);
                }
            }
        }

        // Sort by embedding similarity and take top-k
        filtered.sort_by(|a, b| b.stage_score.partial_cmp(&a.stage_score).unwrap());
        filtered.truncate(self.config.embedding_top_k as usize);

        Ok(filtered)
    }

    /// Stage 5: Fuzzy + Raw text validation
    async fn stage_fuzzy_validation(
        &self,
        request: &Request,
        candidates: Vec<MatchCandidate>,
    ) -> Result<Vec<MatchCandidate>, Error> {
        let mut validated = Vec::new();

        for mut candidate in candidates {
            let fuzzy_score = medication_similarity_with_raw(
                &candidate.offer.medication,
                &request.medication,
                Some(&candidate.offer.medication_raw),
                Some(&request.medication_raw),
            );

            if fuzzy_score >= self.config.fuzzy_min_similarity {
                // Calculate final score using scorer
                let final_score = self.scorer.score_match(
                    &candidate.offer,
                    request,
                    fuzzy_score,
                );

                candidate.stage = MatchStage::FuzzyValidated;
                candidate.final_score = Some(final_score.total);
                validated.push(candidate);
            }
        }

        Ok(validated)
    }
}
```

---

## 6. Confidence Calibration (Platt Scaling)

### Overview

Calibrate raw match scores to true probabilities using Platt scaling learned from historical feedback.

### Implementation Tasks

#### Task 6.1: Create Platt Scaling Calibrator

- **File**: `core/src/matching/platt_calibrator.rs`
- **Priority**: MEDIUM
- **Effort**: 4 hours

```rust
// core/src/matching/platt_calibrator.rs
use std::sync::atomic::{AtomicU64, Ordering};

pub struct PlattCalibrator {
    /// Platt scaling parameter A
    param_a: RwLock<f64>,
    /// Platt scaling parameter B
    param_b: RwLock<f64>,
    /// Training data buffer
    training_buffer: RwLock<Vec<(f64, bool)>>,
    /// Configuration
    config: PlattConfig,
    /// Statistics
    stats: PlattStats,
}

#[derive(Debug, Clone)]
pub struct PlattConfig {
    /// Minimum samples before calibration
    pub min_samples: usize,
    /// Maximum samples to keep in buffer
    pub max_buffer_size: usize,
    /// Learning rate for online updates
    pub learning_rate: f64,
    /// Regularization strength
    pub regularization: f64,
    /// Enable calibration
    pub enabled: bool,
}

impl Default for PlattConfig {
    fn default() -> Self {
        Self {
            min_samples: 100,
            max_buffer_size: 10000,
            learning_rate: 0.01,
            regularization: 0.001,
            enabled: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct PlattStats {
    pub samples_collected: AtomicU64,
    pub calibrations_performed: AtomicU64,
    pub current_ece: RwLock<f64>,  // Expected Calibration Error
}

impl PlattCalibrator {
    pub fn new(config: PlattConfig) -> Self {
        Self {
            param_a: RwLock::new(-1.0),  // Default: slight sigmoid
            param_b: RwLock::new(0.0),
            training_buffer: RwLock::new(Vec::new()),
            config,
            stats: PlattStats::default(),
        }
    }

    /// Calibrate a raw score to probability
    /// P(y=1|s) = 1 / (1 + exp(A*s + B))
    pub fn calibrate(&self, raw_score: f64) -> f64 {
        if !self.config.enabled {
            return raw_score;
        }

        let a = *self.param_a.read().unwrap();
        let b = *self.param_b.read().unwrap();

        1.0 / (1.0 + (a * raw_score + b).exp())
    }

    /// Record outcome for training
    pub async fn record_outcome(&self, raw_score: f64, confirmed: bool) {
        let mut buffer = self.training_buffer.write().await;
        buffer.push((raw_score, confirmed));
        self.stats.samples_collected.fetch_add(1, Ordering::Relaxed);

        // Trim buffer if too large
        if buffer.len() > self.config.max_buffer_size {
            buffer.drain(0..buffer.len() - self.config.max_buffer_size);
        }

        // Recalibrate if enough samples
        if buffer.len() >= self.config.min_samples {
            drop(buffer);
            self.fit().await;
        }
    }

    /// Fit Platt scaling parameters using gradient descent
    async fn fit(&self) {
        let buffer = self.training_buffer.read().await;
        if buffer.len() < self.config.min_samples {
            return;
        }

        let mut a = *self.param_a.read().unwrap();
        let mut b = *self.param_b.read().unwrap();

        // Gradient descent iterations
        for _ in 0..100 {
            let mut grad_a = 0.0;
            let mut grad_b = 0.0;

            for (score, confirmed) in buffer.iter() {
                let y = if *confirmed { 1.0 } else { 0.0 };
                let p = 1.0 / (1.0 + (a * score + b).exp());
                let error = p - y;

                grad_a += error * score;
                grad_b += error;
            }

            // Add regularization
            grad_a += self.config.regularization * a;
            grad_b += self.config.regularization * b;

            // Update parameters
            a -= self.config.learning_rate * grad_a / buffer.len() as f64;
            b -= self.config.learning_rate * grad_b / buffer.len() as f64;
        }

        // Update parameters
        *self.param_a.write().unwrap() = a;
        *self.param_b.write().unwrap() = b;

        // Calculate ECE
        let ece = self.calculate_ece(&buffer);
        *self.stats.current_ece.write().unwrap() = ece;

        self.stats.calibrations_performed.fetch_add(1, Ordering::Relaxed);

        tracing::info!(
            a = %a,
            b = %b,
            ece = %ece,
            samples = %buffer.len(),
            "Platt scaling parameters updated"
        );
    }

    /// Calculate Expected Calibration Error
    fn calculate_ece(&self, data: &[(f64, bool)]) -> f64 {
        let num_bins = 10;
        let mut bins: Vec<Vec<(f64, bool)>> = vec![Vec::new(); num_bins];

        for (score, confirmed) in data {
            let calibrated = self.calibrate(*score);
            let bin_idx = ((calibrated * num_bins as f64) as usize).min(num_bins - 1);
            bins[bin_idx].push((calibrated, *confirmed));
        }

        let mut ece = 0.0;
        let total = data.len() as f64;

        for bin in bins {
            if bin.is_empty() {
                continue;
            }

            let bin_size = bin.len() as f64;
            let avg_confidence: f64 = bin.iter().map(|(c, _)| c).sum::<f64>() / bin_size;
            let accuracy: f64 = bin.iter().filter(|(_, c)| *c).count() as f64 / bin_size;

            ece += (bin_size / total) * (avg_confidence - accuracy).abs();
        }

        ece
    }

    /// Export parameters for persistence
    pub fn export(&self) -> PlattParams {
        PlattParams {
            a: *self.param_a.read().unwrap(),
            b: *self.param_b.read().unwrap(),
        }
    }

    /// Import parameters
    pub fn import(&self, params: PlattParams) {
        *self.param_a.write().unwrap() = params.a;
        *self.param_b.write().unwrap() = params.b;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlattParams {
    pub a: f64,
    pub b: f64,
}
```

---

## 7. Contrastive Validation

### Overview

Use negative sampling to detect false positives by ensuring the match score is significantly higher than random alternatives.

### Implementation Tasks

#### Task 7.1: Create Contrastive Validator

- **File**: `core/src/matching/contrastive_validator.rs`
- **Priority**: MEDIUM
- **Effort**: 4 hours

```rust
// core/src/matching/contrastive_validator.rs
pub struct ContrastiveValidator {
    offer_repo: Arc<dyn OfferRepository>,
    scorer: Arc<Scorer>,
    config: ContrastiveConfig,
    stats: RwLock<ContrastiveStats>,
}

#[derive(Debug, Clone)]
pub struct ContrastiveConfig {
    /// Number of negative samples to compare against
    pub num_negatives: usize,

    /// Minimum margin between positive and negative scores
    pub min_margin: f64,

    /// Require positive to beat ALL negatives
    pub require_beat_all: bool,

    /// Enable contrastive validation
    pub enabled: bool,
}

impl Default for ContrastiveConfig {
    fn default() -> Self {
        Self {
            num_negatives: 3,
            min_margin: 0.15,
            require_beat_all: true,
            enabled: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct ContrastiveStats {
    pub validations_performed: u64,
    pub validations_passed: u64,
    pub validations_failed: u64,
    pub avg_margin: f64,
}

#[derive(Debug, Clone)]
pub struct ContrastiveResult {
    pub is_valid: bool,
    pub positive_score: f64,
    pub negative_scores: Vec<f64>,
    pub margin: f64,
    pub beat_count: usize,
}

impl ContrastiveValidator {
    /// Validate a match using contrastive comparison
    pub async fn validate(
        &self,
        offer: &Offer,
        request: &Request,
        positive_score: f64,
    ) -> Result<ContrastiveResult, Error> {
        if !self.config.enabled {
            return Ok(ContrastiveResult {
                is_valid: true,
                positive_score,
                negative_scores: vec![],
                margin: 1.0,
                beat_count: 0,
            });
        }

        // Get random negative offers (different medications)
        let negatives = self.get_negative_samples(offer, self.config.num_negatives).await?;

        // Score each negative against the request
        let negative_scores: Vec<f64> = negatives.iter()
            .map(|neg_offer| {
                let med_sim = medication_similarity_with_raw(
                    &neg_offer.medication,
                    &request.medication,
                    Some(&neg_offer.medication_raw),
                    Some(&request.medication_raw),
                );
                self.scorer.score_match(neg_offer, request, med_sim).total
            })
            .collect();

        // Calculate margin (positive - max negative)
        let max_negative = negative_scores.iter().cloned().fold(0.0, f64::max);
        let margin = positive_score - max_negative;

        // Count how many negatives the positive beats
        let beat_count = negative_scores.iter()
            .filter(|&&neg| positive_score > neg + self.config.min_margin)
            .count();

        // Determine validity
        let is_valid = if self.config.require_beat_all {
            beat_count == negative_scores.len()
        } else {
            margin >= self.config.min_margin
        };

        // Update stats
        let mut stats = self.stats.write().await;
        stats.validations_performed += 1;
        if is_valid {
            stats.validations_passed += 1;
        } else {
            stats.validations_failed += 1;
        }
        stats.avg_margin = (stats.avg_margin * (stats.validations_performed - 1) as f64 + margin)
            / stats.validations_performed as f64;

        if !is_valid {
            tracing::warn!(
                offer_med = %offer.medication,
                request_med = %request.medication,
                positive_score = %positive_score,
                max_negative = %max_negative,
                margin = %margin,
                "Contrastive validation failed - possible false positive"
            );
        }

        Ok(ContrastiveResult {
            is_valid,
            positive_score,
            negative_scores,
            margin,
            beat_count,
        })
    }

    /// Get random offers that are NOT the same medication
    async fn get_negative_samples(
        &self,
        exclude_offer: &Offer,
        count: usize,
    ) -> Result<Vec<Offer>, Error> {
        // Get random active offers
        let candidates = self.offer_repo
            .get_random_active(count * 3)  // Get more to filter
            .await?;

        // Filter out same medication
        let exclude_normalized = normalize_for_matching(&exclude_offer.medication);

        candidates.into_iter()
            .filter(|o| {
                let normalized = normalize_for_matching(&o.medication);
                normalized != exclude_normalized && o.id != exclude_offer.id
            })
            .take(count)
            .collect::<Vec<_>>()
            .pipe(Ok)
    }
}
```

---

## 8. Uncertainty Quantification

### Overview

Estimate prediction uncertainty using Monte Carlo dropout or ensemble variance.

### Implementation Tasks

#### Task 8.1: Create Uncertainty Estimator

- **File**: `core/src/matching/uncertainty_estimator.rs`
- **Priority**: LOW
- **Effort**: 6 hours

```rust
// core/src/matching/uncertainty_estimator.rs
pub struct UncertaintyEstimator {
    scorer: Arc<Scorer>,
    config: UncertaintyConfig,
}

#[derive(Debug, Clone)]
pub struct UncertaintyConfig {
    /// Number of forward passes for MC estimation
    pub num_samples: usize,

    /// Dropout rate for MC dropout
    pub dropout_rate: f64,

    /// Enable uncertainty estimation
    pub enabled: bool,

    /// High uncertainty threshold
    pub high_uncertainty_threshold: f64,
}

impl Default for UncertaintyConfig {
    fn default() -> Self {
        Self {
            num_samples: 10,
            dropout_rate: 0.1,
            enabled: true,
            high_uncertainty_threshold: 0.15,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UncertaintyResult {
    pub mean_score: f64,
    pub std_dev: f64,
    pub confidence_interval: (f64, f64),
    pub is_high_uncertainty: bool,
    pub samples: Vec<f64>,
}

impl UncertaintyEstimator {
    /// Estimate uncertainty using weight perturbation
    pub async fn estimate(
        &self,
        offer: &Offer,
        request: &Request,
    ) -> Result<UncertaintyResult, Error> {
        if !self.config.enabled {
            let score = self.scorer.score_match_basic(offer, request);
            return Ok(UncertaintyResult {
                mean_score: score,
                std_dev: 0.0,
                confidence_interval: (score, score),
                is_high_uncertainty: false,
                samples: vec![score],
            });
        }

        // Run multiple forward passes with weight perturbation
        let mut samples = Vec::with_capacity(self.config.num_samples);

        for _ in 0..self.config.num_samples {
            // Perturb weights slightly
            let perturbed_weights = self.perturb_weights();
            let score = self.scorer.score_match_with_weights(offer, request, &perturbed_weights);
            samples.push(score);
        }

        // Calculate statistics
        let mean_score = samples.iter().sum::<f64>() / samples.len() as f64;
        let variance = samples.iter()
            .map(|s| (s - mean_score).powi(2))
            .sum::<f64>() / samples.len() as f64;
        let std_dev = variance.sqrt();

        // 95% confidence interval
        let z = 1.96;
        let margin = z * std_dev / (samples.len() as f64).sqrt();
        let confidence_interval = (
            (mean_score - margin).max(0.0),
            (mean_score + margin).min(1.0),
        );

        let is_high_uncertainty = std_dev > self.config.high_uncertainty_threshold;

        if is_high_uncertainty {
            tracing::info!(
                offer_med = %offer.medication,
                request_med = %request.medication,
                mean = %mean_score,
                std_dev = %std_dev,
                "High uncertainty detected in match score"
            );
        }

        Ok(UncertaintyResult {
            mean_score,
            std_dev,
            confidence_interval,
            is_high_uncertainty,
            samples,
        })
    }

    /// Perturb weights with small random noise
    fn perturb_weights(&self) -> Weights {
        let base = self.scorer.get_weights();
        let mut rng = rand::thread_rng();

        Weights {
            medication: (base.medication + rng.gen_range(-0.05..0.05)).clamp(0.0, 1.0),
            dosage: (base.dosage + rng.gen_range(-0.02..0.02)).clamp(0.0, 1.0),
            quantity: (base.quantity + rng.gen_range(-0.02..0.02)).clamp(0.0, 1.0),
            price: (base.price + rng.gen_range(-0.02..0.02)).clamp(0.0, 1.0),
            recency: (base.recency + rng.gen_range(-0.02..0.02)).clamp(0.0, 1.0),
            ai_logic: base.ai_logic,
        }
    }
}
```

---

## 9. Circuit Breaker with Fallback

### Overview

Enhance the existing circuit breaker with graceful degradation to deterministic matching when AI is unavailable.

### Implementation Tasks

#### Task 9.1: Enhance Circuit Breaker with Fallback Strategy

- **File**: `core/src/ai/circuit_breaker.rs` (enhance existing)
- **Priority**: HIGH
- **Effort**: 3 hours

```rust
// core/src/ai/circuit_breaker.rs
#[derive(Debug, Clone, PartialEq)]
pub enum FallbackStrategy {
    /// Use deterministic matching only (master_medication_id, aliases)
    DeterministicOnly,
    /// Use cached embeddings + fuzzy matching
    CachedEmbeddings,
    /// Queue for later processing
    QueueForLater,
    /// Reject all matches
    RejectAll,
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout: Duration,
    pub half_open_max_requests: u32,
    pub fallback_strategy: FallbackStrategy,
}

impl CircuitBreaker {
    /// Execute with fallback on circuit open
    pub async fn execute_with_fallback<T, F, Fut, FB, FBFut>(
        &self,
        operation: F,
        fallback: FB,
    ) -> Result<T, Error>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, Error>>,
        FB: FnOnce() -> FBFut,
        FBFut: Future<Output = Result<T, Error>>,
    {
        match self.state() {
            CircuitState::Open => {
                tracing::warn!("Circuit breaker open, using fallback strategy");
                self.metrics.fallback_invocations.fetch_add(1, Ordering::Relaxed);
                fallback().await
            }
            CircuitState::HalfOpen => {
                // Try operation, fall back on failure
                match operation().await {
                    Ok(result) => {
                        self.record_success();
                        Ok(result)
                    }
                    Err(e) => {
                        self.record_failure();
                        tracing::warn!(error = %e, "Half-open request failed, using fallback");
                        fallback().await
                    }
                }
            }
            CircuitState::Closed => {
                match operation().await {
                    Ok(result) => {
                        self.record_success();
                        Ok(result)
                    }
                    Err(e) => {
                        self.record_failure();
                        Err(e)
                    }
                }
            }
        }
    }
}
```

#### Task 9.2: Create Fallback Matcher

- **File**: `core/src/matching/fallback_matcher.rs`
- **Priority**: HIGH
- **Effort**: 4 hours

```rust
// core/src/matching/fallback_matcher.rs
pub struct FallbackMatcher {
    master_repo: Arc<dyn MasterMedicationRepository>,
    offer_repo: Arc<dyn OfferRepository>,
    embedding_cache: Arc<EmbeddingCache>,
    config: FallbackConfig,
}

#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// Enable deterministic matching
    pub enable_deterministic: bool,
    /// Enable cached embedding matching
    pub enable_cached_embeddings: bool,
    /// Minimum similarity for cached embedding match
    pub cached_embedding_threshold: f64,
}

impl FallbackMatcher {
    /// Match using only deterministic methods (no AI)
    pub async fn match_deterministic(
        &self,
        request: &Request,
    ) -> Result<Vec<MatchCandidate>, Error> {
        let mut candidates = Vec::new();

        // Try master_medication_id match
        if let Some(master_id) = &request.master_medication_id {
            let offers = self.offer_repo
                .find_by_master_medication_id(*master_id)
                .await?;

            for offer in offers {
                candidates.push(MatchCandidate {
                    offer,
                    stage: MatchStage::ExactMatch,
                    stage_score: 1.0,
                    final_score: Some(1.0),
                });
            }
        }

        // Try alias lookup
        if candidates.is_empty() {
            let normalized = normalize_for_matching(&request.medication);
            if let Some(master) = self.master_repo.find_by_alias(&normalized).await? {
                let offers = self.offer_repo
                    .find_by_master_medication_id(master.id)
                    .await?;

                for offer in offers {
                    candidates.push(MatchCandidate {
                        offer,
                        stage: MatchStage::AliasMatch,
                        stage_score: 0.95,
                        final_score: Some(0.95),
                    });
                }
            }
        }

        // Try cached embeddings
        if candidates.is_empty() && self.config.enable_cached_embeddings {
            if let Some(request_emb) = self.embedding_cache.get(&request.medication).await {
                let offers = self.offer_repo.get_active(100, 0).await?;

                for offer in offers {
                    if let Some(offer_emb) = self.embedding_cache.get(&offer.medication).await {
                        let similarity = cosine_similarity(&request_emb, &offer_emb)?;
                        if similarity >= self.config.cached_embedding_threshold {
                            candidates.push(MatchCandidate {
                                offer,
                                stage: MatchStage::EmbeddingMatch,
                                stage_score: similarity,
                                final_score: Some(similarity),
                            });
                        }
                    }
                }
            }
        }

        Ok(candidates)
    }
}
```

---

## 10. Audit Trail with Reproducibility

### Overview

Store complete snapshots of all inputs and parameters to enable debugging and reproducibility.

### Implementation Tasks

#### Task 10.1: Create Match Audit Record Schema

- **File**: `migrations/YYYYMMDD_create_match_audit_records.sql`
- **Priority**: MEDIUM
- **Effort**: 3 hours

```sql
CREATE TABLE match_audit_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    match_id UUID NOT NULL REFERENCES matches(id) ON DELETE CASCADE,

    -- Snapshots at match time
    offer_snapshot JSONB NOT NULL,
    request_snapshot JSONB NOT NULL,

    -- Configuration at match time
    weights_version VARCHAR(50) NOT NULL,
    weights_snapshot JSONB NOT NULL,
    model_version VARCHAR(50),

    -- Scoring details
    score_breakdown JSONB NOT NULL,
    stage_scores JSONB,

    -- Pipeline metadata
    pipeline_version VARCHAR(50) NOT NULL,
    stages_executed JSONB,

    -- Timing
    processing_time_ms INTEGER,

    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_match_audit_match_id ON match_audit_records(match_id);
CREATE INDEX idx_match_audit_created_at ON match_audit_records(created_at);
```

#### Task 10.2: Create Audit Record Service

- **File**: `core/src/matching/audit_recorder.rs`
- **Priority**: MEDIUM
- **Effort**: 4 hours

```rust
// core/src/matching/audit_recorder.rs
pub struct AuditRecorder {
    repo: Arc<dyn MatchAuditRepository>,
    config: AuditRecorderConfig,
}

#[derive(Debug, Clone)]
pub struct AuditRecorderConfig {
    /// Enable audit recording
    pub enabled: bool,
    /// Record all matches or only flagged/rejected
    pub record_all: bool,
    /// Include embeddings in snapshot (large)
    pub include_embeddings: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchAuditRecord {
    pub id: Uuid,
    pub match_id: Uuid,
    pub offer_snapshot: OfferSnapshot,
    pub request_snapshot: RequestSnapshot,
    pub weights_version: String,
    pub weights_snapshot: Weights,
    pub model_version: Option<String>,
    pub score_breakdown: ScoreBreakdown,
    pub stage_scores: Option<Vec<StageScore>>,
    pub pipeline_version: String,
    pub stages_executed: Vec<String>,
    pub processing_time_ms: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferSnapshot {
    pub id: Uuid,
    pub medication: String,
    pub medication_raw: String,
    pub quantity: Option<Decimal>,
    pub price: Option<Decimal>,
    pub master_medication_id: Option<Uuid>,
    pub ai_confidence: f64,
    pub embedding_hash: Option<String>,  // Hash instead of full embedding
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageScore {
    pub stage: String,
    pub score: f64,
    pub candidates_in: usize,
    pub candidates_out: usize,
    pub duration_ms: i32,
}

impl AuditRecorder {
    /// Record match audit trail
    pub async fn record(
        &self,
        match_entity: &Match,
        offer: &Offer,
        request: &Request,
        weights: &Weights,
        score_breakdown: &ScoreBreakdown,
        stages: &[StageScore],
        processing_time: Duration,
    ) -> Result<MatchAuditRecord, Error> {
        if !self.config.enabled {
            return Err(Error::AuditDisabled);
        }

        // Skip if not recording all and match is auto-confirmed
        if !self.config.record_all && match_entity.status == MatchStatus::Pending {
            return Err(Error::AuditSkipped);
        }

        let record = MatchAuditRecord {
            id: Uuid::new_v4(),
            match_id: match_entity.id,
            offer_snapshot: self.snapshot_offer(offer),
            request_snapshot: self.snapshot_request(request),
            weights_version: weights.version.clone(),
            weights_snapshot: weights.clone(),
            model_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            score_breakdown: score_breakdown.clone(),
            stage_scores: Some(stages.to_vec()),
            pipeline_version: "v2.0".to_string(),
            stages_executed: stages.iter().map(|s| s.stage.clone()).collect(),
            processing_time_ms: processing_time.as_millis() as i32,
            created_at: Utc::now(),
        };

        self.repo.save(&record).await?;

        Ok(record)
    }

    fn snapshot_offer(&self, offer: &Offer) -> OfferSnapshot {
        OfferSnapshot {
            id: offer.id,
            medication: offer.medication.clone(),
            medication_raw: offer.medication_raw.clone(),
            quantity: offer.quantity,
            price: offer.price,
            master_medication_id: offer.master_medication_id,
            ai_confidence: offer.ai_confidence,
            embedding_hash: offer.content_embedding.as_ref().map(|e| {
                use sha2::{Sha256, Digest};
                let bytes: Vec<u8> = e.as_slice().iter()
                    .flat_map(|f| f.to_le_bytes())
                    .collect();
                format!("{:x}", Sha256::digest(&bytes))
            }),
        }
    }

    /// Replay a match for debugging
    pub async fn replay(&self, audit_id: Uuid) -> Result<ReplayResult, Error> {
        let record = self.repo.get_by_id(audit_id).await?
            .ok_or(Error::AuditNotFound)?;

        // Reconstruct offer and request from snapshots
        // Re-run scoring with recorded weights
        // Compare with original score

        todo!("Implement replay logic")
    }
}
```

---

## 11. A/B Test Auto-Rollback

### Overview

Automatically rollback A/B tests that show degraded performance.

### Implementation Tasks

#### Task 11.1: Enhance A/B Test Manager with Auto-Rollback

- **File**: `core/src/matching/abtest.rs` (enhance existing)
- **Priority**: MEDIUM
- **Effort**: 4 hours

```rust
// core/src/matching/abtest.rs
#[derive(Debug, Clone)]
pub struct AutoRollbackConfig {
    /// Enable automatic rollback
    pub enabled: bool,

    /// Minimum samples before evaluation
    pub min_samples: i64,

    /// Maximum rejection rate increase to tolerate (e.g., 1.2 = 20% increase)
    pub max_rejection_rate_increase: f64,

    /// Minimum confirmation rate decrease to trigger rollback
    pub min_confirmation_rate_decrease: f64,

    /// P-value threshold for statistical significance
    pub significance_threshold: f64,

    /// Check interval
    pub check_interval: Duration,
}

impl Default for AutoRollbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_samples: 50,
            max_rejection_rate_increase: 1.2,
            min_confirmation_rate_decrease: 0.05,
            significance_threshold: 0.05,
            check_interval: Duration::from_secs(300),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TestDecision {
    Continue,
    Promote,
    Rollback,
    InsufficientData,
}

impl ABTestManager {
    /// Evaluate test and decide on action
    pub async fn evaluate_test(&self, test_id: Uuid) -> Result<TestDecision, Error> {
        let test = self.get_test(test_id).await?
            .ok_or(Error::TestNotFound)?;

        if test.status != TestStatus::Active {
            return Ok(TestDecision::Continue);
        }

        let stats = self.get_test_stats(test_id).await?;

        // Check minimum samples
        if stats.control_samples < self.config.auto_rollback.min_samples
            || stats.test_samples < self.config.auto_rollback.min_samples
        {
            return Ok(TestDecision::InsufficientData);
        }

        // Calculate rates
        let control_confirmation_rate = stats.control_confirmed as f64
            / stats.control_samples as f64;
        let test_confirmation_rate = stats.test_confirmed as f64
            / stats.test_samples as f64;

        let control_rejection_rate = stats.control_rejected as f64
            / stats.control_samples as f64;
        let test_rejection_rate = stats.test_rejected as f64
            / stats.test_samples as f64;

        // Check for degradation (auto-rollback)
        if test_rejection_rate > control_rejection_rate * self.config.auto_rollback.max_rejection_rate_increase {
            tracing::warn!(
                test_id = %test_id,
                control_rejection_rate = %control_rejection_rate,
                test_rejection_rate = %test_rejection_rate,
                "Test showing increased rejection rate - rolling back"
            );
            self.rollback_test(test_id).await?;
            return Ok(TestDecision::Rollback);
        }

        if control_confirmation_rate - test_confirmation_rate
            > self.config.auto_rollback.min_confirmation_rate_decrease
        {
            tracing::warn!(
                test_id = %test_id,
                control_confirmation_rate = %control_confirmation_rate,
                test_confirmation_rate = %test_confirmation_rate,
                "Test showing decreased confirmation rate - rolling back"
            );
            self.rollback_test(test_id).await?;
            return Ok(TestDecision::Rollback);
        }

        // Check for improvement (auto-promote)
        let p_value = self.calculate_p_value(&stats);
        if p_value < self.config.auto_rollback.significance_threshold
            && test_confirmation_rate > control_confirmation_rate * 1.1
        {
            tracing::info!(
                test_id = %test_id,
                p_value = %p_value,
                improvement = %(test_confirmation_rate - control_confirmation_rate),
                "Test showing significant improvement - promoting"
            );
            self.promote_test(test_id).await?;
            return Ok(TestDecision::Promote);
        }

        Ok(TestDecision::Continue)
    }

    /// Rollback test to control weights
    pub async fn rollback_test(&self, test_id: Uuid) -> Result<(), Error> {
        let test = self.get_test(test_id).await?
            .ok_or(Error::TestNotFound)?;

        // End test with rollback status
        self.end_test(test_id, TestEndReason::Rollback).await?;

        // Log audit event
        let audit = AuditLog::ab_test_rollback(test_id, &test.name);
        self.audit_repo.save(&audit).await?;

        tracing::warn!(
            test_id = %test_id,
            test_name = %test.name,
            "A/B test rolled back due to degraded performance"
        );

        Ok(())
    }

    /// Promote test weights to production
    pub async fn promote_test(&self, test_id: Uuid) -> Result<(), Error> {
        let test = self.get_test(test_id).await?
            .ok_or(Error::TestNotFound)?;

        // Apply test weights as new production weights
        self.engine.apply_weights(test.test_weights.clone(), "A/B test promotion").await;

        // End test with promotion status
        self.end_test(test_id, TestEndReason::Promoted).await?;

        // Log audit event
        let audit = AuditLog::ab_test_promoted(test_id, &test.name);
        self.audit_repo.save(&audit).await?;

        tracing::info!(
            test_id = %test_id,
            test_name = %test.name,
            "A/B test promoted to production"
        );

        Ok(())
    }

    /// Background task to periodically evaluate active tests
    pub async fn run_evaluation_loop(&self, mut shutdown: watch::Receiver<bool>) {
        let mut interval = tokio::time::interval(self.config.auto_rollback.check_interval);

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        tracing::info!("A/B test evaluation loop shutting down");
                        break;
                    }
                }
                _ = interval.tick() => {
                    if let Ok(active_tests) = self.get_active_tests().await {
                        for test in active_tests {
                            if let Err(e) = self.evaluate_test(test.id).await {
                                tracing::error!(
                                    error = %e,
                                    test_id = %test.id,
                                    "Failed to evaluate A/B test"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
```

---

## 12. Implementation Priority Matrix

### Priority Levels

| Priority | Description                      | Timeline |
| -------- | -------------------------------- | -------- |
| P0       | Critical for production          | Week 1-2 |
| P1       | High impact, should be done soon | Week 3-4 |
| P2       | Medium impact, nice to have      | Week 5-8 |
| P3       | Low priority, future enhancement | Backlog  |

### Task Summary

| #    | Enhancement            | Task                        | Priority | Effort | Dependencies |
| ---- | ---------------------- | --------------------------- | -------- | ------ | ------------ |
| 1.1  | Master Medication DB   | Create migration files      | P0       | 2h     | None         |
| 1.2  | Master Medication DB   | Repository trait & impl     | P0       | 4h     | 1.1          |
| 1.3  | Master Medication DB   | Medication resolver service | P0       | 6h     | 1.2          |
| 1.4  | Master Medication DB   | Seed initial data           | P1       | 8h     | 1.2          |
| 2.1  | Temperature Controls   | AI config structure         | P0       | 2h     | None         |
| 2.2  | Temperature Controls   | Update AI client            | P0       | 3h     | 2.1          |
| 2.3  | Temperature Controls   | Update parser & reviewer    | P0       | 2h     | 2.2          |
| 3.1  | Consensus Audit        | Create consensus auditor    | P1       | 6h     | 2.3          |
| 3.2  | Consensus Audit        | Multi-model factory         | P1       | 3h     | 3.1          |
| 4.1  | Alias Learning         | Create alias learner        | P1       | 6h     | 1.2          |
| 4.2  | Alias Learning         | Integrate into confirmation | P1       | 2h     | 4.1          |
| 5.1  | Hierarchical Matching  | Create hierarchical matcher | P0       | 8h     | 1.3          |
| 6.1  | Platt Scaling          | Create calibrator           | P2       | 4h     | None         |
| 7.1  | Contrastive Validation | Create validator            | P2       | 4h     | None         |
| 8.1  | Uncertainty Estimation | Create estimator            | P3       | 6h     | None         |
| 9.1  | Circuit Breaker        | Enhance with fallback       | P1       | 3h     | None         |
| 9.2  | Circuit Breaker        | Create fallback matcher     | P1       | 4h     | 9.1, 1.3     |
| 10.1 | Audit Trail            | Create schema               | P2       | 3h     | None         |
| 10.2 | Audit Trail            | Create recorder service     | P2       | 4h     | 10.1         |
| 11.1 | A/B Auto-Rollback      | Enhance A/B manager         | P2       | 4h     | None         |

### Recommended Implementation Order

```
Week 1-2 (P0 - Foundation):
├── 1.1 Master Medication DB Migration
├── 1.2 Master Medication Repository
├── 2.1 AI Config Structure
├── 2.2 Update AI Client
├── 2.3 Update Parser & Reviewer
├── 1.3 Medication Resolver Service
└── 5.1 Hierarchical Matching Pipeline

Week 3-4 (P1 - Core Enhancements):
├── 3.1 Consensus Auditor
├── 3.2 Multi-Model Factory
├── 4.1 Alias Learner
├── 4.2 Alias Learning Integration
├── 9.1 Circuit Breaker Fallback
├── 9.2 Fallback Matcher
└── 1.4 Seed Initial Data

Week 5-8 (P2 - Optimization):
├── 6.1 Platt Scaling Calibrator
├── 7.1 Contrastive Validator
├── 10.1 Audit Trail Schema
├── 10.2 Audit Recorder Service
└── 11.1 A/B Auto-Rollback

Backlog (P3 - Future):
└── 8.1 Uncertainty Estimation
```

### Success Metrics

| Metric                   | Current | Target    | Measurement                      |
| ------------------------ | ------- | --------- | -------------------------------- |
| False Positive Rate      | ~5%     | <1%       | Rejected matches / Total matches |
| Hallucination Detection  | Manual  | Automated | Contrastive validation failures  |
| Deterministic Match Rate | ~10%    | >50%      | Exact + Alias matches / Total    |
| AI Dependency            | 100%    | <50%      | Matches requiring AI / Total     |
| Mean Time to Match       | ~500ms  | <100ms    | P95 latency                      |
| Operator Rejection Rate  | ~15%    | <5%       | Manual rejections / Suggested    |

---

## Appendix: Environment Variables

```bash
# Master Medication Database
MASTER_MED_EMBEDDING_THRESHOLD=0.85
MASTER_MED_MAX_CANDIDATES=10

# AI Configuration
AI_PARSING_TEMPERATURE=0.2
AI_PARSING_TOP_P=0.9
AI_COMPARISON_TEMPERATURE=0.0
AI_COMPARISON_TOP_P=1.0

# Consensus Audit
CONSENSUS_MIN_AGREEMENT=0.67
CONSENSUS_REQUIRE_UNANIMOUS=true
CONSENSUS_TIMEOUT_MS=5000

# Alias Learning
ALIAS_LEARNING_ENABLED=true
ALIAS_MIN_SCORE=0.85
ALIAS_MIN_CONFIRMATIONS=2

# Hierarchical Matching
HIERARCHICAL_FTS_MIN_SCORE=0.3
HIERARCHICAL_EMBEDDING_MIN_SIM=0.7
HIERARCHICAL_FUZZY_MIN_SIM=0.5

# Platt Scaling
PLATT_ENABLED=true
PLATT_MIN_SAMPLES=100

# Contrastive Validation
CONTRASTIVE_ENABLED=true
CONTRASTIVE_NUM_NEGATIVES=3
CONTRASTIVE_MIN_MARGIN=0.15

# Circuit Breaker
CIRCUIT_FALLBACK_STRATEGY=deterministic_only

# A/B Testing
ABTEST_AUTO_ROLLBACK_ENABLED=true
ABTEST_MIN_SAMPLES=50
ABTEST_MAX_REJECTION_INCREASE=1.2
```

---

## Version History

| Version | Date       | Changes                     |
| ------- | ---------- | --------------------------- |
| 1.0.0   | 2026-01-02 | Initial enhancement roadmap |
