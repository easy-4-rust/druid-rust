use druid_metrics::{ServiceIdentity, MAX_SERVICE_ID_COMPONENT_BYTES};

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
        let identity = ServiceIdentity::try_new(service_name, instance_id, boot_id).unwrap();
        let service_id = identity.service_id().to_owned();
        assert_eq!(
            ServiceIdentity::parse(&service_id),
            Ok(identity),
            "canonical external serviceId must be reversible"
        );
    }

    let first = ServiceIdentity::try_new("a/b", "c", "boot").unwrap();
    let second = ServiceIdentity::try_new("a", "b/c", "boot").unwrap();
    assert_ne!(first, second);
    assert_ne!(first.service_id(), second.service_id());
    assert!(ServiceIdentity::parse("a/b/c/boot").is_err());
}

#[test]
fn service_identity_accepts_exact_byte_budget_and_rejects_budget_plus_one() {
    let exact = "界".repeat(MAX_SERVICE_ID_COMPONENT_BYTES / "界".len());
    let exact = format!(
        "{exact}{}",
        "x".repeat(MAX_SERVICE_ID_COMPONENT_BYTES % "界".len())
    );
    let identity = ServiceIdentity::try_new(&exact, "instance", "boot")
        .expect("exact UTF-8 byte budget is accepted");
    assert_eq!(
        identity.service_name().len(),
        MAX_SERVICE_ID_COMPONENT_BYTES
    );
    assert_eq!(
        ServiceIdentity::parse(identity.service_id()),
        Ok(identity.clone())
    );

    let over = format!("{exact}x");
    assert!(ServiceIdentity::try_new(over, "instance", "boot").is_err());
    let oversized_encoded = format!("sid1_{}", "A".repeat(2_000));
    assert!(ServiceIdentity::parse(&oversized_encoded).is_err());
}
