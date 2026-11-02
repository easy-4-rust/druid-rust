// 所有界面只消费版本化管理 API；敏感数据由服务端统一脱敏。
const BASE = '/druid/api/v1'
function normalize(value) {
  if (Array.isArray(value)) return value.map(normalize)
  if (!value || typeof value !== 'object') return value
  const result = Object.fromEntries(Object.entries(value).map(([key, item]) => [key, normalize(item)]))
  for (const key of value.unavailableFields || []) result[key] = null
  return result
}
async function request(path, options = {}) {
  const response = await fetch(`${BASE}${path}`, { credentials: 'same-origin', headers: { Accept: 'application/json' }, ...options })
  const text = await response.text()
  let body
  try { body = text ? JSON.parse(text) : null } catch { body = null }
  if (!response.ok) {
    const messages = { 401: '请先登录管理台', 403: '没有访问权限', 404: '未找到对应数据', 409: '身份不唯一，请从指定来源重新进入', 422: '查询参数无效', 502: '采集来源请求失败' }
    const error = new Error(body?.message || messages[response.status] || `请求失败（${response.status}）`)
    error.status = response.status
    throw error
  }
  if (!body || typeof body !== 'object') throw new Error('管理接口返回无效数据')
  return normalize(body)
}
function query(params = {}) {
  const text = new URLSearchParams(Object.entries(params).filter(([, v]) => v !== undefined && v !== null && v !== '')).toString()
  return text ? `?${text}` : ''
}
export const api = {
  basic: () => request('/basic'),
  datasources: (params) => request('/datasources' + query(params)),
  datasource: (id, params) => request(`/datasources/${encodeURIComponent(id)}` + query(params)),
  sql: (params) => request('/sql' + query(params)),
  sqlDetail: (id, params) => request(`/sql/${encodeURIComponent(id)}` + query(params)),
  wall: (params) => request('/wall' + query(params)),
  health: () => request('/health'),
  history: (params) => request('/history' + query(params)),
  connections: (id, params) => request(`/datasources/${encodeURIComponent(id)}/connections` + query(params)),
  stack: (id, params) => request(`/datasources/${encodeURIComponent(id)}/stack` + query(params)),
  services: () => request('/services'),
  openapi: () => request('/openapi.json'),
}
