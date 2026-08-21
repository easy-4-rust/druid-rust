use std::fmt;

use serde::Deserialize;
use validator::Validate;

/// Java `/submitLogin` 的表单参数。
#[derive(Clone, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    /// Java 参数 `loginUsername`。
    #[validate(length(min = 1))]
    pub login_username: String,
    /// Java 参数 `loginPassword`。
    #[validate(length(min = 1))]
    pub login_password: String,
}

impl fmt::Debug for LoginRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoginRequest")
            .field("login_username", &self.login_username)
            .field("login_password", &"[REDACTED]")
            .finish()
    }
}
