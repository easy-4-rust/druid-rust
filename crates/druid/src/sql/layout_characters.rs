//! SQL lexer 布局字符常量。

/// Lexer 使用的布局字符常量。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.LayoutCharacters`。Java 接口仅承载
/// 常量，Rust 使用不可实例化的值对象表达同一公共命名空间。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutCharacters;

impl LayoutCharacters {
    /// 制表符列增量。
    pub const TAB_INC: i32 = 8;
    /// 制表符。
    pub const TAB: u8 = 0x08;
    /// 换行符。
    pub const LF: u8 = 0x0A;
    /// 换页符。
    pub const FF: u8 = 0x0C;
    /// 回车符。
    pub const CR: u8 = 0x0D;
    /// 输入结束哨兵。
    pub const EOI: u8 = 0x1A;
}
