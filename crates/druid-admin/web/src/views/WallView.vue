<script setup>
import { onMounted, ref } from 'vue'
import { api } from '../api/client.js'

const wall = ref(null)
const loading = ref(false)
const error = ref('')

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
      <el-descriptions :column="4" border size="small" class="counters">
        <el-descriptions-item label="检查次数">{{ wall.checkCount ?? 0 }}</el-descriptions-item>
        <el-descriptions-item label="硬检查">{{ wall.hardCheckCount ?? 0 }}</el-descriptions-item>
        <el-descriptions-item label="违规次数">{{ wall.violationCount ?? 0 }}</el-descriptions-item>
        <el-descriptions-item label="语法错误">{{ wall.syntaxErrorCount ?? 0 }}</el-descriptions-item>
        <el-descriptions-item label="白名单命中">{{ wall.whiteListHitCount ?? 0 }}</el-descriptions-item>
        <el-descriptions-item label="白名单大小">{{ wall.whiteListSize ?? 0 }}</el-descriptions-item>
        <el-descriptions-item label="黑名单命中">{{ wall.blackListHitCount ?? 0 }}</el-descriptions-item>
        <el-descriptions-item label="黑名单大小">{{ wall.blackListSize ?? 0 }}</el-descriptions-item>
      </el-descriptions>

      <h3 class="section">按表统计</h3>
      <el-table :data="wall.tables || []" border stripe size="small" empty-text="没有表统计">
        <el-table-column prop="name" label="表名" min-width="200" />
        <el-table-column prop="selectCount" label="查询" width="100" align="right" />
        <el-table-column prop="fetchRowCount" label="取回行数" width="120" align="right" />
      </el-table>

      <h3 class="section">按函数统计</h3>
      <el-table :data="wall.functions || []" border stripe size="small" empty-text="没有函数统计">
        <el-table-column prop="name" label="函数" min-width="200" />
        <el-table-column prop="invokeCount" label="调用次数" width="120" align="right" />
      </el-table>

      <h3 class="section">白名单</h3>
      <el-table :data="wall.whiteList || []" border stripe size="small" empty-text="白名单为空">
        <el-table-column prop="sql" label="SQL" min-width="320" show-overflow-tooltip />
        <el-table-column prop="executeCount" label="执行次数" width="110" align="right" />
        <el-table-column prop="fetchRowCount" label="取回行数" width="110" align="right" />
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
