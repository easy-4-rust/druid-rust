import { defineConfig } from 'vitepress'

// 本文件由 easy-4-rust.github.io 仓库 scripts/new-docs-site.mjs 生成，请勿手改；
// 修改导航/主题请编辑门户仓 scripts/manifests/druid-rust.json 后重新生成。

const sidebar = [
  {
    text: '开始',
    items: [
      {
        text: '简介',
        link: '/guide/intro'
      },
      {
        text: '快速开始',
        link: '/guide/quick-start'
      }
    ]
  },
  {
    text: '核心',
    items: [
      {
        text: '架构与 Crate 组成',
        link: '/guide/architecture'
      },
      {
        text: '功能详解',
        link: '/guide/features'
      }
    ]
  },
  {
    text: '迁移',
    items: [
      {
        text: '从 Java 迁移',
        link: '/guide/java-migration'
      }
    ]
  },
  {
    text: '更多',
    items: [
      {
        text: '更新日志与路线图',
        link: '/more/changelog'
      }
    ]
  }
]

export default defineConfig({
  title: 'druid-rust',
  description: '连接池、JDBC 语义层、SQL 解析与防火墙、监控统计、多数据源热切换——Druid 全家桶的 Rust 实现',
  lang: 'zh-CN',
  base: '/druid-rust/',
  lastUpdated: true,
  cleanUrls: true,
  head: [
    ['link', { rel: 'icon', href: '/druid-rust/favicon.svg', type: 'image/svg+xml' }],
  ],
  themeConfig: {
    nav: [
  {
    text: '首页',
    link: '/'
  },
  {
    text: '快速开始',
    link: '/guide/quick-start'
  },
  {
    text: '架构',
    link: '/guide/architecture'
  },
  {
    text: '更新日志',
    link: '/more/changelog'
  },
  {
    text: '生态门户',
    link: 'https://easy-4-rust.github.io/'
  },
  {
    text: 'GitHub',
    link: 'https://github.com/easy-4-rust/druid-rust'
  }
],
    sidebar: {
      '/guide/': sidebar,
      '/more/': sidebar,
      '/': sidebar,
    },
    socialLinks: [{ icon: 'github', link: 'https://github.com/easy-4-rust/druid-rust' }],
    search: { provider: 'local' },
    outlineTitle: '本页目录',
    lastUpdatedText: '最后更新',
    docFooter: { prev: '上一页', next: '下一页' },
    editLink: {
      pattern: 'https://github.com/easy-4-rust/druid-rust/edit/main/website/:path',
      text: '在 GitHub 上编辑此页',
    },
    footer: {
      message: 'Apache-2.0 · 迁移进行中 · 0.0.0-design · 未发布 crates.io，Git 依赖接入',
      copyright: 'Copyright © 2026 easy-4-rust · <a href="https://easy-4-rust.github.io/" target="_blank">Easy-4-Rust 生态门户</a>',
    },
  },
})
