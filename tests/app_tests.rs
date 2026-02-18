use ansible_piloteer::app::{App, EditState, TaskHistory};
use ansible_piloteer::config::Config;

fn make_config() -> Config {
    Config {
        openai_api_key: None,
        socket_path: "/tmp/piloteer.sock".to_string(),
        model: "gpt-4".to_string(),
        api_base: "https://api.openai.com/v1".to_string(),
        log_level: "info".to_string(),
        auth_token: None,
        bind_addr: None,
        secret_token: None,
        quota_limit_tokens: None,
        quota_limit_usd: None,
        google_api_key: None,
        google_client_id: None,
        google_client_secret: None,
        zipkin_endpoint: None,
        zipkin_service_name: "ansible-piloteer".to_string(),
        zipkin_sample_rate: 1.0,
        filters: None,
        provider: None,
        anthropic_api_key: None,
        vertex_project_id: None,
        vertex_location: Some("us-central1".to_string()),
    }
}

fn make_app() -> App {
    App::new(make_config())
}

#[test]
fn test_update_velocity() {
    let mut app = make_app();

    assert_eq!(app.event_counter, 0);
    assert!(app.event_velocity.is_empty());

    app.event_counter = 50;
    app.last_velocity_update = std::time::Instant::now();
    let initial_time = app.last_velocity_update;

    app.update_velocity();
    assert_eq!(app.last_velocity_update, initial_time); // not updated yet
    assert_eq!(app.event_counter, 50);
    assert!(app.event_velocity.is_empty());

    std::thread::sleep(std::time::Duration::from_millis(1050));
    app.event_counter = 50;
    app.update_velocity();

    assert_eq!(app.event_counter, 0);
    assert!(!app.event_velocity.is_empty());
    assert_eq!(*app.event_velocity.back().unwrap(), 50);
}

#[test]
fn test_set_task() {
    let mut app = make_app();

    assert_eq!(app.current_task, None);
    assert_eq!(app.task_vars, None);

    let name = "Test Task".to_string();
    let vars = serde_json::json!({"foo": "bar"});
    let facts = Some(serde_json::json!({"ansible_os_family": "Debian"}));

    app.set_task(name.clone(), vars.clone(), facts.clone());

    assert_eq!(app.current_task, Some(name));
    assert_eq!(app.task_vars, Some(vars));
    assert_eq!(app.facts, facts);
    assert!(app.task_start_time.is_some());
}

#[test]
fn test_toggle_breakpoint() {
    let mut app = make_app();
    app.history.push(TaskHistory {
        name: "Task 1".to_string(),
        host: "localhost".to_string(),
        changed: false,
        failed: false,
        duration: 0.0,
        error: None,
        verbose_result: None,
        analysis: None,
    });

    app.analysis_index = 0;
    assert!(!app.breakpoints.contains("Task 1"));

    app.toggle_breakpoint();
    assert!(app.breakpoints.contains("Task 1"));
    assert!(app.notification.is_some());

    app.toggle_breakpoint();
    assert!(!app.breakpoints.contains("Task 1"));
}

#[test]
fn test_variable_editing_flow() {
    let mut app = App::new(Config {
        socket_path: "/tmp/piloteer_test_edit.sock".to_string(),
        ..make_config()
    });

    let vars = serde_json::json!({"test_var": "initial_value", "number": 42});
    app.set_task("Test Task".to_string(), vars, None);

    assert!(app.prepare_edit("test_var".to_string()).is_ok());

    let temp_path = match &app.edit_state {
        EditState::EditingValue { key, temp_file } => {
            assert_eq!(key, "test_var");
            temp_file.clone()
        }
        _ => panic!("App not in EditingValue state"),
    };

    let new_content = serde_json::to_string_pretty(&serde_json::json!("modified_value")).unwrap();
    std::fs::write(&temp_path, new_content).expect("Failed to write to temp file");

    let result = app.apply_edit();
    assert!(result.is_ok());
    let (key, value) = result.unwrap();
    assert_eq!(key, "test_var");
    assert_eq!(value, serde_json::json!("modified_value"));
    assert!(matches!(app.edit_state, EditState::Idle));
    assert!(!temp_path.exists());
}
