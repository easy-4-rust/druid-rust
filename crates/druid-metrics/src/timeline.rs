use std::collections::VecDeque;

/// Timeline resolution tiers as specified in the plan.
///
/// - 15-second resolution: 180 buckets (45 minutes)
/// - 60-second resolution: 360 buckets (6 hours)
/// - 3600-second resolution: 360 buckets (15 days)
#[derive(Debug, Clone)]
pub struct TimelineRingBuffer {
    resolution_secs: u64,
    capacity: usize,
    buffer: VecDeque<TimelineBucket>,
}

#[derive(Debug, Clone)]
pub struct TimelineBucket {
    /// Timestamp (seconds since epoch, aligned to resolution).
    pub timestamp_secs: u64,
    /// Cumulative execution count at this bucket.
    pub exec_count: u64,
    /// Cumulative execution time in milliseconds.
    pub exec_time_millis: u64,
    /// Number of active connections at bucket time.
    pub active_connections: u32,
}

impl TimelineRingBuffer {
    /// Create a new ring buffer with the given resolution and capacity.
    pub fn new(resolution_secs: u64, capacity: usize) -> Self {
        Self {
            resolution_secs,
            capacity,
            buffer: VecDeque::with_capacity(capacity),
        }
    }

    /// Push a bucket into the ring. If full, overwrites the oldest entry.
    pub fn push(&mut self, bucket: TimelineBucket) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(bucket);
    }

    /// Returns an iterator over all buckets from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &TimelineBucket> {
        self.buffer.iter()
    }

    /// Returns the number of buckets currently stored.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns whether the ring is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Returns the resolution in seconds.
    pub fn resolution_secs(&self) -> u64 {
        self.resolution_secs
    }

    /// Returns the maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// A complete timeline snapshot containing all three resolution tiers.
#[derive(Debug, Clone)]
pub struct TimelineSnapshot {
    pub per_15s: TimelineRingBuffer,
    pub per_60s: TimelineRingBuffer,
    pub per_3600s: TimelineRingBuffer,
}

impl TimelineSnapshot {
    /// Create empty timelines with the standard Druid tiers.
    pub fn new() -> Self {
        Self {
            per_15s: TimelineRingBuffer::new(15, 180),
            per_60s: TimelineRingBuffer::new(60, 360),
            per_3600s: TimelineRingBuffer::new(3600, 360),
        }
    }
}

impl Default for TimelineSnapshot {
    fn default() -> Self {
        Self::new()
    }
}
