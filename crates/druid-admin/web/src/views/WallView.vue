<script setup>
import { onMounted, ref } from 'vue'
import { api } from '../api/client.js'
import { formatNumber } from '../utils/format.js'

const wall = ref(null)
const loading = ref(false)
const error = ref('')

const counters = [
  ['检查次数', 'checkCount'],
  ['硬检查', 'hardCheckCount'],
  ['违规次数', 'violationCount'],
  ['违规影响行数', 'violationEffectRowCount'],
  ['语法错误', 'syntaxErrorCount'],
  ['白名单命中', 'whiteListHitCount'],
  ['白名单大小', 'whiteListSize'],
  ['黑名单命中', 'blackListHitCount'],
  ['黑名单大小', 'blackListSize'],
]

async function load() {
  loading.value = true
  error.value = ''
  try {
    const body = await api.wall()
    wall.value = body.wall || {}
  } catch (cause) {
    error.value = cause.message
    wall.value = null
  } finally {
    loading.value = false
  }
}

function entryText(entry) {
  if (entry === null || entry === undefined) return ''
  if (typeof entry === 'string') return entry
  return entry.sql || entry.SQL || JSON.stringify(entry)
}

onMounted(load)
</script>

<template>
  <el-card shadow="never">
    <template #header>
      <div class="card-header">
        <span>SQL 防火墙</span>
        <el-button size="small" @click="load">刷新</el-button>
      </div>
    </template>

    <el-alert v-if="error" type="error" show-icon :closable="false" :title="error" />

    <template v-else-if="wall">
      <el-descriptions :column="5" border size="small" class="counters">
        <el-descriptions-item
          v-for="[label, key] in counters"
          :key="key"
          :label="label"
        >
          <el-tag v-if="key === 'violationCount' && Number(wall[key]) > 0" type="danger" size="small">
            {{ formatNumber(wall[key]) }}
          </el-tag>
          <span v-else>{{ formatNumber(wall[key]) }}</span>
        </el-descriptions-item>
      </el-descriptions>

      <h3 class="section">按表统计</h3>
      <el-table :data="wall.tables || []" border stripe size="small" empty-text="没有表统计">
        <el-table-column prop="name" label="表名" min-width="200" />
        <el-table-column label="查询" width="120" align="right">
          <template #default="{ row }">{{ formatNumber(row.selectCount) }}</template>
        </el-table-column>
        <el-table-column label="取回行数" width="140" align="right">
          <template #default="{ row }">{{ formatNumber(row.fetchRowCount) }}</template>
        </el-table-column>
      </el-table>

      <h3 class="section">按函数统计</h3>
      <el-table :data="wall.functions || []" border stripe size="small" empty-text="没有函数统计">
        <el-table-column prop="name" label="函数" min-width="200" />
        <el-table-column label="调用次数" width="140" align="right">
          <template #default="{ row }">{{ formatNumber(row.invokeCount) }}</template>
        </el-table-column>
      </el-table>

      <h3 class="section">白名单</h3>
      <el-table :data="wall.whiteList || []" border stripe size="small" empty-text="白名单为空">
        <el-table-column label="SQL" min-width="320" show-overflow-tooltip>
          <template #default="{ row }"><code>{{ row.sql }}</code></template>
        </el-table-column>
        <el-table-column label="执行次数" width="120" align="right">
          <template #default="{ row }">{{ formatNumber(row.executeCount) }}</template>
        </el-table-column>
        <el-table-column label="取回行数" width="120" align="right">
          <template #default="{ row }">{{ formatNumber(row.fetchRowCount) }}</template>
        </el-table-column>
      </el-table>

      <h3 class="section">黑名单</h3>
      <el-table :data="wall.blackList || []" border stripe size="small" empty-text="黑名单为空">
        <el-table-column label="SQL" min-width="320" show-overflow-tooltip>
          <template #default="{ row }"><code>{{ entryText(row) }}</code></template>
        </el-table-column>
      </el-table>
    </template>
  </el-card>
</template>

<style scoped>
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.counters {
  margin-bottom: 8px;
}

.section {
  font-size: 14px;
  margin: 20px 0 10px;
}
</style>
