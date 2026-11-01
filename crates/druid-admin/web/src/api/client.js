// 版本化管理 API 客户端。
//
// 前端只依赖这一套契约，不读写 Java 兼容路由，也不依赖任何服务端渲染。
const BASE = '/druid/api/v1'

async function request(path, options = {}) {
  const response = await fetch(`${BASE}${path}`, {
    credentials: 'same-origin',
    headers: { Accept: 'application/json' },
    ...options,
  })
  const text = await response.text()
  let body = null
  if (text) {
    try {
      body = JSON.parse(text)
    } catch {
      body = { message: text }
    }
  }
  if (!response.ok) {
    const error = new Error((body && body.message) || `请求失败：${response.status}`)
    error.status = response.status
    throw error
  }
  return body
}

export const api = {
  basic: () => request('/basic'),
  datasources: () => request('/datasources'),
  datasource: (identity) => request(`/datasources/${identity}`),
  sql: (params = {}) => {
    const query = new URLSearchParams(params).toString()
    return request(`/sql${query ? `?${query}` : ''}`)
  },
  sqlDetail: (id) => request(`/sql/${id}`),
  wall: () => request('/wall'),
  services: () => request('/services'),
  openapi: () => request('/openapi.json'),
}
