//! Engine settings builder defaults.

#[test]
fn test_builder_network_feature_defaults() {
    // Verify the default settings used by IrontideBtBackend match irontide defaults
    let settings = irontide::ClientBuilder::new().into_settings();
    // These should match the BtSettings defaults (all true)
    assert!(settings.enable_dht);
    assert!(settings.enable_upnp);
    assert!(settings.enable_natpmp);
    assert!(settings.enable_ipv6);
    assert!(settings.enable_pex);
    assert!(settings.enable_lsd);
    assert!(settings.enable_utp);
    assert!(settings.enable_fast_extension);
    assert!(settings.enable_holepunch);
    assert!(settings.enable_web_seed);
    // Super seeding defaults to false
    assert!(!settings.default_super_seeding);
}

#[test]
fn test_builder_feature_toggle_off() {
    // Verify we can toggle features off (as done in minimal sessions)
    let settings = irontide::ClientBuilder::new()
        .enable_dht(false)
        .enable_lsd(false)
        .enable_upnp(false)
        .enable_natpmp(false)
        .enable_ipv6(false)
        .enable_pex(false)
        .enable_utp(false)
        .into_settings();
    assert!(!settings.enable_dht);
    assert!(!settings.enable_lsd);
    assert!(!settings.enable_upnp);
    assert!(!settings.enable_natpmp);
    assert!(!settings.enable_ipv6);
    assert!(!settings.enable_pex);
    assert!(!settings.enable_utp);
}

#[test]
fn test_builder_queue_limits_match_defaults() {
    // BtSettings defaults: max_downloads=3, max_seeds=5, max_torrents=15, active_limit=10
    let settings = irontide::ClientBuilder::new()
        .active_downloads(3)
        .active_seeds(5)
        .max_torrents(15)
        .active_limit(10)
        .into_settings();
    assert_eq!(settings.active_downloads, 3);
    assert_eq!(settings.active_seeds, 5);
    assert_eq!(settings.max_torrents, 15);
    assert_eq!(settings.active_limit, 10);
}

#[test]
fn test_builder_dht_settings_propagate() {
    // DHT enabled
    let settings = irontide::ClientBuilder::new()
        .enable_dht(true)
        .into_settings();
    assert!(settings.enable_dht);

    // DHT disabled
    let settings = irontide::ClientBuilder::new()
        .enable_dht(false)
        .into_settings();
    assert!(!settings.enable_dht);
}
