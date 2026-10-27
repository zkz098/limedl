//! Engine settings builder defaults and options.

#[test]
fn test_librqbit_session_options_defaults() {
    let opts = librqbit::SessionOptions::default();
    assert!(opts.listen.is_none());
}

#[test]
fn test_librqbit_listener_options_defaults() {
    let opts = librqbit::ListenerOptions::default();
    assert!(!opts.enable_upnp_port_forwarding);
}
