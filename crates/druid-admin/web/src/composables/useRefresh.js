import { onMounted, onUnmounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { createRequestGuard } from '../utils/requestGuard.js'
import { store } from '../store.js'
// 保持组件实例与筛选；只让最新且仍挂载的请求更新界面。
export function useRequestGuard() {
  const guard = createRequestGuard()
  onUnmounted(guard.dispose)
  return guard.begin
}
export function useRefresh(load) {
  const route = useRoute()
  onMounted(load)
  watch(() => [store.tick, store.serviceId, store.datasourceId, route.fullPath], load)
}
