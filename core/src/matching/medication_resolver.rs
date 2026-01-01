//! Dynamic Medication Resolution Service
//!
//! Automatically resolves parsed medication names to master medication records.
//! Uses a multi-stage approach:
//! 1. Exact alias lookup (instant, 100% confidence)
//! 2. Fuzzy string matching (fast, high confidence)
//! 3. Semantic embedding search (slower, variable confidence)
//!
//! High-confidence matches are auto-approved, low-confidence matches are queued for review.

use std::sync::Arc;
use uuid::Uuid;

use crate::repository::{MedicationAliasRepository, MedicationMasterRepository};

/// Configuration for medication resolution
#[derive(Debug, Clone)]
pub struct MedicationResolverConfig {
    /// Minimum confidence to auto-approve a match (default: 0.92)
    pub auto_approve_threshold: f64,
    /// Minimum confidence to consider a match at all (default: 0.70)
    pub minimum_threshold: f64,
    /// Whether to create aliases for auto-approved matches
    pub create_aliases: bool,
    /// Whether to auto-create master records for high-confidence new medications
    pub auto_create_masters: bool,
}

impl Default for MedicationResolverConfig {
    fn default() -> Self {
        Self {
            auto_approve_threshold: 0.92,
            minimum_threshold: 0.70,
            create_aliases: true,
            auto_create_masters: false, // Conservative default
        }
    }
}

/// Result of medication resolution
#[derive(Debug, Clone)]
pub struct ResolutionResult {
    /// The resolved master medication ID (if found)
    pub master_medication_id: Option<Uuid>,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// How the resolution was achieved
    pub method: ResolutionMethod,
    /// Whether this was auto-approved
    pub auto_approved: bool,
    /// The canonical name if resolved
    pub canonical_name: Option<String>,
}

impl ResolutionResult {
    pub fn not_found() -> Self {
        Self {
            master_medication_id: None,
            confidence: 0.0,
            method: ResolutionMethod::NotFound,
            auto_approved: false,
            canonical_name: None,
        }
    }

    pub fn found(
        master_id: Uuid,
        confidence: f64,
        method: ResolutionMethod,
        auto_approved: bool,
        canonical_name: String,
    ) -> Self {
        Self {
            master_medication_id: Some(master_id),
            confidence,
            method,
            auto_approved,
            canonical_name: Some(canonical_name),
        }
    }
}

/// Method used to resolve the medication
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionMethod {
    /// Exact match on existing alias
    ExactAlias,
    /// Fuzzy string match on master canonical name
    FuzzyMatch,
    /// Semantic embedding similarity
    SemanticMatch,
    /// No match found
    NotFound,
}

impl std::fmt::Display for ResolutionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolutionMethod::ExactAlias => write!(f, "exact_alias"),
            ResolutionMethod::FuzzyMatch => write!(f, "fuzzy_match"),
            ResolutionMethod::SemanticMatch => write!(f, "semantic_match"),
            ResolutionMethod::NotFound => write!(f, "not_found"),
        }
    }
}

/// Dynamic medication resolver
pub struct MedicationResolver {
    config: MedicationResolverConfig,
    master_repo: Arc<dyn MedicationMasterRepository>,
    alias_repo: Arc<dyn MedicationAliasRepository>,
}

impl MedicationResolver {
    pub fn new(
        config: MedicationResolverConfig,
        master_repo: Arc<dyn MedicationMasterRepository>,
        alias_repo: Arc<dyn MedicationAliasRepository>,
    ) -> Self {
        Self {
            config,
            master_repo,
            alias_repo,
        }
    }

    /// Resolve a medication name to a master medication ID
    ///
    /// This is the main entry point for dynamic resolution.
    /// It tries multiple strategies in order of speed/confidence.
    pub async fn resolve(&self, medication_name: &str) -> ResolutionResult {
        let normalized = Self::normalize(medication_name);

        // Stage 1: Exact alias lookup (fastest, 100% confidence)
        if let Ok(Some(alias)) = self.alias_repo.get_by_name(&normalized).await
            && let Some(master_id) = alias.master_medication_id
        {
            // Get canonical name
            let canonical = if let Ok(Some(master)) = self.master_repo.get_by_id(master_id).await {
                master.canonical_name
            } else {
                medication_name.to_string()
            };

            tracing::debug!(
                medication = %medication_name,
                master_id = %master_id,
                "Resolved via exact alias"
            );

            return ResolutionResult::found(
                master_id,
                1.0,
                ResolutionMethod::ExactAlias,
                true, // Already approved alias
                canonical,
            );
        }

        // Stage 2: Exact master name lookup
        if let Ok(Some(master)) = self.master_repo.find_by_name(&normalized).await {
            tracing::debug!(
                medication = %medication_name,
                master_id = %master.id,
                "Resolved via exact master name"
            );

            return ResolutionResult::found(
                master.id,
                1.0,
                ResolutionMethod::ExactAlias,
                true,
                master.canonical_name,
            );
        }

        // Stage 3: Fuzzy string matching
        if let Ok(matches) = self.master_repo.search(&normalized, 3).await {
            for master in matches {
                let similarity = self.calculate_similarity(&normalized, &master.canonical_name);
                if similarity >= self.config.auto_approve_threshold {
                    tracing::debug!(
                        medication = %medication_name,
                        master = %master.canonical_name,
                        similarity = %similarity,
                        "Resolved via fuzzy match (auto-approved)"
                    );

                    // Auto-create alias for future lookups
                    if self.config.create_aliases {
                        self.create_alias(&normalized, master.id, similarity, true)
                            .await;
                    }

                    return ResolutionResult::found(
                        master.id,
                        similarity,
                        ResolutionMethod::FuzzyMatch,
                        true,
                        master.canonical_name,
                    );
                } else if similarity >= self.config.minimum_threshold {
                    tracing::debug!(
                        medication = %medication_name,
                        master = %master.canonical_name,
                        similarity = %similarity,
                        "Resolved via fuzzy match (needs review)"
                    );

                    // Create pending alias for review
                    if self.config.create_aliases {
                        self.create_alias(&normalized, master.id, similarity, false)
                            .await;
                    }

                    return ResolutionResult::found(
                        master.id,
                        similarity,
                        ResolutionMethod::FuzzyMatch,
                        false, // Needs review
                        master.canonical_name,
                    );
                }
            }
        }

        // Stage 4: No match found
        tracing::debug!(
            medication = %medication_name,
            "No master medication match found"
        );

        ResolutionResult::not_found()
    }

    /// Resolve with embedding (for when embedding is already available)
    pub async fn resolve_with_embedding(
        &self,
        medication_name: &str,
        embedding: &[f32],
    ) -> ResolutionResult {
        // First try non-embedding methods
        let result = self.resolve(medication_name).await;
        if result.master_medication_id.is_some() {
            return result;
        }

        let normalized = Self::normalize(medication_name);

        // Stage 4: Semantic embedding search
        if let Ok(matches) = self.master_repo.search_semantic(embedding, 3).await {
            for (master, score) in matches {
                let confidence = score as f64;
                if confidence >= self.config.auto_approve_threshold {
                    tracing::debug!(
                        medication = %medication_name,
                        master = %master.canonical_name,
                        confidence = %confidence,
                        "Resolved via semantic match (auto-approved)"
                    );

                    if self.config.create_aliases {
                        self.create_alias(&normalized, master.id, confidence, true)
                            .await;
                    }

                    return ResolutionResult::found(
                        master.id,
                        confidence,
                        ResolutionMethod::SemanticMatch,
                        true,
                        master.canonical_name,
                    );
                } else if confidence >= self.config.minimum_threshold {
                    tracing::debug!(
                        medication = %medication_name,
                        master = %master.canonical_name,
                        confidence = %confidence,
                        "Resolved via semantic match (needs review)"
                    );

                    if self.config.create_aliases {
                        self.create_alias(&normalized, master.id, confidence, false)
                            .await;
                    }

                    return ResolutionResult::found(
                        master.id,
                        confidence,
                        ResolutionMethod::SemanticMatch,
                        false,
                        master.canonical_name,
                    );
                }
            }
        }

        ResolutionResult::not_found()
    }

    /// Normalize a medication name for comparison
    fn normalize(name: &str) -> String {
        name.trim().to_lowercase()
    }

    /// Calculate string similarity (Jaro-Winkler style)
    fn calculate_similarity(&self, a: &str, b: &str) -> f64 {
        let a_lower = a.to_lowercase();
        let b_lower = b.to_lowercase();

        if a_lower == b_lower {
            return 1.0;
        }

        // Simple Levenshtein-based similarity
        let max_len = a_lower.len().max(b_lower.len());
        if max_len == 0 {
            return 1.0;
        }

        let distance = Self::levenshtein(&a_lower, &b_lower);
        1.0 - (distance as f64 / max_len as f64)
    }

    /// Levenshtein distance
    fn levenshtein(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let a_len = a_chars.len();
        let b_len = b_chars.len();

        if a_len == 0 {
            return b_len;
        }
        if b_len == 0 {
            return a_len;
        }

        let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

        for (i, item) in matrix.iter_mut().enumerate().take(a_len + 1) {
            item[0] = i;
        }
        for (j, item) in matrix.iter_mut().enumerate().take(b_len + 1) {
            item[0] = j;
        }

        for i in 1..=a_len {
            for j in 1..=b_len {
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[a_len][b_len]
    }

    /// Create an alias record
    async fn create_alias(
        &self,
        normalized_name: &str,
        master_id: Uuid,
        confidence: f64,
        approved: bool,
    ) {
        use chrono::Utc;
        use pharma_db::entity::medication_alias::{CurationStatus, Model as AliasModel};

        let alias = AliasModel {
            id: Uuid::new_v4(),
            alias_name: normalized_name.to_string(),
            alias_name_normalized: normalized_name.to_string(),
            master_medication_id: Some(master_id),
            ai_suggestion_confidence: Some(confidence),
            curation_status: if approved {
                CurationStatus::Approved
            } else {
                CurationStatus::Pending
            },
            curated_by: if approved {
                Some("system:auto".to_string())
            } else {
                None
            },
            curated_at: if approved { Some(Utc::now()) } else { None },
            occurrence_count: 1,
            first_seen_at: Utc::now(),
            last_seen_at: Utc::now(),
        };

        if let Err(e) = self.alias_repo.save(&alias).await {
            // Ignore duplicate key errors (alias already exists)
            if !e.to_string().contains("duplicate") {
                tracing::warn!(error = %e, alias = %normalized_name, "Failed to create alias");
            }
        }
    }

    /// Get current configuration
    pub fn config(&self) -> &MedicationResolverConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: MedicationResolverConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize() {
        assert_eq!(
            MedicationResolver::normalize("  Paracetamol 500mg  "),
            "paracetamol 500mg"
        );
        assert_eq!(MedicationResolver::normalize("AUGMENTIN"), "augmentin");
    }

    #[test]
    fn test_levenshtein() {
        assert_eq!(MedicationResolver::levenshtein("kitten", "sitting"), 3);
        assert_eq!(MedicationResolver::levenshtein("", "abc"), 3);
        assert_eq!(MedicationResolver::levenshtein("abc", "abc"), 0);
    }

    #[test]
    fn test_resolution_result_not_found() {
        let result = ResolutionResult::not_found();
        assert!(result.master_medication_id.is_none());
        assert_eq!(result.confidence, 0.0);
        assert_eq!(result.method, ResolutionMethod::NotFound);
    }

    #[test]
    fn test_resolution_method_display() {
        assert_eq!(format!("{}", ResolutionMethod::ExactAlias), "exact_alias");
        assert_eq!(format!("{}", ResolutionMethod::FuzzyMatch), "fuzzy_match");
        assert_eq!(
            format!("{}", ResolutionMethod::SemanticMatch),
            "semantic_match"
        );
        assert_eq!(format!("{}", ResolutionMethod::NotFound), "not_found");
    }

    #[test]
    fn test_default_config() {
        let config = MedicationResolverConfig::default();
        assert_eq!(config.auto_approve_threshold, 0.92);
        assert_eq!(config.minimum_threshold, 0.70);
        assert!(config.create_aliases);
        assert!(!config.auto_create_masters);
    }
}
