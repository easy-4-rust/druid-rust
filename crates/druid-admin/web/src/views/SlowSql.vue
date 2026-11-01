<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { api } from '../api/client.js'
import MiniHistogram from '../components/MiniHistogram.vue'
import { TIME_BUCKETS_8, formatDuration, formatNumber, zipBuckets } from '../utils/format.js'

const router = useRouter()
const rows = ref([])
const loading = ref(false)
const error = ref('')

// 慢 SQL 阈值（毫秒）：maxTimespan 达到该值即视为慢。
// 持久化在 localStorage，对齐 Java 控制台手动调阈值的用法。
const THRESHOLD_KEY = 'druid-admin-slow-threshold-ms'
const threshold = ref(Number(localStorage.getItem(THRESHOLD_KEY)) || 1000)

watch(threshold, (value) => {
  localStorage.setItem(THRESHOLD_KEY, String(value))
})

const presetThresholds = [100, 500, 1000, 5000, 10000]

async function load() {
  loading.value = true
  error.value = ''
  try {
    const body = await api.sql({ page: 1, perPageCount: 100000, orderBy: 'maxTimespan', orderType: 'desc' })
    rows.value = body.rows || []
  } catch (cause) {
    error.value = cause.message
    rows.value = []
  } finally {
    loading.value = false
  }
}

// 慢 SQL 行：maxTimespan 超过阈值。
const slowRows = computed(() =>
  rows.value.filter((row) => Number(row.maxTimespan) >= threshold.value),
)

// 阈值变化后回到第一页。
watch(threshold, () => { page.value = 1 })

// 前端分页：阈值过滤后的结果再分页，数据多时不撑爆表格。
const page = ref(1)
const perPageCount = ref(20)
const pagedRows = computed(() => {
  const from = (page.value - 1) * perPageCount.value
  return slowRows.value.slice(from, from + perPageCount.value)
})

const stats = computed(() => {
  const totalExec = rows.value.reduce((sum, row) => sum + (Number(row.executeCount) || 0), 0)
  const slowExec = slowRows.value.reduce((sum, row) => sum + (Number(row.executeCount) || 0), 0)
  const slowTotalTime = slowRows.value.reduce((sum, row) => sum + (Number(row.totalTime) || 0), 0)
  return [
    { label: '慢 SQL 条数', value: formatNumber(slowRows.value.length) },
    { label: '慢 SQL 累计执行', value: formatNumber(slowExec) },
    { label: '慢 SQL 执行占比', value: totalExec > 0 ? `${((slowExec / totalExec) * 100).toFixed(2)}%` : '—' },
    { label: '慢 SQL 累计耗时', value: formatDuration(slowTotalTime) },
    { label: '阈值', value: formatDuration(threshold.value) },
  ]
})

function histogram(row) {
  return zipBuckets(row.histogram, TIME_BUCKETS_8)
}

function openDetail(row) {
  router.push(`/sql/${row.id}`)
}

onMounted(load)
</script>

<template>
  <el-card shadow="never">
    <template #header>
      <div class="card-header">
        <span>慢 SQL（{{ slowRows.length }} 条 ≥ 阈值）</span>
        <div class="header-actions">
          <span class="threshold-label">阈值</span>
          <el-radio-group v-model="threshold" size="small">
            <el-radio-button v-for="preset in presetThresholds" :key="preset" :value="preset">
              {{ formatDuration(preset) }}
            </el-radio-button>
          </el-radio-group>
          <el-input-number
            v-model="threshold"
            size="small"
            :min="1"
            :step="100"
            :controls="false"
            style="width: 110px"
            placeholder="自定义 ms"
          />
          <el-button size="small" @click="load">刷新</el-button>
        </div>
      </div>
    </template>

    <el-alert v-if="error" type="error" show-icon :closable="false" :title="error" />

    <template v-else>
      <el-descriptions :column="5" border size="small" class="stats">
        <el-descriptions-item v-for="item in stats" :key="item.label" :label="item.label">
          {{ item.value }}
        </el-descriptions-item>
      </el-descriptions>

      <el-table
        v-loading="loading"
        :data="pagedRows"
        border
        stripe
        size="small"
        empty-text="没有超过阈值的慢 SQL"
        @row-click="openDetail"
      >
        <el-table-column label="SQL" min-width="320">
          <template #default="{ row }">
            <el-link type="primary" :underline="false" @click.stop="openDetail(row)">
              <code class="sql-text">{{ row.sql || '—' }}</code>
            </el-link>
          </template>
        </el-table-column>
        <el-table-column label="耗时分布" width="122" align="center">
          <template #default="{ row }"><MiniHistogram :buckets="histogram(row)" /></template>
        </el-table-column>
        <el-table-column label="最大耗时" width="110" align="right" sortable prop="maxTimespan">
          <template #default="{ row }">
            <el-tag type="danger" size="small" effect="plain">{{ formatDuration(row.maxTimespan) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="平均耗时" width="110" align="right">
          <template #default="{ row }">
            {{ formatDuration(Number(row.executeCount) > 0 ? Number(row.totalTime) / Number(row.executeCount) : 0) }}
          </template>
        </el-table-column>
        <el-table-column label="执行次数" width="100" align="right" prop="executeCount" sortable>
          <template #default="{ row }">{{ formatNumber(row.executeCount) }}</template>
        </el-table-column>
        <el-table-column label="总耗时" width="110" align="right" prop="totalTime" sortable>
          <template #default="{ row }">{{ formatDuration(row.totalTime) }}</template>
        </el-table-column>
        <el-table-column label="慢参数" min-width="160" show-overflow-tooltip>
          <template #default="{ row }">
            <code v-if="row.lastSlowParameters && row.lastSlowParameters !== 'null'">{{ row.lastSlowParameters }}</code>
            <span v-else>—</span>
          </template>
        </el-table-column>
        <el-table-column label="数据源/服务" min-width="160">
          <template #default="{ row }">
            {{ [row.name, row.serviceName].filter(Boolean).join(' / ') || '—' }}
          </template>
        </el-table-column>
      </el-table>

      <!-- 分页常显：阈值过滤后的条数驱动。 -->
      <el-pagination
        class="pager"
        layout="total, sizes, prev, pager, next, jumper"
        :total="slowRows.length"
        :current-page="page"
        :page-size="perPageCount"
        :page-sizes="[10, 20, 50, 100]"
        :hide-on-single-page="false"
        background
        @current-change="(value) => { page = value }"
        @size-change="(value) => { perPageCount = value; page = 1 }"
      />
    </template>
  </el-card>
</template>

<style scoped>
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.threshold-label {
  font-size: 13px;
  color: #909399;
}

.stats {
  margin-bottom: 14px;
}

.sql-text {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  display: block;
  max-width: 460px;
}

.pager {
  margin-top: 14px;
  justify-content: flex-end;
}

:deep(.el-table__row) {
  cursor: pointer;
}
</style>
