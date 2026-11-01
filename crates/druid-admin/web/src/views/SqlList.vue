<script setup>
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { api } from '../api/client.js'
import MiniHistogram from '../components/MiniHistogram.vue'
import { ROW_BUCKETS_6, TIME_BUCKETS_8, formatDuration, formatNumber, zipBuckets } from '../utils/format.js'

const router = useRouter()
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

// 数值列定宽：数字右对齐不需要 min-width 撑出的留白。
function columnWidth(prop) {
  switch (prop) {
    case 'errorCount':
      return 76
    case 'totalTime':
    case 'maxTimespan':
      return 92
    case 'fetchRowCount':
    case 'effectedRowCount':
      return 86
    default:
      return 90
  }
}

function histogram(row) {
  return zipBuckets(row.histogram, TIME_BUCKETS_8)
}

function fetchHistogram(row) {
  return zipBuckets(row.fetchRowCountHistogram, ROW_BUCKETS_6)
}

function updateHistogram(row) {
  return zipBuckets(row.effectedRowCountHistogram, ROW_BUCKETS_6)
}

function holdHistogram(row) {
  return zipBuckets(row.executeAndResultHoldTimeHistogram, TIME_BUCKETS_8)
}

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

function openDetail(row) {
  router.push(`/sql/${row.id}`)
}

// 点击 SQL 文本即复制（不进详情），toast 反馈。
async function copySql(row) {
  const text = row.sql || ''
  if (!text) return
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
    } else {
      const textarea = document.createElement('textarea')
      textarea.value = text
      textarea.style.position = 'fixed'
      textarea.style.opacity = '0'
      document.body.appendChild(textarea)
      textarea.select()
      document.execCommand('copy')
      document.body.removeChild(textarea)
    }
    ElMessage.success('已复制 SQL')
  } catch {
    ElMessage.error('复制失败，请手动选择复制')
  }
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
      @row-click="openDetail"
    >
      <el-table-column label="SQL" min-width="300">
        <template #default="{ row }">
          <!-- 点击 SQL 文本复制；点行内其他区域进详情。 -->
          <el-link
            type="primary"
            :underline="false"
            class="sql-link"
            :title="'点击复制：' + (row.sql || '')"
            @click.stop="copySql(row)"
          >
            <code>{{ row.sql || '—' }}</code>
          </el-link>
        </template>
      </el-table-column>

      <el-table-column label="执行耗时" width="130" align="center">
        <template #default="{ row }"><MiniHistogram :buckets="histogram(row)" /></template>
      </el-table-column>
      <el-table-column label="取回行" width="130" align="center">
        <template #default="{ row }"><MiniHistogram :buckets="fetchHistogram(row)" color="#67c23a" /></template>
      </el-table-column>
      <el-table-column label="影响行" width="130" align="center">
        <template #default="{ row }"><MiniHistogram :buckets="updateHistogram(row)" color="#e6a23c" /></template>
      </el-table-column>
      <el-table-column label="执行+持有" width="130" align="center">
        <template #default="{ row }"><MiniHistogram :buckets="holdHistogram(row)" color="#909399" /></template>
      </el-table-column>

      <el-table-column
        v-for="column in sortable.slice(1)"
        :key="column.prop"
        :width="columnWidth(column.prop)"
        align="right"
      >
        <template #header>
          <button class="sort" type="button" @click="sortBy(column.prop)">
            {{ column.label }}
            <span v-if="orderBy === column.prop" class="marker">
              {{ orderType === 'desc' ? '▼' : '▲' }}
            </span>
          </button>
        </template>
        <template #default="{ row }">
          <template v-if="column.prop === 'totalTime' || column.prop === 'maxTimespan'">
            {{ formatDuration(row[column.prop]) }}
          </template>
          <el-tag v-else-if="column.prop === 'errorCount' && Number(row.errorCount) > 0" type="danger" size="small">
            {{ formatNumber(row.errorCount) }}
          </el-tag>
          <span v-else>{{ formatNumber(row[column.prop]) }}</span>
        </template>
      </el-table-column>

      <el-table-column prop="dbType" label="数据库" width="90" />
      <el-table-column prop="serviceName" label="服务" width="110" show-overflow-tooltip />
    </el-table>

    <!-- 分页常显：数据少时是状态提示，数据多时是导航。 -->
    <el-pagination
      class="pager"
      layout="total, sizes, prev, pager, next, jumper"
      :total="total"
      :current-page="page"
      :page-size="perPageCount"
      :page-sizes="[10, 20, 50, 100, 200]"
      :hide-on-single-page="false"
      background
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

.sql-link {
  display: inline-block;
  max-width: 100%;
}

.sql-link code {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  display: block;
}

:deep(.el-table__row) {
  cursor: pointer;
}

/* 压缩行高：small 尺寸默认单元格上下 padding 偏大，数据密表再收紧。 */
:deep(.el-table .el-table__cell) {
  padding: 2px 0;
}

:deep(.el-table .cell) {
  line-height: 1.5;
}
</style>
