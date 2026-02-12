use crate::config::Config;
use crate::quota::QuotaTracker; // [NEW]
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::{Arc, Mutex}; // [NEW]

#[derive(Clone)]
pub struct AiClient {
    client: reqwest::Client,
    api_key: Option<String>,
    model: String,
    api_base: String,
    quota_tracker: Arc<Mutex<QuotaTracker>>, // [NEW]
    config: Config,                          // Store config for limits
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>, // [NEW]
}

#[derive(Deserialize, Debug)]
struct Usage {
    total_tokens: u32,
    _prompt_tokens: u32,
    _completion_tokens: u32,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: MessageContent,
}

#[derive(Deserialize, Debug)]
struct MessageContent {
    content: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Analysis {
    pub analysis: String,
    pub fix: Option<Fix>,
    #[serde(default)]
    pub tokens_used: u32, // [NEW]
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Fix {
    pub key: String,
    pub value: serde_json::Value,
}

impl AiClient {
    pub fn new(config: Config) -> Self {
        let quota_tracker = Arc::new(Mutex::new(QuotaTracker::load().unwrap_or_default()));

        Self {
            client: reqwest::Client::new(),
            api_key: config.openai_api_key.clone(),
            model: config.model.clone(),
            api_base: config.api_base.clone(),
            quota_tracker,
            config,
        }
    }

    pub async fn analyze_failure(
        &self,
        task_name: &str,
        error_msg: &str,
        vars: &serde_json::Value,
        facts: Option<&serde_json::Value>,
    ) -> Result<Analysis> {
        // Create AI span as child of current context (task span)
        let ai_span = crate::tracing::create_child_span(
            "ai.analyze_failure",
            vec![
                opentelemetry::KeyValue::new("ai.model", self.model.clone()),
                opentelemetry::KeyValue::new("ai.task_name", task_name.to_string()),
                opentelemetry::KeyValue::new("ai.api_base", self.api_base.clone()),
            ],
        );

        // Attach span to context (guard is dropped immediately, but span remains in context)
        let _ = crate::tracing::attach_span(ai_span);

        // Record start time for response time tracking
        let start = std::time::Instant::now();

        // Check Quota
        {
            let tracker = self.quota_tracker.lock().unwrap();
            if let Err(e) = tracker.check_limit(&self.config) {
                // Record quota error on span
                crate::tracing::record_error_on_current_span(&format!(
                    "Quota limit exceeded: {}",
                    e
                ));
                return Err(e);
            }
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

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_content.clone(),
                },
            ],
        };

        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));

        let mut builder = self.client.post(&url);

        if let Some(key) = &self.api_key {
            builder = builder.header("Authorization", format!("Bearer {}", key));
        }

        let response = match builder.json(&request).send().await {
            Ok(resp) => resp,
            Err(e) => {
                // Record API error on span
                crate::tracing::record_error_on_current_span(&format!(
                    "Failed to send request to AI: {}",
                    e
                ));
                return Err(anyhow::anyhow!(
                    "Failed to send request to AI at {}: {}",
                    url,
                    e
                ));
            }
        };

        let chat_response: ChatResponse = match response.json().await {
            Ok(resp) => resp,
            Err(e) => {
                // Record parsing error on span
                crate::tracing::record_error_on_current_span(&format!(
                    "Failed to parse AI response: {}",
                    e
                ));
                return Err(anyhow::anyhow!("Failed to parse OpenAI response: {}", e));
            }
        };

        if let Some(choice) = chat_response.choices.first() {
            let content = &choice.message.content;
            let tokens = chat_response
                .usage
                .as_ref()
                .map(|u| u.total_tokens)
                .unwrap_or(0);

            // Log interaction
            if let Err(e) = self.log_interaction(&user_content, content, tokens).await {
                eprintln!("Failed to log AI interaction: {}", e);
            }

            let mut analysis = match Self::parse_response(content) {
                Ok(a) => a,
                Err(e) => {
                    // Record parsing error on span
                    crate::tracing::record_error_on_current_span(&format!(
                        "Failed to parse AI JSON response: {}",
                        e
                    ));
                    return Err(e);
                }
            };

            analysis.tokens_used = tokens;

            // Update Quota
            if let Ok(mut tracker) = self.quota_tracker.lock() {
                let _ = tracker.add_usage(tokens, &self.model);
            }

            // Record success metrics on span
            let duration_ms = start.elapsed().as_millis() as i64;
            crate::tracing::add_attributes_to_current_span(vec![
                opentelemetry::KeyValue::new("ai.response_time_ms", duration_ms),
                opentelemetry::KeyValue::new("ai.tokens_used", tokens as i64),
                opentelemetry::KeyValue::new("ai.success", true),
                opentelemetry::KeyValue::new("ai.fix_suggested", analysis.fix.is_some()),
            ]);

            Ok(analysis)
        } else {
            // Record error on span
            crate::tracing::record_error_on_current_span("No response content from AI");
            Err(anyhow::anyhow!("No response content from AI"))
        }
    }

    pub fn get_usage(&self) -> (u32, f64) {
        if let Ok(tracker) = self.quota_tracker.lock() {
            (tracker.usage_today_tokens, tracker.cost_today_usd)
        } else {
            (0, 0.0)
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
        Ok(())
    }

    // Extracted for testing
    fn parse_response(content: &str) -> Result<Analysis> {
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
