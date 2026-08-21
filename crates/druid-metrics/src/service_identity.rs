use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

const SERVICE_ID_PREFIX: &str = "sid1_";
const SERVICE_ID_MAGIC: &[u8] = b"druid-service-identity-v1\0";

/// gRPC 指标流的稳定服务身份。
///
/// Rust-only 协议对象；内部始终以三个类型化组件参与 HashMap key，外部
/// `serviceId` 使用 length-prefix bytes + base64url 无填充编码，可逆且无分隔符碰撞。
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ServiceIdentity {
    service_name: String,
    instance_id: String,
    boot_id: String,
}

impl ServiceIdentity {
    /// 从服务名、实例 ID 与启动 ID 创建类型化身份；任意 UTF-8（含空串）均可编码。
    #[must_use]
    pub fn new(
        service_name: impl Into<String>,
        instance_id: impl Into<String>,
        boot_id: impl Into<String>,
    ) -> Self {
        Self {
            service_name: service_name.into(),
            instance_id: instance_id.into(),
            boot_id: boot_id.into(),
        }
    }

    /// 返回人类可读服务名组件。
    #[must_use]
    pub fn service_name(&self) -> &str {
        &self.service_name
    }

    /// 返回人类可读实例 ID 组件。
    #[must_use]
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    /// 返回人类可读启动 ID 组件。
    #[must_use]
    pub fn boot_id(&self) -> &str {
        &self.boot_id
    }

    /// 生成确定、无歧义、URL query 可安全携带的 canonical `serviceId`。
    #[must_use]
    pub fn service_id(&self) -> String {
        let components = [
            self.service_name.as_bytes(),
            self.instance_id.as_bytes(),
            self.boot_id.as_bytes(),
        ];
        let payload_capacity = SERVICE_ID_MAGIC.len()
            + components
                .iter()
                .map(|component| 8_usize.saturating_add(component.len()))
                .sum::<usize>();
        let mut payload = Vec::with_capacity(payload_capacity);
        payload.extend_from_slice(SERVICE_ID_MAGIC);
        for component in components {
            payload.extend_from_slice(&(component.len() as u64).to_be_bytes());
            payload.extend_from_slice(component);
        }
        format!("{SERVICE_ID_PREFIX}{}", URL_SAFE_NO_PAD.encode(payload))
    }

    /// 校验并解码 canonical `serviceId`；旧 slash 格式、截断或尾随数据均拒绝。
    #[must_use]
    pub fn from_service_id(service_id: &str) -> Option<Self> {
        let encoded = service_id.strip_prefix(SERVICE_ID_PREFIX)?;
        let payload = URL_SAFE_NO_PAD.decode(encoded).ok()?;
        let mut remaining = payload.as_slice();
        remaining = remaining.strip_prefix(SERVICE_ID_MAGIC)?;
        let service_name = take_component(&mut remaining)?;
        let instance_id = take_component(&mut remaining)?;
        let boot_id = take_component(&mut remaining)?;
        if !remaining.is_empty() {
            return None;
        }
        Some(Self::new(service_name, instance_id, boot_id))
    }
}

fn take_component(remaining: &mut &[u8]) -> Option<String> {
    let length_bytes = remaining.get(..8)?;
    let length = usize::try_from(u64::from_be_bytes(length_bytes.try_into().ok()?)).ok()?;
    let bytes = remaining.get(8..8_usize.checked_add(length)?)?;
    let component = std::str::from_utf8(bytes).ok()?.to_owned();
    *remaining = remaining.get(8_usize.checked_add(length)?..)?;
    Some(component)
}
