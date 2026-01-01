//! Message Pre-processor for Intent Segmentation
//!
//! Splits mixed offer/request messages into segments by intent markers
//! to improve AI parsing accuracy for complex Arabic pharmacy messages.

use regex::Regex;
use std::sync::OnceLock;

/// Intent type for a message segment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentIntent {
    /// Offering medications (selling)
    Offer,
    /// Requesting medications (buying)
    Request,
    /// Unknown intent (no clear marker)
    Unknown,
}

impl SegmentIntent {
    pub fn as_str(&self) -> &'static str {
        match self {
            SegmentIntent::Offer => "OFFER",
            SegmentIntent::Request => "REQUEST",
            SegmentIntent::Unknown => "UNKNOWN",
        }
    }
}

/// A segment of a message with detected intent
#[derive(Debug, Clone)]
pub struct MessageSegment {
    /// The content of this segment
    pub content: String,
    /// Detected intent for this segment
    pub intent: SegmentIntent,
    /// The marker that triggered this segment (if any)
    pub marker: Option<String>,
    /// Start position in original message
    pub start_pos: usize,
    /// End position in original message
    pub end_pos: usize,
}

/// Pre-processor for splitting messages by intent markers
pub struct MessagePreprocessor {
    offer_pattern: Regex,
    request_pattern: Regex,
}

impl Default for MessagePreprocessor {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagePreprocessor {
    /// Create a new preprocessor with Arabic/English intent patterns
    pub fn new() -> Self {
        // Offer markers: موجود, متوفر, عندي, عندنا, يوجد, للبيع, available, i have, for sale
        let offer_pattern = Regex::new(
            r"(?i)(?:^|[\s\n📌💊✅])(?:موجود|متوفر|عندي|عندنا|يوجد|للبيع|available|i\s*have|for\s*sale|selling|in\s*stock)"
        ).expect("Invalid offer regex");

        // Request markers: مطلوب, محتاج, عايز, نقص, ابغى, wanted, need, looking for
        let request_pattern = Regex::new(
            r"(?i)(?:^|[\s\n📢🔊❗])(?:مطلوب|محتاج|عايز|نقص|ابغى|wanted|need|looking\s*for|required|searching\s*for)"
        ).expect("Invalid request regex");

        Self {
            offer_pattern,
            request_pattern,
        }
    }

    /// Get singleton instance
    pub fn instance() -> &'static Self {
        static INSTANCE: OnceLock<MessagePreprocessor> = OnceLock::new();
        INSTANCE.get_or_init(Self::new)
    }

    /// Check if a message contains mixed intents (both offers and requests)
    pub fn has_mixed_intents(&self, content: &str) -> bool {
        let has_offer = self.offer_pattern.is_match(content);
        let has_request = self.request_pattern.is_match(content);
        has_offer && has_request
    }

    /// Split a message into segments by intent markers
    ///
    /// Returns segments in order of appearance, each tagged with its intent.
    /// For messages without mixed intents, returns a single segment.
    pub fn split_by_intent(&self, content: &str) -> Vec<MessageSegment> {
        // Collect all marker positions with their intents
        let mut markers: Vec<(usize, usize, SegmentIntent, String)> = Vec::new();

        // Find all offer markers
        for m in self.offer_pattern.find_iter(content) {
            markers.push((
                m.start(),
                m.end(),
                SegmentIntent::Offer,
                m.as_str().trim().to_string(),
            ));
        }

        // Find all request markers
        for m in self.request_pattern.find_iter(content) {
            markers.push((
                m.start(),
                m.end(),
                SegmentIntent::Request,
                m.as_str().trim().to_string(),
            ));
        }

        // If no markers found, return entire content as unknown
        if markers.is_empty() {
            return vec![MessageSegment {
                content: content.to_string(),
                intent: SegmentIntent::Unknown,
                marker: None,
                start_pos: 0,
                end_pos: content.len(),
            }];
        }

        // Sort markers by position
        markers.sort_by_key(|(start, _, _, _)| *start);

        // Build segments
        let mut segments = Vec::new();
        let content_len = content.len();

        // Handle content before first marker (if any)
        if markers[0].0 > 0 {
            let pre_content = content[..markers[0].0].trim();
            if !pre_content.is_empty() {
                segments.push(MessageSegment {
                    content: pre_content.to_string(),
                    intent: SegmentIntent::Unknown,
                    marker: None,
                    start_pos: 0,
                    end_pos: markers[0].0,
                });
            }
        }

        // Process each marker and its following content
        for (i, (start, end, intent, marker)) in markers.iter().enumerate() {
            // Determine where this segment ends (at next marker or end of content)
            let segment_end = if i + 1 < markers.len() {
                markers[i + 1].0
            } else {
                content_len
            };

            // Extract content after the marker
            let segment_content = content[*end..segment_end].trim();

            if !segment_content.is_empty() {
                segments.push(MessageSegment {
                    content: segment_content.to_string(),
                    intent: *intent,
                    marker: Some(marker.clone()),
                    start_pos: *start,
                    end_pos: segment_end,
                });
            }
        }

        segments
    }

    /// Build an enhanced prompt hint for the AI based on detected segments
    ///
    /// This adds context to help the AI understand the message structure.
    pub fn build_intent_hint(&self, content: &str) -> Option<String> {
        if !self.has_mixed_intents(content) {
            return None;
        }

        let segments = self.split_by_intent(content);
        if segments.len() <= 1 {
            return None;
        }

        let mut hint = String::from("\n# INTENT STRUCTURE DETECTED\n");
        hint.push_str(
            "This message contains BOTH offers AND requests. Parse each section correctly:\n",
        );

        for (i, seg) in segments.iter().enumerate() {
            if seg.intent != SegmentIntent::Unknown {
                hint.push_str(&format!(
                    "- Section {}: {} (marker: {})\n",
                    i + 1,
                    seg.intent.as_str(),
                    seg.marker.as_deref().unwrap_or("none")
                ));
            }
        }

        hint.push_str("\nItems after موجود/متوفر = OFFER\nItems after مطلوب/محتاج = REQUEST\n");

        Some(hint)
    }

    /// Pre-process content and return enhanced content with intent hints
    ///
    /// If the message has mixed intents, prepends a hint to help the AI.
    pub fn preprocess(&self, content: &str) -> String {
        match self.build_intent_hint(content) {
            Some(hint) => format!("{}\n{}", hint, content),
            None => content.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_offer_markers() {
        let pp = MessagePreprocessor::new();

        assert!(pp.offer_pattern.is_match("موجود اوجمنتين"));
        assert!(pp.offer_pattern.is_match("متوفر كونكور"));
        assert!(pp.offer_pattern.is_match("عندي انسولين"));
        assert!(pp.offer_pattern.is_match("📌موجود دواء"));
        assert!(pp.offer_pattern.is_match("I have Augmentin"));
        assert!(pp.offer_pattern.is_match("Available: Concor"));
    }

    #[test]
    fn test_detect_request_markers() {
        let pp = MessagePreprocessor::new();

        assert!(pp.request_pattern.is_match("مطلوب اوجمنتين"));
        assert!(pp.request_pattern.is_match("محتاج كونكور"));
        assert!(pp.request_pattern.is_match("عايز انسولين"));
        assert!(pp.request_pattern.is_match("📢مطلوب دواء"));
        assert!(pp.request_pattern.is_match("Need Augmentin"));
        assert!(pp.request_pattern.is_match("Looking for Concor"));
    }

    #[test]
    fn test_has_mixed_intents() {
        let pp = MessagePreprocessor::new();

        // Mixed message
        let mixed = "موجود اوجمنتين مطلوب كونكور";
        assert!(pp.has_mixed_intents(mixed));

        // Offer only
        let offer_only = "موجود اوجمنتين كونكور";
        assert!(!pp.has_mixed_intents(offer_only));

        // Request only
        let request_only = "مطلوب اوجمنتين مطلوب كونكور";
        assert!(!pp.has_mixed_intents(request_only));
    }

    #[test]
    fn test_split_by_intent_mixed() {
        let pp = MessagePreprocessor::new();

        let content = "📌موجود زاكتاجيكت ديسفيرال انبريل 50 📢مطلوب زولادكس صغير مطلوب لانتوس";
        let segments = pp.split_by_intent(content);

        assert!(segments.len() >= 2);

        // First segment should be offer
        let offer_seg = segments.iter().find(|s| s.intent == SegmentIntent::Offer);
        assert!(offer_seg.is_some());
        assert!(offer_seg.unwrap().content.contains("زاكتاجيكت"));

        // Should have request segments
        let request_segs: Vec<_> = segments
            .iter()
            .filter(|s| s.intent == SegmentIntent::Request)
            .collect();
        assert!(!request_segs.is_empty());
    }

    #[test]
    fn test_split_by_intent_single() {
        let pp = MessagePreprocessor::new();

        // Single intent message
        let content = "موجود اوجمنتين كونكور";
        let segments = pp.split_by_intent(content);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].intent, SegmentIntent::Offer);
    }

    #[test]
    fn test_split_by_intent_no_markers() {
        let pp = MessagePreprocessor::new();

        let content = "اوجمنتين كونكور";
        let segments = pp.split_by_intent(content);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].intent, SegmentIntent::Unknown);
    }

    #[test]
    fn test_build_intent_hint() {
        let pp = MessagePreprocessor::new();

        let mixed = "موجود اوجمنتين مطلوب كونكور";
        let hint = pp.build_intent_hint(mixed);

        assert!(hint.is_some());
        let hint_text = hint.unwrap();
        assert!(hint_text.contains("OFFER"));
        assert!(hint_text.contains("REQUEST"));
    }

    #[test]
    fn test_preprocess_mixed() {
        let pp = MessagePreprocessor::new();

        let content = "موجود اوجمنتين مطلوب كونكور";
        let processed = pp.preprocess(content);

        // Should contain hint
        assert!(processed.contains("INTENT STRUCTURE DETECTED"));
        // Should contain original content
        assert!(processed.contains(content));
    }

    #[test]
    fn test_preprocess_single_intent() {
        let pp = MessagePreprocessor::new();

        let content = "موجود اوجمنتين كونكور";
        let processed = pp.preprocess(content);

        // Should NOT contain hint (single intent)
        assert!(!processed.contains("INTENT STRUCTURE DETECTED"));
        // Should be unchanged
        assert_eq!(processed, content);
    }

    #[test]
    fn test_real_world_message() {
        let pp = MessagePreprocessor::new();

        let content = "📌موجود زاكتاجيكت ديسفيرال انبريل 50 سيتروتايد ربع 6/26 جونابيور 150 جونابيور 75 سيمبوني سنفسك ون شوت علبه جليكسامبي 10/5 ملجم 📢📢📢📢📢📢📢📢📢📢 مطلوب زولادكس صغير سعر قديم مطلوب لانتوس اقلام علب مطلوب جونال 900 ريكورمون 5000 سرنجات مطلوب اكواليكس حقن مطلوب كوريمون مطلوب ديكاببتيل مطلوب سيمبوني فرط تاريخ قريب مطلوب فكتوزا";

        assert!(pp.has_mixed_intents(content));

        let segments = pp.split_by_intent(content);

        // Should have offer segment
        let offers: Vec<_> = segments
            .iter()
            .filter(|s| s.intent == SegmentIntent::Offer)
            .collect();
        assert!(!offers.is_empty());

        // Offer segment should contain the offered medications
        let offer_content = &offers[0].content;
        assert!(offer_content.contains("زاكتاجيكت") || offer_content.contains("ديسفيرال"));

        // Should have request segments
        let requests: Vec<_> = segments
            .iter()
            .filter(|s| s.intent == SegmentIntent::Request)
            .collect();
        assert!(!requests.is_empty());
    }
}
