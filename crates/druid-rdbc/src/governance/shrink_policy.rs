//! 空闲连接收缩（shrink）的纯策略。
//!
//! 逐条对应 Java: `DruidDataSource#shrink(boolean checkTime, boolean keepAlive)`
//! 的 `checkTime == true` 分支；只把分支判断抽成纯函数，保留原始顺序与边界条件。

/// 收缩时对单个空闲连接做出的判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShrinkDecision {
    /// 放入 `evictConnections`，本次收缩后关闭。
    Evict,
    /// 放入 `keepAliveConnections`，本次收缩做保活探测。
    KeepAlive,
    /// 保留在池中（含移动到 `remaining` 位置）。
    Retain,
}

/// 收缩策略参数，字段名与 Java 属性一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShrinkPolicy {
    /// `minIdle`。
    pub min_idle: usize,
    /// `minEvictableIdleTimeMillis`。
    pub min_evictable_idle_millis: u64,
    /// `maxEvictableIdleTimeMillis`。
    pub max_evictable_idle_millis: u64,
    /// `keepAlive`。
    pub keep_alive: bool,
    /// `keepAliveBetweenTimeMillis`。
    pub keep_alive_between_millis: u64,
    /// `phyTimeoutMillis`；0 表示不启用。
    pub phy_timeout_millis: u64,
}

impl Default for ShrinkPolicy {
    fn default() -> Self {
        Self {
            min_idle: 0,
            min_evictable_idle_millis: 1_800_000,
            max_evictable_idle_millis: 7_200_000,
            keep_alive: false,
            keep_alive_between_millis: 120_000,
            phy_timeout_millis: 0,
        }
    }
}

/// 单个空闲连接的收缩判定。
///
/// 参数与 Java 局部变量对应：
/// - `index`：`connections` 中的下标 `i`；
/// - `pooling_count`：当前空闲连接数，决定 `checkCount = poolingCount - minIdle`；
/// - `idle_millis`：`currentTimeMillis - lastActiveTimeMillis`；
/// - `connect_age_millis`：`currentTimeMillis - connectTimeMillis`；
/// - `since_last_keep_millis`：`currentTimeMillis - lastKeepTimeMillis`。
pub fn classify(
    index: usize,
    pooling_count: usize,
    idle_millis: u64,
    connect_age_millis: u64,
    since_last_keep_millis: u64,
    policy: &ShrinkPolicy,
) -> ShrinkDecision {
    let check_count = pooling_count.saturating_sub(policy.min_idle);

    if policy.phy_timeout_millis > 0 && connect_age_millis > policy.phy_timeout_millis {
        return ShrinkDecision::Evict;
    }

    // Java: idleMillis < minEvictableIdleTimeMillis && idleMillis < keepAliveBetweenTimeMillis -> break
    if idle_millis < policy.min_evictable_idle_millis
        && idle_millis < policy.keep_alive_between_millis
    {
        return ShrinkDecision::Retain;
    }

    if idle_millis >= policy.min_evictable_idle_millis
        && (index < check_count || idle_millis > policy.max_evictable_idle_millis)
    {
        return ShrinkDecision::Evict;
    }

    if policy.keep_alive
        && idle_millis >= policy.keep_alive_between_millis
        && since_last_keep_millis >= policy.keep_alive_between_millis
    {
        return ShrinkDecision::KeepAlive;
    }

    ShrinkDecision::Retain
}

#[cfg(test)]
mod tests {
    use super::{classify, ShrinkDecision, ShrinkPolicy};

    fn policy() -> ShrinkPolicy {
        ShrinkPolicy {
            min_idle: 2,
            min_evictable_idle_millis: 100,
            max_evictable_idle_millis: 300,
            keep_alive: false,
            keep_alive_between_millis: 50,
            phy_timeout_millis: 0,
        }
    }

    #[test]
    fn below_both_thresholds_retains_without_scanning_further() {
        let decision = classify(0, 4, 40, 1_000, 0, &policy());
        assert_eq!(decision, ShrinkDecision::Retain);
    }

    #[test]
    fn index_below_check_count_evicts_when_idle_threshold_reached() {
        // checkCount = 4 - 2 = 2，index 0/1 可淘汰。
        assert_eq!(
            classify(0, 4, 120, 1_000, 0, &policy()),
            ShrinkDecision::Evict
        );
        assert_eq!(
            classify(1, 4, 120, 1_000, 0, &policy()),
            ShrinkDecision::Evict
        );
        // index 2 达到 minEvictable 但未超过 maxEvictable，保留。
        assert_eq!(
            classify(2, 4, 120, 1_000, 0, &policy()),
            ShrinkDecision::Retain
        );
    }

    #[test]
    fn beyond_max_evictable_evicts_even_above_check_count() {
        assert_eq!(
            classify(3, 4, 301, 1_000, 0, &policy()),
            ShrinkDecision::Evict
        );
    }

    #[test]
    fn physical_timeout_evicts_before_idle_checks() {
        let mut policy = policy();
        policy.phy_timeout_millis = 500;
        assert_eq!(classify(3, 4, 0, 501, 0, &policy), ShrinkDecision::Evict);
        assert_eq!(classify(3, 4, 0, 500, 0, &policy), ShrinkDecision::Retain);
    }

    #[test]
    fn keep_alive_wins_over_retain_when_due() {
        let mut policy = policy();
        policy.keep_alive = true;
        // index 3 在 checkCount 之外、未超 maxEvictable，但保活到期。
        assert_eq!(
            classify(3, 4, 60, 1_000, 60, &policy),
            ShrinkDecision::KeepAlive
        );
        // 距上次保活不足，保留。
        assert_eq!(
            classify(3, 4, 60, 1_000, 10, &policy),
            ShrinkDecision::Retain
        );
    }
}
