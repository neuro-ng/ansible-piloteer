use crate::app::{App, TaskHistory};
use chrono::{DateTime, Utc};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Write};

#[derive(Serialize, Deserialize)]
pub struct Session {
    pub timestamp: DateTime<Utc>,
    pub history: Vec<TaskHistory>,
    pub logs: Vec<(String, Color)>,
    pub facts: Option<serde_json::Value>,
    pub task_vars: Option<serde_json::Value>,
    pub hosts: std::collections::HashMap<String, crate::app::HostStatus>,
    pub play_recap: Option<serde_json::Value>,
    pub unreachable_hosts: std::collections::HashSet<String>,
}

impl Session {
    pub fn from_app(app: &App) -> Self {
        let logs: Vec<_> = app.logs.iter().cloned().collect();
        Self {
            timestamp: Utc::now(),
            history: app.history.clone(),
            logs,
            facts: app.facts.clone(),
            task_vars: app.task_vars.clone(),
            hosts: app.hosts.clone(),
            play_recap: app.play_recap.clone(),
            unreachable_hosts: app.unreachable_hosts.clone(),
        }
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut encoder = GzEncoder::new(file, Compression::default());
        let json = serde_json::to_string(self)?;
        encoder.write_all(json.as_bytes())?;
        encoder.finish()?;
        Ok(())
    }

    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let mut decoder = GzDecoder::new(file);
        let mut json = String::new();
        decoder.read_to_string(&mut json)?;
        let session: Session = serde_json::from_str(&json)?;
        Ok(session)
    }

    pub fn restore_to_app(self, app: &mut App) {
        app.history = self.history;
        app.logs = VecDeque::from(self.logs);
        app.facts = self.facts;
        app.task_vars = self.task_vars;
        app.hosts = self.hosts;
        app.play_recap = self.play_recap;
        app.unreachable_hosts = self.unreachable_hosts;
    }
}
