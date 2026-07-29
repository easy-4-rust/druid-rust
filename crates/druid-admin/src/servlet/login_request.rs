use serde::Deserialize;
use validator::Validate;

/// Java `/submitLogin` 的表单参数。
#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    /// Java 参数 `loginUsername`。
    #[validate(length(min = 1))]
    pub login_username: String,
    /// Java 参数 `loginPassword`。
    #[validate(length(min = 1))]
    pub login_password: String,
}
