// 只允许仍挂载组件的最近一次请求提交结果。
export function createRequestGuard() {
  let version = 0
  let active = true
  return {
    begin() { const current = ++version; return () => active && current === version },
    dispose() { active = false; version++ },
  }
}
