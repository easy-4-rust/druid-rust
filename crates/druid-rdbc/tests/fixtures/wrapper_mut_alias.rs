use druid_rdbc::wrapper::{StandardUnwrappedMut, Wrapper};
use std::any::TypeId;

struct WrapperObject;
druid_rdbc::impl_standard_wrapper!(WrapperObject => Wrapper);

fn main() {
    let mut object = WrapperObject;
    let first: StandardUnwrappedMut<'_> = object
        .unwrap_mut(Some(TypeId::of::<dyn Wrapper>()))
        .unwrap();
    let second = object
        .unwrap_mut(Some(TypeId::of::<dyn Wrapper>()))
        .unwrap();
    println!("{first:?} {second:?}");
}
