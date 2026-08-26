//! Canonical prepared-parameter value extraction.

use druid_rdbc::statement::PreparedParameter;
use druid_rdbc::types::Value;

pub(crate) struct PreparedParameterMaterializer;

impl PreparedParameterMaterializer {
    pub(crate) fn materialize_standard(parameter: PreparedParameter) -> (usize, Value) {
        (parameter.index.get(), parameter.value)
    }
}
