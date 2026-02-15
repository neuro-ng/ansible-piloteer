use crate::ai::{AiProvider, ChatMessage, ProviderResponse};
use crate::config::Config;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

// --- Anthropic API Request/Response Types ---

#[derive(Serialize)]
#[allow(dead_code)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    usage: AnthropicUsage,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModelEntry>,
}

#[derive(Deserialize)]
struct AnthropicModelEntry {
    id: String,
}

// --- Error Response ---

#[derive(Deserialize)]
struct AnthropicErrorResponse {
    error: Option<AnthropicErrorDetail>,
}

#[derive(Deserialize)]
struct AnthropicErrorDetail {
    message: String,
}

impl AnthropicProvider {
    pub fn new(config: &Config) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: config.anthropic_api_key.clone().unwrap_or_default(),
            model: config.model.clone(),
        }
    }
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    async fn send_message(&self, messages: Vec<ChatMessage>) -> Result<ProviderResponse> {
        // Convert ChatMessages to Anthropic format
        // Anthropic requires alternating user/assistant — filter out system for now
        let anthropic_messages: Vec<AnthropicMessage> = messages
            .iter()
            .filter(|m| m.role == "user" || m.role == "assistant")
            .map(|m| AnthropicMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        // Extract system message if present
        let system_content: Option<String> = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        let mut request = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": anthropic_messages,
        });

        if let Some(sys) = system_content {
            request["system"] = serde_json::Value::String(sys);
        }

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Anthropic API request failed")?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            // Try to parse error
            if let Ok(err) = serde_json::from_str::<AnthropicErrorResponse>(&body)
                && let Some(detail) = err.error
            {
                anyhow::bail!("Anthropic API Error ({}): {}", status, detail.message);
            }
            anyhow::bail!("Anthropic API Error ({}): {}", status, body);
        }

        let parsed: AnthropicResponse =
            serde_json::from_str(&body).context("Failed to parse Anthropic response")?;

        let content = parsed
            .content
            .iter()
            .filter_map(|b| b.text.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join("");

        let tokens = parsed.usage.input_tokens + parsed.usage.output_tokens;

        Ok(ProviderResponse { content, tokens })
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let response = self
            .client
            .get("https://api.anthropic.com/v1/models")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .send()
            .await
            .context("Failed to list Anthropic models")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Anthropic list models failed ({}): {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }

        let body = response.text().await?;
        let parsed: AnthropicModelsResponse =
            serde_json::from_str(&body).context("Failed to parse Anthropic models response")?;

        let mut models: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
        models.sort();
        Ok(models)
    }
}
