<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { api } from './api/client.js'
import SourceStatus from './components/SourceStatus.vue'
import { useRequestGuard } from './composables/useRefresh.js'
import { refreshOptions, store } from './store.js'

const route = useRoute()
const basic = ref(null)
const error = ref('')
const sources = ref([])
const begin = useRequestGuard()
const sourceOptions = computed(() => [...new Map(sources.value.map(row => [row.serviceId, {value:row.serviceId, label:[row.serviceName,row.instanceId].filter(Boolean).join(' / ')}])).values()])
const datasourceOptions = computed(() => sources.value.filter(row => !store.serviceId || row.serviceId === store.serviceId))
const selectedDatasource = computed({
 get: () => store.datasourceId === '' ? '' : JSON.stringify([store.serviceId, store.datasourceId]),
 set: value => { if (!value) { store.datasourceId = ''; return }; const [service, id] = JSON.parse(value); store.serviceId = service; store.datasourceId = id },
})
function selectService(event) { store.serviceId = event.target.value; store.datasourceId = '' }

const tabs = [
  { path: '/overview', label: '主页' },
  { path: '/datasources', label: '数据源' },
  { path: '/sql', label: 'SQL 统计' },
  { path: '/slow-sql', label: '慢 SQL' },
  { path: '/wall', label: 'SQL 防火墙' },
  { path: '/health', label: '采集健康' },
  { path: '/capabilities', label: '驱动与能力' },
  { path: '/api', label: 'API 索引' },
]

const activePath = computed(() => {
  if (route.path.startsWith('/sql')) return '/sql'
  if (route.path.startsWith('/datasources')) return '/datasources'
  return route.path
})

const summary = computed(() => {
  if (!basic.value) return []
  return [
    { label: '版本', value: basic.value.version },
    { label: '数据源', value: basic.value.datasourceCount },
    { label: 'SQL 条目', value: basic.value.sqlCount },
    { label: '驱动', value: (basic.value.drivers || []).join('、') || '—' },
    { label: '后端', value: ({poll:'轮询',push:'推送',mixed:'混合'})[basic.value.backend] || '未上报' },
  ]
})

async function loadBasic() {
  const current = begin()
  try {
    const [result, data] = await Promise.all([api.basic(), api.datasources()])
    if (!current()) return
    basic.value = result
    sources.value = data.datasources || []
    store.sqlTextVisible = result.sqlTextVisible !== false
    error.value = ''
  } catch (cause) {
    if (current()) error.value = cause.message
  }
}

onMounted(loadBasic)
// 自动刷新节拍驱动顶栏摘要同步。
watch(() => store.tick, loadBasic)
</script>

<template>
  <el-container class="shell">
    <el-header class="header">
      <div class="brand">
        <span class="title">Druid Admin</span>
        <el-tag size="small" type="info" effect="plain">{{ basic?.apiVersion || 'v1' }}</el-tag>
      </div>
      <nav class="nav">
        <router-link v-for="tab in tabs" :key="tab.path" :to="tab.path" class="nav-item" :class="{ active: activePath === tab.path }">
          {{ tab.label }}
        </router-link>
      </nav>
      <div class="refresh">
        <el-select v-model="store.autoRefreshSeconds" size="small" style="width: 130px">
          <el-option v-for="opt in refreshOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
      </div>
    </el-header>

    <el-main class="main">
      <el-alert v-if="error" type="error" show-icon :closable="false" :title="error" class="alert" />

      <el-descriptions v-if="basic" :column="5" border size="small" class="summary">
        <el-descriptions-item v-for="item in summary" :key="item.label" :label="item.label">
          {{ item.value }}
        </el-descriptions-item>
      </el-descriptions>

      <SourceStatus :snapshot="basic" />
      <div v-if="!['/api','/health','/wall'].includes(route.path) && !route.params.identity && !route.params.id && !route.query.serviceId" class="scope-toolbar">
        <label>服务 / 实例 <select  :value="store.serviceId" @change="selectService"><option value="">全部来源</option><option v-for="item in sourceOptions" :key="item.value" :value="item.value">{{item.label}}</option></select></label>
        <label>数据源 <select v-model="selectedDatasource"><option value="">全部数据源</option><option v-for="row in datasourceOptions" :key="`${row.serviceId}/${row.identity}`" :value="JSON.stringify([row.serviceId, row.identity])">{{row.name || row.identity}} · {{row.serviceName}}</option></select></label>
        <el-button size="small" @click="store.tick++">刷新</el-button>
      </div>
      <router-view v-slot="{ Component }">
        <component :is="Component" :key="route.path" />
      </router-view>
    </el-main>
  </el-container>
</template>

<style scoped>
.shell {
  min-height: 100vh;
}

.header {
  display: flex;
  align-items: center;
  gap: 24px;
  background: #1f2d3d;
  color: #fff;
  padding: 0 24px;
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
}

.title {
  font-size: 18px;
  font-weight: 600;
  letter-spacing: 0.5px;
}

.nav {
  display: flex;
  gap: 4px;
  flex: 1;
}

.nav-item {
  color: #c0c4cc;
  padding: 8px 16px;
  font-size: 14px;
  text-decoration: none;
  border-radius: 4px;
}

.nav-item:hover {
  color: #fff;
  background: rgba(255, 255, 255, 0.08);
}

.nav-item.active {
  color: #fff;
  background: #409eff;
}

.main {
  padding: 20px 24px 40px;
  background: #f5f7fa;
}

.alert {
  margin-bottom: 16px;
}

.summary {
  margin-bottom: 18px;
}
</style>

<style scoped>
.header{gap:16px}.brand{flex-shrink:0}.nav{min-width:0;overflow-x:auto;white-space:nowrap}.nav-item{font-size:13px;padding:9px 14px;flex-shrink:0}.main{min-width:0}.refresh{flex-shrink:0}
@media(max-width:1100px){.header{height:auto;flex-wrap:wrap;padding:12px 16px;gap:10px}.nav{order:3;width:100%;flex-basis:100%}.refresh{margin-left:auto}.main{padding:16px}}
@media(max-width:600px){.main{padding:12px}.summary{overflow-x:auto}.summary :deep(.el-descriptions__body){min-width:820px}}
</style>
