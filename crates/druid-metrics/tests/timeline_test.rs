use druid_metrics::timeline::{TimelineBucket, TimelineRingBuffer, TimelineSnapshot};

#[test]
fn ring_buffer_15s_has_capacity_180() {
    let buf = TimelineRingBuffer::new(15, 180);
    assert_eq!(buf.resolution_secs(), 15);
    assert_eq!(buf.capacity(), 180);
    assert!(buf.is_empty());
}

#[test]
fn ring_buffer_60s_has_capacity_360() {
    let buf = TimelineRingBuffer::new(60, 360);
    assert_eq!(buf.resolution_secs(), 60);
    assert_eq!(buf.capacity(), 360);
}

#[test]
fn ring_buffer_3600s_has_capacity_360() {
    let buf = TimelineRingBuffer::new(3600, 360);
    assert_eq!(buf.resolution_secs(), 3600);
    assert_eq!(buf.capacity(), 360);
}

#[test]
fn ring_buffer_push_increases_len() {
    let mut buf = TimelineRingBuffer::new(15, 180);
    assert_eq!(buf.len(), 0);

    buf.push(TimelineBucket {
        timestamp_secs: 1000,
        exec_count: 10,
        exec_time_millis: 500,
        active_connections: 2,
    });
    assert_eq!(buf.len(), 1);
}

#[test]
fn ring_buffer_full_overwrites_oldest() {
    let mut buf = TimelineRingBuffer::new(15, 3); // capacity 3 for easy testing

    buf.push(TimelineBucket {
        timestamp_secs: 100,
        exec_count: 1,
        exec_time_millis: 10,
        active_connections: 1,
    });
    buf.push(TimelineBucket {
        timestamp_secs: 200,
        exec_count: 2,
        exec_time_millis: 20,
        active_connections: 2,
    });
    buf.push(TimelineBucket {
        timestamp_secs: 300,
        exec_count: 3,
        exec_time_millis: 30,
        active_connections: 3,
    });
    assert_eq!(buf.len(), 3);

    // Push a 4th -- should overwrite timestamp_secs=100
    buf.push(TimelineBucket {
        timestamp_secs: 400,
        exec_count: 4,
        exec_time_millis: 40,
        active_connections: 4,
    });
    assert_eq!(buf.len(), 3);

    let buckets: Vec<_> = buf.iter().collect();
    assert_eq!(buckets[0].timestamp_secs, 200); // oldest is now 200
    assert_eq!(buckets[1].timestamp_secs, 300);
    assert_eq!(buckets[2].timestamp_secs, 400);
}

#[test]
fn timeline_snapshot_has_all_three_tiers() {
    let snap = TimelineSnapshot::new();
    assert_eq!(snap.per_15s.resolution_secs(), 15);
    assert_eq!(snap.per_15s.capacity(), 180);
    assert_eq!(snap.per_60s.resolution_secs(), 60);
    assert_eq!(snap.per_60s.capacity(), 360);
    assert_eq!(snap.per_3600s.resolution_secs(), 3600);
    assert_eq!(snap.per_3600s.capacity(), 360);
}

#[test]
fn ring_buffer_overflow_preserves_count() {
    // Fill a capacity-5 buffer with 10 items; only last 5 should remain.
    let mut buf = TimelineRingBuffer::new(15, 5);
    for i in 0..10 {
        buf.push(TimelineBucket {
            timestamp_secs: i * 15,
            exec_count: i,
            exec_time_millis: i * 100,
            active_connections: 0,
        });
    }
    assert_eq!(buf.len(), 5);

    let buckets: Vec<_> = buf.iter().collect();
    assert_eq!(buckets[0].timestamp_secs, 5 * 15); // items 5..9
    assert_eq!(buckets[4].timestamp_secs, 9 * 15);
}
