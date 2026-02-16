//! Hierarchical Medication Matcher with pg_textsearch
//!
//! Implements a multi-stage matching pipeline:
//! 1. Master Medication ID (deterministic, O(1))
//! 2. pg_textsearch BM25 (fast candidate generation, O(log n))
//! 3. Embedding Similarity (semantic matching, O(k))
//! 4. Fuzzy String Matching (fallback, O(n))
//!
//! Each stage filters candidates for the next stage.
//! Works even with empty master medication table.

use std::sync::Arc;
use uuid::Uuid;

use crate::domain::{Offer, Request};
use crate::matching::{cosine_similarity, medication_similarity};
use crate::repository::OfferRepository;

/// Configuration for hierarchical matching
#[derive(Debug, Clone)]
pub struct HierarchicalConfig {
    /// Enable pg_textsearch BM25 stage
    pub enable_bm25: bool,
    /// Minimum BM25 score to consider (negative values, lower = better)
    pub bm25_min_score: f64,
    /// Maximum candidates from BM25 stage
    pub bm25_max_candidates: usize,

    /// Enable embedding similarity stage
    pub enable_embeddings: bool,
    /// Minimum embedding similarity (0.0 to 1.0)
    pub embedding_min_similarity: f64,
    /// Maximum candidates from embedding stage
    pub embedding_max_candidates: usize,

    /// Enable fuzzy matching stage
    pub enable_fuzzy: bool,
    /// Minimum fuzzy similarity (0.0 to 1.0)
    pub fuzzy_min_similarity: f64,

    /// Minimum final score to return a match
    pub min_final_score: f64,
}

impl Default for HierarchicalConfig {
    fn default() -> Self {
        Self {
            enable_bm25: true,
            bm25_min_score: -1.0, // pg_textsearch returns negative scores
            bm25_max_candidates: 20,

            enable_embeddings: true,
            embedding_min_similarity: 0.85, // Increased from 0.7 to reduce false positives
            embedding_max_candidates: 10,

            enable_fuzzy: true,
            fuzzy_min_similarity: 0.75, // Increased from 0.6 to reduce false positives

            min_final_score: 0.75, // Increased from 0.6 to reduce false positives
        }
    }
}

/// Match result from hierarchical matching
#[derive(Debug, Clone)]
pub struct HierarchicalMatch {
    pub offer_id: Uuid,
    pub request_id: Uuid,
    pub score: f64,
    pub method: MatchMethod,
    pub explanation: String,
}

/// Method used to find the match
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMethod {
    /// Matched via master_medication_id
    MasterID,
    /// Matched via pg_textsearch BM25
    BM25,
    /// Matched via embedding similarity
    Embedding,
    /// Matched via fuzzy string matching
    Fuzzy,
}

impl std::fmt::Display for MatchMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchMethod::MasterID => write!(f, "master_id"),
            MatchMethod::BM25 => write!(f, "bm25"),
            MatchMethod::Embedding => write!(f, "embedding"),
            MatchMethod::Fuzzy => write!(f, "fuzzy"),
        }
    }
}

/// Hierarchical medication matcher
pub struct HierarchicalMatcher {
    config: HierarchicalConfig,
    offer_repo: Option<Arc<dyn OfferRepository>>,
}

impl HierarchicalMatcher {
    pub fn new(config: HierarchicalConfig, offer_repo: Option<Arc<dyn OfferRepository>>) -> Self {
        Self { config, offer_repo }
    }

    /// Match a request against a list of offers using hierarchical approach
    pub async fn match_request(
        &self,
        request: &Request,
        offers: &[Offer],
    ) -> Vec<HierarchicalMatch> {
        if offers.is_empty() {
            return vec![];
        }

        // Stage 1: Master Medication ID (deterministic)
        if let Some(request_master_id) = request.master_medication_id {
            let master_matches: Vec<HierarchicalMatch> = offers
                .iter()
                .filter_map(|offer| {
                    if offer.master_medication_id == Some(request_master_id) {
                        Some(HierarchicalMatch {
                            offer_id: offer.id,
                            request_id: request.id,
                            score: 1.0,
                            method: MatchMethod::MasterID,
                            explanation: format!(
                                "Exact match via master_medication_id: {}",
                                request_master_id
                            ),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            if !master_matches.is_empty() {
                tracing::debug!(
                    request_id = %request.id,
                    matches = master_matches.len(),
                    "Stage 1 (Master ID): Found deterministic matches"
                );
                return master_matches;
            }
        }

        // Stage 2: pg_textsearch BM25 (fast candidate generation)
        let candidates = if self.config.enable_bm25 {
            if let Some(offer_repo) = &self.offer_repo {
                match offer_repo
                    .search_bm25(
                        &request.medication,
                        self.config.bm25_max_candidates as i64,
                        self.config.bm25_min_score,
                    )
                    .await
                {
                    Ok(bm25_results) if !bm25_results.is_empty() => {
                        tracing::debug!(
                            request_id = %request.id,
                            request_med = %request.medication,
                            candidates = bm25_results.len(),
                            "Stage 2 (BM25): Found candidates"
                        );

                        // Filter to only the offers we have in memory
                        let offer_ids: std::collections::HashSet<Uuid> =
                            offers.iter().map(|o| o.id).collect();

                        let filtered: Vec<Offer> = bm25_results
                            .into_iter()
                            .filter_map(|(offer, _score)| {
                                if offer_ids.contains(&offer.id) {
                                    Some(offer)
                                } else {
                                    None
                                }
                            })
                            .collect();

                        if filtered.is_empty() {
                            tracing::debug!(
                                request_id = %request.id,
                                "Stage 2 (BM25): No candidates match in-memory offers, using all"
                            );
                            offers.to_vec()
                        } else {
                            filtered
                        }
                    }
                    Ok(_) => {
                        tracing::debug!(
                            request_id = %request.id,
                            "Stage 2 (BM25): No candidates found, using all offers"
                        );
                        offers.to_vec()
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            request_id = %request.id,
                            "Stage 2 (BM25): Search failed, falling back to all offers"
                        );
                        offers.to_vec()
                    }
                }
            } else {
                tracing::debug!(
                    request_id = %request.id,
                    "Stage 2 (BM25): No offer repository, using all offers"
                );
                offers.to_vec()
            }
        } else {
            offers.to_vec()
        };

        // Stage 3: Embedding Similarity (semantic matching)
        if self.config.enable_embeddings
            && let Some(request_emb) = &request.content_embedding
        {
            let mut embedding_matches: Vec<(f64, &Offer)> = candidates
                .iter()
                .filter_map(|offer| {
                    if let Some(offer_emb) = &offer.content_embedding
                        && let Ok(similarity) =
                            cosine_similarity(offer_emb.as_slice(), request_emb.as_slice())
                        && similarity >= self.config.embedding_min_similarity
                    {
                        return Some((similarity, offer));
                    }
                    None
                })
                .collect();

            // Sort by similarity (descending)
            embedding_matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

            // Take top candidates
            embedding_matches.truncate(self.config.embedding_max_candidates);

            if !embedding_matches.is_empty() {
                tracing::debug!(
                    request_id = %request.id,
                    matches = embedding_matches.len(),
                    "Stage 3 (Embedding): Found semantic matches"
                );

                // Validate with fuzzy matching to prevent false positives
                let validated_matches: Vec<HierarchicalMatch> = embedding_matches
                    .into_iter()
                    .filter_map(|(emb_score, offer)| {
                        let fuzzy_score =
                            medication_similarity(&offer.medication, &request.medication);

                        // If embedding is high but fuzzy is low, it's likely a false positive
                        // Stricter validation: reject if fuzzy score is significantly lower than embedding
                        if emb_score > 0.75 && fuzzy_score < 0.7 {
                            tracing::warn!(
                                offer_med = %offer.medication,
                                request_med = %request.medication,
                                emb_score = %emb_score,
                                fuzzy_score = %fuzzy_score,
                                "Embedding-fuzzy mismatch - rejecting potential false positive"
                            );
                            return None;
                        }

                        // Additional check: if fuzzy score is too low, reject regardless of embedding
                        if fuzzy_score < 0.7 {
                            tracing::debug!(
                                offer_med = %offer.medication,
                                request_med = %request.medication,
                                fuzzy_score = %fuzzy_score,
                                "Fuzzy score too low - rejecting"
                            );
                            return None;
                        }

                        // Use weighted combination: embedding 40% + fuzzy 60%
                        let final_score = emb_score * 0.4 + fuzzy_score * 0.6;

                        if final_score >= self.config.min_final_score {
                            Some(HierarchicalMatch {
                                offer_id: offer.id,
                                request_id: request.id,
                                score: final_score,
                                method: MatchMethod::Embedding,
                                explanation: format!(
                                    "Embedding: {:.0}%, Fuzzy: {:.0}%",
                                    emb_score * 100.0,
                                    fuzzy_score * 100.0
                                ),
                            })
                        } else {
                            None
                        }
                    })
                    .collect();

                if !validated_matches.is_empty() {
                    return validated_matches;
                }
            }
        }

        // Stage 4: Fuzzy String Matching (fallback)
        if self.config.enable_fuzzy {
            let fuzzy_matches: Vec<HierarchicalMatch> = candidates
                .iter()
                .filter_map(|offer| {
                    let fuzzy_score = medication_similarity(&offer.medication, &request.medication);

                    if fuzzy_score >= self.config.fuzzy_min_similarity
                        && fuzzy_score >= self.config.min_final_score
                    {
                        Some(HierarchicalMatch {
                            offer_id: offer.id,
                            request_id: request.id,
                            score: fuzzy_score,
                            method: MatchMethod::Fuzzy,
                            explanation: format!("Fuzzy: {:.0}%", fuzzy_score * 100.0),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            if !fuzzy_matches.is_empty() {
                tracing::debug!(
                    request_id = %request.id,
                    matches = fuzzy_matches.len(),
                    "Stage 4 (Fuzzy): Found string matches"
                );
                return fuzzy_matches;
            }
        }

        // No matches found
        tracing::debug!(
            request_id = %request.id,
            request_med = %request.medication,
            "No matches found in any stage"
        );
        vec![]
    }

    /// Get current configuration
    pub fn config(&self) -> &HierarchicalConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: HierarchicalConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use pgvector::Vector as PgVector;

    use crate::domain::{ItemStatus, UrgencyLevel};

    fn create_test_offer(id: &str, medication: &str, embedding: Option<Vec<f32>>) -> Offer {
        Offer {
            id: Uuid::parse_str(id).unwrap(),
            raw_message_id: Uuid::new_v4(),
            participant_id: Uuid::new_v4(),
            group_id: Uuid::new_v4(),
            medication: medication.to_string(),
            form: None,
            concentration: None,
            status: ItemStatus::Active,
            urgency_level: UrgencyLevel::Normal,
            expiry_info: None,
            ai_confidence: 0.9,
            content_embedding: embedding.map(PgVector::from),
            master_medication_id: None,
            medication_curated: false,
            confirmed_match_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_test_request(id: &str, medication: &str, embedding: Option<Vec<f32>>) -> Request {
        Request {
            id: Uuid::parse_str(id).unwrap(),
            raw_message_id: Uuid::new_v4(),
            participant_id: Uuid::new_v4(),
            group_id: Uuid::new_v4(),
            medication: medication.to_string(),
            form: None,
            concentration: None,
            urgency_level: UrgencyLevel::Normal,
            expiry_requirement: None,
            ai_confidence: 0.9,
            status: ItemStatus::Active,
            content_embedding: embedding.map(PgVector::from),
            master_medication_id: None,
            medication_curated: false,
            confirmed_match_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_master_id_match() {
        let config = HierarchicalConfig::default();
        let matcher = HierarchicalMatcher::new(config, None);

        let master_id = Uuid::new_v4();
        let mut offer =
            create_test_offer("00000000-0000-0000-0000-000000000001", "Augmentin 1g", None);
        offer.master_medication_id = Some(master_id);

        let mut request = create_test_request(
            "00000000-0000-0000-0000-000000000002",
            "اوجمنتين ١ جم",
            None,
        );
        request.master_medication_id = Some(master_id);

        let matches = matcher.match_request(&request, &[offer]).await;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].method, MatchMethod::MasterID);
        assert_eq!(matches[0].score, 1.0);
    }

    #[tokio::test]
    async fn test_fuzzy_match() {
        let config = HierarchicalConfig::default();
        let matcher = HierarchicalMatcher::new(config, None);

        let offer = create_test_offer("00000000-0000-0000-0000-000000000001", "Augmentin", None);
        let request =
            create_test_request("00000000-0000-0000-0000-000000000002", "Augmentin", None);

        let matches = matcher.match_request(&request, &[offer]).await;

        assert!(!matches.is_empty());
        assert_eq!(matches[0].method, MatchMethod::Fuzzy);
        assert!(matches[0].score > 0.9);
    }

    #[tokio::test]
    async fn test_embedding_match_with_validation() {
        let config = HierarchicalConfig::default();
        let matcher = HierarchicalMatcher::new(config, None);

        // Similar embeddings AND similar names (should match)
        let offer_emb = vec![0.5; 768];
        let mut request_emb = vec![0.5; 768];
        request_emb[0] = 0.51; // Slightly different

        let offer = create_test_offer(
            "00000000-0000-0000-0000-000000000001",
            "Paracetamol",
            Some(offer_emb),
        );
        let request = create_test_request(
            "00000000-0000-0000-0000-000000000002",
            "Paracetamol",
            Some(request_emb),
        );

        let matches = matcher.match_request(&request, &[offer]).await;

        assert!(!matches.is_empty());
        assert_eq!(matches[0].method, MatchMethod::Embedding);
    }

    #[tokio::test]
    async fn test_no_match_different_medications() {
        let config = HierarchicalConfig::default();
        let matcher = HierarchicalMatcher::new(config, None);

        let offer = create_test_offer("00000000-0000-0000-0000-000000000001", "Aspirin", None);
        let request = create_test_request("00000000-0000-0000-0000-000000000002", "Insulin", None);

        let matches = matcher.match_request(&request, &[offer]).await;

        assert!(matches.is_empty());
    }
}
