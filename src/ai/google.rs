use crate::ai::{AiProvider, ProviderResponse};
use crate::config::Config;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub struct GoogleProvider {
    client: reqwest::Client,
    api_key: String,
    auth_token: Option<String>, // [NEW]
    model: String,
    api_base: String,
}

impl GoogleProvider {
    pub fn new(config: &Config) -> Self {
        // Prefer specific google key, fallback to openai key if user is reusing it
        let api_key = config
            .google_api_key
            .clone()
            .or(std::env::var("PILOTEER_GOOGLE_API_KEY").ok())
            .unwrap_or_default();

        // Use configured api_base if it's not the default OpenAI one, otherwise use Google's default
        let default_openai = "https://api.openai.com/v1";
        let api_base = if config.api_base != default_openai {
            config.api_base.clone()
        } else {
            "https://generativelanguage.googleapis.com/v1beta".to_string()
        };

        Self {
            client: reqwest::Client::new(),
            api_key,
            auth_token: config.auth_token.clone(),
            model: config.model.clone(),
            api_base,
        }
    }
}

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

#[async_trait]
impl AiProvider for GoogleProvider {
    async fn send_message(
        &self,
        messages: Vec<crate::ai::ChatMessage>,
    ) -> Result<ProviderResponse> {
        let mut contents = Vec::new();

        for msg in messages {
            let role = match msg.role.as_str() {
                "user" => "user",
                "assistant" => "model",
                "system" => "user", // Fallback for system prompt
                _ => "user",
            };

            contents.push(Content {
                role: role.to_string(),
                parts: vec![Part { text: msg.content }],
            });
        }

        let request = GenerateContentRequest { contents };

        // Default to gemini-3.0-flash-preview if model is generic (older models may be deprecated)
        let model_name = if self.model.starts_with("gemini") {
            self.model.clone()
        } else {
            "gemini-3-flash-preview".to_string()
        };

        let mut url = format!(
            "{}/models/{}:generateContent",
            self.api_base.trim_end_matches('/'),
            model_name
        );

        // If we have an API key, add it to query params.
        // If we ONLY have an auth token, we don't need the key param (usually).
        // It's safe to always add key if present.
        if !self.api_key.is_empty() {
            url.push_str(&format!("?key={}", self.api_key));
        }

        let mut builder = self.client.post(&url);

        // Add Bearer token ONLY if we don't have an API key (or if we decide to support combined auth later)
        if self.api_key.is_empty()
            && let Some(token) = &self.auth_token
        {
            builder = builder.header("Authorization", format!("Bearer {}", token));
        }

        let response = builder
            .json(&request)
            .send()
            .await
            .context(format!("Failed to send request to Google AI at {}", url))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Google AI Request Failed: {}", error_text));
        }

        let gen_response: GenerateContentResponse = response
            .json()
            .await
            .context("Failed to parse Google AI response")?;

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
            Err(anyhow::anyhow!("No content in Google AI response"))
        }
    }

    async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let mut url = format!("{}/models", self.api_base.trim_end_matches('/'));
        if !self.api_key.is_empty() {
            url.push_str(&format!("?key={}", self.api_key));
        }

        let mut builder = self.client.get(&url);
        if self.api_key.is_empty()
            && let Some(token) = &self.auth_token
        {
            builder = builder.header("Authorization", format!("Bearer {}", token));
        }

        let response = builder
            .send()
            .await
            .context("Failed to fetch Google AI models")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to list Google models: {}",
                error_text
            ));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Google models response")?;

        let mut models = Vec::new();
        if let Some(items) = body.get("models").and_then(|m| m.as_array()) {
            for item in items {
                if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                    // Only include models that support generateContent
                    let supports_generate = item
                        .get("supportedGenerationMethods")
                        .and_then(|m| m.as_array())
                        .map(|methods| {
                            methods
                                .iter()
                                .any(|m| m.as_str() == Some("generateContent"))
                        })
                        .unwrap_or(false);
                    if supports_generate {
                        // Strip "models/" prefix
                        let clean_name = name.strip_prefix("models/").unwrap_or(name);
                        models.push(clean_name.to_string());
                    }
                }
            }
        }

        Ok(models)
    }
}
