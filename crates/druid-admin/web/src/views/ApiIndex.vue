<script setup>
import { onMounted, ref } from 'vue'
import { api } from '../api/client.js'

const document = ref(null)
const error = ref('')

onMounted(async () => {
  try {
    document.value = await api.openapi()
  } catch (cause) {
    error.value = cause.message
  }
})
</script>

<template>
  <el-card shadow="never">
    <template #header>
      <div class="card-header">
        <span>API 索引</span>
        <el-button size="small" @click="() => api.openapi().then((body) => (document = body))">刷新</el-button>
      </div>
    </template>

    <el-alert v-if="error" type="error" show-icon :closable="false" :title="error" />

    <template v-else-if="document">
      <p class="note">
        前端只依赖这一套版本化契约。Java 兼容的
        <code>/druid/*.json</code> 路由保持原有形状，供既有嵌入方使用。
      </p>
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
