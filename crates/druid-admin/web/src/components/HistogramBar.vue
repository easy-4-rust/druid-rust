<script setup>
import { computed } from 'vue'
import { formatNumber } from '../utils/format.js'

const props = defineProps({
  // [{ label, count }]
  buckets: { type: Array, default: () => [] },
  height: { type: Number, default: 120 },
  color: { type: String, default: '#409eff' },
})

const total = computed(() => props.buckets.reduce((sum, b) => sum + (Number(b.count) || 0), 0))

const bars = computed(() => {
  const max = Math.max(...props.buckets.map((b) => Number(b.count) || 0), 1)
  return props.buckets.map((bucket) => ({
    ...bucket,
    percent: total.value > 0 ? ((Number(bucket.count) || 0) / total.value) * 100 : 0,
    heightPercent: ((Number(bucket.count) || 0) / max) * 100,
  }))
})
</script>

<template>
  <div v-if="buckets.length" class="histogram">
    <div class="chart" :style="{ height: `${height}px` }">
      <el-tooltip
        v-for="bar in bars"
        :key="bar.label"
        :content="`${bar.label}：${formatNumber(bar.count)} 次（${bar.percent.toFixed(1)}%）`"
        placement="top"
      >
        <div class="bar-wrap">
          <div
            class="bar"
            :style="{ height: `${Math.max(bar.heightPercent, bar.count > 0 ? 4 : 1)}%`, background: color }"
          />
          <span class="bar-count">{{ formatNumber(bar.count) }}</span>
        </div>
      </el-tooltip>
    </div>
    <div class="labels">
      <span v-for="bar in bars" :key="bar.label" class="label">{{ bar.label }}</span>
    </div>
    <div class="total">合计 {{ formatNumber(total) }}</div>
  </div>
  <el-empty v-else description="暂无直方图数据" :image-size="60" />
</template>

<style scoped>
.histogram {
  width: 100%;
}

.chart {
  display: flex;
  align-items: flex-end;
  gap: 6px;
  border-bottom: 1px solid #dcdfe6;
  padding: 0 4px;
}

.bar-wrap {
  flex: 1;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  align-items: center;
  height: 100%;
  min-width: 0;
}

.bar {
  width: 70%;
  max-width: 48px;
  border-radius: 3px 3px 0 0;
  transition: height 0.3s ease;
}

.bar-count {
  font-size: 11px;
  color: #606266;
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

.labels {
  display: flex;
  gap: 6px;
  padding: 4px;
}

.label {
  flex: 1;
  text-align: center;
  font-size: 11px;
  color: #909399;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.total {
  margin-top: 6px;
  font-size: 12px;
  color: #909399;
  text-align: right;
}
</style>
