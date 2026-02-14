use ansible_piloteer::app::{App, TaskHistory};
use ansible_piloteer::config::Config;
use ansible_piloteer::ipc::Message;

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
        provider: None,
    })
}

#[test]
fn test_unreachable_message_parsing() {
    let json = r#"{"TaskUnreachable":{"name":"test_task","host":"host1","error":"Connection refused","result":{}}}"#;
    let msg: Message = serde_json::from_str(json).unwrap();

    match msg {
        Message::TaskUnreachable {
            name,
            host,
            error,
            result,
        } => {
            assert_eq!(name, "test_task");
            assert_eq!(host, "host1");
            assert_eq!(error, "Connection refused");
            assert!(result.is_object());
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_unreachable_message_with_details() {
    let json = r#"{"TaskUnreachable":{"name":"ssh_connect","host":"192.168.1.100","error":"SSH connection timeout","result":{"msg":"Failed to connect to the host via ssh","unreachable":true}}}"#;
    let msg: Message = serde_json::from_str(json).unwrap();

    match msg {
        Message::TaskUnreachable {
            name,
            host,
            error,
            result,
        } => {
            assert_eq!(name, "ssh_connect");
            assert_eq!(host, "192.168.1.100");
            assert_eq!(error, "SSH connection timeout");
            assert_eq!(result["unreachable"], true);
            assert!(result["msg"].is_string());
        }
        _ => panic!("Expected TaskUnreachable message"),
    }
}

#[test]
fn test_unreachable_app_state_tracking() {
    let config = create_test_config();

    let mut app = App::new(config);

    // Initially no unreachable hosts
    assert_eq!(app.unreachable_hosts.len(), 0);

    // Add unreachable host
    app.set_unreachable(
        "test_task".to_string(),
        "host1".to_string(),
        "Connection refused".to_string(),
        serde_json::json!({"msg": "SSH connection failed"}),
    );

    // Verify tracking
    assert_eq!(app.unreachable_hosts.len(), 1);
    assert!(app.unreachable_hosts.contains("host1"));

    // Verify history entry
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.history[0].name, "test_task");
    assert_eq!(app.history[0].host, "host1");
    assert!(app.history[0].failed);
    assert!(!app.history[0].changed);
    assert!(app.history[0].error.is_some());
}

#[test]
fn test_multiple_unreachable_hosts() {
    let config = create_test_config();

    let mut app = App::new(config);

    // Add multiple unreachable hosts
    app.set_unreachable(
        "connect".to_string(),
        "host1".to_string(),
        "Timeout".to_string(),
        serde_json::json!({}),
    );

    app.set_unreachable(
        "connect".to_string(),
        "host2".to_string(),
        "Connection refused".to_string(),
        serde_json::json!({}),
    );

    app.set_unreachable(
        "connect".to_string(),
        "host3".to_string(),
        "Network unreachable".to_string(),
        serde_json::json!({}),
    );

    // Verify all tracked
    assert_eq!(app.unreachable_hosts.len(), 3);
    assert!(app.unreachable_hosts.contains("host1"));
    assert!(app.unreachable_hosts.contains("host2"));
    assert!(app.unreachable_hosts.contains("host3"));

    // Verify history
    assert_eq!(app.history.len(), 3);
}

#[test]
fn test_unreachable_session_persistence() {
    let config = create_test_config();

    let mut app = App::new(config);

    // Add unreachable hosts
    app.set_unreachable(
        "task1".to_string(),
        "unreachable_host1".to_string(),
        "Connection timeout".to_string(),
        serde_json::json!({"msg": "Timeout"}),
    );

    app.set_unreachable(
        "task2".to_string(),
        "unreachable_host2".to_string(),
        "SSH refused".to_string(),
        serde_json::json!({"msg": "Connection refused"}),
    );

    // Add some logs
    app.log("Test log entry".to_string(), None);

    // Save session
    let filename = "test_unreachable_session.json.gz";
    app.save_session(filename).expect("Failed to save session");

    // Load session
    let loaded_app = App::from_session(filename).expect("Failed to load session");

    // Verify unreachable hosts restored
    assert_eq!(loaded_app.unreachable_hosts.len(), 2);
    assert!(loaded_app.unreachable_hosts.contains("unreachable_host1"));
    assert!(loaded_app.unreachable_hosts.contains("unreachable_host2"));

    // Verify history restored
    assert_eq!(loaded_app.history.len(), 2);
    assert!(
        loaded_app
            .history
            .iter()
            .any(|h| h.host == "unreachable_host1")
    );
    assert!(
        loaded_app
            .history
            .iter()
            .any(|h| h.host == "unreachable_host2")
    );

    // Verify all entries marked as failed
    assert!(loaded_app.history.iter().all(|h| h.failed));

    // Verify replay mode
    assert!(loaded_app.replay_mode);

    // Cleanup
    std::fs::remove_file(filename).unwrap_or(());
}

#[test]
fn test_unreachable_with_normal_tasks() {
    let config = create_test_config();

    let mut app = App::new(config);

    // Add successful task
    app.history.push(TaskHistory {
        name: "successful_task".to_string(),
        host: "host1".to_string(),
        changed: false,
        failed: false,
        duration: 0.0,
        error: None,
        verbose_result: None,
        analysis: None,
    });

    // Add unreachable host
    app.set_unreachable(
        "connect_task".to_string(),
        "host2".to_string(),
        "Unreachable".to_string(),
        serde_json::json!({}),
    );

    // Add failed task (different from unreachable)
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

    // Verify counts
    assert_eq!(app.history.len(), 3);
    assert_eq!(app.unreachable_hosts.len(), 1);
    assert!(app.unreachable_hosts.contains("host2"));

    // Verify only unreachable host is in the set
    assert!(!app.unreachable_hosts.contains("host1"));
    assert!(!app.unreachable_hosts.contains("host3"));
}
