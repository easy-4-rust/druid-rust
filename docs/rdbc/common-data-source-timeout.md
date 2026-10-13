# CommonDataSource 登录超时迁移

`druid-rdbc::data_source::CommonDataSource` 的旧 `login_timeout()` 返回 `Option<Duration>`，不能表达底层读取失败。新代码应使用可失败入口：

```rust
use druid_rdbc::data_source::CommonDataSource;
use std::time::Duration;

fn login_timeout(source: &dyn CommonDataSource) -> Result<Option<Duration>, druid_rdbc::error::RdbcError> {
    source.try_login_timeout()
}
```

`None` 表示没有显式配置，沿用系统或驱动默认值；`Some(Duration::ZERO)` 表示明确不限制登录等待；正时长保留原精度。不要把 `Err` 转为 `None` 或零值。

既有实现只覆盖 `login_timeout()` 时仍可编译，新 `try_login_timeout()` 的默认实现会以 `Ok(old_value)` 适配它。实现可以观察底层读取错误时，应覆盖新方法并返回原始 `RdbcError`。旧方法已弃用，只作为迁移桥梁；`set_login_timeout()` 本身已有可失败结果，拒绝或转换失败必须保留错误。

适配器若与 Java `CommonDataSource` 的有符号秒数交互，须检查负值与目标范围，不能截断、饱和或悄悄回退。Rust `Duration` 的秒和纳秒精度由合同值原样保留。
