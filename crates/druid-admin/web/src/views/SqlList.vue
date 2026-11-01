<script setup>
import { onMounted, ref } from 'vue'
import { api } from '../api/client.js'

const rows = ref([])
const total = ref(0)
const loading = ref(false)
const error = ref('')
const page = ref(1)
const perPageCount = ref(50)
const orderBy = ref('executeCount')
const orderType = ref('desc')

const sortable = [
  { prop: 'sql', label: 'SQL' },
  { prop: 'executeCount', label: '执行次数' },
  { prop: 'errorCount', label: '错误次数' },
  { prop: 'totalTime', label: '总耗时' },
  { prop: 'maxTimespan', label: '最大耗时' },
  { prop: 'fetchRowCount', label: '取回行数' },
  { prop: 'effectedRowCount', label: '影响行数' },
  { prop: 'concurrentMax', label: '并发峰值' },
]

async function load() {
  loading.value = true
  error.value = ''
  try {
    const body = await api.sql({
      page: page.value,
      perPageCount: perPageCount.value,
      orderBy: orderBy.value,
      orderType: orderType.value,
    })
    rows.value = body.rows || []
    total.value = body.total ?? rows.value.length
  } catch (cause) {
    error.value = cause.message
    rows.value = []
  } finally {
    loading.value = false
  }
}

function sortBy(prop) {
  if (orderBy.value === prop) {
    orderType.value = orderType.value === 'desc' ? 'asc' : 'desc'
  } else {
    orderBy.value = prop
    orderType.value = 'desc'
  }
  page.value = 1
  load()
}

onMounted(load)
</script>

<template>
  <el-card shadow="never">
    <template #header>
      <div class="card-header">
        <span>SQL 统计（共 {{ total }} 条）</span>
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
      empty-text="没有 SQL 统计"
    >
      <el-table-column
        v-for="column in sortable"
        :key="column.prop"
        :label="column.label"
        :min-width="column.prop === 'sql' ? 320 : 110"
        show-overflow-tooltip
      >
        <template #header>
          <button class="sort" type="button" @click="sortBy(column.prop)">
            {{ column.label }}
            <span v-if="orderBy === column.prop" class="marker">
              {{ orderType === 'desc' ? '▼' : '▲' }}
            </span>
          </button>
        </template>
        <template #default="scope">
          <code v-if="column.prop === 'sql'">{{ scope.row.sql || scope.row.formattedSql || '—' }}</code>
          <span v-else>{{ scope.row[column.prop] ?? '—' }}</span>
        </template>
      </el-table-column>
      <el-table-column prop="dbType" label="数据库类型" width="120" />
      <el-table-column prop="serviceName" label="服务" width="140" />
    </el-table>

    <el-pagination
      v-if="total > perPageCount"
      class="pager"
      layout="total, sizes, prev, pager, next"
      :total="total"
      :current-page="page"
      :page-size="perPageCount"
      :page-sizes="[20, 50, 100, 200]"
      @current-change="(value) => { page = value; load() }"
      @size-change="(value) => { perPageCount = value; page = 1; load() }"
    />
  </el-card>
</template>

<style scoped>
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.sort {
  background: none;
  border: none;
  padding: 0;
  cursor: pointer;
  font: inherit;
  color: inherit;
}

.marker {
  color: #409eff;
  margin-left: 4px;
}

.pager {
  margin-top: 14px;
  justify-content: flex-end;
}
</style>
