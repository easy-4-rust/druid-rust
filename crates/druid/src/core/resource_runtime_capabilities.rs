//! Druid connection-bound resource runtime capabilities。

bitflags::bitflags! {
    /// Druid owner 可对一个 resource 执行的运行期操作集合。
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub(crate) struct ResourceRuntimeCapabilities: u16 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const SEARCH = 1 << 2;
        const TRUNCATE = 1 << 3;
        const STREAM = 1 << 4;
        const RANGE = 1 << 5;
        const TYPE_MAP = 1 << 6;
        const RESULT_SET = 1 << 7;
        const FREE = 1 << 8;
    }
}

#[allow(dead_code)]
impl ResourceRuntimeCapabilities {
    pub(crate) const fn large_object() -> Self {
        Self::READ
            .union(Self::WRITE)
            .union(Self::SEARCH)
            .union(Self::TRUNCATE)
            .union(Self::STREAM)
            .union(Self::RANGE)
            .union(Self::FREE)
    }
}
