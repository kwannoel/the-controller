use the_controller_lib::daemon_gateway::{
    daemon_gateway_placeholder_response, daemon_socket_path, DaemonGatewayConfig,
};

#[test]
fn daemon_socket_path_uses_controller_state_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = DaemonGatewayConfig {
        state_dir: tmp.path().to_path_buf(),
    };

    assert_eq!(daemon_socket_path(&cfg), tmp.path().join("daemon.sock"));
}

#[test]
fn gateway_paths_must_stay_under_api_daemon() {
    assert_eq!(
        the_controller_lib::daemon_gateway::normalize_daemon_path("/api/daemon/profiles").unwrap(),
        "/profiles"
    );
    assert!(
        the_controller_lib::daemon_gateway::normalize_daemon_path("/api/list_projects").is_err()
    );
    assert!(the_controller_lib::daemon_gateway::normalize_daemon_path("/api/daemonish").is_err());
    assert!(
        the_controller_lib::daemon_gateway::normalize_daemon_path("/api/daemon/../projects")
            .is_err()
    );
    assert!(the_controller_lib::daemon_gateway::normalize_daemon_path(
        "/api/daemon/%2e%2e/projects"
    )
    .is_err());
    assert!(the_controller_lib::daemon_gateway::normalize_daemon_path(
        "/api/daemon/%2E%2E/projects"
    )
    .is_err());
}

#[test]
fn gateway_placeholder_only_succeeds_for_health() {
    let response = daemon_gateway_placeholder_response("/health").unwrap();
    assert_eq!(response["path"], "/health");
    assert_eq!(response["status"], "gateway-ready");

    let error = daemon_gateway_placeholder_response("/profiles").unwrap_err();
    assert_eq!(error, "daemon gateway proxy is not implemented yet");
}
