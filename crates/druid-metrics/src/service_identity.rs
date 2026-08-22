use std::sync::Arc;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

const SERVICE_ID_PREFIX: &str = "sid1_";
const SERVICE_ID_MAGIC: &[u8] = b"druid-service-identity-v1\0";

/// 单个服务身份组件允许的最大 UTF-8 字节数。
pub const MAX_SERVICE_ID_COMPONENT_BYTES: usize = 256;
const MAX_IDENTITY_PAYLOAD_BYTES: usize =
    SERVICE_ID_MAGIC.len() + 3 * (8 + MAX_SERVICE_ID_COMPONENT_BYTES);
/// canonical `serviceId` 允许的最大 ASCII 字节数。
pub const MAX_CANONICAL_SERVICE_ID_BYTES: usize =
    SERVICE_ID_PREFIX.len() + (MAX_IDENTITY_PAYLOAD_BYTES * 4).div_ceil(3);

/// gRPC 指标流的稳定服务身份。
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ServiceIdentity {
    service_name: Arc<str>,
    instance_id: Arc<str>,
    boot_id: Arc<str>,
    service_id: Arc<str>,
}

impl ServiceIdentity {
    /// 校验字节预算并创建身份；错误不包含攻击者输入。
    pub fn try_new(
        service_name: impl AsRef<str>,
        instance_id: impl AsRef<str>,
        boot_id: impl AsRef<str>,
    ) -> Result<Self, &'static str> {
        let components = [
            service_name.as_ref(),
            instance_id.as_ref(),
            boot_id.as_ref(),
        ];
        if components
            .iter()
            .any(|component| component.len() > MAX_SERVICE_ID_COMPONENT_BYTES)
        {
            return Err("service identity component exceeds byte budget");
        }
        let service_id = encode_components(components)?;
        Ok(Self {
            service_name: Arc::from(components[0]),
            instance_id: Arc::from(components[1]),
            boot_id: Arc::from(components[2]),
            service_id: Arc::from(service_id),
        })
    }

    /// 返回服务名。
    #[must_use]
    pub fn service_name(&self) -> &str {
        &self.service_name
    }
    /// 返回实例 ID。
    #[must_use]
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }
    /// 返回启动 ID。
    #[must_use]
    pub fn boot_id(&self) -> &str {
        &self.boot_id
    }
    /// 返回缓存的 canonical `serviceId`，不会重复编码或分配。
    #[must_use]
    pub fn service_id(&self) -> &str {
        &self.service_id
    }
    /// 返回共享 canonical `serviceId`。
    #[must_use]
    pub fn shared_service_id(&self) -> Arc<str> {
        Arc::clone(&self.service_id)
    }
    /// 返回共享服务名。
    #[must_use]
    pub fn shared_service_name(&self) -> Arc<str> {
        Arc::clone(&self.service_name)
    }
    /// 返回共享实例 ID。
    #[must_use]
    pub fn shared_instance_id(&self) -> Arc<str> {
        Arc::clone(&self.instance_id)
    }
    /// 返回共享启动 ID。
    #[must_use]
    pub fn shared_boot_id(&self) -> Arc<str> {
        Arc::clone(&self.boot_id)
    }

    /// 严格解析 canonical `serviceId`。
    pub fn parse(service_id: &str) -> Result<Self, &'static str> {
        if service_id.len() > MAX_CANONICAL_SERVICE_ID_BYTES {
            return Err("canonical service identity exceeds byte budget");
        }
        let encoded = service_id
            .strip_prefix(SERVICE_ID_PREFIX)
            .ok_or("canonical service identity prefix is invalid")?;
        let payload = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| "canonical service identity encoding is invalid")?;
        if payload.len() > MAX_IDENTITY_PAYLOAD_BYTES {
            return Err("canonical service identity payload exceeds byte budget");
        }
        let mut remaining = payload.as_slice();
        remaining = remaining
            .strip_prefix(SERVICE_ID_MAGIC)
            .ok_or("canonical service identity magic is invalid")?;
        let service_name = take_component(&mut remaining)?;
        let instance_id = take_component(&mut remaining)?;
        let boot_id = take_component(&mut remaining)?;
        if !remaining.is_empty() {
            return Err("canonical service identity has trailing data");
        }
        let identity = Self::try_new(service_name, instance_id, boot_id)?;
        if identity.service_id() != service_id {
            return Err("canonical service identity is not normalized");
        }
        Ok(identity)
    }
}

fn encode_components(components: [&str; 3]) -> Result<String, &'static str> {
    let mut capacity = SERVICE_ID_MAGIC.len();
    for component in components {
        capacity = capacity
            .checked_add(8)
            .and_then(|value| value.checked_add(component.len()))
            .ok_or("service identity size overflow")?;
    }
    let mut payload = Vec::with_capacity(capacity);
    payload.extend_from_slice(SERVICE_ID_MAGIC);
    for component in components {
        let length =
            u64::try_from(component.len()).map_err(|_| "service identity size overflow")?;
        payload.extend_from_slice(&length.to_be_bytes());
        payload.extend_from_slice(component.as_bytes());
    }
    let encoded = URL_SAFE_NO_PAD.encode(payload);
    let total = SERVICE_ID_PREFIX
        .len()
        .checked_add(encoded.len())
        .ok_or("service identity size overflow")?;
    if total > MAX_CANONICAL_SERVICE_ID_BYTES {
        return Err("canonical service identity exceeds byte budget");
    }
    Ok(format!("{SERVICE_ID_PREFIX}{encoded}"))
}

fn take_component<'a>(remaining: &mut &'a [u8]) -> Result<&'a str, &'static str> {
    let length_bytes = remaining
        .get(..8)
        .ok_or("canonical service identity is truncated")?;
    let array: [u8; 8] = length_bytes
        .try_into()
        .map_err(|_| "canonical service identity is truncated")?;
    let length = usize::try_from(u64::from_be_bytes(array))
        .map_err(|_| "canonical service identity length is invalid")?;
    if length > MAX_SERVICE_ID_COMPONENT_BYTES {
        return Err("service identity component exceeds byte budget");
    }
    let end = 8_usize
        .checked_add(length)
        .ok_or("canonical service identity length is invalid")?;
    let bytes = remaining
        .get(8..end)
        .ok_or("canonical service identity is truncated")?;
    let component = std::str::from_utf8(bytes)
        .map_err(|_| "canonical service identity component is not UTF-8")?;
    *remaining = remaining
        .get(end..)
        .ok_or("canonical service identity is truncated")?;
    Ok(component)
}
