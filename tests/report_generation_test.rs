use ansible_piloteer::ai::{Analysis, Fix};
use ansible_piloteer::app::{App, TaskHistory};
use ansible_piloteer::config::Config;
use ansible_piloteer::report::ReportGenerator;

fn create_test_config() -> Config {
    Config::new().unwrap_or_else(|_| Config {
        openai_api_key: None,
        socket_path: "/tmp/test.sock".to_string(),
        model: "gpt-4".to_string(),
        api_base: "https://api.openai.com/v1".to_string(),
        log_level: "info".to_string(),
        auth_token: None,
        bind_addr: None,
        secret_token: None,
        quota_limit_tokens: None,
        quota_limit_usd: None,
        google_client_id: None,
        google_client_secret: None,
        zipkin_endpoint: None,
        zipkin_service_name: "ansible-piloteer".to_string(),
        zipkin_sample_rate: 1.0,
        filters: None,
    })
}

#[test]
fn test_report_with_unreachable_hosts() {
    let config = create_test_config();
    let mut app = App::new(config);

    // Add unreachable host
    app.set_unreachable(
        "connect_task".to_string(),
        "unreachable_host".to_string(),
        "Connection timeout".to_string(),
        serde_json::json!({"msg": "SSH timeout after 30s"}),
    );

    // Add successful task
    app.history.push(TaskHistory {
        name: "successful_task".to_string(),
        host: "working_host".to_string(),
        changed: false,
        failed: false,
        duration: 0.0,
        error: None,
        verbose_result: None,
        analysis: None,
    });

    // Generate report
    let generator = ReportGenerator::new(&app);
    let report = generator.generate_markdown();

    // Verify report contains unreachable information
    assert!(report.contains("unreachable_host"));
    assert!(report.contains("Connection timeout") || report.contains("SSH timeout"));
    assert!(report.contains("connect_task"));

    // Verify report also contains successful task
    assert!(report.contains("successful_task"));
    assert!(report.contains("working_host"));
}

#[test]
fn test_report_with_ai_analysis() {
    let config = create_test_config();
    let mut app = App::new(config);

    // Add failed task with AI analysis
    app.history.push(TaskHistory {
        name: "failed_task".to_string(),
        host: "test_host".to_string(),
        changed: false,
        failed: true,
        duration: 0.0,
        error: Some("Module failed".to_string()),
        verbose_result: None,
        analysis: Some(Analysis {
            analysis: "The task failed because the package is not available in the repository."
                .to_string(),
            fix: Some(Fix {
                key: "package_repo".to_string(),
                value: serde_json::json!("universe"),
            }),
            tokens_used: 100,
        }),
    });

    // Generate report
    let generator = ReportGenerator::new(&app);
    let report = generator.generate_markdown();

    // Verify AI analysis is included
    assert!(report.contains("failed_task"));
    assert!(report.contains("AI Analysis") || report.contains("Analysis"));
    assert!(report.contains("package is not available"));
}

#[test]
fn test_report_with_mixed_results() {
    let config = create_test_config();
    let mut app = App::new(config);

    // Add OK task
    app.history.push(TaskHistory {
        name: "ok_task".to_string(),
        host: "host1".to_string(),
        changed: false,
        failed: false,
        duration: 0.0,
        error: None,
        verbose_result: None,
        analysis: None,
    });

    // Add changed task
    app.history.push(TaskHistory {
        name: "changed_task".to_string(),
        host: "host2".to_string(),
        changed: true,
        failed: false,
        duration: 0.0,
        error: None,
        verbose_result: None,
        analysis: None,
    });

    // Add failed task
    app.history.push(TaskHistory {
        name: "failed_task".to_string(),
        host: "host3".to_string(),
        changed: false,
        failed: true,
        duration: 0.0,
        error: Some("Task error".to_string()),
        verbose_result: None,
        analysis: None,
    });

    // Add unreachable host
    app.set_unreachable(
        "connect".to_string(),
        "host4".to_string(),
        "Unreachable".to_string(),
        serde_json::json!({}),
    );

    // Generate report
    let generator = ReportGenerator::new(&app);
    let report = generator.generate_markdown();

    // Verify all task types are present
    assert!(report.contains("ok_task"));
    assert!(report.contains("changed_task"));
    assert!(report.contains("failed_task"));
    assert!(report.contains("host4"));

    // Verify all hosts are mentioned
    assert!(report.contains("host1"));
    assert!(report.contains("host2"));
    assert!(report.contains("host3"));
    assert!(report.contains("host4"));
}

#[test]
fn test_report_structure() {
    let config = create_test_config();
    let mut app = App::new(config);

    // Add some tasks
    app.history.push(TaskHistory {
        name: "test_task".to_string(),
        host: "localhost".to_string(),
        changed: true,
        failed: false,
        duration: 0.0,
        error: None,
        verbose_result: None,
        analysis: None,
    });

    // Generate report
    let generator = ReportGenerator::new(&app);
    let report = generator.generate_markdown();

    // Verify basic markdown structure
    assert!(report.contains("# Ansible Piloteer"));
    assert!(report.contains("## ")); // Has sections
    assert!(report.contains("test_task"));
}

#[test]
fn test_empty_report() {
    let config = create_test_config();
    let app = App::new(config);

    // Generate report with no history
    let generator = ReportGenerator::new(&app);
    let report = generator.generate_markdown();

    // Should still have basic structure
    assert!(report.contains("# Ansible Piloteer"));
    assert!(!report.is_empty());
}

#[test]
fn test_report_with_multiple_analyses() {
    let config = create_test_config();
    let mut app = App::new(config);

    // Add multiple failed tasks with analyses
    app.history.push(TaskHistory {
        name: "fail1".to_string(),
        host: "host1".to_string(),
        changed: false,
        failed: true,
        duration: 0.0,
        error: Some("Error 1".to_string()),
        verbose_result: None,
        analysis: Some(Analysis {
            analysis: "Analysis 1".to_string(),
            fix: Some(Fix {
                key: "var1".to_string(),
                value: serde_json::json!("fix1"),
            }),
            tokens_used: 50,
        }),
    });

    app.history.push(TaskHistory {
        name: "fail2".to_string(),
        host: "host2".to_string(),
        changed: false,
        failed: true,
        duration: 0.0,
        error: Some("Error 2".to_string()),
        verbose_result: None,
        analysis: Some(Analysis {
            analysis: "Analysis 2".to_string(),
            fix: Some(Fix {
                key: "var2".to_string(),
                value: serde_json::json!("fix2"),
            }),
            tokens_used: 60,
        }),
    });

    // Generate report
    let generator = ReportGenerator::new(&app);
    let report = generator.generate_markdown();

    // Verify both analyses are included
    assert!(report.contains("Analysis 1"));
    assert!(report.contains("Analysis 2"));
}
