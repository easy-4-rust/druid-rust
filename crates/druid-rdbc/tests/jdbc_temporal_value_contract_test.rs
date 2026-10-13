use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use druid_rdbc::types::{Time, Timestamp, Value};
use std::process::Command;

#[test]
fn timestamp_epoch_conversion_uses_the_process_local_timezone() {
    let epoch = Timestamp::from_millis(0).unwrap();
    let system_zone = jiff::tz::TimeZone::system();
    let expected_epoch = jiff::Timestamp::new(0, 0)
        .unwrap()
        .to_zoned(system_zone.clone())
        .datetime();
    assert_eq!(
        epoch
            .to_local_date_time()
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string(),
        expected_epoch.to_string()
    );
    assert_eq!(epoch.millis().unwrap(), 0);

    let local_midnight = NaiveDate::from_ymd_opt(1970, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let expected_midnight = system_zone
        .to_ambiguous_zoned(jiff::civil::DateTime::constant(1970, 1, 1, 0, 0, 0, 0))
        .disambiguate(jiff::tz::Disambiguation::Compatible)
        .unwrap()
        .timestamp()
        .as_millisecond();
    assert_eq!(
        Timestamp::from_local_date_time(local_midnight)
            .unwrap()
            .millis()
            .unwrap(),
        expected_midnight
    );
}

#[test]
fn timestamp_negative_epoch_and_java_fraction_format_are_preserved() {
    let negative = Timestamp::from_millis(-1).unwrap();
    assert_eq!(negative.millis().unwrap(), -1);

    let date = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
    let zero = Timestamp::from_local_date_time(date.and_hms_opt(3, 4, 5).unwrap()).unwrap();
    let one_nano =
        Timestamp::from_local_date_time(date.and_hms_nano_opt(3, 4, 5, 1).unwrap()).unwrap();
    let twelve_hundredths =
        Timestamp::from_local_date_time(date.and_hms_nano_opt(3, 4, 5, 120_000_000).unwrap())
            .unwrap();
    assert_eq!(zero.to_string(), "2024-01-02 03:04:05.0");
    assert_eq!(one_nano.to_string(), "2024-01-02 03:04:05.000000001");
    assert_eq!(twelve_hundredths.to_string(), "2024-01-02 03:04:05.12");
}

#[test]
fn timestamp_invalid_nanos_leave_the_existing_value_unchanged() {
    let mut value = Timestamp::value_of("2024-01-02 03:04:05.12").unwrap();
    let before = value;
    assert!(value.set_nanos(1_000_000_000).is_err());
    assert_eq!(value, before);
}

#[test]
fn jdbc_time_discards_fractional_seconds_but_sql_time_remains_independent() {
    let local = NaiveTime::from_hms_nano_opt(12, 34, 56, 123_456_789).unwrap();
    let jdbc = Time::from_local_time(local);
    assert_eq!(
        jdbc.to_local_time(),
        NaiveTime::from_hms_opt(12, 34, 56).unwrap()
    );
    assert_eq!(jdbc.to_local_time().nanosecond(), 0);
    assert_eq!(jdbc.to_string(), "12:34:56");

    let rust_sql_time = Value::Time(local);
    let Value::Time(value) = rust_sql_time else {
        panic!("the Rust SQL TIME value must remain a distinct high-precision value")
    };
    assert_eq!(value.nanosecond(), 123_456_789);
}

#[test]
fn timestamp_value_of_preserves_fractional_nanos_and_local_fields() {
    let parsed = Timestamp::value_of("2024-01-02 03:04:05.000000001").unwrap();
    assert_eq!(
        parsed.to_local_date_time(),
        NaiveDateTime::parse_from_str("2024-01-02 03:04:05.000000001", "%Y-%m-%d %H:%M:%S%.f")
            .unwrap()
    );
    assert_eq!(parsed.nanos(), 1);
}

#[test]
fn timestamp_identity_is_the_instant_and_out_of_range_millis_are_errors() {
    let from_epoch = Timestamp::from_millis(0).unwrap();
    let from_local = Timestamp::from_local_date_time(from_epoch.to_local_date_time()).unwrap();
    assert_eq!(from_epoch, from_local);
    assert_eq!(from_epoch.millis().unwrap(), from_local.millis().unwrap());
    assert!(Timestamp::from_millis(i64::MIN).is_err());
    assert!(Timestamp::from_millis(i64::MAX).is_err());
}

#[test]
fn isolated_jdbc_timezone_differential_matches_java8_oracle() {
    for (case, zone) in [
        ("utc", "UTC"),
        ("shanghai", "Asia/Shanghai"),
        ("new-york", "America/New_York"),
        ("change", "UTC"),
    ] {
        let output = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("jdbc_timezone_child_contract")
            .arg("--nocapture")
            .env("RDBC_TZ_CHILD", case)
            .env("TZ", zone)
            .output()
            .expect("timezone child process must start");
        assert!(
            output.status.success(),
            "{case} child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn jdbc_timezone_child_contract() {
    let Ok(case) = std::env::var("RDBC_TZ_CHILD") else {
        return;
    };
    if case == "change" {
        std::env::set_var("TZ", "UTC");
        let timestamp = Timestamp::from_millis(0).unwrap();
        assert_eq!(
            timestamp.to_local_date_time().to_string(),
            "1970-01-01 00:00:00"
        );
        std::env::set_var("TZ", "Asia/Shanghai");
        assert_eq!(
            timestamp.to_local_date_time().to_string(),
            "1970-01-01 08:00:00"
        );
        assert_eq!(timestamp.millis().unwrap(), 0);
        return;
    }
    let (epoch_local, local_midnight, gap, fold) = match case.as_str() {
        "utc" => (
            "1970-01-01 00:00:00",
            0,
            1_710_037_800_000,
            1_730_597_400_000,
        ),
        "shanghai" => (
            "1970-01-01 08:00:00",
            -28_800_000,
            1_710_009_000_000,
            1_730_568_600_000,
        ),
        "new-york" => (
            "1969-12-31 19:00:00",
            18_000_000,
            1_710_055_800_000,
            1_730_615_400_000,
        ),
        _ => panic!("unknown timezone child case: {case}"),
    };
    assert_eq!(
        Timestamp::from_millis(0)
            .unwrap()
            .to_local_date_time()
            .to_string(),
        epoch_local
    );
    assert_eq!(
        Timestamp::value_of("1970-01-01 00:00:00")
            .unwrap()
            .millis()
            .unwrap(),
        local_midnight
    );
    assert_eq!(
        Timestamp::value_of("2024-03-10 02:30:00")
            .unwrap()
            .millis()
            .unwrap(),
        gap
    );
    assert_eq!(
        Timestamp::value_of("2024-11-03 01:30:00")
            .unwrap()
            .millis()
            .unwrap(),
        fold
    );
}
