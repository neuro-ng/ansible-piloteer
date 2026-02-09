use ansible_piloteer::execution::ExecutionDetails;
use serde_json::json;

#[test]
fn test_execution_details_parsing() {
    // Sample verbose output structure based on research
    let raw_json = json!({
        "_ansible_no_log": false,
        "_ansible_parsed": true,
        "changed": true,
        "cmd": [
            "echo",
            "Verbose Test"
        ],
        "delta": "0:00:00.003132",
        "end": "2026-02-08 20:59:29.555060",
        "failed": false,
        "invocation": {
            "module_args": {
                "_raw_params": "echo \"Verbose Test\"",
                "_uses_shell": false
            }
        },
        "msg": "",
        "rc": 0,
        "stdout": "Verbose Test",
        "stderr": ""
    });

    let details = ExecutionDetails::new(raw_json);

    assert_eq!(details.stdout(), Some("Verbose Test"));
    assert_eq!(details.stderr(), Some(""));
    assert_eq!(details.cmd(), Some("echo Verbose Test".to_string()));

    let invocation = details.invocation().expect("Should have invocation");
    assert_eq!(
        invocation["module_args"]["_raw_params"],
        "echo \"Verbose Test\""
    );
}

#[test]
fn test_execution_details_cmd_string() {
    let raw_json = json!({
        "cmd": "echo 'Hello World'",
        "stdout": "Hello World"
    });

    let details = ExecutionDetails::new(raw_json);
    assert_eq!(details.cmd(), Some("echo 'Hello World'".to_string()));
}
