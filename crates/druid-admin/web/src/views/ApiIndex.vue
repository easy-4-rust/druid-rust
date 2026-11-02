<script setup>
import { ref } from 'vue'
import { api } from '../api/client.js'

import { useRefresh, useRequestGuard } from '../composables/useRefresh.js'
const begin = useRequestGuard()
const document = ref(null)
const error = ref('')

async function load() {
  const current = begin()
  try {
    const body = await api.openapi()
    if (!current()) return
    document.value = body
    error.value = ''
  } catch (cause) {
    if (current()) error.value = cause.message
  }
}
useRefresh(load)
</script>

<template>
  <el-card shadow="never">
    <template #header>
      <div class="card-header">
        <span>API 索引</span>
        <el-button size="small" @click="load">刷新</el-button>
      </div>
    </template>

    <el-alert v-if="error" type="error" show-icon :closable="false" :title="error" />

    <template v-else-if="document">
      <el-table :data="Object.entries(document.paths || {})" border size="small">
        <el-table-column label="路径" min-width="320">
          <template #default="scope">
            <code>{{ scope.row[0] }}</code>
          </template>
        </el-table-column>
        <el-table-column label="说明" min-width="260">
          <template #default="scope">
            {{ scope.row[1]?.get?.summary || '—' }}
          </template>
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

.note {
  color: #606266;
  font-size: 13px;
  margin: 0 0 12px;
}
</style>
