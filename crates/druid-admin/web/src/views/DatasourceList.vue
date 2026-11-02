<script setup>
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { api } from '../api/client.js'
import { useRefresh, useRequestGuard } from '../composables/useRefresh.js'
import { scopeQuery, detailLocation } from '../store.js'
import SourceStatus from '../components/SourceStatus.vue'
import { formatNumber } from '../utils/format.js'

const router = useRouter()
const rows = ref([])
const activeTab = ref('runtime')
const extraColumns = {
 lifecycle: [['逻辑借出','logicConnectCount'],['逻辑归还','logicCloseCount'],['借出错误','logicConnectErrorCount'],['物理创建','driverConnectCount'],['物理关闭','driverCloseCount'],['物理连接错误','driverConnectErrorCount'],['保活检查','keepAliveCheckCount'],['回收错误','recycleErrorCount']],
 config: [['初始连接','initialSize'],['最小空闲','minIdle'],['最大连接','maxActive'],['最大等待（ms）','maxWait'],['借出校验','testOnBorrow'],['归还校验','testOnReturn'],['空闲校验','testWhileIdle'],['泄漏回收','removeAbandoned'],['PS 缓存','poolPreparedStatements']],
}
function display(value) { return typeof value === 'boolean' ? String(value) : formatNumber(value) }
const loading = ref(false)
const error = ref('')
const snapshot = ref(null)
const beginRequest = useRequestGuard()

async function load() {
  const isCurrent = beginRequest()
  loading.value = true
  error.value = ''
  try {
    const body = await api.datasources(scopeQuery())
    if (!isCurrent()) return
    snapshot.value = body
    rows.value = body.datasources || []
  } catch (cause) {
    if (!isCurrent()) return
    error.value = cause.message
  } finally {
    if (isCurrent()) loading.value = false
  }
}

function openDetail(row) {
  router.push(detailLocation('datasource', row))
}

function errorType({ row }) {
  return Number(row.errorCount) > 0 ? 'danger' : 'primary'
}

useRefresh(load)
</script>

<template>
  <el-card shadow="never" class="card">
    <template #header>
      <div class="card-header">
        <span>数据源（{{ rows.length }}）</span>
        <el-button size="small" @click="load">刷新</el-button>
      </div>
    </template>

    <el-tabs v-model="activeTab"><el-tab-pane label="运行状态" name="runtime"/><el-tab-pane label="连接生命周期" name="lifecycle"/><el-tab-pane label="配置与校验" name="config"/></el-tabs>
    <SourceStatus :snapshot="snapshot" />
    <el-alert v-if="error" type="error" show-icon :closable="false" :title="error" />

    <el-table
      v-if="!error && activeTab === 'runtime'"
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
          <span v-else>{{ formatNumber(row.waitThreadCount) }}</span>
        </template>
      </el-table-column>
      <el-table-column prop="executeCount" label="执行" width="100" align="right">
        <template #default="{ row }">{{ formatNumber(row.executeCount) }}</template>
      </el-table-column>
      <el-table-column prop="errorCount" label="错误" width="80" align="right">
        <template #default="{ row }">
          <el-tag v-if="Number(row.errorCount) > 0" type="danger" size="small">{{ formatNumber(row.errorCount) }}</el-tag>
          <span v-else>{{ formatNumber(row.errorCount) }}</span>
        </template>
      </el-table-column>
      <el-table-column label="提交/回滚" width="130" align="right">
        <template #default="{ row }">
          {{ formatNumber(row.commitCount) }} / {{ formatNumber(row.rollbackCount) }}
        </template>
      </el-table-column>
      <el-table-column prop="source" label="来源" width="70" />
      <el-table-column prop="instanceId" label="实例" min-width="120" />
      <el-table-column prop="serviceName" label="服务" min-width="130" show-overflow-tooltip />
    </el-table>
    <el-table v-if="!error && activeTab !== 'runtime'" v-loading="loading" :data="rows" border stripe size="small" empty-text="尚未收到数据源统计" @row-click="openDetail">
      <el-table-column label="数据源 / 来源" min-width="220"><template #default="{row}"><el-link type="primary" @click.stop="openDetail(row)">{{row.name || row.identity}}</el-link><div class="source-text">{{row.serviceName}} / {{row.instanceId || row.serviceId}}</div></template></el-table-column>
      <el-table-column v-for="[label,key] in extraColumns[activeTab]" :key="key" :label="label" min-width="135"><template #default="{row}">{{display(row[key])}}</template></el-table-column>
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
