use anyhow::{Context, Result};
use config::{Config as ConfigLoader, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

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
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
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
            .set_default("google_client_id", None::<String>)?
            .set_default("google_client_secret", None::<String>)?
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

        // Fallback for OPENAI_API_KEY if not in config/PILOTEER_ envs
        #[allow(clippy::collapsible_if)]
        if config.openai_api_key.is_none() {
            if let Ok(key) = env::var("OPENAI_API_KEY") {
                config.openai_api_key = Some(key);
            }
        }

        Ok(config)
    }

    pub fn save_auth_token(token: &str) -> Result<()> {
        let path = Self::get_auth_config_path()?;
        let json = serde_json::json!({
            "auth_token": token
        });
        let file = fs::File::create(path)?;
        serde_json::to_writer_pretty(file, &json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static CONFIG_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_config() {
        let _guard = CONFIG_LOCK.lock().unwrap();
        // Ensure envs are clean
        unsafe {
            env::remove_var("PILOTEER_MODEL");
            env::remove_var("PILOTEER_SOCKET_PATH");
            env::remove_var("OPENAI_API_KEY");
        }

        let config = Config::new().unwrap();
        assert_eq!(config.model, "gpt-4-turbo-preview");
        assert_eq!(config.socket_path, "/tmp/piloteer.sock");
        assert_eq!(config.api_base, "https://api.openai.com/v1");
        assert!(config.openai_api_key.is_none());
        assert!(config.auth_token.is_none());
    }

    #[test]
    fn test_env_override() {
        let _guard = CONFIG_LOCK.lock().unwrap();
        unsafe {
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
        }
    }
}
