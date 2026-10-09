struct ObjectShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
}

impl ObjectShortCircuitFilter {
    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
    }
}

impl ResultSetFilter for ObjectShortCircuitFilter {
    fn result_set_get_object(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<Value, DruidError> {
        self.record(format!("result_set_get_object:{column_index}"));
        Ok(Value::Int(901))
    }

    fn result_set_get_object_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<Value, DruidError> {
        self.record(format!("result_set_get_object_by_label:{column_label}"));
        Ok(Value::String("filtered-label-object".to_string()))
    }

    fn result_set_get_object_with_type_map(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        let identity = type_map_identity(type_map);
        self.record(format!(
            "result_set_get_object_with_type_map:{column_index}:{identity}"
        ));
        Ok(RdbcObject::String(format!("filtered-map:{identity}")))
    }

    fn result_set_get_object_by_label_with_type_map(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        let identity = type_map_identity(type_map);
        self.record(format!(
            "result_set_get_object_by_label_with_type_map:{column_label}:{identity}"
        ));
        Ok(RdbcObject::String(format!("filtered-label-map:{identity}")))
    }

    fn result_set_get_object_typed(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, DruidError> {
        self.record(format!(
            "result_set_get_object_typed:{column_index}:{target_type:?}"
        ));
        Ok(RdbcObject::String(format!(
            "filtered-typed:{target_type:?}"
        )))
    }

    fn result_set_get_object_typed_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, DruidError> {
        self.record(format!(
            "result_set_get_object_typed_by_label:{column_label}:{target_type:?}"
        ));
        if column_label == "fail" {
            Err(DruidError::DriverError(
                "filtered object failure".to_string(),
            ))
        } else {
            Ok(RdbcObject::String(format!(
                "filtered-label-typed:{target_type:?}"
            )))
        }
    }
}

fn type_map_identity(type_map: Option<&RdbcTypeMap>) -> String {
    match type_map {
        None => "null".to_string(),
        Some(type_map) if type_map.is_empty() => "empty".to_string(),
        Some(type_map) => {
            let mut entries = type_map
                .mappings()
                .iter()
                .map(|(name, target)| format!("{name}={target:?}"))
                .collect::<Vec<_>>();
            entries.sort();
            entries.join(",")
        }
    }
}
