use crate::config::Config;
use crate::quota::QuotaTracker;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::{Arc, Mutex};

pub mod anthropic;
pub mod google;
pub mod vertex;

#[derive(Clone)]
pub struct AiClient {
    provider: Arc<Box<dyn AiProvider>>, // Use Arc for thread safety cloning
    model: String,
    quota_tracker: Arc<Mutex<QuotaTracker>>,
    config: Config,
}

#[derive(Debug)]
pub struct ProviderResponse {
    pub content: String,
    pub tokens: u32,
}

#[derive(Debug, Clone)]
pub struct QuotaStatus {
    pub used_tokens: u32,
    pub limit_tokens: Option<u32>,
    pub used_usd: f64,
    pub limit_usd: Option<f64>,
    pub reset_in: std::time::Duration,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip)]
    pub collapsed: bool,
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn send_message(&self, messages: Vec<ChatMessage>) -> Result<ProviderResponse>;
    async fn list_models(&self) -> Result<Vec<String>> {
        Ok(vec![]) // Default: no dynamic discovery
    }
}

// --- OpenAI Implementation ---

pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: Option<String>,
    api_base: String,
    model: String,
}

impl OpenAiProvider {
    pub fn new(config: &Config) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: config.openai_api_key.clone(),
            api_base: config.api_base.clone(),
            model: config.model.clone(),
        }
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize, Debug)]
struct Usage {
    total_tokens: u32,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: MessageContent,
}

#[derive(Deserialize, Debug)]
struct MessageContent {
    content: String,
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    async fn send_message(&self, messages: Vec<ChatMessage>) -> Result<ProviderResponse> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
        };

        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));
        let mut builder = self.client.post(&url);

        if let Some(key) = &self.api_key {
            builder = builder.header("Authorization", format!("Bearer {}", key));
        }

        let response = builder
            .json(&request)
            .send()
            .await
            .context(format!("Failed to send request to AI at {}", url))?;

        let chat_response: ChatResponse = response
            .json()
            .await
            .context("Failed to parse AI response")?;

        if let Some(choice) = chat_response.choices.into_iter().next() {
            let tokens = chat_response.usage.map(|u| u.total_tokens).unwrap_or(0);
            Ok(ProviderResponse {
                content: choice.message.content,
                tokens,
            })
        } else {
            Err(anyhow::anyhow!("No response content from AI"))
        }
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/models", self.api_base.trim_end_matches('/'));
        let mut builder = self.client.get(&url);
        if let Some(key) = &self.api_key {
            builder = builder.header("Authorization", format!("Bearer {}", key));
        }
        let response = builder.send().await.context("Failed to fetch models")?;
        let models: ModelsResponse = response
            .json()
            .await
            .context("Failed to parse models response")?;
        Ok(models.data.into_iter().map(|m| m.id).collect())
    }
}

// --- End OpenAI Implementation ---

// Response types for OpenAI models API
#[derive(Deserialize, Debug)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize, Debug)]
struct ModelEntry {
    id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Analysis {
    pub analysis: String,
    pub fix: Option<Fix>,
    #[serde(default)]
    pub tokens_used: u32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Fix {
    pub key: String,
    pub value: serde_json::Value,
}

impl AiClient {
    pub fn new(config: Config) -> Self {
        let quota_tracker = Arc::new(Mutex::new(QuotaTracker::load().unwrap_or_default()));

        // Factory logic for provider
        let provider: Box<dyn AiProvider> = if let Some(p) = &config.provider {
            match p.as_str() {
                "google" => Box::new(google::GoogleProvider::new(&config)),
                "anthropic" => Box::new(anthropic::AnthropicProvider::new(&config)),
                "vertex" => Box::new(vertex::VertexAiProvider::new(&config)),
                _ => Box::new(OpenAiProvider::new(&config)),
            }
        } else {
            // Default
            Box::new(OpenAiProvider::new(&config))
        };

        Self {
            provider: Arc::new(provider),
            model: config.model.clone(),
            quota_tracker,
            config,
        }
    }

    /// Returns the current model name
    pub fn get_model(&self) -> &str {
        &self.model
    }

    /// Build a context summary string from the current app state for injection
    /// into the chat system prompt or as a /context response.
    pub fn build_context_summary(
        task: Option<&str>,
        vars: Option<&serde_json::Value>,
        failed_task: Option<&str>,
        failed_result: Option<&serde_json::Value>,
    ) -> String {
        let mut parts = Vec::new();

        if let Some(t) = task {
            parts.push(format!("**Current Task:** {}", t));
        }
        if let Some(ft) = failed_task {
            parts.push(format!("**Failed Task:** {}", ft));
        }
        if let Some(fr) = failed_result {
            let json_str = serde_json::to_string_pretty(fr).unwrap_or_else(|_| fr.to_string());
            // Truncate very large results
            let truncated = if json_str.len() > 2000 {
                format!("{}\n... (truncated)", &json_str[..2000])
            } else {
                json_str
            };
            parts.push(format!("**Failure Details:**\n```json\n{}\n```", truncated));
        }
        if let Some(v) = vars
            && !v.is_null()
        {
            let json_str = serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string());
            let truncated = if json_str.len() > 2000 {
                format!("{}\n... (truncated)", &json_str[..2000])
            } else {
                json_str
            };
            parts.push(format!("**Task Variables:**\n```json\n{}\n```", truncated));
        }

        if parts.is_empty() {
            "No active context. Start a playbook to populate context.".to_string()
        } else {
            parts.join("\n\n")
        }
    }

    // [NEW] Chat Interface
    pub async fn chat(&self, history: Vec<ChatMessage>) -> Result<ChatMessage> {
        // Create AI span
        let ai_span = crate::telemetry::create_child_span(
            "ai.chat",
            vec![opentelemetry::KeyValue::new("ai.model", self.model.clone())],
        );
        let _ = crate::telemetry::attach_span(ai_span);
        let start = std::time::Instant::now();

        // Check Quota
        {
            let tracker = self.quota_tracker.lock().unwrap();
            tracker.check_limit(&self.config)?;
        }

        // Call Provider
        let mut messages = history.clone();
        if !messages.iter().any(|m| m.role == "system") {
            messages.insert(
                0,
                ChatMessage {
                    role: "system".to_string(),
                    content: "You are a helpful AI assistant integrated into Ansible Piloteer. \
                           Help the user debug playbooks, explain errors, and suggest fixes. \
                           When providing code, use markdown code blocks."
                        .to_string(),
                    collapsed: false,
                },
            );
        }

        let response = match self.provider.send_message(messages).await {
            Ok(r) => r,
            Err(e) => {
                crate::telemetry::record_error_on_current_span(&format!(
                    "AI Request Failed: {}",
                    e
                ));
                return Err(e);
            }
        };

        // Log interaction (last user message vs response)
        let last_user_msg = history.last().map(|m| m.content.as_str()).unwrap_or("");
        if let Err(e) = self
            .log_interaction(last_user_msg, &response.content, response.tokens)
            .await
        {
            eprintln!("Failed to log AI interaction: {}", e);
        }

        // Update Quota
        if let Ok(mut tracker) = self.quota_tracker.lock() {
            let _ = tracker.add_usage(response.tokens, &self.model);
        }

        // Record metrics
        let duration_ms = start.elapsed().as_millis() as i64;
        crate::telemetry::add_attributes_to_current_span(vec![
            opentelemetry::KeyValue::new("ai.response_time_ms", duration_ms),
            opentelemetry::KeyValue::new("ai.tokens_used", response.tokens as i64),
            opentelemetry::KeyValue::new("ai.success", true),
        ]);

        Ok(ChatMessage {
            role: "assistant".to_string(),
            content: response.content,
            collapsed: false,
        })
    }

    pub async fn list_models(&self) -> Vec<String> {
        // Try dynamic discovery from the provider
        match self.provider.list_models().await {
            Ok(models) if !models.is_empty() => {
                let mut result: Vec<String> = models;
                // Ensure current model is in the list
                if !result.contains(&self.model) {
                    result.push(self.model.clone());
                }
                result.sort();
                result
            }
            _ => {
                // Fallback to defaults
                vec![
                    "gpt-5-latest".to_string(),
                    "gpt-5-mini".to_string(),
                    "gemini-flash-latest".to_string(),
                    "gemini-3.0-flash-preview".to_string(),
                    "gemini-3.0-pro-preview".to_string(),
                    "claude-opus-4-6".to_string(),
                    "claude-sonnet-4-5".to_string(),
                    "claude-haiku-4-5".to_string(),
                    self.model.clone(),
                ]
                .into_iter()
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect()
            }
        }
    }

    pub fn set_model(&mut self, model_name: &str) {
        self.model = model_name.to_string();
        self.config.model = model_name.to_string();

        // Auto-detect provider from model name
        if model_name.starts_with("claude") {
            // Prefer direct Anthropic if API key is configured, otherwise try Vertex
            if self.config.anthropic_api_key.is_some() {
                self.config.provider = Some("anthropic".to_string());
                self.provider = Arc::new(Box::new(anthropic::AnthropicProvider::new(&self.config)));
            } else if self.config.vertex_project_id.is_some() {
                self.config.provider = Some("vertex".to_string());
                self.provider = Arc::new(Box::new(vertex::VertexAiProvider::new(&self.config)));
            } else {
                // Fallback to Anthropic (will error at runtime if no key)
                self.config.provider = Some("anthropic".to_string());
                self.provider = Arc::new(Box::new(anthropic::AnthropicProvider::new(&self.config)));
            }
        } else if model_name.starts_with("gemini") || model_name.starts_with("gemma") {
            self.config.provider = Some("google".to_string());
            self.provider = Arc::new(Box::new(google::GoogleProvider::new(&self.config)));
        } else if model_name.starts_with("gpt") {
            self.config.provider = Some("openai".to_string());
            self.provider = Arc::new(Box::new(OpenAiProvider::new(&self.config)));
        } else {
            // Unknown prefix — use OpenAI-compatible as default
            self.config.provider = Some("openai".to_string());
            self.provider = Arc::new(Box::new(OpenAiProvider::new(&self.config)));
        }
    }

    pub async fn analyze_failure(
        &self,
        task_name: &str,
        error_msg: &str,
        vars: &serde_json::Value,
        facts: Option<&serde_json::Value>,
    ) -> Result<Analysis> {
        // Create AI span
        let ai_span = crate::telemetry::create_child_span(
            "ai.analyze_failure",
            vec![
                opentelemetry::KeyValue::new("ai.model", self.model.clone()),
                opentelemetry::KeyValue::new("ai.task_name", task_name.to_string()),
            ],
        );
        let _ = crate::telemetry::attach_span(ai_span);
        let start = std::time::Instant::now();

        // Check Quota
        {
            let tracker = self.quota_tracker.lock().unwrap();
            tracker.check_limit(&self.config)?;
        }

        let system_prompt = "You are an expert Ansible debugger. \
            Analyze the following task failure and provided variables. \
            Explain why it failed and suggest a specific variable change or fix. \
            Output ONLY valid JSON in the following format: \
            { \"analysis\": \"...explanation...\", \"fix\": { \"key\": \"variable_name\", \"value\": ...val... } } \
            If no fix is possible, omit the \"fix\" field.";

        let user_content = format!(
            "Task: {}\nError: {}\nVariables: {}\nFacts: {}",
            task_name,
            error_msg,
            serde_json::to_string_pretty(vars).unwrap_or_default(),
            if let Some(f) = facts {
                serde_json::to_string_pretty(f).unwrap_or_default()
            } else {
                "None".to_string()
            }
        );

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
                collapsed: false,
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_content.clone(),
                collapsed: false,
            },
        ];

        // Call Provider
        let response = match self.provider.send_message(messages).await {
            Ok(r) => r,
            Err(e) => {
                crate::telemetry::record_error_on_current_span(&format!(
                    "AI Request Failed: {}",
                    e
                ));
                return Err(e);
            }
        };

        // Log interaction
        if let Err(e) = self
            .log_interaction(&user_content, &response.content, response.tokens)
            .await
        {
            eprintln!("Failed to log AI interaction: {}", e);
        }

        // Parse
        let mut analysis = match Self::parse_response(&response.content) {
            Ok(a) => a,
            Err(e) => {
                crate::telemetry::record_error_on_current_span(&format!(
                    "Failed to parse JSON: {}",
                    e
                ));
                return Err(e);
            }
        };

        analysis.tokens_used = response.tokens;

        // Update Quota
        if let Ok(mut tracker) = self.quota_tracker.lock() {
            let _ = tracker.add_usage(response.tokens, &self.model);
        }

        // Record metrics
        let duration_ms = start.elapsed().as_millis() as i64;
        crate::telemetry::add_attributes_to_current_span(vec![
            opentelemetry::KeyValue::new("ai.response_time_ms", duration_ms),
            opentelemetry::KeyValue::new("ai.tokens_used", response.tokens as i64),
            opentelemetry::KeyValue::new("ai.success", true),
            opentelemetry::KeyValue::new("ai.fix_suggested", analysis.fix.is_some()),
        ]);

        Ok(analysis)
    }

    pub fn get_usage(&self) -> (u32, f64) {
        if let Ok(tracker) = self.quota_tracker.lock() {
            (tracker.usage_today_tokens, tracker.cost_today_usd)
        } else {
            (0, 0.0)
        }
    }

    pub fn get_quota_status(&self) -> QuotaStatus {
        let (used_tokens, used_usd, reset_in) = if let Ok(tracker) = self.quota_tracker.lock() {
            (
                tracker.usage_today_tokens,
                tracker.cost_today_usd,
                tracker.time_until_reset(),
            )
        } else {
            (0, 0.0, std::time::Duration::from_secs(0))
        };

        QuotaStatus {
            used_tokens,
            limit_tokens: self.config.quota_limit_tokens,
            used_usd,
            limit_usd: self.config.quota_limit_usd,
            reset_in,
        }
    }

    async fn log_interaction(&self, prompt: &str, response: &str, tokens: u32) -> Result<()> {
        let dir = Config::get_config_dir()?;
        let path = dir.join("ai_history.jsonl");

        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "model": self.model,
            "prompt": prompt,
            "response": response,
            "tokens": tokens,
        });

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        writeln!(file, "{}", serde_json::to_string(&entry)?)?;

        // [Phase 36] Sync usage to Antigravity IDE shared location
        if (std::env::var("ANTIGRAVITY_SESSION").is_ok()
            || std::env::var("ANTIGRAVITY_WORKSPACE").is_ok())
            && let Ok(home) = std::env::var("HOME")
        {
            let ag_dir = std::path::PathBuf::from(&home)
                .join(".config")
                .join("antigravity");
            let _ = std::fs::create_dir_all(&ag_dir);
            let ag_path = ag_dir.join("piloteer_usage.jsonl");
            let usage_entry = serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "tool": "ansible-piloteer",
                "model": self.model,
                "tokens": tokens,
            });
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(ag_path)
            {
                let _ = writeln!(
                    f,
                    "{}",
                    serde_json::to_string(&usage_entry).unwrap_or_default()
                );
            }
        }

        Ok(())
    }

    // Extracted for testing
    pub fn parse_response(content: &str) -> Result<Analysis> {
        // Cleanup markdown code blocks if present
        let clean_content = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        serde_json::from_str::<Analysis>(clean_content).context(format!(
            "Failed to parse AI JSON response: {}",
            clean_content
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_json() {
        let input = r#"
        {
            "analysis": "The variable is wrong",
            "fix": {
                "key": "foo",
                "value": true
            }
        }
        "#;
        let result = AiClient::parse_response(input).unwrap();
        assert_eq!(result.analysis, "The variable is wrong");
        assert_eq!(result.fix.unwrap().key, "foo");
    }

    #[test]
    fn test_parse_markdown_json() {
        let input = r#"
        ```json
        {
            "analysis": "Wrapped in markdown",
            "fix": null
        }
        ```
        "#;
        let result = AiClient::parse_response(input).unwrap();
        assert_eq!(result.analysis, "Wrapped in markdown");
        assert!(result.fix.is_none());
    }

    #[test]
    fn test_parse_invalid_json() {
        let input = "Not JSON";
        let result = AiClient::parse_response(input);
        assert!(result.is_err());
    }
}
