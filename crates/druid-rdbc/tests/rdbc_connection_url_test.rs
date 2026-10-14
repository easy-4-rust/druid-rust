use druid_rdbc::error::RdbcError;
use druid_rdbc::url::RdbcConnectionUrl;

#[test]
fn parses_product_profile_subprotocol_form_and_redacts_properties() {
    let url = RdbcConnectionUrl::parse(
        "rdbc:postgresql://db.example:5432/app?sslmode=require&password=s%40cret",
    )
    .expect("valid product URL");

    assert_eq!(url.profile(), "postgresql");
    assert_eq!(url.endpoint(), "db.example:5432");
    assert_eq!(url.database(), "app");
    assert_eq!(url.property("sslmode"), Some("require"));
    assert_eq!(url.property("password"), Some("s@cret"));
    assert_eq!(url.redacted(), "rdbc:postgresql://db.example:5432/app");
    let debug = format!("{url:?}");
    assert!(!debug.contains("s@cret"));
    assert!(!debug.contains("password="));
}

#[test]
fn parses_legacy_authority_form_and_keeps_last_duplicate_property() {
    let url = RdbcConnectionUrl::parse("rdbc://postgresql/db.example:5432/app?mode=old&mode=new")
        .expect("valid legacy product URL");

    assert_eq!(url.profile(), "postgresql");
    assert_eq!(url.endpoint(), "db.example:5432");
    assert_eq!(url.database(), "app");
    assert_eq!(url.property("mode"), Some("new"));
    assert_eq!(url.redacted(), "rdbc://postgresql/db.example:5432/app");
}

#[test]
fn builds_network_url_and_encodes_credentials_only_for_driver_use() {
    let url = RdbcConnectionUrl::parse(
        "rdbc:postgresql://db.example:5432/app?user=app%40example&password=s%40cret",
    )
    .expect("valid product URL");

    assert_eq!(
        url.network_url("postgresql").expect("endpoint exists"),
        "postgresql://db.example:5432/app"
    );
    let authenticated = url
        .authenticated_network_url("postgresql")
        .expect("credentials can be encoded");
    assert!(authenticated.contains("app%40example"));
    assert!(authenticated.contains("s%40cret"));
}

#[test]
fn rejects_credential_bearing_fragmented_and_non_rdbc_urls() {
    for input in [
        "rdbc:postgresql://user:password@db.example/app",
        "rdbc:postgresql://db.example/app#fragment",
        "postgresql://db.example/app",
        "rdbc:postgresql://",
    ] {
        let error = RdbcConnectionUrl::parse(input).expect_err("invalid product URL");
        assert!(matches!(error, RdbcError::InvalidArgument(_)));
        assert!(!error.to_string().contains("password"));
    }
}

#[test]
fn rejects_profile_malformed_subprotocol_and_missing_endpoint_for_network_url() {
    assert!(RdbcConnectionUrl::parse("rdbc://").is_err());
    assert!(RdbcConnectionUrl::parse("rdbc:postgresql//db.example/app").is_err());

    assert!(matches!(
        RdbcConnectionUrl::parse("rdbc:sqlite:///tmp/app.db"),
        Err(RdbcError::InvalidArgument(_))
    ));
}
