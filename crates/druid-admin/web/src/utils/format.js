// 数字与时长格式化工具。

export function formatNumber(value) {
  if (value === null || value === undefined || Number.isNaN(Number(value))) return '0'
  return Number(value).toLocaleString('en-US')
}

export function formatDuration(millis) {
  const ms = Number(millis) || 0
  if (ms < 1) return `${ms} ms`
  if (ms < 1000) return `${formatNumber(Math.round(ms))} ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(2)} s`
  if (ms < 3_600_000) return `${(ms / 60_000).toFixed(2)} min`
  return `${(ms / 3_600_000).toFixed(2)} h`
}

// Java 控制台使用的直方图分桶标签（与 druid-admin 资源页一致）。
export const TIME_BUCKETS_8 = ['0-1 ms', '1-10 ms', '10-100 ms', '100 ms-1 s', '1-10 s', '10-100 s', '100-1000 s', '> 1000 s']
export const TIME_BUCKETS_7 = ['0-1 ms', '1-10 ms', '10-100 ms', '100 ms-1 s', '1-10 s', '10-100 s', '> 100 s']
export const ROW_BUCKETS_6 = ['0', '1-9', '10-99', '100-999', '1000-9999', '> 9999']

// 把后端直方图数组组装为 { label, count } 列表。
export function zipBuckets(values, labels) {
  if (!Array.isArray(values)) return []
  return values.map((count, index) => ({
    label: labels[index] || `#${index}`,
    count: Number(count) || 0,
  }))
}

// 简易 SQL 格式化：按关键字换行缩进，仅用于展示。
export function formatSql(sql) {
  if (!sql) return ''
  const keywords = ['SELECT', 'FROM', 'WHERE', 'GROUP BY', 'ORDER BY', 'HAVING', 'LIMIT', 'SET', 'VALUES', 'INSERT INTO', 'UPDATE', 'DELETE FROM']
  let text = sql.replace(/\s+/g, ' ').trim()
  for (const kw of keywords) {
    const re = new RegExp(`\\b${kw.replace(' ', '\\s+')}\\b`, 'gi')
    text = text.replace(re, `\n${kw} `)
  }
  return text.replace(/^\n/, '').replace(/ , /g, ', ')
}

// 简易 SQL 解析：提取表名、字段、条件，对应 Java 详情页的 parsed* 字段。
export function parseSqlMeta(sql) {
  if (!sql) return { tables: [], fields: [], conditions: [] }
  const normalized = sql.replace(/\s+/g, ' ').trim()

  const tables = []
  const tableRe = /\b(?:FROM|INTO|UPDATE|JOIN)\s+([a-zA-Z_][\w.]*)/gi
  let match = tableRe.exec(normalized)
  while (match) {
    if (!tables.includes(match[1])) tables.push(match[1])
    match = tableRe.exec(normalized)
  }

  const fields = []
  const selectMatch = /\bSELECT\s+(.*?)\s+FROM\b/i.exec(normalized)
  if (selectMatch && selectMatch[1] !== '*') {
    for (const item of selectMatch[1].split(',')) {
      const name = item.trim()
      if (name) fields.push(name)
    }
  }

  const conditions = []
  const whereMatch = /\bWHERE\s+(.*)$/i.exec(normalized)
  if (whereMatch) {
    const clause = whereMatch[1].replace(/\b(ORDER BY|GROUP BY|LIMIT)\b.*$/i, '')
    for (const cond of clause.split(/\bAND\b/i)) {
      const text = cond.trim()
      if (text) conditions.push(text)
    }
  }

  return { tables, fields, conditions }
}
