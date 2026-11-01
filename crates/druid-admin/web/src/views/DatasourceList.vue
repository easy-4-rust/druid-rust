<script setup>
import { onMounted, ref } from 'vue'
import { api } from '../api/client.js'

const rows = ref([])
const loading = ref(false)
const error = ref('')

const columns = [
  { prop: 'identity', label: 'ID', width: 90 },
  { prop: 'name', label: '名称', minWidth: 160 },
  { prop: 'dbType', label: '数据库类型', width: 120 },
  { prop: 'driverClassName', label: '驱动', minWidth: 160 },
  { prop: 'url', label: '连接地址', minWidth: 220, showOverflowTooltip: true },
  { prop: 'activeCount', label: '活跃', width: 80, align: 'right' },
  { prop: 'poolingCount', label: '空闲', width: 80, align: 'right' },
  { prop: 'maxActive', label: '上限', width: 80, align: 'right' },
  { prop: 'waitThreadCount', label: '等待', width: 80, align: 'right' },
  { prop: 'executeCount', label: '执行', width: 100, align: 'right' },
  { prop: 'errorCount', label: '错误', width: 90, align: 'right' },
  { prop: 'commitCount', label: '提交', width: 90, align: 'right' },
  { prop: 'rollbackCount', label: '回滚', width: 90, align: 'right' },
  { prop: 'sqlCount', label: 'SQL', width: 80, align: 'right' },
  { prop: 'wallCheckCount', label: 'Wall 检查', width: 110, align: 'right' },
  { prop: 'wallViolationCount', label: 'Wall 拒绝', width: 110, align: 'right' },
  { prop: 'serviceName', label: '服务', minWidth: 140 },
]

async function load() {
  loading.value = true
  error.value = ''
  try {
    const body = await api.datasources()
    rows.value = body.datasources || []
  } catch (cause) {
    error.value = cause.message
    rows.value = []
  } finally {
    loading.value = false
  }
}

onMounted(load)
</script>

<template>
  <el-card shadow="never" class="card">
    <template #header>
      <div class="card-header">
        <span>数据源</span>
        <el-button size="small" @click="load">刷新</el-button>
      </div>
    </template>

    <el-alert v-if="error" type="error" show-icon :closable="false" :title="error" />

    <el-table
      v-else
      v-loading="loading"
      :data="rows"
      :columns="columns"
      border
      stripe
      size="small"
      empty-text="没有数据源"
    >
      <el-table-column
        v-for="column in columns"
        :key="column.prop"
        :prop="column.prop"
        :label="column.label"
        :min-width="column.minWidth"
        :width="column.width"
        :align="column.align || 'left'"
        show-overflow-tooltip
      />
    </el-table>
  </el-card>
</template>

<style scoped>
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
</style>
