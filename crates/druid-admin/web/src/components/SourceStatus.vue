<script setup>
defineProps({ snapshot: { type: Object, default: null } })
</script>
<template>
  <el-alert v-if="snapshot?.partial" type="warning" :closable="false" show-icon class="source-status" title="部分采集来源不可用，当前只展示已取得的数据">
    <div v-for="(failure, index) in snapshot.failures || []" :key="index">{{ failure.serviceName || failure.serviceId || '采集来源' }}：{{ failure.reason }}</div>
  </el-alert>
</template>
<style scoped>.source-status{margin-bottom:12px}</style>
