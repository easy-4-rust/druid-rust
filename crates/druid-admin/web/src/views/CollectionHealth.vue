<script setup>
import { ref, computed } from 'vue'
import { api } from '../api/client.js'
import { useRefresh, useRequestGuard } from '../composables/useRefresh.js'
import { formatNumber } from '../utils/format.js'
import SourceStatus from '../components/SourceStatus.vue'
const body=ref(null),history=ref(null),error=ref(''),historyError=ref(''),loading=ref(false),begin=useRequestGuard()
const metrics=computed(()=>{const r=body.value?.repository||{};return [
 ['接收批次',r.ingestCount],['拒绝批次',r.rejectedCount],['容量拒绝',r.capacityRejectedCount],['TTL 回收',r.ttlEvictedCount],['指标输出拒绝',r.metricsOutputRejectedCount],
 ['状态序列化字节',r.retainedStateBytes],['状态字节预算',r.maxTotalStateBytes],['无活动 TTL（ms）',r.entryTtlMillis],['最大服务数',r.maxServices],['每池 SQL 上限',r.maxSqlEntriesPerDatasource],
 ['进程 RSS（字节）',body.value?.processRssBytes],['待 ACK 数',body.value?.pendingAckCount],['队列深度',body.value?.queueDepth],['未刷出数',body.value?.unflushedCount],
 ].map(([name,value])=>({name,value:formatNumber(value)}))})
async function load(){
 const current=begin();loading.value=true
 try{
  const [healthResult,historyResult]=await Promise.allSettled([api.health(),api.history({limit:100})])
  if(!current())return
  if(healthResult.status==='fulfilled'){body.value=healthResult.value;error.value=''}
  else{body.value=null;error.value=healthResult.reason?.message || '采集健康读取失败'}
  if(historyResult.status==='fulfilled'){history.value=historyResult.value;historyError.value=''}
  else{history.value=null;historyError.value=historyResult.reason?.message || '采样历史读取失败'}
 }finally{if(current())loading.value=false}
}
useRefresh(load)
</script>
<template><div v-loading="loading"><el-alert v-if="error" :title="error" type="error" :closable="false"/><SourceStatus :snapshot="body"/>
 <section class="monitor-panel"><header><h2>采集来源</h2><el-button size="small" @click="load">刷新</el-button></header>
 <el-table :data="body?.sources || []" border stripe size="small" empty-text="尚未收到采集数据">
 <el-table-column label="服务 / 实例" min-width="210"><template #default="{row}">{{row.serviceName}}<div class="source-text">{{row.instanceId || row.serviceId}}</div></template></el-table-column>
 <el-table-column prop="name" label="数据源" min-width="160"/><el-table-column prop="source" label="模式" width="70"/><el-table-column prop="bootId" label="启动批次" min-width="160"><template #default="{row}">{{row.bootId || '未上报'}}</template></el-table-column>
 <el-table-column label="采样时间" min-width="180"><template #default="{row}">{{row.samplingTimeMillis ? new Date(row.samplingTimeMillis).toLocaleString() : '未上报'}}</template></el-table-column>
 <el-table-column label="接收后经过（ms）" width="145"><template #default="{row}">{{formatNumber(row.receivedAgeMillis)}}</template></el-table-column><el-table-column label="序列" width="100"><template #default="{row}">{{formatNumber(row.sequence)}}</template></el-table-column><el-table-column label="Epoch" width="100"><template #default="{row}">{{formatNumber(row.epoch)}}</template></el-table-column>
 </el-table></section>
 <section class="monitor-panel"><header><h2>推送采样历史 · 连接 / SQL / Wall</h2><span class="muted">进程内保留 {{formatNumber(history?.storage?.storedSamples)}} / {{formatNumber(history?.storage?.maxSamples)}} 条，重启后清空</span></header>
 <el-alert v-if="historyError" :title="historyError" type="error" :closable="false"/>
 <el-table :data="[...(history?.samples || [])].reverse()" border stripe size="small" empty-text="尚未收到推送采样">
 <el-table-column label="接收时间" min-width="170"><template #default="{row}">{{new Date(row.receivedTimeMillis).toLocaleString()}}</template></el-table-column>
 <el-table-column label="服务 / 数据源" min-width="175"><template #default="{row}">{{row.serviceName}} / {{row.datasourceId}}<div class="source-text">{{row.instanceId}} · #{{row.sequence}}</div></template></el-table-column>
 <el-table-column label="活跃 / 空闲 / 上限" min-width="165"><template #default="{row}">{{formatNumber(row.activeCount)}} / {{formatNumber(row.poolingCount)}} / {{formatNumber(row.maxActive)}}</template></el-table-column>
 <el-table-column label="等待" width="80"><template #default="{row}">{{formatNumber(row.waitThreadCount)}}</template></el-table-column>
 <el-table-column label="SQL 累计 / 增量" min-width="145"><template #default="{row}">{{formatNumber(row.executeCount)}} / {{formatNumber(row.executeCountDelta)}}</template></el-table-column>
 <el-table-column label="SQL 总耗时 ms" min-width="120"><template #default="{row}">{{formatNumber(row.sqlTotalTimeMillis)}}</template></el-table-column>
 <el-table-column label="取回 / 影响行" min-width="150"><template #default="{row}">{{formatNumber(row.fetchRowCount)}} / {{formatNumber(row.effectedRowCount)}}</template></el-table-column>
 <el-table-column label="Wall 检查 / 拒绝 / 违规" min-width="175"><template #default="{row}">{{formatNumber(row.wallCheckCount)}} / {{formatNumber(row.wallDenyCount)}} / {{formatNumber(row.wallViolationCount)}}</template></el-table-column>
 </el-table></section>
 <section class="monitor-panel"><header><h2>仓库与管线状态</h2><span class="muted">状态字节为序列化投影大小</span></header><el-table :data="metrics" border stripe size="small"><el-table-column prop="name" label="指标"/><el-table-column prop="value" label="当前值"/></el-table></section>
 </div></template>
