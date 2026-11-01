<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { api } from './api/client.js'
import { refreshOptions, store } from './store.js'

const route = useRoute()
const basic = ref(null)
const error = ref('')

const tabs = [
  { path: '/datasources', label: '数据源' },
  { path: '/sql', label: 'SQL 统计' },
  { path: '/slow-sql', label: '慢 SQL' },
  { path: '/wall', label: 'SQL 防火墙' },
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
    { label: '后端', value: basic.value.backend === 'poll' ? '轮询' : '推送' },
  ]
})

async function loadBasic() {
  try {
    basic.value = await api.basic()
    error.value = ''
  } catch (cause) {
    error.value = cause.message
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

      <router-view v-slot="{ Component }">
        <component :is="Component" :key="route.fullPath + ':' + store.tick" />
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
