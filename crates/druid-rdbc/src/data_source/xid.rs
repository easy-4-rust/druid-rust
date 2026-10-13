use super::{XaError, XaErrorCode};
use std::fmt;

/// XA 事务分支的完整标识。对应 Java: `javax.transaction.xa.Xid`。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Xid {
    format_id: i32,
    global_transaction_id: Vec<u8>,
    branch_qualifier: Vec<u8>,
}

/// XA 全局事务标识的最大字节数。对应 Java: `Xid#MAXGTRIDSIZE`。
pub const XID_MAX_GTRID_LENGTH: usize = 64;
/// XA 分支限定符的最大字节数。对应 Java: `Xid#MAXBQUALSIZE`。
pub const XID_MAX_BQUAL_LENGTH: usize = 64;

impl Xid {
    /// 创建并校验一个 XA 标识。
    ///
    /// 参数分别是格式编号、全局事务标识和分支限定符；成功时返回完整标识，超长时返回
    /// `XAER_INVAL`，不会截断输入。
    pub fn new(
        format_id: i32,
        global_transaction_id: Vec<u8>,
        branch_qualifier: Vec<u8>,
    ) -> Result<Self, XaError> {
        if global_transaction_id.len() > XID_MAX_GTRID_LENGTH {
            return Err(XaError::new(
                XaErrorCode::XAER_INVAL,
                format!(
                    "global_transaction_id length {} exceeds maximum {}",
                    global_transaction_id.len(),
                    XID_MAX_GTRID_LENGTH
                ),
            ));
        }
        if branch_qualifier.len() > XID_MAX_BQUAL_LENGTH {
            return Err(XaError::new(
                XaErrorCode::XAER_INVAL,
                format!(
                    "branch_qualifier length {} exceeds maximum {}",
                    branch_qualifier.len(),
                    XID_MAX_BQUAL_LENGTH
                ),
            ));
        }
        Ok(Self {
            format_id,
            global_transaction_id,
            branch_qualifier,
        })
    }

    /// 返回事务标识格式编号。
    #[must_use]
    pub fn format_id(&self) -> i32 {
        self.format_id
    }

    /// 返回全局事务标识字节。
    #[must_use]
    pub fn global_transaction_id(&self) -> &[u8] {
        &self.global_transaction_id
    }

    /// 返回分支限定符字节。
    #[must_use]
    pub fn branch_qualifier(&self) -> &[u8] {
        &self.branch_qualifier
    }
}

impl fmt::Display for Xid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        std::write!(
            formatter,
            "Xid(format_id={}, gtrid={}, bqual={})",
            self.format_id,
            hex_encode(&self.global_transaction_id),
            hex_encode(&self.branch_qualifier)
        )
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
