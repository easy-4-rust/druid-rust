use super::jdbc_agent_connection::JdbcAgentTransport;
use druid_rdbc::data_source::{
    XaError, XaErrorCode, XaPrepareResult, XaResource, XaResourceManagerIdentity, Xid,
};
use druid_rdbc::error::RdbcError;
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 将一个已打开的 JDBC Agent session 映射为标准 XAResource。
/// 对应 Java: `javax.transaction.xa.XAResource` 经 `AgentServer#dispatchSession`。
pub(crate) struct JdbcAgentXaResource {
    transport: Arc<Mutex<Box<dyn JdbcAgentTransport>>>,
    supports_xa: bool,
    manager_identity: XaResourceManagerIdentity,
}

impl JdbcAgentXaResource {
    /// 创建绑定到单一 session 的 XA 资源。
    pub(super) fn new(
        transport: Arc<Mutex<Box<dyn JdbcAgentTransport>>>,
        supports_xa: bool,
        manager_identity: XaResourceManagerIdentity,
    ) -> Self {
        Self {
            transport,
            supports_xa,
            manager_identity,
        }
    }

    async fn request(
        &self,
        operation: &'static str,
        payload: JsonValue,
    ) -> Result<JsonValue, XaError> {
        if !self.supports_xa {
            return Err(XaError::feature_not_supported(operation));
        }
        self.transport
            .lock()
            .await
            .request(operation, payload)
            .await
            .map_err(Self::from_rdbc_error)
    }

    fn from_rdbc_error(error: RdbcError) -> XaError {
        let code = match &error {
            RdbcError::SqlException(exception) if exception.error_code() != 0 => {
                XaErrorCode(exception.error_code())
            }
            RdbcError::LoginTimeout | RdbcError::ConnectionDiscarded => XaErrorCode::XAER_RMFAIL,
            _ => XaErrorCode::XAER_RMERR,
        };
        XaError::with_source(code, error.to_string(), error)
    }

    fn protocol_error(message: impl Into<String>) -> XaError {
        let message = message.into();
        XaError::with_source(
            XaErrorCode::XAER_RMERR,
            message.clone(),
            RdbcError::DriverError(message),
        )
    }

    fn xid_payload(xid: &Xid) -> JsonValue {
        json!({
            "formatId": xid.format_id(),
            "globalTransactionId": xid.global_transaction_id(),
            "branchQualifier": xid.branch_qualifier(),
        })
    }

    fn parse_xid(value: &JsonValue) -> Result<Xid, XaError> {
        let format_id = value
            .get("formatId")
            .and_then(JsonValue::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| Self::protocol_error("XA recover XID has invalid formatId"))?;
        let global_transaction_id = Self::parse_bytes(value, "globalTransactionId")?;
        let branch_qualifier = Self::parse_bytes(value, "branchQualifier")?;
        Xid::new(format_id, global_transaction_id, branch_qualifier)
    }

    fn parse_bytes(value: &JsonValue, field: &str) -> Result<Vec<u8>, XaError> {
        value
            .get(field)
            .and_then(JsonValue::as_array)
            .ok_or_else(|| Self::protocol_error(format!("XA XID has invalid {field}")))?
            .iter()
            .map(|byte| {
                byte.as_u64()
                    .and_then(|byte| u8::try_from(byte).ok())
                    .ok_or_else(|| Self::protocol_error(format!("XA XID has invalid {field}")))
            })
            .collect()
    }

    fn required_bool(payload: &JsonValue, field: &str) -> Result<bool, XaError> {
        payload
            .get(field)
            .and_then(JsonValue::as_bool)
            .ok_or_else(|| Self::protocol_error(format!("XA response is missing boolean {field}")))
    }
}

#[async_trait::async_trait]
impl XaResource for JdbcAgentXaResource {
    async fn start(&mut self, xid: &Xid, flags: i32) -> Result<(), XaError> {
        let result = self
            .request(
                "xa_start",
                json!({"xid": Self::xid_payload(xid), "flags": flags}),
            )
            .await?;
        if result.get("started").and_then(JsonValue::as_i64) != Some(1) {
            return Err(Self::protocol_error("XA start response is invalid"));
        }
        Ok(())
    }

    async fn end(&mut self, xid: &Xid, flags: i32) -> Result<(), XaError> {
        self.request(
            "xa_end",
            json!({"xid": Self::xid_payload(xid), "flags": flags}),
        )
        .await?;
        Ok(())
    }

    async fn prepare(&mut self, xid: &Xid) -> Result<XaPrepareResult, XaError> {
        let result = self
            .request("xa_prepare", json!({"xid": Self::xid_payload(xid)}))
            .await?;
        match result.get("vote").and_then(JsonValue::as_i64) {
            Some(0) => Ok(XaPrepareResult::Ok),
            Some(3) => Ok(XaPrepareResult::ReadOnly),
            _ => Err(Self::protocol_error("XA prepare response has invalid vote")),
        }
    }

    async fn commit(&mut self, xid: &Xid, one_phase: bool) -> Result<(), XaError> {
        self.request(
            "xa_commit",
            json!({"xid": Self::xid_payload(xid), "onePhase": one_phase}),
        )
        .await?;
        Ok(())
    }

    async fn rollback(&mut self, xid: &Xid) -> Result<(), XaError> {
        self.request("xa_rollback", json!({"xid": Self::xid_payload(xid)}))
            .await?;
        Ok(())
    }

    async fn forget(&mut self, xid: &Xid) -> Result<(), XaError> {
        self.request("xa_forget", json!({"xid": Self::xid_payload(xid)}))
            .await?;
        Ok(())
    }

    async fn recover(&self, flags: i32) -> Result<Vec<Xid>, XaError> {
        let result = self.request("xa_recover", json!({"flags": flags})).await?;
        let xids = result
            .get("xids")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| Self::protocol_error("XA recover response is missing xids"))?;
        xids.iter().map(Self::parse_xid).collect()
    }

    fn resource_manager_identity(&self) -> Option<XaResourceManagerIdentity> {
        self.supports_xa.then(|| self.manager_identity.clone())
    }

    async fn is_same_rm(&self, other: &dyn XaResource) -> Result<bool, XaError> {
        if !self.supports_xa {
            return Err(XaError::feature_not_supported("xa_is_same_rm"));
        }
        let Some(other_identity) = other.resource_manager_identity() else {
            return Ok(false);
        };
        Ok(self.manager_identity == other_identity)
    }

    async fn get_transaction_timeout(&self) -> Result<u64, XaError> {
        let result = self.request("xa_get_timeout", json!({})).await?;
        result
            .get("value")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| Self::protocol_error("XA timeout response has invalid value"))
    }

    async fn set_transaction_timeout(&mut self, seconds: u64) -> Result<bool, XaError> {
        let seconds = i32::try_from(seconds)
            .map_err(|_| XaError::new(XaErrorCode::XAER_INVAL, "XA timeout exceeds i32"))?;
        let result = self
            .request("xa_set_timeout", json!({"seconds": seconds}))
            .await?;
        Self::required_bool(&result, "applied")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use druid_rdbc::data_source::{XaFlags, XaPrepareResult};
    use druid_rdbc::error::SqlException;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::{Arc as StdArc, Mutex as StdMutex};

    #[derive(Default)]
    struct FakeTransport {
        calls: StdArc<StdMutex<Vec<(String, JsonValue)>>>,
        responses: VecDeque<Result<JsonValue, RdbcError>>,
        closed: bool,
    }

    #[async_trait::async_trait]
    impl JdbcAgentTransport for FakeTransport {
        async fn request(
            &mut self,
            operation: &str,
            payload: JsonValue,
        ) -> Result<JsonValue, RdbcError> {
            self.calls
                .lock()
                .expect("calls lock")
                .push((operation.to_owned(), payload));
            if self.closed {
                return Err(RdbcError::ConnectionDiscarded);
            }
            self.responses.pop_front().unwrap_or(Ok(json!({})))
        }

        async fn close(&mut self) -> Result<(), RdbcError> {
            self.closed = true;
            Ok(())
        }

        fn is_unusable(&self) -> bool {
            self.closed
        }
    }

    fn resource(
        responses: impl IntoIterator<Item = Result<JsonValue, RdbcError>>,
        supports_xa: bool,
    ) -> (
        JdbcAgentXaResource,
        StdArc<StdMutex<Vec<(String, JsonValue)>>>,
    ) {
        resource_with_identity(responses, supports_xa, XaResourceManagerIdentity::new())
    }

    fn resource_with_identity(
        responses: impl IntoIterator<Item = Result<JsonValue, RdbcError>>,
        supports_xa: bool,
        manager_identity: XaResourceManagerIdentity,
    ) -> (
        JdbcAgentXaResource,
        StdArc<StdMutex<Vec<(String, JsonValue)>>>,
    ) {
        let calls = StdArc::new(StdMutex::new(Vec::new()));
        let transport = FakeTransport {
            calls: StdArc::clone(&calls),
            responses: responses.into_iter().collect(),
            closed: false,
        };
        (
            JdbcAgentXaResource::new(
                Arc::new(Mutex::new(Box::new(transport))),
                supports_xa,
                manager_identity,
            ),
            calls,
        )
    }

    fn xid() -> Xid {
        Xid::new(0x01020304, vec![0, 127, 128, 255], vec![2, 0, 254]).unwrap()
    }

    #[tokio::test]
    async fn xa_rpc_round_trips_xid_flags_votes_and_recovery_bytes() {
        let recovered = xid();
        let (mut resource, calls) = resource(
            [
                Ok(json!({"started": 1})),
                Ok(json!({})),
                Ok(json!({"vote": 3})),
                Ok(json!({})),
                Ok(json!({})),
                Ok(json!({})),
                Ok(json!({"xids": [{
                    "formatId": recovered.format_id(),
                    "globalTransactionId": recovered.global_transaction_id(),
                    "branchQualifier": recovered.branch_qualifier()
                }]})),
                Ok(json!({"value": 7})),
                Ok(json!({"applied": true})),
            ],
            true,
        );

        resource
            .start(&recovered, XaFlags::TMNOFLAGS)
            .await
            .unwrap();
        resource.end(&recovered, XaFlags::TMSUCCESS).await.unwrap();
        assert_eq!(
            resource.prepare(&recovered).await.unwrap(),
            XaPrepareResult::ReadOnly
        );
        resource.commit(&recovered, true).await.unwrap();
        resource.rollback(&recovered).await.unwrap();
        resource.forget(&recovered).await.unwrap();
        assert_eq!(
            resource.recover(XaFlags::TMSTARTRSCAN).await.unwrap(),
            [recovered.clone()]
        );
        assert_eq!(resource.get_transaction_timeout().await.unwrap(), 7);
        assert!(resource.set_transaction_timeout(9).await.unwrap());

        let calls = calls.lock().expect("calls lock");
        assert_eq!(
            calls.iter().map(|call| call.0.as_str()).collect::<Vec<_>>(),
            [
                "xa_start",
                "xa_end",
                "xa_prepare",
                "xa_commit",
                "xa_rollback",
                "xa_forget",
                "xa_recover",
                "xa_get_timeout",
                "xa_set_timeout"
            ]
        );
        let expected_xid = json!({
            "formatId": 0x01020304_i32,
            "globalTransactionId": [0, 127, 128, 255],
            "branchQualifier": [2, 0, 254]
        });
        assert_eq!(
            calls[0].1,
            json!({"xid": expected_xid, "flags": XaFlags::TMNOFLAGS})
        );
        assert_eq!(
            calls[1].1,
            json!({"xid": expected_xid, "flags": XaFlags::TMSUCCESS})
        );
        assert_eq!(calls[3].1, json!({"xid": expected_xid, "onePhase": true}));
        assert_eq!(calls[6].1, json!({"flags": XaFlags::TMSTARTRSCAN}));
        assert_eq!(calls[8].1, json!({"seconds": 9}));
    }

    #[tokio::test]
    async fn ordinary_agent_session_fails_closed_before_sending_xa_rpc() {
        let (mut resource, calls) = resource([], false);
        let error = resource
            .start(&xid(), XaFlags::TMNOFLAGS)
            .await
            .unwrap_err();
        let Some(RdbcError::SqlException(source)) = error.source_error() else {
            panic!("unsupported XA must preserve a SQL exception source");
        };
        assert_eq!(source.sql_state(), Some("0A000"));
        assert!(calls.lock().expect("calls lock").is_empty());
        assert!(resource.resource_manager_identity().is_none());
    }

    #[tokio::test]
    async fn xa_rpc_preserves_timeout_and_structured_vendor_error_code() {
        let timeout = RdbcError::LoginTimeout;
        let sql = RdbcError::SqlException(Box::new(SqlException::new(
            0x1234,
            Some("XA000".to_owned()),
            Some("structured XA failure".to_owned()),
        )));
        let (mut resource, _) = resource([Err(timeout), Err(sql)], true);
        let timeout_error = resource
            .start(&xid(), XaFlags::TMNOFLAGS)
            .await
            .unwrap_err();
        assert_eq!(timeout_error.code(), XaErrorCode::XAER_RMFAIL);
        assert!(matches!(
            timeout_error.source_error(),
            Some(RdbcError::LoginTimeout)
        ));
        let structured_error = resource
            .start(&xid(), XaFlags::TMNOFLAGS)
            .await
            .unwrap_err();
        assert_eq!(structured_error.code(), XaErrorCode(0x1234));
        let Some(RdbcError::SqlException(source)) = structured_error.source_error() else {
            panic!("structured SQL error must be retained");
        };
        assert_eq!(source.sql_state(), Some("XA000"));
        assert_eq!(source.error_code(), 0x1234);
    }

    #[tokio::test]
    async fn xa_prepare_rejects_unknown_vote_and_timeout_overflow_without_rpc() {
        let (mut resource, calls) = resource([Ok(json!({"vote": 9}))], true);
        assert!(resource.prepare(&xid()).await.is_err());
        assert_eq!(
            resource
                .set_transaction_timeout(u64::from(u32::MAX))
                .await
                .unwrap_err()
                .code(),
            XaErrorCode::XAER_INVAL
        );
        assert_eq!(calls.lock().expect("calls lock").len(), 1);
    }

    #[tokio::test]
    async fn xa_resource_manager_identity_requires_the_shared_identity_token() {
        let identity = XaResourceManagerIdentity::new();
        let (resource_a, _) = resource_with_identity([], true, identity.clone());
        let (resource_b, _) = resource_with_identity([], true, identity);
        let (resource_c, _) = resource([], true);
        assert!(resource_a.is_same_rm(&resource_b).await.unwrap());
        assert!(!resource_a.is_same_rm(&resource_c).await.unwrap());
    }
}
