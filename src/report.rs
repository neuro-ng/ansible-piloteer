use crate::app::App;
use chrono::Local;
use std::fs::File;
use std::io::Write;

pub struct ReportGenerator<'a> {
    app: &'a App,
}

impl<'a> ReportGenerator<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub fn generate_markdown(&self) -> String {
        let mut md = String::new();

        // 1. Header
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        md.push_str("# Ansible Piloteer Execution Report\n\n");
        md.push_str(&format!("**Date:** {}\n\n", timestamp));

        // 2. Host Summary
        md.push_str("## Host Summary\n\n");
        if self.app.hosts.is_empty() {
            md.push_str("_No host data captured._\n\n");
        } else {
            md.push_str("| Host | OK | Changed | Failed |\n");
            md.push_str("|---|---|---|---|\n");
            for host in self.app.hosts.values() {
                md.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    host.name, host.ok_tasks, host.changed_tasks, host.failed_tasks
                ));
            }
            md.push('\n');
        }

        // 3. Task History
        md.push_str("## Task Execution History\n\n");
        if self.app.history.is_empty() {
            md.push_str("_No tasks executed._\n\n");
        } else {
            for (i, task) in self.app.history.iter().enumerate() {
                let status = if task.failed {
                    "FAILED"
                } else if task.changed {
                    "CHANGED"
                } else {
                    "OK"
                };

                let icon = if task.failed {
                    "❌"
                } else if task.changed {
                    "⚠️"
                } else {
                    "✅"
                };

                md.push_str(&format!("### {}. {} [{}]\n", i + 1, task.name, status));
                md.push_str(&format!("- **Host:** {}\n", task.host));
                md.push_str(&format!("- **Status:** {} {}\n", icon, status));

                if let Some(err) = &task.error {
                    md.push_str(&format!("- **Error:**\n```\n{}\n```\n", err));
                }

                // If we captured verbose result (e.g. stdout/stderr), include it
                // Logic to extract relevant parts from verbose_result if available
                if let Some(_details) = &task.verbose_result {
                    // We might serialize it or just show a summary
                    // For now, let's keep it simple.
                    md.push_str("- **Details Captured** (view in JSON export for full data)\n");
                }

                if let Some(analysis) = &task.analysis {
                    md.push_str("\n#### 🤖 AI Analysis\n");
                    md.push_str(&format!("> {}\n\n", analysis.analysis));
                    if let Some(fix) = &analysis.fix {
                        md.push_str("**Suggested Fix:**\n");
                        md.push_str(&format!("- Variable: `{}`\n", fix.key));
                        md.push_str(&format!("- Value: `{}`\n", fix.value));
                    }
                }

                md.push('\n');
            }
        }

        // 4. Play Recap (if available in logs or stored)
        // We have app.play_recap now
        if let Some(recap) = &self.app.play_recap {
            md.push_str("## Play Recap\n\n");
            md.push_str("```json\n");
            md.push_str(&serde_json::to_string_pretty(recap).unwrap_or_default());
            md.push_str("\n```\n\n");
        }

        md
    }

    pub fn save_to_file(&self, filename: &str) -> std::io::Result<()> {
        let content = self.generate_markdown();
        let mut file = File::create(filename)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }
}
