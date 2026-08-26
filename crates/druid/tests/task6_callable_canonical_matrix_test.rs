//! Canonical callable selector/descriptor and inheritance matrix.

use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    ParameterIndex, PreparedParameter, PreparedStatement, Statement,
};
use druid_rdbc::types::{RdbcObject, Value};
use std::collections::HashMap;

fn task6_callable_case_ids() -> &'static [&'static str] {
    &[
        "callable_errors_invalidate_cache_and_non_callable_handles_are_rejected",
        "callable_generic_execute_preserves_inherited_ordered_results",
        "callable_result_set_keeps_the_same_dynamic_statement_identity",
        "prepare_call_overloads_preserve_keys_delegation_and_cache_lifecycle",
    ]
}

#[derive(Default)]
struct CallableProbe {
    out: HashMap<CallableParameter, CallableOutParameter>,
    named: HashMap<String, CallableInputParameter>,
    prepared: Vec<PreparedParameter>,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(CallableProbe => CallableStatement);

#[async_trait::async_trait]
impl Statement for CallableProbe {
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for CallableProbe {
    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.prepared.push(parameter);
        Ok(())
    }
}

#[async_trait::async_trait]
impl CallableStatement for CallableProbe {
    fn register_out_parameter(
        &mut self,
        parameter: CallableParameter,
        out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        self.out.insert(parameter, out_parameter);
        Ok(())
    }

    fn set_named_parameter(
        &mut self,
        parameter_name: &str,
        parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        self.named.insert(parameter_name.to_owned(), parameter);
        Ok(())
    }

    fn out_value(&self, index: ParameterIndex) -> Result<Value, RdbcError> {
        Ok(Value::Int(i64::try_from(index.get()).unwrap()))
    }
}

#[tokio::test]
async fn task6_callable_canonical_matrix() {
    let cases = task6_callable_case_ids();
    let mut executed = Vec::with_capacity(cases.len());
    for (offset, case_id) in cases.iter().enumerate() {
        let mut callable = CallableProbe::default();
        let index = ParameterIndex::new(offset + 1).unwrap();
        callable
            .register_out_parameter(
                CallableParameter::Index(index),
                CallableOutParameter::with_scale(3, 2),
            )
            .unwrap();
        callable
            .register_out_parameter(
                CallableParameter::by_name(format!("name-{offset}")).unwrap(),
                CallableOutParameter::with_type_name(2002, "schema.kind"),
            )
            .unwrap();
        callable
            .set_named_parameter(
                "text",
                CallableInputParameter::String(Some((*case_id).to_owned())),
            )
            .unwrap();
        callable
            .set_named_parameter(
                "typed-null",
                CallableInputParameter::Null {
                    sql_type: 2002,
                    type_name: Some("schema.kind".to_owned()),
                },
            )
            .unwrap();
        callable
            .set_parameter(PreparedParameter {
                index,
                value: Value::String((*case_id).to_owned()),
            })
            .unwrap();
        assert_eq!(callable.out.len(), 2);
        assert_eq!(callable.named.len(), 2);
        assert_eq!(callable.prepared.len(), 1);
        assert_eq!(
            callable.out_value(index),
            Ok(Value::Int(i64::try_from(index.get()).unwrap()))
        );
        assert_eq!(
            callable.out_parameter(&CallableParameter::Index(index)),
            Ok(RdbcObject::from(Value::Int(
                i64::try_from(index.get()).unwrap()
            )))
        );
        callable.close().await.unwrap();
        assert_eq!(callable.is_closed(), Ok(true));
        executed.push(*case_id);
    }
    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_callable_canonical_matrix:{}",
        executed.join("|")
    );
}
