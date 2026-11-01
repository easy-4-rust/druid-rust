<script setup>
import { computed } from 'vue'
// 只引 core + sql 语法，等价 markdown 渲染器的 SQL 代码块高亮。
import hljs from 'highlight.js/lib/core'
import sql from 'highlight.js/lib/languages/sql'
import { formatSql } from '../utils/format.js'

hljs.registerLanguage('sql', sql)

const props = defineProps({
  sql: { type: String, default: '' },
  // 服务端已格式化好的 SQL（详情路由的 formattedSql）；传入则跳过客端格式化。
  formattedSql: { type: String, default: '' },
})

// highlight.js 输出 token span；解析失败时降级为纯文本。
const highlighted = computed(() => {
  const source = props.formattedSql || formatSql(props.sql)
  if (!source) return ''
  try {
    return hljs.highlight(source, { language: 'sql' }).value
  } catch {
    return source
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
  }
})
</script>

<template>
  <pre class="sql-code"><code v-html="highlighted" /></pre>
</template>

<style scoped>
/* 深色代码块 + github-dark 配色（hljs 官方主题色值内联，避免引整份主题 css）。 */
.sql-code {
  margin: 0;
  padding: 14px 18px;
  background: #0d1117;
  color: #c9d1d9;
  border-radius: 6px;
  font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
  font-size: 13px;
  line-height: 1.7;
  overflow-x: auto;
  white-space: pre-wrap;
  word-break: break-all;
}

/* 覆盖全局 code 底色，保证代码块内部干净。 */
.sql-code code {
  background: transparent;
  padding: 0;
  font-size: inherit;
}

.sql-code :deep(.hljs-keyword) {
  color: #ff7b72;
}

.sql-code :deep(.hljs-string) {
  color: #a5d6ff;
}

.sql-code :deep(.hljs-number) {
  color: #79c0ff;
}

.sql-code :deep(.hljs-comment) {
  color: #8b949e;
  font-style: italic;
}

.sql-code :deep(.hljs-literal) {
  color: #79c0ff;
}

.sql-code :deep(.hljs-built_in) {
  color: #d2a8ff;
}

.sql-code :deep(.hljs-title) {
  color: #d2a8ff;
}

.sql-code :deep(.hljs-attr) {
  color: #79c0ff;
}

.sql-code :deep(.hljs-variable) {
  color: #ffa657;
}

::selection {
  background: #264f78;
}
</style>
