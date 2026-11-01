<script setup>
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api } from '../api/client.js'
import HistogramBar from '../components/HistogramBar.vue'
import SqlCode from '../components/SqlCode.vue'
import { ROW_BUCKETS_6, TIME_BUCKETS_8, formatDuration, formatNumber, parseSqlMeta, zipBuckets } from '../utils/format.js'

const route = useRoute()
const router = useRouter()
const row = ref(null)
const loading = ref(false)
const error = ref('')

const sqlId = computed(() => Number(route.params.id))

const meta = computed(() => parseSqlMeta(row.value?.sql || ''))

const histograms = computed(() => {
  const r = row.value
  if (!r) return []
  return [
    { title: '执行耗时分布 Histogram', buckets: zipBuckets(r.histogram, TIME_BUCKETS_8), color: '#409eff' },
    { title: '取回行数分布 FetchRowCountHistogram', buckets: zipBuckets(r.fetchRowCountHistogram, ROW_BUCKETS_6), color: '#67c23a' },
    { title: '影响行数分布 EffectedRowCountHistogram', buckets: zipBuckets(r.effectedRowCountHistogram, ROW_BUCKETS_6), color: '#e6a23c' },
    { title: '执行+结果持有分布 ExecuteAndResultHoldTimeHistogram', buckets: zipBuckets(r.executeAndResultHoldTimeHistogram, TIME_BUCKETS_8), color: '#909399' },
  ]
})

const counters = computed(() => {
  const r = row.value
  if (!r) return []
  return [
    ['执行次数 ExecuteCount', formatNumber(r.executeCount)],
    ['错误次数 ErrorCount', formatNumber(r.errorCount)],
    ['总耗时 TotalTime', formatDuration(r.totalTime)],
    ['最大耗时 MaxTimespan', formatDuration(r.maxTimespan)],
    ['最大耗时发生时间', r.maxTimespanOccurTime || '—'],
    ['最后执行时间', r.lastTime || '—'],
    ['取回行数 FetchRowCount', formatNumber(r.fetchRowCount)],
    ['取回行数峰值 FetchRowCountMax', formatNumber(r.fetchRowCountMax)],
    ['影响行数 EffectedRowCount', formatNumber(r.effectedRowCount)],
    ['影响行数峰值 EffectedRowCountMax', formatNumber(r.effectedRowCountMax)],
    ['批大小峰值 BatchSizeMax', formatNumber(r.batchSizeMax)],
    ['批大小总量 BatchSizeTotal', formatNumber(r.batchSizeTotal)],
    ['并发峰值 ConcurrentMax', formatNumber(r.concurrentMax)],
    ['正在执行 RunningCount', formatNumber(r.runningCount)],
    ['事务中执行 InTransactionCount', formatNumber(r.inTransactionCount)],
    ['结果集持有 ResultSetHoldTime', formatDuration(r.resultSetHoldTime)],
    ['执行+结果集持有 ExecuteAndResultSetHoldTime', formatDuration(r.executeAndResultSetHoldTime)],
    ['读取字符数 ReadStringLength', formatNumber(r.readStringLength)],
    ['读取字节数 ReadBytesLength', formatNumber(r.readBytesLength)],
    ['Clob 打开 ClobOpenCount', formatNumber(r.clobOpenCount)],
    ['Blob 打开 BlobOpenCount', formatNumber(r.blobOpenCount)],
    ['输入流打开 InputStreamOpenCount', formatNumber(r.inputStreamOpenCount)],
    ['Reader 打开 ReaderOpenCount', formatNumber(r.readerOpenCount)],
  ]
})

const hasError = computed(() => {
  const r = row.value
  return r && (Number(r.errorCount) > 0 || (r.lastErrorMessage && r.lastErrorMessage !== 'null' && r.lastErrorMessage !== null))
})

async function load() {
  loading.value = true
  error.value = ''
  try {
    // 版本化 API 暂无单条 SQL 路由；列表按 id 过滤（数据量即 SQL 条目数）。
    const body = await api.sql({ page: 1, perPageCount: 100000, orderBy: 'id', orderType: 'asc' })
    const found = (body.rows || []).find((item) => Number(item.id) === sqlId.value)
    if (found) {
      row.value = found
    } else {
      error.value = `未找到 ID 为 ${sqlId.value} 的 SQL 统计`
    }
  } catch (cause) {
    error.value = cause.message
  } finally {
    loading.value = false
  }
}

onMounted(load)
</script>

<template>
  <div v-loading="loading">
    <el-page-header class="page-header" @back="router.push('/sql')">
      <template #content>
        <span class="title">SQL #{{ sqlId }}</span>
        <el-tag v-if="row?.dbType" size="small" class="db-tag">{{ row.dbType }}</el-tag>
      </template>
      <template #extra>
        <el-button size="small" @click="load">刷新</el-button>
      </template>
    </el-page-header>

    <el-alert v-if="error" type="error" show-icon :closable="false" :title="error" class="alert" />

    <template v-if="row">
      <el-card shadow="never" class="section">
        <template #header>SQL</template>
        <SqlCode :sql="row.sql || ''" />
      </el-card>

      <el-alert v-if="hasError" type="error" show-icon :closable="false" class="alert">
        <template #title>
          执行错误{{ row.lastErrorTime ? `（${row.lastErrorTime}）` : '' }}
        </template>
        <div class="error-body">
          <div v-if="row.lastErrorClass && row.lastErrorClass !== 'null'" class="error-line">
            <b>异常类：</b>{{ row.lastErrorClass }}
          </div>
          <div v-if="row.lastErrorMessage && row.lastErrorMessage !== 'null'" class="error-line">
            <b>消息：</b>{{ row.lastErrorMessage }}
          </div>
          <div v-if="row.lastSlowParameters && row.lastSlowParameters !== 'null'" class="error-line">
            <b>慢参数：</b>{{ row.lastSlowParameters }}
          </div>
        </div>
      </el-alert>

      <el-row :gutter="16">
        <el-col v-for="chart in histograms" :key="chart.title" :xs="24" :md="12">
          <el-card shadow="never" class="section">
            <template #header>{{ chart.title }}</template>
            <HistogramBar :buckets="chart.buckets" :color="chart.color" />
          </el-card>
        </el-col>
      </el-row>

      <el-row :gutter="16">
        <el-col :xs="24" :lg="14">
          <el-card shadow="never" class="section">
            <template #header>执行计数器</template>
            <el-descriptions :column="2" border size="small">
              <el-descriptions-item v-for="[label, value] in counters" :key="label" :label="label">
                {{ value }}
              </el-descriptions-item>
            </el-descriptions>
          </el-card>
        </el-col>
        <el-col :xs="24" :lg="10">
          <el-card shadow="never" class="section">
            <template #header>解析元数据（客户端解析）</template>
            <el-descriptions :column="1" border size="small">
              <el-descriptions-item label="涉及表 ParsedTable">
                {{ meta.tables.length ? meta.tables.join(', ') : '—' }}
              </el-descriptions-item>
              <el-descriptions-item label="字段 ParsedFields">
                {{ meta.fields.length ? meta.fields.join(', ') : '—' }}
              </el-descriptions-item>
              <el-descriptions-item label="条件 ParsedConditions">
                <div v-if="meta.conditions.length">
                  <div v-for="cond in meta.conditions" :key="cond">{{ cond }}</div>
                </div>
                <span v-else>—</span>
              </el-descriptions-item>
            </el-descriptions>
          </el-card>

          <el-card shadow="never" class="section">
            <template #header>来源</template>
            <el-descriptions :column="1" border size="small">
              <el-descriptions-item label="服务">{{ row.serviceName || '—' }}</el-descriptions-item>
              <el-descriptions-item label="实例">{{ row.instanceId || '—' }}</el-descriptions-item>
              <el-descriptions-item label="地址">{{ row.address ? `${row.address}:${row.port ?? ''}` : '—' }}</el-descriptions-item>
              <el-descriptions-item label="HASH">{{ row.hash }}</el-descriptions-item>
            </el-descriptions>
          </el-card>
        </el-col>
      </el-row>
    </template>
  </div>
</template>

<style scoped>
.page-header {
  margin-bottom: 16px;
}

.title {
  font-size: 16px;
  font-weight: 600;
}

.db-tag {
  margin-left: 10px;
  text-transform: uppercase;
}

.alert {
  margin-bottom: 16px;
}

.section {
  margin-bottom: 16px;
}

.error-body {
  margin-top: 6px;
  font-size: 13px;
}

.error-line {
  margin: 2px 0;
}
</style>
