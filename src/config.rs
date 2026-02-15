use anyhow::{Context, Result};
use config::{Config as ConfigLoader, Environment, File};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf; // [NEW]

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub openai_api_key: Option<String>,
    pub socket_path: String,
    pub model: String,
    pub api_base: String,
    #[allow(dead_code)]
    pub log_level: String,
    pub auth_token: Option<String>,
    pub bind_addr: Option<String>,
    pub secret_token: Option<String>,
    pub quota_limit_tokens: Option<u32>, // [NEW]
    pub quota_limit_usd: Option<f64>,    // [NEW]
    pub google_api_key: Option<String>,  // [NEW]
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub zipkin_endpoint: Option<String>,
    pub zipkin_service_name: String,
    pub zipkin_sample_rate: f64,
    pub filters: Option<HashMap<String, String>>, // [NEW]
    pub provider: Option<String>,                 // [NEW]
    pub anthropic_api_key: Option<String>,        // [NEW] Phase 35
    pub vertex_project_id: Option<String>,        // [NEW] Phase 35
    pub vertex_location: Option<String>,          // [NEW] Phase 35
}

impl Config {
    pub fn new() -> Result<Self> {
        Self::load_from_env()
    }

    pub fn get_config_dir() -> Result<PathBuf> {
        let home = env::var("HOME").context("HOME environment variable not set")?;
        let config_dir = PathBuf::from(home).join(".config").join("ansible-piloteer");
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }
        Ok(config_dir)
    }

    fn get_auth_config_path() -> Result<PathBuf> {
        Ok(Self::get_config_dir()?.join("auth.json"))
    }

    fn load_from_env() -> Result<Self> {
        let mut builder = ConfigLoader::builder()
            .set_default("socket_path", "/tmp/piloteer.sock")?
            .set_default("model", "gpt-4-turbo-preview")?
            .set_default("api_base", "https://api.openai.com/v1")?
            .set_default("log_level", "info")?
            .set_default("auth_token", None::<String>)?
            .set_default("auth_token", None::<String>)?
            .set_default("bind_addr", None::<String>)?
            .set_default("secret_token", None::<String>)?
            .set_default("quota_limit_tokens", None::<u32>)?
            .set_default("quota_limit_usd", None::<f64>)?
            .set_default("google_api_key", None::<String>)? // [NEW]
            .set_default("google_client_id", None::<String>)?
            .set_default("google_client_secret", None::<String>)?
            .set_default("zipkin_endpoint", None::<String>)?
            .set_default("zipkin_service_name", "ansible-piloteer")?
            .set_default("zipkin_sample_rate", 1.0)?
            .set_default("filters", None::<HashMap<String, String>>)? // [NEW]
            .set_default("provider", None::<String>)? // [NEW]
            .set_default("anthropic_api_key", None::<String>)? // [NEW] Phase 35
            .set_default("vertex_project_id", None::<String>)? // [NEW] Phase 35
            .set_default("vertex_location", "us-central1")? // [NEW] Phase 35
            .add_source(File::with_name("piloteer").required(false)) // CWD
            .add_source(Environment::with_prefix("PILOTEER"));

        // Load specific config file from ~/.config/ansible-piloteer/piloteer.toml
        if let Ok(config_dir) = Self::get_config_dir() {
            let config_path = config_dir.join("piloteer.toml");
            if config_path.exists() {
                builder = builder.add_source(File::from(config_path).required(false));
            }
        }

        // Try to load auth config
        if let Ok(auth_path) = Self::get_auth_config_path()
            && auth_path.exists()
        {
            builder = builder.add_source(File::from(auth_path).required(false));
        }

        let s = builder.build()?;
        let mut config: Config = s.try_deserialize()?;

        // Manually load auth token from new auth.json structure if not already set
        if config.auth_token.is_none() {
            // Default to "default" profile and "google" backend for now
            // Future improvement: Allow configuring active profile in piloteer.toml
            if let Ok(Some(token)) = Self::get_auth_token("default", "google") {
                config.auth_token = Some(token);
                if config.provider.is_none() {
                    config.provider = Some("google".to_string());
                }
            }
        }

        // Fallback for OPENAI_API_KEY if not in config/PILOTEER_ envs
        #[allow(clippy::collapsible_if)]
        if config.openai_api_key.is_none() {
            if let Ok(key) = env::var("OPENAI_API_KEY") {
                config.openai_api_key = Some(key);
            }
        }

        // [Phase 36] Auto-detect Antigravity IDE environment
        if env::var("ANTIGRAVITY_SESSION").is_ok() || env::var("ANTIGRAVITY_WORKSPACE").is_ok() {
            // Running inside Antigravity IDE — default to Google provider
            if config.provider.is_none() {
                config.provider = Some("google".to_string());
            }
        }

        if config.google_api_key.is_none()
            && let Ok(key) = env::var("PILOTEER_GOOGLE_API_KEY")
        {
            config.google_api_key = Some(key);
        }

        Ok(config)
    }

    pub fn load_auth_data() -> Result<HashMap<String, HashMap<String, String>>> {
        let path = Self::get_auth_config_path()?;
        if path.exists() {
            let file = fs::File::open(path)?;
            let reader = std::io::BufReader::new(file);
            let data: HashMap<String, HashMap<String, String>> =
                serde_json::from_reader(reader).unwrap_or_default();
            Ok(data)
        } else {
            Ok(HashMap::new())
        }
    }

    pub fn save_auth_token(profile: &str, backend: &str, token: &str) -> Result<()> {
        let mut data = Self::load_auth_data()?;

        data.entry(profile.to_string())
            .or_default()
            .insert(backend.to_string(), token.to_string());

        let path = Self::get_auth_config_path()?;
        let file = fs::File::create(path)?;
        serde_json::to_writer_pretty(file, &data)?;
        Ok(())
    }

    pub fn get_auth_token(profile: &str, backend: &str) -> Result<Option<String>> {
        let data = Self::load_auth_data()?;
        if let Some(backends) = data.get(profile) {
            Ok(backends.get(backend).cloned())
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static CONFIG_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_config() {
        let _guard = CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let orig_home = env::var("HOME").ok();
        let orig_cwd = env::current_dir().ok();

        unsafe {
            env::set_var("HOME", tmp.path());
            env::remove_var("PILOTEER_MODEL");
            env::remove_var("PILOTEER_SOCKET_PATH");
            env::remove_var("OPENAI_API_KEY");
            env::remove_var("PILOTEER_GOOGLE_API_KEY");
            env::remove_var("PILOTEER_AUTH_TOKEN");
            env::remove_var("PILOTEER_PROVIDER");
        }
        // Change CWD to avoid picking up project's piloteer.toml
        let _ = env::set_current_dir(tmp.path());

        let config = Config::new().unwrap();
        assert_eq!(config.model, "gpt-4-turbo-preview");
        assert_eq!(config.socket_path, "/tmp/piloteer.sock");
        assert_eq!(config.api_base, "https://api.openai.com/v1");
        assert!(config.openai_api_key.is_none());
        assert!(config.auth_token.is_none());
        assert!(config.provider.is_none());
        assert!(config.google_api_key.is_none());

        // Restore HOME and CWD
        unsafe {
            if let Some(h) = orig_home {
                env::set_var("HOME", h);
            }
        }
        if let Some(d) = orig_cwd {
            let _ = env::set_current_dir(d);
        }
    }

    #[test]
    fn test_env_override() {
        let _guard = CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let orig_home = env::var("HOME").ok();

        unsafe {
            env::set_var("HOME", tmp.path());
            env::set_var("PILOTEER_MODEL", "gpt-3.5-turbo");
            env::set_var("OPENAI_API_KEY", "test-key");
        }

        let config = Config::new().unwrap();
        assert_eq!(config.model, "gpt-3.5-turbo");
        assert_eq!(config.openai_api_key.as_deref(), Some("test-key"));

        // Cleanup
        unsafe {
            env::remove_var("PILOTEER_MODEL");
            env::remove_var("OPENAI_API_KEY");
            if let Some(h) = orig_home {
                env::set_var("HOME", h);
            }
        }
    }
}
