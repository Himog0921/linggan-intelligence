# 插件授权协议

> 目标：把“谁能用插件”与“是哪一个浏览器插件在接单”拆成两层，但用户只需要完成一次连接，且都以内容工作台为唯一事实源。

## 1. 两层身份

### 1.1 授权码

- 作用：决定某个人 / 某台浏览器是否有资格使用插件
- 管理入口：内容工作台 → 设置 → 插件授权
- 特性：
  - 由管理员生成
  - 授权成功时自动准备工位
  - 可撤销、可过期、可限制席位和设备数
  - 未授权时，插件不开放采集、导出、同步、Dashboard、Cookie/账号管理和远程接单

### 1.2 工位

- 作用：让工作台区分同一账号下的多个浏览器插件，并记录每个插件的在线状态、接单开关、账号健康和任务记录
- 管理入口：内容工作台 → 设置 → 插件工位
- 特性：
  - 插件本地生成稳定 `stationKey`
  - 授权连接或审批领取时，工作台按 `stationKey` 自动创建或复用工位
  - 默认名称按成员和浏览器自动生成，例如“张三 Chrome 工位”
  - 一个账号名下可以存在多个浏览器插件，每个插件对应一个工位
  - 用户可以在工作台改名、暂停接单、查看账号健康或删除工位
  - 配对码只保留给旧版插件或自动连接失败后的修复入口

## 2. 内容工作台设置

内容工作台设置页至少需要三个管理区：

1. 插件授权
   - 生成授权码
   - 查看授权码状态（未使用 / 已激活 / 已撤销 / 已过期）
   - 查看归属成员、设备、过期时间、最近活跃时间
2. 插件工位
   - 查看自动创建的工位
   - 改名、暂停接单、恢复接单、删除工位
   - 仅在旧版插件或修复场景生成旧版配对码
3. 设备管理
   - 查看设备 `deviceId / stationId / 浏览器标识 / 最近心跳`
   - 撤销设备授权
   - 清空工位绑定

## 3. 插件侧本地状态

### 3.1 存储键

- `workbenchPluginAuthorization`
  - `deviceId`
  - `authorizationId`
  - `authorizationToken`
  - `status`
  - `teamName`
  - `memberName`
  - `seatName`
  - `expiresAt`
- `workbenchExecutionStation`
  - `stationId`
  - `stationToken`
  - `stationKey`
  - `displayName`
  - `role`

### 3.2 门禁规则

插件必须满足下面条件才算“可用”：

1. 已配置工作台地址
2. 已成功激活授权码，或已领取工作台审批通过的授权
3. 已收到工作台返回的工位身份

未满足第 2 条时，Popup、页内按钮、Background 接单都必须拒绝启动任务。

## 4. API

### 4.1 激活授权

```text
POST /api/plugin-authorizations/activate
```

请求：

```json
{
  "authorizationCode": "AUTH-001",
  "deviceId": "browser-device-uuid",
  "stationKey": "browser-station-uuid",
  "pluginVersion": "2.0.0",
  "browserLabel": "Chrome 135 / macOS",
  "capabilities": ["xhs:author_baseline"]
}
```

响应：

```json
{
  "authorizationId": "auth_123",
  "authorizationToken": "pat_123",
  "status": "active",
  "teamId": "team_1",
  "teamName": "内容团队",
  "memberId": "member_1",
  "memberName": "张三",
  "seatId": "seat_1",
  "seatName": "团队授权席位 A",
  "expiresAt": "2026-05-01T00:00:00.000Z",
  "station": {
    "stationId": "station_123",
    "stationToken": "est_123",
    "displayName": "张三 Chrome 工位",
    "role": "execution",
    "status": "offline"
  }
}
```

### 4.2 旧版执行工位注册

```text
POST /api/execution-stations/register
Authorization: Bearer <authorizationToken>
```

仅旧版插件或自动连接失败后的修复入口使用。日常连接流程不再调用这个接口。

请求体必须同时带：

- `authorizationId`
- `pairingCode`
- `stationKey`

### 4.3 心跳 / 接单 / 续租 / 同步

以下请求都必须携带插件授权 Bearer Token；V1.1 派单/续租身份以请求头为准，不再要求 body 重复携带旧 `authorizationId`：

- `POST /api/execution-stations/sync`
- `POST /api/execution-tasks/manual-import`（Dashboard / Popup 手动同步，先落 RawSnapshot）
- `POST /api/collection-tasks/:taskId/ingest`
- `GET /api/collection-tasks/:taskId/control-requests`

接单通过 `/api/execution-stations/sync` 的 `capacity → reservations[] → start_job` 完成；运行中的续租和进度上报通过同一路径的 `progress_update` operation 完成，不再调用旧 `/api/collection-tasks/:taskId/lease`。

执行工位模式下，插件不再调用 `GET /api/collection-tasks` 恢复扫描任务列表，也不再调用旧的 `heartbeat / reconcile / dispatch` 工位接口；旧版本如果继续高频轮询，会被工作台入口层拦截。

普通同步数据归属规则：插件授权只证明“这台浏览器可以使用插件”，不代表普通采集数据应该写入授权者账号。Dashboard 手动同步和批量同步在写入前必须先通过 `POST /api/plugin-data-workspace` 绑定当前登录的内容工作台使用者账号，并在后续请求里携带 `X-Plugin-Data-Token`。如果没有使用者登录或绑定失败，工作台应拒绝同步，而不是回退写到授权者、系统所有者或共享执行池。手动同步统一走 `POST /api/execution-tasks/manual-import`：工作台为每个平台建立只导入、不派单的可追踪任务；笔记、评论和博主先落 RawSnapshot / RawRecord。笔记在同一事务中创建或复用 ContentAsset，并将媒体清单登记到唯一 MediaItem / MediaOrigin / ContentMediaUsage 账本，再由账本 outbox 请求标准物化；相同内容重复提交直接复用，不重复写入。

媒体同步规则：插件只回传采集事实，不再把封面或视频字节上传到单独资产接口，也不把平台原始地址改写成展示地址。跨端笔记协议只能使用 `coverUrl`、有序 `imageUrls`、`videoUrl` 三个媒体字段；插件内部的 `cover / coverImage / images / video / videoStreams` 等平台采集字段只可保留在 `rawData` 原始证据中，不能作为并行的业务字段出站。`imageUrls` 的每一个位置代表一张实际图片；同一图片的主、备 URL 只能选择一个确定来源，备用地址继续留在 `rawData.imageCandidates`，不能展开成虚假的下一张图片。工作台将规范字段登记到唯一媒体账本并异步物化；明确给出但不是 HTTP(S) 的来源必须计入 `mediaInvalid`，不能静默忽略。导入结果除了数量外必须以 `mediaRegistrationConfirmed: true` 明确确认账本登记，前端不得由计数推断成功；“已登记/已入队”不等同于媒体文件已经下载完成。

## 5. 失效与撤销

当内容工作台撤销授权时，插件下一次 `/sync`、续租或数据同步必须立即失败，并引导用户重新激活授权码。
若用户在插件里主动清除授权，本地必须同时清掉：

1. `authorizationToken`
2. 工位身份
3. 本地任务租约快照

## 6. 当前实现范围（2026-06-25）

本仓库已落地插件执行端：

1. 授权码与自动工位分离
2. Popup 配置区新增“插件授权”
3. 授权连接或审批领取时自动创建 / 复用工位
4. Popup / 页内按钮 / Background 接单链路接入授权门禁
5. 工作台请求在心跳 / 租约 / 同步时带授权信息和工位身份
6. 所有手动同步与任务增量只回传规范媒体源字段；媒体账本负责关联、去重和后续物化

内容工作台设置页已保留旧版配对入口，但日常路径应以“连接插件后自动出现工位”为准。
