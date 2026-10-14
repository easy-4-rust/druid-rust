use druid::core::xa;
use druid::core::xa_flags;
use druid_rdbc::data_source::XaFlags;

#[test]
fn legacy_xa_constants_reexport_the_standard_values() {
    assert_eq!(xa::flags::TMNOFLAGS, XaFlags::TMNOFLAGS);
    assert_eq!(xa::flags::TMJOIN, XaFlags::TMJOIN);
    assert_eq!(xa::flags::TMRESUME, XaFlags::TMRESUME);
    assert_eq!(xa::flags::TMSUCCESS, XaFlags::TMSUCCESS);
    assert_eq!(xa::flags::TMFAIL, XaFlags::TMFAIL);
    assert_eq!(xa::flags::TMSUSPEND, XaFlags::TMSUSPEND);
    assert_eq!(xa::flags::TMONEPHASE, XaFlags::TMONEPHASE);
    assert_eq!(xa::flags::XA_RDONLY, XaFlags::XA_RDONLY);
    assert_eq!(xa::flags::TMASYNC, XaFlags::TMSUSPEND);
    assert_eq!(xa_flags::TMASYNC, XaFlags::TMSUSPEND);
    assert_eq!(
        xa::XID_MAX_BQUAL_LENGTH,
        druid_rdbc::data_source::XID_MAX_BQUAL_LENGTH
    );
    assert_eq!(
        xa::XID_MAX_GTRID_LENGTH,
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
