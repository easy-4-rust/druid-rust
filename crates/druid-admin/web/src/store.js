import { reactive, watch } from 'vue'

// 控制台全局状态：自动刷新设置 + 刷新节拍。
// 视图订阅 tick 重新拉取数据，等价于 Java 控制台的定时刷新。
export const store = reactive({
  autoRefreshSeconds: 0,
  tick: 0,
  lastRefreshAt: null,
})

let timer = null

watch(
  () => store.autoRefreshSeconds,
  (seconds) => {
    if (timer) {
      clearInterval(timer)
      timer = null
    }
    if (seconds > 0) {
      timer = setInterval(() => {
        store.tick += 1
        store.lastRefreshAt = new Date()
      }, seconds * 1000)
    }
  },
)

export const refreshOptions = [
  { value: 0, label: '不自动刷新' },
  { value: 5, label: '每 5 秒' },
  { value: 15, label: '每 15 秒' },
  { value: 60, label: '每 60 秒' },
]
