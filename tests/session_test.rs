use ansible_piloteer::app::{App, HostStatus, TaskHistory};
use ansible_piloteer::config::Config;

#[test]
fn test_session_save_and_load() {
    // 1. Create a dummy App with data
    let config = Config::new().unwrap_or_else(|_| Config {
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
    });

    // We need to construct App manually or via new
    // App::new requires valid config.
    let mut app = App::new(config);

    // Populate history
    app.history.push(TaskHistory {
        name: "Test Task 1".to_string(),
        host: "localhost".to_string(),
        changed: true,
        failed: false,
        error: None,
        verbose_result: None,
        analysis: None,
    });

    // Populate hosts
    app.hosts.insert(
        "localhost".to_string(),
        HostStatus {
            name: "localhost".to_string(),
            ok_tasks: 1,
            changed_tasks: 0,
            failed_tasks: 0,
        },
    );

    // Populate logs
    app.log("Test Log 1".to_string(), None);
    app.log("Test Log 2".to_string(), None);

    // 2. Save Session
    let filename = "test_session_persistence.json.gz";
    app.save_session(filename).expect("Failed to save session");

    // 3. Load Session
    let loaded_app = App::from_session(filename).expect("Failed to load session");

    // 4. Verify Data
    assert_eq!(loaded_app.history.len(), 1);
    assert_eq!(loaded_app.history[0].name, "Test Task 1");
    assert_eq!(loaded_app.hosts.len(), 1);
    assert!(loaded_app.hosts.contains_key("localhost"));
    assert_eq!(loaded_app.logs.len(), 2);
    assert_eq!(loaded_app.logs[0].0, "Test Log 1");
    assert!(loaded_app.replay_mode);

    // Cleanup
    std::fs::remove_file(filename).unwrap_or(());
}
