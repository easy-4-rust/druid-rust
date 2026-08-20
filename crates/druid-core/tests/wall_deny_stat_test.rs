extern crate druid_core as druid;
use druid_core::sql::WallDenyStat;

#[test]
fn wall_deny_stat_default() {
    let stat = WallDenyStat::default();
    assert_eq!(stat.deny_count(), 0);
    assert!(stat.last_deny_time_millis().is_none());
    assert_eq!(stat.reset_count(), 0);
}

#[test]
fn wall_deny_stat_increment() {
    let stat = WallDenyStat::default();
    let count = stat.increment_and_get_deny_count();
    assert_eq!(count, 1);
    assert_eq!(stat.deny_count(), 1);
    assert!(stat.last_deny_time_millis().is_some());
}

#[test]
fn wall_deny_stat_increment_multiple() {
    let stat = WallDenyStat::default();
    assert_eq!(stat.increment_and_get_deny_count(), 1);
    assert_eq!(stat.increment_and_get_deny_count(), 2);
    assert_eq!(stat.increment_and_get_deny_count(), 3);
    assert_eq!(stat.deny_count(), 3);
}

#[test]
fn wall_deny_stat_reset() {
    let stat = WallDenyStat::default();
    stat.increment_and_get_deny_count();
    stat.increment_and_get_deny_count();
    stat.reset();
    assert_eq!(stat.deny_count(), 0);
    assert!(stat.last_deny_time_millis().is_none());
    assert_eq!(stat.reset_count(), 1);
}

#[test]
fn wall_deny_stat_reset_count_increments() {
    let stat = WallDenyStat::default();
    stat.reset();
    stat.reset();
    assert_eq!(stat.reset_count(), 2);
}

#[test]
fn wall_deny_stat_debug() {
    let stat = WallDenyStat::default();
    let dbg = format!("{stat:?}");
    assert!(dbg.contains("WallDenyStat"));
}
