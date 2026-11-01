//! Oracle 配置的 Integer.parseInt 边界；复用真实共享状态和两个生产分类器。
use druid::core::{
    AbstractOracleExceptionSorter, ExceptionSorter, ExceptionSorterProperties,
    OceanBaseOracleExceptionSorter, OracleExceptionSorter, SqlException,
    ORACLE_FATAL_ERROR_CODES_PROPERTY,
};
use std::collections::BTreeSet;

fn properties(value: &str) -> ExceptionSorterProperties {
    ExceptionSorterProperties::from([(
        ORACLE_FATAL_ERROR_CODES_PROPERTY.to_owned(),
        value.to_owned(),
    )])
}

#[test]
fn java_integer_boundaries_preserve_exact_sets_append_and_replace() {
    // 覆盖 Java configFromProperties 的合法数字、溢出、空项、不 trim 与追加语义。
    let mut sorter = AbstractOracleExceptionSorter::new();
    for (input, expected) in [
        ("１２３４５,١٢٣٤٥,+१२३४५,1٢３", BTreeSet::from([123, 12345])),
        (
            "-٢١٤٧٤٨٣٦٤٨,+２１４７４８３６４７",
            BTreeSet::from([i32::MIN, i32::MAX]),
        ),
        (
            "2147483648,-2147483649,＋1,−1,²,𝟙, 1,1 ,+,-,",
            BTreeSet::new(),
        ),
        ("0,-0,+0,００００１２３,", BTreeSet::from([0, 123])),
    ] {
        sorter.set_fatal_error_codes(BTreeSet::new());
        sorter.config_from_properties(Some(&properties(input)));
        assert_eq!(sorter.fatal_error_codes(), &expected, "input={input:?}");
    }
    sorter.config_from_properties(Some(&properties("１２３,४५६")));
    assert_eq!(sorter.fatal_error_codes(), &BTreeSet::from([0, 123, 456]));
    sorter.config_from_properties(None);
    sorter.config_from_properties(Some(&ExceptionSorterProperties::new()));
    assert_eq!(sorter.fatal_error_codes(), &BTreeSet::from([0, 123, 456]));
    sorter.set_fatal_error_codes(BTreeSet::from([7]));
    assert_eq!(sorter.fatal_error_codes(), &BTreeSet::from([7]));
}

#[test]
fn both_oracle_classifiers_use_unicode_property_codes_without_widening_defaults() {
    let mut oracle = OracleExceptionSorter::new();
    let mut oceanbase = OceanBaseOracleExceptionSorter::new();
    oracle.set_fatal_error_codes(BTreeSet::new());
    oceanbase.set_fatal_error_codes(BTreeSet::new());
    for sorter in [&mut oracle as &mut dyn ExceptionSorter, &mut oceanbase] {
        let exception = SqlException::driver(1234567890, "ordinary");
        assert!(!sorter.is_exception_fatal(&exception));
        sorter.config_from_properties(Some(&properties("１２３４５６７８９０")));
        assert!(sorter.is_exception_fatal(&exception));
        assert!(sorter.is_exception_fatal(&SqlException::driver(-1234567890, "ordinary")));
        assert!(!sorter.is_exception_fatal(&SqlException::driver(1234567891, "ordinary")));
    }
}
