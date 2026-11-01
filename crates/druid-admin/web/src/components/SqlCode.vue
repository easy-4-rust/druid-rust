<script setup>
import { computed } from 'vue'
import { formatSql } from '../utils/format.js'

const props = defineProps({
  sql: { type: String, default: '' },
})

// 简易关键字高亮：仅着色，不做完整词法分析。
const highlighted = computed(() => {
  const formatted = formatSql(props.sql)
  const escaped = formatted
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
  return escaped.replace(
    /\b(SELECT|FROM|WHERE|AND|OR|ORDER BY|GROUP BY|HAVING|LIMIT|INSERT INTO|UPDATE|DELETE FROM|SET|VALUES|JOIN|LEFT JOIN|RIGHT JOIN|INNER JOIN|ON|AS|DESC|ASC|NOT|NULL|IN|LIKE|BETWEEN)\b/gi,
    '<span class="kw">$1</span>',
  )
})
</script>

<template>
  <pre class="sql-code"><code v-html="highlighted" /></pre>
</template>

<style scoped>
.sql-code {
  margin: 0;
  padding: 12px 16px;
  background: #282c34;
  color: #abb2bf;
  border-radius: 6px;
  font-family: 'JetBrains Mono', Consolas, Monaco, monospace;
  font-size: 13px;
  line-height: 1.7;
  overflow-x: auto;
  white-space: pre-wrap;
  word-break: break-all;
}

.sql-code :deep(.kw) {
  color: #61afef;
  font-weight: 600;
}
</style>
