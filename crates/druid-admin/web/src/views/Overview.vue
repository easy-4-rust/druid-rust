<script setup>
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { api } from '../api/client.js'
import { scopeQuery, detailLocation } from '../store.js'
import { useRefresh, useRequestGuard } from '../composables/useRefresh.js'
import { formatNumber } from '../utils/format.js'
import SourceStatus from '../components/SourceStatus.vue'
const router = useRouter()
const data = ref(null), sql = ref(null), error = ref(''), loading = ref(false)
const begin = useRequestGuard()
const rows = computed(() => data.value?.datasources || [])
const sum = key => { const values = rows.value.map(r => r[key]).filter(v => typeof v === 'number'); return values.length ? formatNumber(values.reduce((a,b) => a+b,0)) : '未上报' }
const errors = computed(() => (sql.value?.rows || []).filter(r => Number(r.errorCount) > 0))
const facts = computed(() => [
  ['服务 / 实例', `${new Set(rows.value.map(r=>r.serviceName)).size} / ${new Set(rows.value.map(r=>r.serviceId)).size}`],
  ['数据源', formatNumber(rows.value.length)], ['活跃 / 空闲', `${sum('activeCount')} / ${sum('poolingCount')}`], ['容量上限', sum('maxActive')],
  ['等待连接', sum('waitThreadCount')], ['累计执行', sum('executeCount')], ['已知累计错误', sum('errorCount')],
  ['错误统计覆盖', `${rows.value.filter(r=>r.errorCount !== null && r.errorCount !== undefined).length} / ${rows.value.length} 来源`],
])
const issues = computed(() => rows.value.flatMap(row => {
  const result = []
  if (Number(row.waitThreadCount) > 0) result.push({ row, kind: '等待连接', value: `${row.waitThreadCount} 个请求等待连接`, direction: '检查连接持有和 SQL 耗时', target: 'pool' })
  if (Number(row.errorCount) > 0) result.push({ row, kind: '累计错误', value: `${formatNumber(row.errorCount)} 次`, direction: '查看错误 SQL 与驱动错误', target: 'sql' })
  if (row.errorCount == null) result.push({ row, kind: '指标缺失', value: '错误统计未上报', direction: '检查来源能力', target: 'capabilities' })
  return result
}))
function open(row) { router.push(detailLocation('datasource', row)) }
function locate(issue) {
  if (issue.target === 'pool') return open(issue.row)
  if (issue.target === 'capabilities') return router.push('/capabilities')
  router.push({path:'/sql', query:{serviceId:issue.row.serviceId, datasourceId:issue.row.identity, orderBy:'errorCount'}})
}
async function load() {
 const current=begin();loading.value=true
 try { const [sources, stats] = await Promise.all([api.datasources(scopeQuery()), api.sql({...scopeQuery(),orderBy:'errorCount',orderType:'desc',perPageCount:5})]);if(!current())return;data.value=sources;sql.value=stats;error.value='' }
 catch(cause){if(current())error.value=cause.message}
 finally{if(current())loading.value=false}
}
useRefresh(load)
</script>
<template>
 <div v-loading="loading">
  <div class="page-heading"><h1>主页 · 运行概况</h1><span class="muted">{{ data?.observedAt ? new Date(data.observedAt).toLocaleString() : '正在读取' }}</span></div>
  <el-alert v-if="error" :title="error + (data ? '；下方保留上次成功结果' : '')" type="error" :closable="false" />
  <SourceStatus :snapshot="data" /><SourceStatus :snapshot="sql" />
  <template v-if="data">
   <dl class="monitor-facts"><div v-for="[label,value] in facts" :key="label"><dt>{{label}}</dt><dd>{{value}}</dd></div></dl>
   <section class="monitor-panel"><header><h2>问题定位</h2><span class="muted">最新连接状态 / 累计错误</span></header>
    <el-table :data="issues" border stripe size="small" :empty-text="rows.length ? '当前已上报指标未发现等待或累计错误' : '尚未收到数据源统计'">
     <el-table-column prop="kind" label="观察项" width="115"/><el-table-column label="数据源" min-width="180"><template #default="{row}">{{row.row.name || row.row.identity}}<div class="source-text">{{row.row.serviceName}} / {{row.row.instanceId || row.row.serviceId}}</div></template></el-table-column>
     <el-table-column prop="value" label="观察结果" min-width="170"/><el-table-column prop="direction" label="诊断方向" min-width="220"/>
     <el-table-column label="定位" width="110"><template #default="{row}"><el-button link type="primary" @click="locate(row)">查看详情</el-button></template></el-table-column>
    </el-table>
   </section>
   <section class="monitor-panel"><header><h2>服务实例与连接池</h2><router-link to="/datasources">全部指标</router-link></header>
    <el-table :data="rows" border stripe size="small" empty-text="尚未收到数据源统计" @row-click="open">
     <el-table-column label="数据源 / 实例" min-width="235"><template #default="{row}"><el-link type="primary" @click.stop="open(row)">{{row.name || row.identity}}</el-link><div class="source-text">{{row.serviceName}} / {{row.instanceId || row.serviceId}}</div></template></el-table-column>
     <el-table-column prop="dbType" label="数据库" width="110"/><el-table-column label="活跃 / 上限" width="155"><template #default="{row}"><span class="tiny-meter"><i :style="{width:row.maxActive > 0 ? Math.min(100,row.activeCount/row.maxActive*100)+'%' : '0%'}"/></span>{{formatNumber(row.activeCount)}} / {{formatNumber(row.maxActive)}}</template></el-table-column>
     <el-table-column label="空闲" width="85"><template #default="{row}">{{formatNumber(row.poolingCount)}}</template></el-table-column><el-table-column label="等待" width="85"><template #default="{row}"><span :class="{'warning-text':row.waitThreadCount>0}">{{formatNumber(row.waitThreadCount)}}</span></template></el-table-column>
     <el-table-column prop="source" label="来源" width="80"/><el-table-column prop="bootId" label="启动批次" min-width="150"><template #default="{row}">{{row.bootId || '未上报'}}</template></el-table-column>
    </el-table>
   </section>
   <section class="monitor-panel"><header><h2>错误 SQL 快速定位</h2><router-link to="/sql">全部 SQL</router-link></header>
    <el-table :data="errors" border stripe size="small" empty-text="没有已上报的错误 SQL" @row-click="row=>router.push(detailLocation('sql',row))">
     <el-table-column label="SQL / 来源" min-width="400"><template #default="{row}"><code>{{row.sql ?? '原文未上报'}}</code><div class="source-text">{{row.serviceName}} / {{row.instanceId || row.serviceId}} / {{row.dataSource}}</div></template></el-table-column>
     <el-table-column label="累计执行" width="130"><template #default="{row}">{{formatNumber(row.executeCount)}}</template></el-table-column><el-table-column label="累计错误" width="130"><template #default="{row}"><span class="danger-text">{{formatNumber(row.errorCount)}}</span></template></el-table-column>
    </el-table>
   </section>
  </template>
 </div>
</template>
