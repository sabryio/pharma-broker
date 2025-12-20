//! Prompt builder utilities

/// Builder for constructing prompts
#[derive(Debug, Clone, Default)]
pub struct PromptBuilder {
    system: Option<String>,
    context: Vec<String>,
    user_content: Option<String>,
}

impl PromptBuilder {
    /// Create a new prompt builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the system prompt
    pub fn system(mut self, prompt: impl Into<String>) -> Self {
        self.system = Some(prompt.into());
        self
    }

    /// Add context (e.g., examples, mappings)
    pub fn context(mut self, ctx: impl Into<String>) -> Self {
        self.context.push(ctx.into());
        self
    }

    /// Set the user content to process
    pub fn user(mut self, content: impl Into<String>) -> Self {
        self.user_content = Some(content.into());
        self
    }

    /// Build the system prompt (combining base + context)
    pub fn build_system(&self) -> String {
        let mut result = self.system.clone().unwrap_or_default();
        for ctx in &self.context {
            result.push_str("\n\n");
            result.push_str(ctx);
        }
        result
    }

    /// Build the user prompt
    pub fn build_user(&self) -> String {
        self.user_content.clone().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_builder() {
        let builder = PromptBuilder::new()
            .system("You are an AI assistant.")
            .context("## Rules\n- Be helpful")
            .user("Hello world");

        let system = builder.build_system();
        assert!(system.contains("You are an AI assistant."));
        assert!(system.contains("## Rules"));

        let user = builder.build_user();
        assert_eq!(user, "Hello world");
    }
}
