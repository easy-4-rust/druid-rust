# Design

Extend `DruidRdbcDataSource::canonical_unwrap` with exact `TypeId` branches for `dyn DataSource` and `dyn CommonDataSource`, returning the corresponding `StandardUnwrapped` view of `self`. Retain the existing `Wrapper` branch. The trait default keeps unknown targets unsupported. An external test compares data pointers to prove every view refers to the same object rather than a constructed adapter or pool.
