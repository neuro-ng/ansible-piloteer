use crate::ai::{AiProvider, ProviderResponse};
use crate::config::Config;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Vertex AI Provider — uses Google Cloud's Vertex AI endpoint.
/// Supports both Gemini models (native) and Claude models (via Model Garden).
/// Auth: Uses OAuth token from `auth_token` or `GOOGLE_APPLICATION_CREDENTIALS`.
pub struct VertexAiProvider {
    client: reqwest::Client,
    project_id: String,
    location: String,
    auth_token: Option<String>,
    model: String,
}

impl VertexAiProvider {
    pub fn new(config: &Config) -> Self {
        Self {
            client: reqwest::Client::new(),
            project_id: config.vertex_project_id.clone().unwrap_or_default(),
            location: config
                .vertex_location
                .clone()
                .unwrap_or_else(|| "us-central1".to_string()),
            auth_token: config.auth_token.clone(),
            model: config.model.clone(),
        }
    }

    /// Determine if the model is a Claude model (served via Model Garden / Anthropic on Vertex)
    fn is_claude_model(&self) -> bool {
        self.model.starts_with("claude")
    }
}

// --- Gemini on Vertex (same format as Google AI) ---

#[derive(Serialize)]
struct GenerateContentRequest {
    contents: Vec<Content>,
}

#[derive(Serialize)]
struct Content {
    role: String,
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Part {
    text: String,
}

#[derive(Deserialize, Debug)]
struct GenerateContentResponse {
    candidates: Option<Vec<Candidate>>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Deserialize, Debug)]
struct Candidate {
    content: Option<CandidateContent>,
}

#[derive(Deserialize, Debug)]
struct CandidateContent {
    parts: Option<Vec<PartResponse>>,
}

#[derive(Deserialize, Debug)]
struct PartResponse {
    text: String,
}

#[derive(Deserialize, Debug)]
struct UsageMetadata {
    #[serde(rename = "totalTokenCount")]
    total_token_count: Option<u32>,
}

// --- Claude on Vertex (Anthropic format with Vertex endpoint) ---

#[derive(Serialize)]
struct ClaudeVertexRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ClaudeMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    anthropic_version: String,
}

#[derive(Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ClaudeVertexResponse {
    content: Vec<ClaudeContentBlock>,
    usage: ClaudeUsage,
}

#[derive(Deserialize)]
struct ClaudeContentBlock {
    text: Option<String>,
}

#[derive(Deserialize)]
struct ClaudeUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[async_trait]
impl AiProvider for VertexAiProvider {
    async fn send_message(
        &self,
        messages: Vec<crate::ai::ChatMessage>,
    ) -> Result<ProviderResponse> {
        if self.project_id.is_empty() {
            anyhow::bail!(
                "Vertex AI requires vertex_project_id. Set PILOTEER_VERTEX_PROJECT_ID or add vertex_project_id to piloteer.toml"
            );
        }

        let token = self
            .auth_token
            .as_ref()
            .context("Vertex AI requires an auth token. Run `piloteer auth login` or set PILOTEER_AUTH_TOKEN")?;

        if self.is_claude_model() {
            self.send_claude_message(messages, token).await
        } else {
            self.send_gemini_message(messages, token).await
        }
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        // Vertex AI doesn't have a simple list endpoint like Google AI.
        // Return a curated list of known available models.
        let mut models = vec![
            // Gemini models on Vertex
            "gemini-3-flash-preview".to_string(),
            "gemini-3-pro-preview".to_string(),
            // Claude models on Vertex (via Model Garden)
            "claude-opus-4-6".to_string(),
            "claude-sonnet-4-5".to_string(),
            "claude-haiku-4-5".to_string(),
        ];
        models.sort();
        Ok(models)
    }
}

impl VertexAiProvider {
    async fn send_gemini_message(
        &self,
        messages: Vec<crate::ai::ChatMessage>,
        token: &str,
    ) -> Result<ProviderResponse> {
        let mut contents = Vec::new();
        for msg in &messages {
            let role = match msg.role.as_str() {
                "user" => "user",
                "assistant" => "model",
                "system" => "user",
                _ => "user",
            };
            contents.push(Content {
                role: role.to_string(),
                parts: vec![Part {
                    text: msg.content.clone(),
                }],
            });
        }

        let request = GenerateContentRequest { contents };

        let url = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
            self.location, self.project_id, self.location, self.model
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&request)
            .send()
            .await
            .context("Vertex AI Gemini request failed")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Vertex AI Error: {}", error_text);
        }

        let gen_response: GenerateContentResponse = response
            .json()
            .await
            .context("Failed to parse Vertex AI response")?;

        if let Some(candidates) = gen_response.candidates
            && let Some(first) = candidates.first()
            && let Some(content) = &first.content
            && let Some(parts) = &content.parts
            && let Some(first_part) = parts.first()
        {
            let tokens = gen_response
                .usage_metadata
                .and_then(|u| u.total_token_count)
                .unwrap_or(0);

            Ok(ProviderResponse {
                content: first_part.text.clone(),
                tokens,
            })
        } else {
            Err(anyhow::anyhow!("No content in Vertex AI response"))
        }
    }

    async fn send_claude_message(
        &self,
        messages: Vec<crate::ai::ChatMessage>,
        token: &str,
    ) -> Result<ProviderResponse> {
        // Extract system message
        let system_content: Option<String> = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        let claude_messages: Vec<ClaudeMessage> = messages
            .iter()
            .filter(|m| m.role == "user" || m.role == "assistant")
            .map(|m| ClaudeMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        let request = ClaudeVertexRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages: claude_messages,
            system: system_content,
            anthropic_version: "vertex-2023-10-16".to_string(),
        };

        // Claude on Vertex uses the Anthropic-compatible endpoint
        let url = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/anthropic/models/{}:rawPredict",
            self.location, self.project_id, self.location, self.model
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Vertex AI Claude request failed")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Vertex AI Claude Error: {}", error_text);
        }

        let parsed: ClaudeVertexResponse = response
            .json()
            .await
            .context("Failed to parse Vertex AI Claude response")?;

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
}
