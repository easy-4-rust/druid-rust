use druid_metrics::ServiceIdentity;

#[test]
fn canonical_service_identity_round_trips_arbitrary_utf8_without_old_slash_collisions() {
    let cases = [
        ("a/b", "c", "boot"),
        ("a", "b/c", "boot"),
        ("", "", ""),
        ("服务/一", "实例%2F", "YmFzZTY0LWxvb2tpbmc"),
        ("dots.and:colons", "slashes//", "启动-🚀"),
    ];
    for (service_name, instance_id, boot_id) in cases {
        let identity = ServiceIdentity::new(service_name, instance_id, boot_id);
        let service_id = identity.service_id();
        assert_eq!(
            ServiceIdentity::from_service_id(&service_id),
            Some(identity),
            "canonical external serviceId must be reversible"
        );
    }

    let first = ServiceIdentity::new("a/b", "c", "boot");
    let second = ServiceIdentity::new("a", "b/c", "boot");
    assert_ne!(first, second);
    assert_ne!(first.service_id(), second.service_id());
    assert!(ServiceIdentity::from_service_id("a/b/c/boot").is_none());
}
