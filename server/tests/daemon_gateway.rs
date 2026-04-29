use the_controller_lib::daemon_gateway::{daemon_socket_path, DaemonGatewayConfig};

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
    assert!(
        the_controller_lib::daemon_gateway::normalize_daemon_path("/api/daemon/profiles").is_ok()
    );
    assert!(
        the_controller_lib::daemon_gateway::normalize_daemon_path("/api/list_projects").is_err()
    );
    assert!(the_controller_lib::daemon_gateway::normalize_daemon_path("/api/daemonish").is_err());
    assert!(
        the_controller_lib::daemon_gateway::normalize_daemon_path("/api/daemon/../projects")
            .is_err()
    );
}
