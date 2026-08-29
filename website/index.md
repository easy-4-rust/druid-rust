---
layout: home

hero:
  name: "druid-rust"
  text: "阿里 Druid 1.2.28 的 Rust 完整语义迁移"
  tagline: "连接池、JDBC 语义层、SQL 解析与防火墙、监控统计、多数据源热切换——Druid 全家桶的 Rust 实现<br/>迁移进行中 · 0.0.0-design · 未发布 crates.io，Git 依赖接入"
  actions:
    - theme: brand
      text: 快速开始
      link: /guide/quick-start
    - theme: alt
      text: GitHub
      link: https://github.com/easy-4-rust/druid-rust

features:
  - icon: 🔌
    title: "DruidPool 连接池"
    details: "完整池语义迁移：初始化、收缩、保活、连接泄漏检测"
  - icon: 🗄️
    title: "druid-rdbc 标准 trait 树"
    details: "JDBC 语义层：Connection / Statement / ResultSet / Driver / 元数据"
  - icon: 🛡️
    title: "SQL 解析与 Wall 防火墙"
    details: "SQL AST 解析 + WallFilter 防注入"
  - icon: 📊
    title: "监控与统计"
    details: "Stat 指标 + 非阻塞 metrics 运行时（Prometheus / gRPC）"
  - icon: 🔄
    title: "多数据源热切换"
    details: "HA / ArcSwap 动态数据源"
  - icon: 🧩
    title: "Rust 生态适配"
    details: "SQLx / RBDC / bb8 / deadpool / Toasty / DuckDB / libSQL / JDBC Agent"
  - icon: 🖥️
    title: "druid-admin 管理面"
    details: "Axum 实现 /druid/* 监控端点，认证与 TLS/mTLS"
  - icon: 🤝
    title: "XA / 2PC"
    details: "XaResource 两阶段提交协议"
  - icon: ✅
    title: "契约式迁移"
    details: "对象总账 + 语义契约表 + 真实数据库证据三重验证"
---
