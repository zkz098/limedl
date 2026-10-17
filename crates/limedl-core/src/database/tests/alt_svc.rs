use super::Database;
use crate::http::AltSvcRecord;

#[test]
fn test_alt_svc_repo_crud() {
    let db = Database::open_in_memory().unwrap();

    let records = db.load_all_alt_svc().unwrap();
    assert!(records.is_empty());

    let rec1 = AltSvcRecord {
        origin_host: "example.com".into(),
        origin_port: 443,
        protocol: "h3".into(),
        alt_host: "example.com".into(),
        alt_port: 443,
        expires_at_ms: 100_000,
        persist: true,
        fail_count: 0,
        broken_until_ms: 0,
    };
    db.upsert_alt_svc(&rec1).unwrap();

    let loaded = db.load_all_alt_svc().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0], rec1);

    // Update with broken status and fail_count
    let rec1_updated = AltSvcRecord {
        origin_host: "example.com".into(),
        origin_port: 443,
        protocol: "h3".into(),
        alt_host: "example.com".into(),
        alt_port: 443,
        expires_at_ms: 200_000,
        persist: true,
        fail_count: 2,
        broken_until_ms: 150_000,
    };
    db.upsert_alt_svc(&rec1_updated).unwrap();

    let loaded2 = db.load_all_alt_svc().unwrap();
    assert_eq!(loaded2.len(), 1);
    assert_eq!(loaded2[0], rec1_updated);

    // Add second record on different origin
    let rec2 = AltSvcRecord {
        origin_host: "cdn.test.org".into(),
        origin_port: 443,
        protocol: "h3".into(),
        alt_host: "cdn.test.org".into(),
        alt_port: 8443,
        expires_at_ms: 50_000,
        persist: false,
        fail_count: 0,
        broken_until_ms: 0,
    };
    db.upsert_alt_svc(&rec2).unwrap();
    assert_eq!(db.load_all_alt_svc().unwrap().len(), 2);

    // Clean expired records (now_ms = 60_000 should purge rec2)
    let cleaned = db.clean_expired_alt_svc(60_000).unwrap();
    assert_eq!(cleaned, 1);
    let loaded3 = db.load_all_alt_svc().unwrap();
    assert_eq!(loaded3.len(), 1);
    assert_eq!(loaded3[0].origin_host, "example.com");

    // Clear origin
    db.clear_alt_svc_for_origin("example.com", 443).unwrap();
    assert!(db.load_all_alt_svc().unwrap().is_empty());
}
