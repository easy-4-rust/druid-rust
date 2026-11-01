<script setup>
import { computed, ref } from 'vue'
// 只引 core + sql 语法，等价 markdown 渲染器的 SQL 代码块高亮。
import hljs from 'highlight.js/lib/core'
import sql from 'highlight.js/lib/languages/sql'
import { ElMessage } from 'element-plus'
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

const copying = ref(false)

// 复制的是格式化后的 SQL（与服务端 formattedSql 口径一致）。
function source() {
  return props.formattedSql || formatSql(props.sql)
}

async function copy() {
  const text = source()
  if (!text) return
  copying.value = true
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
    } else {
      // 非安全上下文（http + 非 localhost）降级：临时 textarea + execCommand。
      const textarea = document.createElement('textarea')
      textarea.value = text
      textarea.style.position = 'fixed'
      textarea.style.opacity = '0'
      document.body.appendChild(textarea)
      textarea.select()
      document.execCommand('copy')
      document.body.removeChild(textarea)
    }
    ElMessage.success('已复制 SQL')
  } catch {
    ElMessage.error('复制失败，请手动选择复制')
  } finally {
    copying.value = false
  }
}
</script>

<template>
  <div class="sql-wrap">
    <el-button class="copy-btn" size="small" text @click="copy">
      {{ copying ? '复制中…' : '复制' }}
    </el-button>
    <pre class="sql-code"><code v-html="highlighted" /></pre>
  </div>
</template>

<style scoped>
/* 相对定位容器：复制按钮悬浮在代码块右上角。 */
.sql-wrap {
  position: relative;
}

.copy-btn {
  position: absolute;
  top: 8px;
  right: 8px;
  z-index: 1;
  color: #8b949e;
  background: rgba(177, 186, 196, 0.12);
  border-radius: 6px;
  padding: 4px 10px;
}

.copy-btn:hover {
  color: #c9d1d9;
  background: rgba(177, 186, 196, 0.22);
}

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
