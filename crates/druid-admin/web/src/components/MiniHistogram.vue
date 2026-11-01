<script setup>
import { computed } from 'vue'
import { formatNumber } from '../utils/format.js'

const props = defineProps({
  buckets: { type: Array, default: () => [] },
  color: { type: String, default: '#409eff' },
})

// 正方形单元格：宽度固定，高度与宽度一致，hover 热区扩大到整行高。
const CELL = 8

const max = computed(() => Math.max(...props.buckets.map((b) => Number(b.count) || 0), 1))

const cells = computed(() =>
  props.buckets.map((bucket) => ({
    ...bucket,
    opacity: 0.25 + 0.75 * ((Number(bucket.count) || 0) / max.value),
  })),
)
</script>

<template>
  <div v-if="buckets.length" class="mini" :style="{ height: `${CELL * 2 + 4}px` }">
    <el-tooltip
      v-for="cell in cells"
      :key="cell.label"
      :content="`${cell.label}：${formatNumber(cell.count)}`"
      placement="top"
    >
      <div class="hit">
        <div
          class="cell"
          :style="{ width: `${CELL}px`, height: `${CELL}px`, background: color, opacity: cell.opacity }"
        />
      </div>
    </el-tooltip>
  </div>
  <span v-else class="empty">—</span>
</template>

<style scoped>
.mini {
  display: flex;
  align-items: center;
  gap: 2px;
}

/* 整行都是 hover 热区，单元格本体保持正方形。 */
.hit {
  display: flex;
  align-items: center;
  height: 100%;
}

.cell {
  border-radius: 1px;
}

.empty {
  color: #c0c4cc;
}
</style>
