# 反检测策略

> 防止被小红书风控系统识别为自动化工具。

## 策略清单

| 策略 | 实现 | 所在文件 |
|------|------|---------|
| 随机延迟 | `randomDelay(min, max)` | `shared/utils.js` |
| 拟人滚动 | `humanScroll(container, step)` | `content/antiDetect.js` |
| 分级节流 | `throttle(count)` — 采集量越多，间隔越长 | `content/antiDetect.js` |
| 验证码检测 | `detectCaptcha()` — 检测页面中的验证码元素 | `content/antiDetect.js` |
| 验证码监控 | `watchCaptcha(callback)` — 轮询检测并触发回调 | `content/antiDetect.js` |
| 媒体资源屏蔽 | `BLOCK_MEDIA` action — 批量采集时可选屏蔽图片/视频 | `background/index.js` |

## 参数配置

定义在 `shared/constants.js` 的 `BATCH_CONFIG`：

| 参数 | 说明 |
|------|------|
| 主评论间停顿 | 200-500ms |
| 滚动等待 | 800-1500ms |
| 子评论展开间隔 | 1.2-2.2s |
| 最大展开次数 | 20 次 |

## 风控触发时的行为

1. `detectCaptcha()` 检测到验证码页面
2. 自动暂停当前任务
3. 显示浮层提示用户手动处理
4. 用户解决后点"继续"恢复任务
