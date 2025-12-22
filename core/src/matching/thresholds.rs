use crate::domain::ConfidenceBand;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for smooth threshold transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmoothThresholdConfig {
    /// Transition zone width (as fraction of threshold gap)
    pub transition_width: f64, // Default: 0.1
    /// Enable smooth transitions
    pub enable_smoothing: bool,
    /// Current thresholds
    pub auto_threshold: f64, // Default: 0.90
    pub suggest_threshold: f64, // Default: 0.70
    pub review_threshold: f64,  // Default: 0.50
}

impl Default for SmoothThresholdConfig {
    fn default() -> Self {
        Self {
            transition_width: 0.1,
            enable_smoothing: true,
            auto_threshold: 0.90,
            suggest_threshold: 0.70,
            review_threshold: 0.50,
        }
    }
}

/// Result of smooth confidence calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmoothConfidenceResult {
    pub primary_band: ConfidenceBand,
    pub raw_score: f64,
    pub smooth_confidence: f64,
    pub band_strength: f64,
    pub in_transition_zone: bool,
    pub near_boundary: Option<String>,
}

/// SmoothThresholdCalculator provides smooth confidence transitions
pub struct SmoothThresholdCalculator {
    config: Arc<RwLock<SmoothThresholdConfig>>,
}

impl SmoothThresholdCalculator {
    pub fn new(config: SmoothThresholdConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
        }
    }

    pub async fn calculate(&self, score: f64) -> SmoothConfidenceResult {
        let config = self.config.read().await;

        let primary_band = if score >= config.auto_threshold {
            ConfidenceBand::Auto
        } else if score >= config.suggest_threshold {
            ConfidenceBand::Suggest
        } else if score >= config.review_threshold {
            ConfidenceBand::Review
        } else {
            ConfidenceBand::None
        };

        if !config.enable_smoothing {
            return SmoothConfidenceResult {
                primary_band,
                raw_score: score,
                smooth_confidence: score,
                band_strength: 1.0,
                in_transition_zone: false,
                near_boundary: None,
            };
        }

        match primary_band {
            ConfidenceBand::Auto => self.calculate_auto_zone(score, &config),
            ConfidenceBand::Suggest => self.calculate_suggest_zone(score, &config),
            ConfidenceBand::Review => self.calculate_review_zone(score, &config),
            ConfidenceBand::None => self.calculate_none_zone(score, &config),
        }
    }

    fn calculate_auto_zone(
        &self,
        score: f64,
        config: &SmoothThresholdConfig,
    ) -> SmoothConfidenceResult {
        let band_width = 1.0 - config.auto_threshold;
        let transition_width = band_width * config.transition_width;
        let distance = score - config.auto_threshold;

        if distance < transition_width {
            let t = distance / transition_width;
            let strength = smooth_step(t);
            SmoothConfidenceResult {
                primary_band: ConfidenceBand::Auto,
                raw_score: score,
                smooth_confidence: config.auto_threshold
                    + (score - config.auto_threshold) * strength,
                band_strength: strength,
                in_transition_zone: true,
                near_boundary: Some("lower".to_string()),
            }
        } else {
            SmoothConfidenceResult {
                primary_band: ConfidenceBand::Auto,
                raw_score: score,
                smooth_confidence: score,
                band_strength: 1.0,
                in_transition_zone: false,
                near_boundary: None,
            }
        }
    }

    fn calculate_suggest_zone(
        &self,
        score: f64,
        config: &SmoothThresholdConfig,
    ) -> SmoothConfidenceResult {
        let band_width = config.auto_threshold - config.suggest_threshold;
        let transition_width = band_width * config.transition_width;
        let dist_lower = score - config.suggest_threshold;
        let dist_upper = config.auto_threshold - score;

        if dist_upper < transition_width {
            let t = dist_upper / transition_width;
            let strength = smooth_step(t);
            SmoothConfidenceResult {
                primary_band: ConfidenceBand::Suggest,
                raw_score: score,
                smooth_confidence: score + (config.auto_threshold - score) * (1.0 - strength) * 0.5,
                band_strength: strength,
                in_transition_zone: true,
                near_boundary: Some("upper".to_string()),
            }
        } else if dist_lower < transition_width {
            let t = dist_lower / transition_width;
            let strength = smooth_step(t);
            SmoothConfidenceResult {
                primary_band: ConfidenceBand::Suggest,
                raw_score: score,
                smooth_confidence: config.suggest_threshold
                    + (score - config.suggest_threshold) * strength,
                band_strength: strength,
                in_transition_zone: true,
                near_boundary: Some("lower".to_string()),
            }
        } else {
            SmoothConfidenceResult {
                primary_band: ConfidenceBand::Suggest,
                raw_score: score,
                smooth_confidence: score,
                band_strength: 1.0,
                in_transition_zone: false,
                near_boundary: None,
            }
        }
    }

    fn calculate_review_zone(
        &self,
        score: f64,
        config: &SmoothThresholdConfig,
    ) -> SmoothConfidenceResult {
        let band_width = config.suggest_threshold - config.review_threshold;
        let transition_width = band_width * config.transition_width;
        let dist_lower = score - config.review_threshold;
        let dist_upper = config.suggest_threshold - score;

        if dist_upper < transition_width {
            let t = dist_upper / transition_width;
            let strength = smooth_step(t);
            SmoothConfidenceResult {
                primary_band: ConfidenceBand::Review,
                raw_score: score,
                smooth_confidence: score
                    + (config.suggest_threshold - score) * (1.0 - strength) * 0.5,
                band_strength: strength,
                in_transition_zone: true,
                near_boundary: Some("upper".to_string()),
            }
        } else if dist_lower < transition_width {
            let t = dist_lower / transition_width;
            let strength = smooth_step(t);
            SmoothConfidenceResult {
                primary_band: ConfidenceBand::Review,
                raw_score: score,
                smooth_confidence: config.review_threshold
                    + (score - config.review_threshold) * strength,
                band_strength: strength,
                in_transition_zone: true,
                near_boundary: Some("lower".to_string()),
            }
        } else {
            SmoothConfidenceResult {
                primary_band: ConfidenceBand::Review,
                raw_score: score,
                smooth_confidence: score,
                band_strength: 1.0,
                in_transition_zone: false,
                near_boundary: None,
            }
        }
    }

    fn calculate_none_zone(
        &self,
        score: f64,
        config: &SmoothThresholdConfig,
    ) -> SmoothConfidenceResult {
        let transition_width = config.review_threshold * config.transition_width;
        let dist_upper = config.review_threshold - score;

        if dist_upper < transition_width && score > 0.0 {
            let t = dist_upper / transition_width;
            let strength = smooth_step(t);
            SmoothConfidenceResult {
                primary_band: ConfidenceBand::None,
                raw_score: score,
                smooth_confidence: score * strength,
                band_strength: strength,
                in_transition_zone: true,
                near_boundary: Some("upper".to_string()),
            }
        } else {
            SmoothConfidenceResult {
                primary_band: ConfidenceBand::None,
                raw_score: score,
                smooth_confidence: score,
                band_strength: 1.0,
                in_transition_zone: false,
                near_boundary: None,
            }
        }
    }

    pub fn config(&self) -> Arc<RwLock<SmoothThresholdConfig>> {
        self.config.clone()
    }

    pub async fn update_config(&self, new_config: SmoothThresholdConfig) {
        let mut config = self.config.write().await;
        *config = new_config;
    }
}

/// Hermite interpolation: 3t² - 2t³
fn smooth_step(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_smooth_step() {
        assert_eq!(smooth_step(0.0), 0.0);
        assert_eq!(smooth_step(1.0), 1.0);
        assert!(smooth_step(0.5) > 0.4 && smooth_step(0.5) < 0.6);
    }

    #[tokio::test]
    async fn test_calculator_auto_transition() {
        let calc = SmoothThresholdCalculator::new(SmoothThresholdConfig::default());

        // Score exactly at threshold
        let res = calc.calculate(0.90).await;
        assert_eq!(res.primary_band, ConfidenceBand::Auto);
        assert!(res.in_transition_zone);
        assert_eq!(res.near_boundary, Some("lower".to_string()));
        assert_eq!(res.smooth_confidence, 0.90);

        // Score slightly above threshold
        let res = calc.calculate(0.901).await;
        assert!(res.smooth_confidence > 0.90);
        assert!(res.smooth_confidence < 0.901); // Pulled down towards threshold
    }
}
