# Task6 最终验收工具补齐方案（待用户确认）

本机只读核对未发现 cargo-public-api.exe 或 cargo-semver-checks.exe；已安装工具链列表不含1.95，也不含2026-08-07的 Windows nightly。现有同日期 Apple ARM nightly不能在Windows执行。

API门禁已通过RED/GREEN改为保留nightly-2026-08-07日期、由rustup选择当前主机；没有改变项目默认1.97.1、声明MSRV1.95或验收规则。真实API/SemVer/MSRV验收尚未运行通过。

## 已核对的官方依据

- [cargo-public-api官方安装文档及兼容矩阵](https://github.com/cargo-public-api/cargo-public-api)：需要安装CLI与兼容nightly；0.52.x兼容nightly-2025-11-22及之后的文档格式。
- [cargo-public-api v0.52.0官方发布](https://github.com/cargo-public-api/cargo-public-api/releases/tag/v0.52.0)。
- [cargo-semver-checks官方安装文档](https://github.com/obi1kenobi/cargo-semver-checks)：使用cargo install并保留locked；实际兼容性须以安装后运行结果确认。
- [cargo-semver-checks v0.50.0官方发布](https://github.com/obi1kenobi/cargo-semver-checks/releases/tag/v0.50.0)。
- [rustup工具链命名规则](https://rust-lang.github.io/rustup/concepts/toolchains.html)：日期和版本可固定，省略host由当前主机解析。
- 官方分发清单只读HEAD核对：`https://static.rust-lang.org/dist/channel-rust-1.95.0.toml` 与 `https://static.rust-lang.org/dist/2026-08-07/channel-rust-nightly.toml` 均返回HTTP200。

## 安装范围与影响

仅当前用户profile：Rust工具链位于 `C:/Users/hnxyh/.rustup/toolchains`；CLI位于 `C:/Users/hnxyh/.cargo/bin`，并更新用户级Cargo安装记录和下载缓存。会联网下载并占用磁盘。不会修改仓库Cargo.toml、Cargo.lock、rust-toolchain.toml、规格、Git分支或系统安全规则；安装过程在仓库外独立目录执行，不与当前C2构建共享target。

不升级或覆盖已有CLI：当前目标可执行文件确实缺失；执行前再次核对。如发现已有不同版本，停止并说明，不使用--force。

## 待授权执行的固定命令

```powershell
rustup toolchain install 1.95.0 --profile minimal
rustup toolchain install nightly-2026-08-07 --profile minimal
cargo +1.97.1 install cargo-public-api --version 0.52.0 --locked
cargo +1.97.1 install cargo-semver-checks --version 0.50.0 --locked
```

安装后重新核对每个实际版本及Windows host；使用现有门禁取得真实API、SemVer和1.95 MSRV证据。安装成功不等于门禁通过，也不允许自动更新快照掩盖差异。

状态：PROPOSED_NOT_INSTALLED。用户授权未到达前不执行安装。
