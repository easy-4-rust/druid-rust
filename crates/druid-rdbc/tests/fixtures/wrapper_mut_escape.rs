use druid_rdbc::wrapper::{StandardUnwrappedMut, Wrapper};
use std::any::TypeId;

struct WrapperObject;
druid_rdbc::impl_standard_wrapper!(WrapperObject => Wrapper);

fn escape() -> StandardUnwrappedMut<'static> {
    let mut object = WrapperObject;
    object
        .unwrap_mut(Some(TypeId::of::<dyn Wrapper>()))
        .unwrap()
}

fn main() {
    println!("{:?}", escape());
}
