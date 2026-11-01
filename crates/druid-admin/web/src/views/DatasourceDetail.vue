<script setup>
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api } from '../api/client.js'
import HistogramBar from '../components/HistogramBar.vue'
import { TIME_BUCKETS_7, TIME_BUCKETS_8, formatDuration, formatNumber, zipBuckets } from '../utils/format.js'

const route = useRoute()
const router = useRouter()
const detail = ref(null)
const loading = ref(false)
const error = ref('')

const identity = computed(() => Number(route.params.identity))

// 分组渲染完整配置，对齐 Java datasource.html 的全部字段。
const groups = computed(() => {
  const d = detail.value
  if (!d) return []
  return [
    {
      title: '基本信息',
      items: [
        ['Identity', d.identity],
        ['Name', d.name],
        ['DbType', d.dbType],
        ['DriverClassName', d.driverClassName],
        ['URL', d.url],
        ['UserName', d.userName],
        ['ServiceName', d.serviceName],
        ['InstanceId', d.instanceId],
        ['BootId', d.bootId],
        ['FilterClassNames', (d.filterClassNames || []).join(', ') || null],
      ],
    },
    {
      title: '池配置',
      items: [
        ['InitialSize', d.initialSize],
        ['MinIdle', d.minIdle],
        ['MaxActive', d.maxActive],
        ['MaxWait', formatDuration(d.maxWait)],
        ['MaxWaitThreadCount', d.maxWaitThreadCount],
        ['QueryTimeout', d.queryTimeout ? `${d.queryTimeout} s` : null],
        ['TransactionQueryTimeout', d.transactionQueryTimeout ? `${d.transactionQueryTimeout} s` : null],
        ['LoginTimeout', d.loginTimeout ? `${d.loginTimeout} s` : null],
        ['TestOnBorrow', d.testOnBorrow],
        ['TestOnReturn', d.testOnReturn],
        ['TestWhileIdle', d.testWhileIdle],
        ['DefaultAutoCommit', d.defaultAutoCommit],
        ['RemoveAbandoned', d.removeAbandoned],
        ['KeepAlive', d.keepAlive],
        ['FailFast', d.failFast],
        ['PoolPreparedStatements', d.poolPreparedStatements],
        ['MaxPoolPreparedStatementPerConnectionSize', d.maxPoolPreparedStatementPerConnectionSize],
        ['MinEvictableIdleTimeMillis', formatDuration(d.minEvictableIdleTimeMillis)],
        ['MaxEvictableIdleTimeMillis', formatDuration(d.maxEvictableIdleTimeMillis)],
        ['UseUnfairLock', d.useUnfairLock],
        ['LogDifferentThread', d.logDifferentThread],
        ['ValidConnectionCheckerClassName', d.validConnectionCheckerClassName],
        ['ExceptionSorterClassName', d.exceptionSorterClassName],
      ],
    },
    {
      title: '活跃与等待',
      items: [
        ['ActiveCount', d.activeCount],
        ['ActivePeak', d.activePeak],
        ['ActivePeakTime', d.activePeakTime],
        ['PoolingCount', d.poolingCount],
        ['PoolingPeak', d.poolingPeak],
        ['PoolingPeakTime', d.poolingPeakTime],
        ['WaitThreadCount', d.waitThreadCount],
        ['NotEmptyWaitCount', d.notEmptyWaitCount],
        ['NotEmptyWaitMillis', formatDuration(d.notEmptyWaitMillis)],
      ],
    },
    {
      title: '执行统计',
      items: [
        ['ExecuteCount', d.executeCount],
        ['ExecuteQueryCount', d.executeQueryCount],
        ['ExecuteUpdateCount', d.executeUpdateCount],
        ['ExecuteBatchCount', d.executeBatchCount],
        ['ErrorCount', d.errorCount],
        ['CommitCount', d.commitCount],
        ['RollbackCount', d.rollbackCount],
        ['StartTransactionCount', d.startTransactionCount],
      ],
    },
    {
      title: '连接统计',
      items: [
        ['LogicConnectCount', d.logicConnectCount],
        ['LogicCloseCount', d.logicCloseCount],
        ['LogicConnectErrorCount', d.logicConnectErrorCount],
        ['DriverConnectCount', d.driverConnectCount],
        ['DriverCloseCount', d.driverCloseCount],
        ['DriverConnectErrorCount', d.driverConnectErrorCount],
        ['KeepAliveCheckCount', d.keepAliveCheckCount],
        ['RecycleErrorCount', d.recycleErrorCount],
      ],
    },
    {
      title: '缓存与大对象',
      items: [
        ['PSCacheAccessCount', d.psCacheAccessCount],
        ['PSCacheHitCount', d.psCacheHitCount],
        ['PSCacheMissCount', d.psCacheMissCount],
        ['PreparedStatementOpenCount', d.preparedStatementOpenCount],
        ['PreparedStatementClosedCount', d.preparedStatementClosedCount],
        ['ClobOpenCount', d.clobOpenCount],
        ['BlobOpenCount', d.blobOpenCount],
      ],
    },
    {
      title: '其他',
      items: [
        ['DefaultReadOnly', d.defaultReadOnly],
        ['DefaultTransactionIsolation', d.defaultTransactionIsolation],
        ['InitGlobalVariants', d.initGlobalVariants],
        ['InitVariants', d.initVariants],
      ],
    },
  ]
})

const transactionHistogram = computed(() => zipBuckets(detail.value?.transactionHistogram, TIME_BUCKETS_7))
const holdTimeHistogram = computed(() => zipBuckets(detail.value?.connectionHoldTimeHistogram, TIME_BUCKETS_8))

function display(value) {
  if (value === null || value === undefined || value === '') return '—'
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') return formatNumber(value)
  return value
}

async function load() {
  loading.value = true
  error.value = ''
  try {
    const body = await api.datasource(identity.value)
    detail.value = body.datasource
  } catch (cause) {
    error.value = cause.message
    detail.value = null
  } finally {
    loading.value = false
  }
}

onMounted(load)
</script>

<template>
  <div v-loading="loading">
    <el-page-header class="page-header" @back="router.push('/datasources')">
      <template #content>
        <span class="title">{{ detail?.name || `数据源 ${identity}` }}</span>
        <el-tag size="small" class="db-tag">{{ detail?.dbType || '—' }}</el-tag>
      </template>
      <template #extra>
        <el-button size="small" @click="load">刷新</el-button>
      </template>
    </el-page-header>

    <el-alert v-if="error" type="error" show-icon :closable="false" :title="error" class="alert" />

    <template v-if="detail">
      <el-row :gutter="16" class="histogram-row">
        <el-col :xs="24" :md="12">
          <el-card shadow="never" class="histogram-card">
            <template #header>事务耗时分布 TransactionHistogram</template>
            <HistogramBar :buckets="transactionHistogram" color="#409eff" />
          </el-card>
        </el-col>
        <el-col :xs="24" :md="12">
          <el-card shadow="never" class="histogram-card">
            <template #header>连接持有时间分布 ConnectionHoldTimeHistogram</template>
            <HistogramBar :buckets="holdTimeHistogram" color="#67c23a" />
          </el-card>
        </el-col>
      </el-row>

      <el-card v-for="group in groups" :key="group.title" shadow="never" class="group-card">
        <template #header>{{ group.title }}</template>
        <el-descriptions :column="3" border size="small">
          <el-descriptions-item v-for="[label, value] in group.items" :key="label" :label="label">
            {{ display(value) }}
          </el-descriptions-item>
        </el-descriptions>
      </el-card>
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

.histogram-row {
  margin-bottom: 16px;
}

.histogram-card {
  margin-bottom: 16px;
}

.group-card {
  margin-bottom: 16px;
}
</style>
