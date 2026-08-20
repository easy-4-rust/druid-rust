bitflags::bitflags! {
    /// Operations supported by one concrete RDBC resource instance.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct RdbcResourceCapabilities: u16 {
        /// Reads scalar or materialized resource content.
        const READ = 1 << 0;
        /// Mutates resource content.
        const WRITE = 1 << 1;
        /// Searches resource content.
        const SEARCH = 1 << 2;
        /// Truncates large-object content.
        const TRUNCATE = 1 << 3;
        /// Opens a binary or character stream.
        const STREAM = 1 << 4;
        /// Reads an indexed range.
        const RANGE = 1 << 5;
        /// Applies an explicit SQL type map.
        const TYPE_MAP = 1 << 6;
        /// Exposes an `Array` as a `ResultSet`.
        const RESULT_SET = 1 << 7;
        /// Supports explicit release through `free()`.
        const FREE = 1 << 8;
    }
}

impl RdbcResourceCapabilities {
    /// Complete capability set for an RDBC `Array` implementation.
    #[must_use]
    pub const fn array() -> Self {
        Self::READ
            .union(Self::RANGE)
            .union(Self::TYPE_MAP)
            .union(Self::RESULT_SET)
            .union(Self::FREE)
    }

    /// Complete capability set for an RDBC `Blob` implementation.
    #[must_use]
    pub const fn blob() -> Self {
        Self::READ
            .union(Self::WRITE)
            .union(Self::SEARCH)
            .union(Self::TRUNCATE)
            .union(Self::STREAM)
            .union(Self::RANGE)
            .union(Self::FREE)
    }

    /// Complete capability set for an RDBC `Clob` or `NClob` implementation.
    #[must_use]
    pub const fn clob() -> Self {
        Self::READ
            .union(Self::WRITE)
            .union(Self::SEARCH)
            .union(Self::TRUNCATE)
            .union(Self::STREAM)
            .union(Self::RANGE)
            .union(Self::FREE)
    }

    /// Complete capability set for an RDBC `Ref` implementation.
    #[must_use]
    pub const fn reference() -> Self {
        Self::READ.union(Self::WRITE).union(Self::TYPE_MAP)
    }

    /// Complete capability set for an RDBC `SQLXML` implementation.
    #[must_use]
    pub const fn sql_xml() -> Self {
        Self::READ
            .union(Self::WRITE)
            .union(Self::STREAM)
            .union(Self::FREE)
    }
}
