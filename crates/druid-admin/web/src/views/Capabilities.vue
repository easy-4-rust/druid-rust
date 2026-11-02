<script setup>
import { computed, ref } from 'vue'
import { api } from '../api/client.js'
import { scopeQuery } from '../store.js'
import { useRefresh, useRequestGuard } from '../composables/useRefresh.js'
import SourceStatus from '../components/SourceStatus.vue'
const body=ref(null),error=ref(''),begin=useRequestGuard()
const fields=[['连接状态','activeCount'],['等待连接','waitThreadCount'],['累计执行','executeCount'],['累计错误','errorCount'],['事务提交','commitCount'],['事务回滚','rollbackCount'],['PS 缓存','psCacheHitCount'],['事务分布','transactionHistogram'],['连接持有分布','connectionHoldTimeHistogram'],['连接校验','testOnBorrow']]
const rows=computed(()=>body.value?.datasources||[])
const capabilities=computed(()=>fields.map(([name,key])=>({name,key})))
function state(row,key){return row.unavailableFields?.includes(key)?'不支持 / 未上报':row[key]==null?'未上报':'已上报'}
async function load(){const current=begin();try{const result=await api.datasources(scopeQuery());if(current()){body.value=result;error.value=''}}catch(cause){if(current())error.value=cause.message}}
useRefresh(load)
</script>
<template><div><el-alert v-if="error" :title="error" type="error" :closable="false"/><SourceStatus :snapshot="body"/>
<section class="monitor-panel"><header><h2>数据库与驱动</h2><el-button size="small" @click="load">刷新</el-button></header><el-table :data="rows" border stripe size="small" empty-text="尚未收到数据源统计"><el-table-column prop="name" label="数据源" min-width="180"/><el-table-column prop="dbType" label="数据库" min-width="130"/><el-table-column label="驱动" min-width="200"><template #default="{row}">{{row.driverClassName || '未上报'}}</template></el-table-column><el-table-column label="驱动版本" width="130"><template #default="{row}">{{row.driverVersion || '未上报'}}</template></el-table-column><el-table-column prop="source" label="来源" width="80"/></el-table></section>
<section class="monitor-panel"><header><h2>当前来源能力</h2></header><el-table :data="capabilities" border stripe size="small"><el-table-column prop="name" label="监控内容" width="160"/><el-table-column v-for="row in rows" :key="`${row.serviceId}/${row.identity}`" :label="`${row.name || row.identity} / ${row.instanceId || row.serviceName}`" min-width="180"><template #default="{row:item}">{{state(row,item.key)}}</template></el-table-column></el-table></section>
</div></template>
