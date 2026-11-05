//! `java.sql.Timestamp` / `java.sql.Date` / `java.sql.Time` 薄扩展 trait 的语义合同测试。
//!
//! 期望值全部来自 JDK 8（1.8.0_202）的 `java.sql` 三个类型的实际行为：
//! `Timestamp#toString` 的小数部分左补零到九位后裁掉末尾的零，`Date#toString` 输出
//! `yyyy-mm-dd`，`Time#toString` 输出 `hh:mm:ss`，`Timestamp` 的先后比较先比较毫秒
//! 再比较纳秒，`java.sql.Date` 与 `java.sql.Time` 的时刻/日期访问器在 JDK 8 中恒抛
//! `IllegalArgumentException`，两者的 `toInstant` 恒抛 `UnsupportedOperationException`。

use std::cmp::Ordering;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_rdbc::types::{DateExt, TimeExt, TimestampExt};

/// 构造 2024-01-02 的本地日期时间，便于断言稳定的文本形态。
fn timestamp(hour: u32, minute: u32, second: u32, nanos: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2024, 1, 2)
        .unwrap()
        .and_hms_nano_opt(hour, minute, second, nanos)
        .unwrap()
}

#[test]
fn timestamp_to_string_matches_the_jdk8_nanosecond_format() {
    assert_eq!(
        timestamp(3, 4, 5, 0).to_jdbc_string(),
        "2024-01-02 03:04:05.0"
    );
    assert_eq!(
        timestamp(3, 4, 5, 1).to_jdbc_string(),
        "2024-01-02 03:04:05.000000001"
    );
    assert_eq!(
        timestamp(3, 4, 5, 10).to_jdbc_string(),
        "2024-01-02 03:04:05.00000001"
    );
    assert_eq!(
        timestamp(3, 4, 5, 100_000_000).to_jdbc_string(),
        "2024-01-02 03:04:05.1"
    );
    assert_eq!(
        timestamp(3, 4, 5, 120_000_000).to_jdbc_string(),
        "2024-01-02 03:04:05.12"
    );
    assert_eq!(
        timestamp(3, 4, 5, 123_456_789).to_jdbc_string(),
        "2024-01-02 03:04:05.123456789"
    );
}

#[test]
fn timestamp_value_of_accepts_the_jdk8_escape_format() {
    let plain = NaiveDateTime::value_of("2024-01-02 03:04:05").unwrap();
    assert_eq!(plain, timestamp(3, 4, 5, 0));
    assert_eq!(plain.to_jdbc_string(), "2024-01-02 03:04:05.0");

    // JDK 8 允许省略前导零，并裁掉文本两端的空白。
    assert_eq!(
        NaiveDateTime::value_of("2024-1-2 3:4:5").unwrap(),
        timestamp(3, 4, 5, 0)
    );
    assert_eq!(
        NaiveDateTime::value_of(" 2024-01-02 03:04:05 ").unwrap(),
        timestamp(3, 4, 5, 0)
    );

    // JDK 8 把小数部分右补零到九位。
    assert_eq!(
        NaiveDateTime::value_of("2024-01-02 03:04:05.12")
            .unwrap()
            .get_nanos(),
        120_000_000
    );
    assert_eq!(
        NaiveDateTime::value_of("2024-01-02 03:04:05.1")
            .unwrap()
            .get_nanos(),
        100_000_000
    );
    assert_eq!(
        NaiveDateTime::value_of("2024-01-02 03:04:05.000000001")
            .unwrap()
            .get_nanos(),
        1
    );

    // JDK 8 的宽松日历会把越界的日、时进位。
    assert_eq!(
        NaiveDateTime::value_of("2024-02-30 00:00:00").unwrap(),
        NaiveDate::from_ymd_opt(2024, 3, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    );
    assert_eq!(
        NaiveDateTime::value_of("2024-01-02 25:00:00").unwrap(),
        NaiveDate::from_ymd_opt(2024, 1, 3)
            .unwrap()
            .and_hms_opt(1, 0, 0)
            .unwrap()
    );
}

#[test]
fn timestamp_value_of_rejects_the_jdk8_invalid_formats() {
    for invalid in [
        "2024-01-02",
        "2024-01-02 03:04",
        "2024-01-02 03:04:05.",
        "2024-01-02 03:04:05.abc",
        "2024-01-02 03:04:05.1234567890",
        "2024-01-32 00:00:00",
        "2024-13-01 00:00:00",
        "2024-01-00 00:00:00",
        "202-01-02 03:04:05",
    ] {
        assert!(
            NaiveDateTime::value_of(invalid).is_err(),
            "the JDK 8 parser rejects '{invalid}'"
        );
    }
}

#[test]
fn timestamp_nanos_round_trip_and_rejects_out_of_range_values() {
    let mut value = timestamp(3, 4, 5, 1);
    assert_eq!(value.get_nanos(), 1);
    value.set_nanos(123_456_789).unwrap();
    assert_eq!(value.get_nanos(), 123_456_789);
    assert_eq!(value.to_jdbc_string(), "2024-01-02 03:04:05.123456789");
    value.set_nanos(0).unwrap();
    assert_eq!(value.get_nanos(), 0);
    assert_eq!(value.to_jdbc_string(), "2024-01-02 03:04:05.0");

    let before = value;
    assert!(value.set_nanos(1_000_000_000).is_err());
    assert!(value.set_nanos(-1).is_err());
    assert_eq!(value, before);
}

#[test]
fn timestamp_after_before_and_compare_use_millis_then_nanos() {
    let smaller = timestamp(3, 4, 5, 100_000_000);
    let larger = timestamp(3, 4, 5, 200_000_000);
    assert!(smaller.before(&larger).unwrap());
    assert!(!smaller.after(&larger).unwrap());
    assert!(larger.after(&smaller).unwrap());
    assert!(!larger.before(&smaller).unwrap());
    assert_eq!(smaller.compare_to(&larger).unwrap(), Ordering::Less);
    assert_eq!(larger.compare_to(&smaller).unwrap(), Ordering::Greater);
    assert_eq!(smaller.compare_to(&smaller).unwrap(), Ordering::Equal);
    assert!(!smaller.before(&smaller).unwrap());
    assert!(!smaller.after(&smaller).unwrap());

    // 同一秒内毫秒相同、只有纳秒不同的两个值，按纳秒继续排序。
    let same_millis = timestamp(3, 4, 5, 100_000_999);
    assert_eq!(smaller.compare_to(&same_millis).unwrap(), Ordering::Less);
    assert!(smaller.before(&same_millis).unwrap());
}

#[test]
fn timestamp_compare_to_date_keeps_the_jdk8_millisecond_precision() {
    let smaller = timestamp(3, 4, 5, 100_000_000);
    let same_millis = timestamp(3, 4, 5, 100_000_999);
    let later = timestamp(3, 4, 5, 200_000_000);

    // java.util.Date 路径只保留毫秒精度，JDK 8 因此把这两个值判为相等。
    assert_eq!(
        smaller.compare_to_date(&same_millis).unwrap(),
        Ordering::Equal
    );
    assert_eq!(smaller.compare_to_date(&later).unwrap(), Ordering::Less);
    assert_eq!(later.compare_to_date(&smaller).unwrap(), Ordering::Greater);
}

#[test]
fn timestamp_equals_and_hash_code_match_the_jdk8_rules() {
    let value = timestamp(3, 4, 5, 100_000_000);
    let same = timestamp(3, 4, 5, 100_000_000);
    let same_millis = timestamp(3, 4, 5, 100_000_999);
    let later = timestamp(3, 4, 6, 0);

    assert!(value.equals_timestamp(&same).unwrap());
    assert!(value.equals_object(&same).unwrap());
    assert!(!value.equals_timestamp(&same_millis).unwrap());
    assert!(!value.equals_timestamp(&later).unwrap());

    // JDK 8 的 hashCode 只由毫秒决定，纳秒不参与。
    assert_eq!(value.hash_code().unwrap(), same.hash_code().unwrap());
    assert_eq!(value.hash_code().unwrap(), same_millis.hash_code().unwrap());
    assert_ne!(value.hash_code().unwrap(), later.hash_code().unwrap());
}

#[test]
fn timestamp_instant_conversion_round_trips() {
    let value = timestamp(3, 4, 5, 123_456_789);
    let instant = value.to_instant().unwrap();
    assert_eq!(instant.timestamp_subsec_nanos(), 123_456_789);
    assert_eq!(NaiveDateTime::from_instant(instant).unwrap(), value);
    assert_eq!(
        value.to_local_date_time(),
        value,
        "toLocalDateTime returns the same local view"
    );
}

#[test]
fn timestamp_set_time_and_get_time_use_epoch_millis() {
    let mut value = timestamp(3, 4, 5, 0);
    value.set_time(1_704_135_845_100).unwrap();
    assert_eq!(value.get_time().unwrap(), 1_704_135_845_100);

    value.set_time(1_234).unwrap();
    assert_eq!(value.get_time().unwrap(), 1_234);

    // JDK 8 的 setTime 对负值同样保留毫秒精度。
    value.set_time(-1).unwrap();
    assert_eq!(value.get_time().unwrap(), -1);

    let constructed = NaiveDateTime::new_timestamp_from_millis(1_704_135_845_100).unwrap();
    assert_eq!(constructed.get_time().unwrap(), 1_704_135_845_100);
    assert_eq!(constructed.to_jdbc_string(), value_at(1_704_135_845_100));
}

/// 断言 epoch 毫秒在默认时区下的 JDK 8 文本形态。
fn value_at(millis: i64) -> String {
    NaiveDateTime::new_timestamp_from_millis(millis)
        .unwrap()
        .to_jdbc_string()
}

#[test]
fn timestamp_constructor_uses_the_java_year_and_month_convention() {
    // java.sql.Timestamp(int,...) 的 year 以 1900 为基准，month 以 0 表示一月。
    assert_eq!(
        NaiveDateTime::new_timestamp(124, 0, 2, 3, 4, 5, 1).unwrap(),
        timestamp(3, 4, 5, 1)
    );
    assert_eq!(
        NaiveDateTime::new_timestamp(124, 0, 2, 3, 4, 5, 1)
            .unwrap()
            .to_jdbc_string(),
        "2024-01-02 03:04:05.000000001"
    );

    // JDK 8 的宽松日历会让越界的月份和日期进位。
    assert_eq!(
        NaiveDateTime::new_timestamp(124, 13, 2, 3, 4, 5, 0)
            .unwrap()
            .to_jdbc_string(),
        "2025-02-02 03:04:05.0"
    );
    assert_eq!(
        NaiveDateTime::new_timestamp(124, 0, 32, 3, 4, 5, 0)
            .unwrap()
            .to_jdbc_string(),
        "2024-02-01 03:04:05.0"
    );

    assert!(NaiveDateTime::new_timestamp(124, 0, 2, 3, 4, 5, 1_000_000_000).is_err());
    assert!(NaiveDateTime::new_timestamp(124, 0, 2, 3, 4, 5, -1).is_err());
}

#[test]
fn date_to_string_and_value_of_match_the_jdk8_format() {
    let value = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
    assert_eq!(value.to_jdbc_string(), "2024-01-02");
    assert_eq!(NaiveDate::value_of("2024-01-02").unwrap(), value);
    assert_eq!(NaiveDate::value_of("2024-1-2").unwrap(), value);
    assert_eq!(
        NaiveDate::value_of("2024-01-02").unwrap().to_jdbc_string(),
        "2024-01-02"
    );

    // JDK 8 的宽松日历会把越界的日期进位。
    assert_eq!(
        NaiveDate::value_of("2024-02-30").unwrap(),
        NaiveDate::from_ymd_opt(2024, 3, 1).unwrap()
    );

    for invalid in [
        "not-a-date",
        "2024-13-01",
        "2024-00-01",
        "2024-01-00",
        "2024-01-32",
        "24-01-02",
        "2024-01-02 03:04:05",
    ] {
        assert!(
            NaiveDate::value_of(invalid).is_err(),
            "the JDK 8 parser rejects '{invalid}'"
        );
    }
}

#[test]
fn date_to_local_date_and_constructor_match_the_jdk8_rules() {
    let value = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
    assert_eq!(value.to_local_date(), value);
    // java.sql.Date(int,int,int) 的 year 以 1900 为基准，month 以 0 表示一月。
    assert_eq!(NaiveDate::new_date(124, 0, 2).unwrap(), value);
    assert_eq!(
        NaiveDate::new_date(124, 13, 2).unwrap(),
        NaiveDate::from_ymd_opt(2025, 2, 2).unwrap()
    );
}

#[test]
fn date_set_time_replaces_the_local_date() {
    let mut value = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
    value.set_time(0).unwrap();
    assert_eq!(value, NaiveDate::new_date_from_millis(0).unwrap());
    value.set_time(1_704_135_845_100).unwrap();
    assert_eq!(
        value,
        NaiveDate::new_date_from_millis(1_704_135_845_100).unwrap()
    );
    assert_ne!(
        value,
        NaiveDate::new_date_from_millis(0).unwrap(),
        "a distant instant selects a different local date"
    );
}

#[test]
fn date_to_instant_is_unsupported_like_jdk8() {
    let value = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
    let error = value.to_instant().unwrap_err();
    assert!(
        matches!(
            error,
            druid_rdbc::RdbcError::UnsupportedOperation { operation } if operation == "java.sql.Date#toInstant"
        ),
        "JDK 8 raises UnsupportedOperationException, got {error:?}"
    );
}

#[test]
#[allow(deprecated)]
fn date_deprecated_time_accessors_are_rejected_like_jdk8() {
    let mut value = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
    assert!(value.get_hours().is_err());
    assert!(value.get_minutes().is_err());
    assert!(value.get_seconds().is_err());

    let before = value;
    assert!(value.set_hours(1).is_err());
    assert!(value.set_minutes(1).is_err());
    assert!(value.set_seconds(1).is_err());
    assert_eq!(value, before);
}

#[test]
fn time_to_string_and_value_of_match_the_jdk8_format() {
    let value = NaiveTime::from_hms_opt(12, 34, 56).unwrap();
    assert_eq!(value.to_jdbc_string(), "12:34:56");
    assert_eq!(NaiveTime::value_of("12:34:56").unwrap(), value);
    assert_eq!(
        NaiveTime::value_of("1:2:3").unwrap().to_jdbc_string(),
        "01:02:03"
    );

    // JDK 8 的宽松日历会把越界的时刻进位。
    assert_eq!(
        NaiveTime::value_of("25:00:00").unwrap().to_jdbc_string(),
        "01:00:00"
    );

    for invalid in ["bad", "12:34", "12", "", ":", "12:34:"] {
        assert!(
            NaiveTime::value_of(invalid).is_err(),
            "the JDK 8 parser rejects '{invalid}'"
        );
    }
}

#[test]
fn time_to_local_time_and_constructor_match_the_jdk8_rules() {
    let value = NaiveTime::from_hms_opt(12, 34, 56).unwrap();
    assert_eq!(value.to_local_time(), value);
    assert_eq!(NaiveTime::new_time(12, 34, 56).unwrap(), value);
    assert_eq!(
        NaiveTime::new_time(25, 0, 0).unwrap().to_jdbc_string(),
        "01:00:00"
    );
}

#[test]
fn time_set_time_replaces_the_local_time() {
    let mut value = NaiveTime::from_hms_opt(12, 34, 56).unwrap();
    value.set_time(0).unwrap();
    assert_eq!(value, NaiveTime::new_time_from_millis(0).unwrap());
    value.set_time(1_704_135_845_100).unwrap();
    assert_eq!(
        value,
        NaiveTime::new_time_from_millis(1_704_135_845_100).unwrap()
    );
}

#[test]
fn time_to_instant_is_unsupported_like_jdk8() {
    let value = NaiveTime::from_hms_opt(12, 34, 56).unwrap();
    let error = value.to_instant().unwrap_err();
    assert!(
        matches!(
            error,
            druid_rdbc::RdbcError::UnsupportedOperation { operation } if operation == "java.sql.Time#toInstant"
        ),
        "JDK 8 raises UnsupportedOperationException, got {error:?}"
    );
}

#[test]
#[allow(deprecated)]
fn time_deprecated_date_accessors_are_rejected_like_jdk8() {
    let mut value = NaiveTime::from_hms_opt(12, 34, 56).unwrap();
    assert!(value.get_year().is_err());
    assert!(value.get_month().is_err());
    assert!(value.get_day().is_err());
    assert!(value.get_date().is_err());

    let before = value;
    assert!(value.set_year(1).is_err());
    assert!(value.set_month(1).is_err());
    assert!(value.set_date(1).is_err());
    assert_eq!(value, before);
}

#[test]
fn jdbc_type_entries_are_reachable_through_the_extension_traits() {
    // java.sql.Timestamp#TYPE / java.sql.Date#TYPE / java.sql.Time#TYPE 的类型入口。
    assert_eq!(
        NaiveDateTime::value_of("2024-01-02 03:04:05")
            .unwrap()
            .to_jdbc_string(),
        "2024-01-02 03:04:05.0"
    );
    assert_eq!(
        NaiveDate::value_of("2024-01-02").unwrap().to_jdbc_string(),
        "2024-01-02"
    );
    assert_eq!(
        NaiveTime::value_of("12:34:56").unwrap().to_jdbc_string(),
        "12:34:56"
    );
}

#[test]
fn timestamp_value_of_local_date_time_is_identity() {
    // 对应 Java: Timestamp#valueOf(LocalDateTime)；null 入参 Java 抛 NPE，Rust 非空载体。
    let value = timestamp(10, 20, 30, 123_456_789);
    assert_eq!(
        <NaiveDateTime as TimestampExt>::value_of_local_date_time(value),
        value,
        "valueOf(LocalDateTime) 必须保留纳秒精度"
    );
}

#[test]
fn date_value_of_local_date_is_identity() {
    // 对应 Java: Date#valueOf(LocalDate)
    let value = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
    assert_eq!(<NaiveDate as DateExt>::value_of_local_date(value), value);
}

#[test]
fn time_value_of_local_time_is_identity() {
    // 对应 Java: Time#valueOf(LocalTime)
    let value = NaiveTime::from_hms_nano_opt(8, 9, 10, 111_111_111).unwrap();
    assert_eq!(<NaiveTime as TimeExt>::value_of_local_time(value), value);
}
