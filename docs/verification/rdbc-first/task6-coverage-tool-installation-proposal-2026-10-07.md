# Task6 编译覆盖工具补齐方案（待确认）

实际执行当前1.97.1 cargo llvm-cov --version返回no such command；rustup component list --toolchain 1.97.1 --installed不含llvm-tools-preview。当前不具备七family/105精确函数身份再生成所需工具。既有API/SemVer/MSRV安装申请未包含此工具，本方案单独列明新增范围；授权未到达前不执行。

## 官方核对

- [官方当前稳定发布0.9.1](https://github.com/taiki-e/cargo-llvm-cov/releases/tag/v0.9.1)。
- [固定0.9.1安装与使用说明](https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.1/README.md)：支持Cargo test及LLVM source-based coverage、JSON导出，安装使用cargo install。
- [固定0.9.1 Cargo manifest](https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.1/Cargo.toml)：工具本身MSRV1.87；使用现有1.97.1满足该下限，实际运行兼容性仍需安装后检验。

## 待授权命令和影响

```powershell
rustup component add llvm-tools-preview --toolchain 1.97.1
cargo +1.97.1 install cargo-llvm-cov --version 0.9.1 --locked
```

新增CLI写入当前用户C:/Users/hnxyh/.cargo/bin并更新用户级安装记录/下载缓存；LLVM组件写入现有1.97.1用户工具链。会下载并占用磁盘。执行前再次确认工具缺失，发现已有不同版本时不覆盖、不使用--force。安装构建在仓库外使用独立target，不与当前C2的target并发构建；不改变项目依赖、Cargo.lock、rust-toolchain.toml、默认工具链、分支或系统安全配置。

安装后核对cargo llvm-cov --version和组件清单；等待当前完整C2终态后，执行既有regenerate_task6_coverage_manifest.py，保留七个独立run-owned编译报告、profraw/profdata和105精确身份验证，全部通过才允许更新清单。不得仅更换hash、使用旧报告、共享profile或将源码预检写成compiler PASS。

## 授权依据

父级[AGENTS.md](E:/workspaces/workspace-github-easy-4-rust/AGENTS.md:278)要求安装CLI先说明影响并获得确认；工具缺失章节要求核对官方文档、说明范围、确认后执行。状态PROPOSED_NOT_INSTALLED。本方案不替代之前API/SemVer/MSRV申请，也不授权防火墙操作。
