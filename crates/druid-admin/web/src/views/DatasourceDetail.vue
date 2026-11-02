<script setup>
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api } from '../api/client.js'
import { useRefresh, useRequestGuard } from '../composables/useRefresh.js'
import { scopeQuery, detailLocation } from '../store.js'
import SourceStatus from '../components/SourceStatus.vue'
import HistogramBar from '../components/HistogramBar.vue'
import { TIME_BUCKETS_7, TIME_BUCKETS_8, formatDuration, formatNumber, zipBuckets } from '../utils/format.js'

const route = useRoute()
const router = useRouter()
const detail = ref(null)
const loading = ref(false)
const error = ref('')
const snapshot = ref(null)
const beginRequest = useRequestGuard()

const connectionColumns = [['连接 ID','connectionId'],['使用次数','useCount'],['创建时间','connectTime'],['最后活跃','lastActiveTime'],['隔离级别','transactionIsolation'],['自动提交','autoCommit'],['只读','readoOnly'],['保活检查','keepAliveCheckCount'],['Holdability','holdability']]
const connectionTab = ref('connections')
const connectionData = ref(null), connectionError = ref(''), connectionLoading = ref(false)
const connectionBegin = useRequestGuard()
async function loadConnections() {
 const current = connectionBegin(); connectionLoading.value = true; connectionError.value = ''; connectionData.value = null
 try { const body = await api[connectionTab.value](identity.value, route.query); if(current()) connectionData.value = body }
 catch(cause) { if(current()) connectionError.value = cause.message }
 finally { if(current()) connectionLoading.value = false }
}
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
        ['QueryTimeout', d.queryTimeout != null ? `${d.queryTimeout} s` : null],
        ['TransactionQueryTimeout', d.transactionQueryTimeout != null ? `${d.transactionQueryTimeout} s` : null],
        ['LoginTimeout', d.loginTimeout != null ? `${d.loginTimeout} s` : null],
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
  if (value === null || value === undefined || value === '') return '未上报'
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') return formatNumber(value)
  return value
}

async function load() {
  const isCurrent = beginRequest()
  loading.value = true
  error.value = ''
  try {
    const body = await api.datasource(identity.value, route.query)
    if (!isCurrent()) return
    snapshot.value = body
    detail.value = body.datasource
    if (connectionData.value) loadConnections()
  } catch (cause) {
    if (!isCurrent()) return
    error.value = cause.message
  } finally {
    if (isCurrent()) loading.value = false
  }
}

useRefresh(load)
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

    <SourceStatus :snapshot="snapshot" />
    <el-alert v-if="error" type="error" show-icon :closable="false" :title="error" class="alert" />

    <template v-if="detail">
      <section class="monitor-panel"><header><h2>连接诊断</h2><el-button size="small" @click="loadConnections">读取连接信息</el-button></header>
        <el-tabs v-model="connectionTab" @tab-change="loadConnections"><el-tab-pane label="连接列表" name="connections"/><el-tab-pane label="活跃连接借出堆栈" name="stack"/></el-tabs>
        <div v-loading="connectionLoading"><el-alert v-if="connectionError" :title="connectionError" type="error" :closable="false"/>
        <p v-else-if="connectionData?.supported === false" class="muted">不支持：{{ connectionData.reason }}</p>
        <el-table v-else-if="connectionData?.supported && connectionTab === 'connections'" :data="connectionData.rows || []" border size="small" empty-text="当前没有池化连接">
          <el-table-column type="expand"><template #default="{row}"><h4>预编译语句缓存</h4><pre class="connection-output">{{row.pscache == null ? '未上报' : JSON.stringify(row.pscache, null, 2)}}</pre></template></el-table-column>
          <el-table-column v-for="[label,key] in connectionColumns" :key="key" :label="label" min-width="135"><template #default="{row}">{{display(row[key])}}</template></el-table-column>
        </el-table>
        <template v-else-if="connectionData?.supported"><p v-if="!connectionData.rows?.length" class="muted">当前没有活跃连接堆栈</p><pre v-for="(trace,index) in connectionData.rows || []" :key="index" class="connection-output">{{typeof trace === 'string' ? trace : JSON.stringify(trace,null,2)}}</pre></template>
        <p v-else class="muted">点击读取连接信息获取当前节点状态。</p></div>
      </section>
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
