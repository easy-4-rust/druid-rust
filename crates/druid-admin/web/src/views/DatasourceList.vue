<script setup>
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { api } from '../api/client.js'
import { formatNumber } from '../utils/format.js'

const router = useRouter()
const rows = ref([])
const loading = ref(false)
const error = ref('')

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

function openDetail(row) {
  router.push(`/datasources/${row.identity}`)
}

function errorType({ row }) {
  return Number(row.errorCount) > 0 ? 'danger' : 'primary'
}

onMounted(load)
</script>

<template>
  <el-card shadow="never" class="card">
    <template #header>
      <div class="card-header">
        <span>数据源（{{ rows.length }}）</span>
        <el-button size="small" @click="load">刷新</el-button>
      </div>
    </template>

    <el-alert v-if="error" type="error" show-icon :closable="false" :title="error" />

    <el-table
      v-else
      v-loading="loading"
      :data="rows"
      border
      stripe
      size="small"
      empty-text="没有数据源"
      @row-click="openDetail"
    >
      <el-table-column prop="identity" label="ID" width="70" align="right" />
      <el-table-column prop="name" label="名称" min-width="150">
        <template #default="{ row }">
          <el-link type="primary" :underline="false" @click.stop="openDetail(row)">{{ row.name }}</el-link>
        </template>
      </el-table-column>
      <el-table-column prop="dbType" label="数据库" width="100" />
      <el-table-column prop="driverClassName" label="驱动" min-width="140" show-overflow-tooltip />
      <el-table-column prop="url" label="连接地址" min-width="220" show-overflow-tooltip />
      <el-table-column label="活跃/空闲/上限" width="140" align="right">
        <template #default="{ row }">
          {{ formatNumber(row.activeCount) }} / {{ formatNumber(row.poolingCount) }} / {{ formatNumber(row.maxActive) }}
        </template>
      </el-table-column>
      <el-table-column label="等待" width="80" align="right">
        <template #default="{ row }">
          <el-tag v-if="Number(row.waitThreadCount) > 0" type="warning" size="small">{{ row.waitThreadCount }}</el-tag>
          <span v-else>0</span>
        </template>
      </el-table-column>
      <el-table-column prop="executeCount" label="执行" width="100" align="right">
        <template #default="{ row }">{{ formatNumber(row.executeCount) }}</template>
      </el-table-column>
      <el-table-column prop="errorCount" label="错误" width="80" align="right">
        <template #default="{ row }">
          <el-tag v-if="Number(row.errorCount) > 0" type="danger" size="small">{{ formatNumber(row.errorCount) }}</el-tag>
          <span v-else>0</span>
        </template>
      </el-table-column>
      <el-table-column label="提交/回滚" width="130" align="right">
        <template #default="{ row }">
          {{ formatNumber(row.commitCount) }} / {{ formatNumber(row.rollbackCount) }}
        </template>
      </el-table-column>
      <el-table-column prop="serviceName" label="服务" min-width="130" show-overflow-tooltip />
    </el-table>
  </el-card>
</template>

<style scoped>
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

:deep(.el-table__row) {
  cursor: pointer;
}
</style>
