use crate::config::Config;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuotaTracker {
    pub usage_today_tokens: u32,
    pub cost_today_usd: f64,
    pub last_reset: DateTime<Utc>,
    pub total_tokens_all_time: u32,
    pub total_cost_all_time_usd: f64,
}

impl Default for QuotaTracker {
    fn default() -> Self {
        Self {
            usage_today_tokens: 0,
            cost_today_usd: 0.0,
            last_reset: Utc::now(),
            total_tokens_all_time: 0,
            total_cost_all_time_usd: 0.0,
        }
    }
}

impl QuotaTracker {
    pub fn load() -> Result<Self> {
        let path = Self::get_quota_path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let mut tracker: QuotaTracker = serde_json::from_str(&content)?;
            tracker.check_reset();
            Ok(tracker)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::get_quota_path()?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    fn get_quota_path() -> Result<PathBuf> {
        Ok(Config::get_config_dir()?.join("quota.json"))
    }

    fn check_reset(&mut self) {
        let now = Utc::now();
        // Reset if it's a different day
        if self.last_reset.date_naive() != now.date_naive() {
            self.usage_today_tokens = 0;
            self.cost_today_usd = 0.0;
            self.last_reset = now;
        }
    }

    pub fn add_usage(&mut self, tokens: u32, model: &str) -> Result<()> {
        self.check_reset();

        let cost = self.calculate_cost(tokens, model);

        self.usage_today_tokens += tokens;
        self.cost_today_usd += cost;
        self.total_tokens_all_time += tokens;
        self.total_cost_all_time_usd += cost;

        self.save()?;
        Ok(())
    }

    fn calculate_cost(&self, tokens: u32, model: &str) -> f64 {
        // Rough estimate based on OpenAI pricing (Input/Output split needed for accuracy,
        // but for now we treat all as blended or just inputs)
        // GPT-4 Turbo: $10/1M input, $30/1M output. Avg ~$20/1M = $0.00002 per token
        // GPT-3.5 Turbo: $0.50/1M input, $1.50/1M output. Avg ~$1/1M = $0.000001 per token

        let rate = if model.contains("gpt-4") {
            0.00002
        } else if model.contains("gpt-3.5") {
            0.000001
        } else {
            0.0 // Local/Other
        };

        (tokens as f64) * rate
    }

    pub fn check_limit(&self, config: &Config) -> Result<()> {
        if let Some(limit) = config.quota_limit_tokens
            && self.usage_today_tokens >= limit
        {
            return Err(anyhow::anyhow!(
                "Daily token quota exceeded ({} / {})",
                self.usage_today_tokens,
                limit
            ));
        }

        if let Some(limit) = config.quota_limit_usd
            && self.cost_today_usd >= limit
        {
            return Err(anyhow::anyhow!(
                "Daily cost quota exceeded (${:.2} / ${:.2})",
                self.cost_today_usd,
                limit
            ));
        }

        Ok(())
    }
}
