use druid::core::xa;
use druid_rdbc::data_source::XaFlags;

#[test]
fn legacy_xa_constants_reexport_the_standard_values() {
    assert_eq!(xa::TMNOFLAGS, XaFlags::TMNOFLAGS);
    assert_eq!(xa::TMJOIN, XaFlags::TMJOIN);
    assert_eq!(xa::TMRESUME, XaFlags::TMRESUME);
    assert_eq!(xa::TMSUCCESS, XaFlags::TMSUCCESS);
    assert_eq!(xa::TMFAIL, XaFlags::TMFAIL);
    assert_eq!(xa::TMSUSPEND, XaFlags::TMSUSPEND);
    assert_eq!(xa::TMONEPHASE, XaFlags::TMONEPHASE);
    assert_eq!(xa::XA_RDONLY, XaFlags::XA_RDONLY);
    assert_eq!(
        xa::XID_MAX_BQUAL_LENGTH,
        druid_rdbc::data_source::XID_MAX_BQUAL_LENGTH
    );
    assert_eq!(
        xa::XID_MAX_GTRID_LENGTH,
        druid_rdbc::data_source::XID_MAX_GTRID_LENGTH
    );

    assert_eq!(druid::core::TMNOFLAGS, XaFlags::TMNOFLAGS);
    assert_eq!(
        druid::core::XID_MAX_BQUAL_LENGTH,
        druid_rdbc::data_source::XID_MAX_BQUAL_LENGTH
    );
    assert_eq!(
        druid::core::XID_MAX_GTRID_LENGTH,
        druid_rdbc::data_source::XID_MAX_GTRID_LENGTH
    );

    let from_module = xa::XaPrepareResult::ReadOnly;
    let from_core: druid::core::XaPrepareResult = from_module;
    let canonical: druid_rdbc::data_source::XaPrepareResult = from_core;
    assert_eq!(
        canonical,
        druid_rdbc::data_source::XaPrepareResult::ReadOnly
    );
}
