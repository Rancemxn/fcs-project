use fcs_core::v5::version::{
    EXECUTION_ABI_VERSION, FCBC_FORMAT_VERSION, FCS_SOURCE_VERSION, Version,
};

#[test]
fn exposes_independent_fcs_fcbc_and_abi_versions() {
    assert_eq!(FCS_SOURCE_VERSION, Version::new(5, 0, 0));
    assert_eq!(FCBC_FORMAT_VERSION, Version::new(2, 0, 0));
    assert_eq!(EXECUTION_ABI_VERSION, Version::new(1, 0, 0));
    assert_eq!(FCS_SOURCE_VERSION.to_string(), "5.0.0");
}
