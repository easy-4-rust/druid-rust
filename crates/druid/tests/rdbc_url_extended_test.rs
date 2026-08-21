use druid::sql::RdbcUrl;

// ── Error paths ────────────────────────────────────────────────

#[test]
fn rdbc_url_invalid_scheme() {
    let result = RdbcUrl::parse("https://host/db");
    assert!(result.is_err());
}

#[test]
fn rdbc_url_rdbc_scheme_no_profile() {
    let result = RdbcUrl::parse("rdbc://");
    assert!(result.is_err());
}

#[test]
fn rdbc_url_subprotocol_empty_profile() {
    let result = RdbcUrl::parse("rdbc:///host/db");
    assert!(result.is_err());
}

#[test]
fn rdbc_url_subprotocol_missing_separator() {
    let result = RdbcUrl::parse("rdbc:nocolon");
    assert!(result.is_err());
}

#[test]
fn rdbc_url_forbids_user_info() {
    let result = RdbcUrl::parse("rdbc:mysql://user:pass@host/db");
    assert!(result.is_err());
}

#[test]
fn rdbc_url_forbids_fragment() {
    let result = RdbcUrl::parse("rdbc:mysql://host/db#frag");
    assert!(result.is_err());
}

#[test]
fn rdbc_url_empty_input() {
    let result = RdbcUrl::parse("");
    assert!(result.is_err());
}

// ── Subprotocol style (rdbc:<profile>://...) ───────────────────

#[test]
fn rdbc_url_subprotocol_basic() {
    let url = RdbcUrl::parse("rdbc:mysql://localhost:3306/mydb").unwrap();
    assert_eq!(url.profile(), "mysql");
    assert_eq!(url.endpoint(), "localhost:3306");
    assert_eq!(url.database(), "mydb");
}

#[test]
fn rdbc_url_subprotocol_no_port() {
    let url = RdbcUrl::parse("rdbc:sqlite://host/mydb").unwrap();
    assert_eq!(url.endpoint(), "host");
    assert_eq!(url.database(), "mydb");
}

#[test]
fn rdbc_url_subprotocol_trailing_slash_no_database() {
    let url = RdbcUrl::parse("rdbc:mysql://localhost:3306/").unwrap();
    assert_eq!(url.endpoint(), "localhost:3306");
    assert_eq!(url.database(), "");
}

#[test]
fn rdbc_url_subprotocol_with_properties() {
    let url = RdbcUrl::parse("rdbc:mysql://host/db?charset=utf8&timeout=30").unwrap();
    assert_eq!(url.property("charset"), Some("utf8"));
    assert_eq!(url.property("timeout"), Some("30"));
    assert_eq!(url.property("missing"), None);
}

#[test]
fn rdbc_url_subprotocol_url_decoded() {
    let url = RdbcUrl::parse("rdbc:mysql://host/db?tz=GMT%2B8").unwrap();
    assert_eq!(url.property("tz"), Some("GMT+8"));
}

// ── Legacy style (rdbc://<profile>/...) ────────────────────────

#[test]
fn rdbc_url_legacy_basic() {
    let url = RdbcUrl::parse("rdbc://mysql/localhost:3306/mydb").unwrap();
    assert_eq!(url.profile(), "mysql");
    assert_eq!(url.endpoint(), "localhost:3306");
    assert_eq!(url.database(), "mydb");
}

#[test]
fn rdbc_url_legacy_no_database() {
    let url = RdbcUrl::parse("rdbc://mysql/localhost:3306").unwrap();
    assert_eq!(url.endpoint(), "localhost:3306");
    assert_eq!(url.database(), "");
}

// ── network_url ────────────────────────────────────────────────

#[test]
fn rdbc_url_network_url_basic() {
    let url = RdbcUrl::parse("rdbc:mysql://host:3306/db").unwrap();
    assert_eq!(url.network_url("mysql").unwrap(), "mysql://host:3306/db");
}

#[test]
fn rdbc_url_network_url_no_database() {
    let url = RdbcUrl::parse("rdbc:mysql://host:3306/").unwrap();
    assert_eq!(url.network_url("mysql").unwrap(), "mysql://host:3306");
}

#[test]
fn rdbc_url_legacy_with_double_slash() {
    let url = RdbcUrl::parse("rdbc://mysql//db").unwrap();
    assert_eq!(url.profile(), "mysql");
    let _ = url.network_url("mysql");
}

// ── authenticated_network_url ──────────────────────────────────

#[test]
fn rdbc_url_authenticated_with_credentials() {
    let url = RdbcUrl::parse("rdbc:mysql://host:3306/db?user=admin&password=secret").unwrap();
    let auth = url.authenticated_network_url("mysql").unwrap();
    assert!(auth.contains("admin"));
    assert!(auth.contains("secret"));
    assert!(auth.contains("host:3306"));
}

#[test]
fn rdbc_url_authenticated_without_credentials() {
    let url = RdbcUrl::parse("rdbc:mysql://host:3306/db").unwrap();
    let auth = url.authenticated_network_url("mysql").unwrap();
    assert!(!auth.contains('@'));
}

#[test]
fn rdbc_url_authenticated_user_only() {
    let url = RdbcUrl::parse("rdbc:mysql://host:3306/db?user=admin").unwrap();
    let auth = url.authenticated_network_url("mysql").unwrap();
    assert!(auth.contains("admin@"));
}

// ── redacted ───────────────────────────────────────────────────

#[test]
fn rdbc_url_redacted_subprotocol() {
    let url = RdbcUrl::parse("rdbc:mysql://host:3306/db?password=secret").unwrap();
    let redacted = url.redacted();
    assert!(!redacted.contains("secret"));
    assert!(redacted.contains("rdbc:mysql://host:3306/db"));
}

#[test]
fn rdbc_url_redacted_legacy() {
    let url = RdbcUrl::parse("rdbc://mysql/localhost:3306/mydb?password=x").unwrap();
    let redacted = url.redacted();
    assert!(!redacted.contains('x'));
    assert!(redacted.contains("rdbc://mysql/localhost:3306/mydb"));
}

#[test]
fn rdbc_url_redacted_no_database() {
    let url = RdbcUrl::parse("rdbc:mysql://host:3306/").unwrap();
    assert_eq!(url.redacted(), "rdbc:mysql://host:3306");
}

// ── as_str / properties / Debug ────────────────────────────────

#[test]
fn rdbc_url_as_str() {
    let input = "rdbc:mysql://host:3306/db?k=v";
    let url = RdbcUrl::parse(input).unwrap();
    assert_eq!(url.as_str(), input);
}

#[test]
fn rdbc_url_properties_map() {
    let url = RdbcUrl::parse("rdbc:mysql://host/db?a=1&b=2").unwrap();
    let props = url.properties();
    assert_eq!(props.len(), 2);
}

#[test]
fn rdbc_url_debug() {
    let url = RdbcUrl::parse("rdbc:mysql://host/db?secret=val").unwrap();
    let dbg = format!("{url:?}");
    assert!(dbg.contains("RdbcUrl"));
    assert!(!dbg.contains("val"));
}

// ── IPv6 endpoint ──────────────────────────────────────────────

#[test]
fn rdbc_url_ipv6_endpoint() {
    let url = RdbcUrl::parse("rdbc:mysql://[::1]:3306/db").unwrap();
    assert!(url.endpoint().contains("::1"));
    assert!(url.network_url("mysql").unwrap().contains("::1"));
}

// ── Hierarchical database path ─────────────────────────────────

#[test]
fn rdbc_url_hierarchical_database() {
    let url = RdbcUrl::parse("rdbc:mysql://host:3306/app/main/data").unwrap();
    assert_eq!(url.database(), "app/main/data");
}
