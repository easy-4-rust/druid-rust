<script setup>
import { computed } from 'vue'
import { formatNumber } from '../utils/format.js'

const props = defineProps({
  buckets: { type: Array, default: () => [] },
  color: { type: String, default: '#409eff' },
})

const max = computed(() => Math.max(...props.buckets.map((b) => Number(b.count) || 0), 1))

const cells = computed(() =>
  props.buckets.map((bucket) => ({
    ...bucket,
    opacity: 0.25 + 0.75 * ((Number(bucket.count) || 0) / max.value),
  })),
)
</script>

<template>
  <div v-if="buckets.length" class="mini">
    <el-tooltip
      v-for="cell in cells"
      :key="cell.label"
      :content="`${cell.label}：${formatNumber(cell.count)}`"
      placement="top"
    >
      <div class="cell" :style="{ background: color, opacity: cell.opacity }" />
    </el-tooltip>
  </div>
  <span v-else class="empty">—</span>
</template>

<style scoped>
.mini {
  display: flex;
  align-items: flex-end;
  gap: 2px;
  height: 22px;
}

.cell {
  width: 8px;
  height: 100%;
  border-radius: 1px;
}

.empty {
  color: #c0c4cc;
}
</style>
