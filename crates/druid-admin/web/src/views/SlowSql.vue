<script setup>
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { api } from '../api/client.js'
import { useRefresh, useRequestGuard } from '../composables/useRefresh.js'
import { scopeQuery, detailLocation } from '../store.js'
import SourceStatus from '../components/SourceStatus.vue'
import MiniHistogram from '../components/MiniHistogram.vue'
import { TIME_BUCKETS_8, formatDuration, formatNumber, zipBuckets } from '../utils/format.js'

const router = useRouter()
const rows = ref([])
const total = ref(0)
const loading = ref(false)
const error = ref('')
const snapshot = ref(null)
const beginRequest = useRequestGuard()

// 慢 SQL 阈值（毫秒）：maxTimespan 达到该值即视为慢。
// 持久化在 localStorage，对齐 Java 控制台手动调阈值的用法。
const THRESHOLD_KEY = 'druid-admin-slow-threshold-ms'
const threshold = ref(Number(localStorage.getItem(THRESHOLD_KEY)) || 1000)

watch(threshold, (value) => {
  localStorage.setItem(THRESHOLD_KEY, String(value))
})

const presetThresholds = [100, 500, 1000, 5000, 10000]

async function load() {
  const isCurrent = beginRequest()
  loading.value = true
  error.value = ''
  try {
    const body = await api.sql({ ...scopeQuery(), page: page.value, perPageCount: perPageCount.value, minMaxTimespan: threshold.value, orderBy: 'maxTimespan', orderType: 'desc' })
    if (!isCurrent()) return
    snapshot.value = body
    rows.value = body.rows || []
    total.value = body.total ?? 0
  } catch (cause) {
    if (!isCurrent()) return
    error.value = cause.message
  } finally {
    if (isCurrent()) loading.value = false
  }
}

const page = ref(1)
const perPageCount = ref(20)
watch(threshold, () => { page.value = 1; load() })
const stats = computed(() => [
 { label: '达到阈值的 SQL 条目', value: formatNumber(total.value) },
 { label: '最大执行耗时阈值', value: formatDuration(threshold.value) },
 { label: '慢执行次数', value: '未上报' },
])

function histogram(row) {
  return zipBuckets(row.histogram, TIME_BUCKETS_8)
}

function openDetail(row) {
  router.push(detailLocation('sql', row))
}

useRefresh(load)
</script>

<template>
  <el-card shadow="never">
    <template #header>
      <div class="card-header">
        <span>慢 SQL（{{ total }} 条 ≥ 阈值）</span>
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

    <SourceStatus :snapshot="snapshot" />
    <el-alert v-if="error" type="error" show-icon :closable="false" :title="error" />

    <template v-else>
      <el-descriptions :column="3" border size="small" class="stats">
        <el-descriptions-item v-for="item in stats" :key="item.label" :label="item.label">
          {{ item.value }}
        </el-descriptions-item>
      </el-descriptions>

      <el-table
        v-loading="loading"
        :data="rows"
        border
        stripe
        size="small"
        empty-text="没有已上报最大耗时且达到阈值的 SQL"
        @row-click="openDetail"
      >
        <el-table-column label="SQL" min-width="320">
          <template #default="{ row }">
            <el-link type="primary" :underline="false" @click.stop="openDetail(row)">
              <code class="sql-text">{{ row.sql || '—' }}</code>
            </el-link>
          </template>
        </el-table-column>
        <el-table-column label="耗时分布" width="130" align="center">
          <template #default="{ row }"><MiniHistogram :buckets="histogram(row)" /></template>
        </el-table-column>
        <el-table-column label="最大耗时" width="110" align="right" sortable prop="maxTimespan">
          <template #default="{ row }">
            <el-tag type="danger" size="small" effect="plain">{{ formatDuration(row.maxTimespan) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="平均耗时" width="110" align="right">
          <template #default="{ row }">
            {{ formatDuration(row.totalTime != null && Number(row.executeCount) > 0 ? Number(row.totalTime) / Number(row.executeCount) : null) }}
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
        :total="total"
        :current-page="page"
        :page-size="perPageCount"
        :page-sizes="[10, 20, 50, 100]"
        :hide-on-single-page="false"
        background
        @current-change="(value) => { page = value; load() }"
        @size-change="(value) => { perPageCount = value; page = 1; load() }"
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
