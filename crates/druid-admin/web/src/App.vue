<script setup>
import { computed, onMounted, ref } from 'vue'
import { api } from './api/client.js'
import DatasourceList from './views/DatasourceList.vue'
import SqlList from './views/SqlList.vue'
import WallView from './views/WallView.vue'
import ApiIndex from './views/ApiIndex.vue'

const basic = ref(null)
const error = ref('')
const view = ref('datasources')

const tabs = [
  { key: 'datasources', label: '数据源' },
  { key: 'sql', label: 'SQL 统计' },
  { key: 'wall', label: 'SQL 防火墙' },
  { key: 'api', label: 'API 索引' },
]

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

onMounted(async () => {
  try {
    basic.value = await api.basic()
  } catch (cause) {
    error.value = cause.message
  }
})
</script>

<template>
  <el-container class="shell">
    <el-header class="header">
      <div class="brand">
        <span class="title">Druid Admin</span>
        <el-tag size="small" type="info" effect="plain">{{ basic?.apiVersion || 'v1' }}</el-tag>
      </div>
      <nav class="nav">
        <button
          v-for="tab in tabs"
          :key="tab.key"
          class="nav-item"
          :class="{ active: view === tab.key }"
          type="button"
          @click="view = tab.key"
        >
          {{ tab.label }}
        </button>
      </nav>
    </el-header>

    <el-main class="main">
      <el-alert
        v-if="error"
        type="error"
        show-icon
        :closable="false"
        :title="error"
        class="alert"
      />

      <el-descriptions v-if="basic" :column="5" border size="small" class="summary">
        <el-descriptions-item
          v-for="item in summary"
          :key="item.label"
          :label="item.label"
        >
          {{ item.value }}
        </el-descriptions-item>
      </el-descriptions>

      <DatasourceList v-if="view === 'datasources'" />
      <SqlList v-else-if="view === 'sql'" />
      <WallView v-else-if="view === 'wall'" />
      <ApiIndex v-else-if="view === 'api'" />
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
  justify-content: space-between;
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
}

.nav-item {
  background: transparent;
  border: none;
  color: #c0c4cc;
  padding: 8px 16px;
  font-size: 14px;
  cursor: pointer;
  border-radius: 4px 4px 0 0;
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
