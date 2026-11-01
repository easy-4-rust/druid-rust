import { createRouter, createWebHashHistory } from 'vue-router'

// 控制台路由。使用 hash 路由：Admin 的内嵌分发把未命中路径
// 回落到 index.html，hash 路由不依赖服务端重写规则。
const routes = [
  { path: '/', redirect: '/datasources' },
  { path: '/datasources', name: 'datasources', component: () => import('./views/DatasourceList.vue'), meta: { title: '数据源' } },
  { path: '/datasources/:identity', name: 'datasource-detail', component: () => import('./views/DatasourceDetail.vue'), meta: { title: '数据源详情' } },
  { path: '/sql', name: 'sql', component: () => import('./views/SqlList.vue'), meta: { title: 'SQL 统计' } },
  { path: '/sql/:id', name: 'sql-detail', component: () => import('./views/SqlDetail.vue'), meta: { title: 'SQL 详情' } },
  { path: '/slow-sql', name: 'slow-sql', component: () => import('./views/SlowSql.vue'), meta: { title: '慢 SQL' } },
  { path: '/wall', name: 'wall', component: () => import('./views/WallView.vue'), meta: { title: 'SQL 防火墙' } },
  { path: '/api', name: 'api', component: () => import('./views/ApiIndex.vue'), meta: { title: 'API 索引' } },
]

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
})

router.afterEach((to) => {
  document.title = to.meta.title ? `${to.meta.title} · Druid Admin` : 'Druid Admin'
})
