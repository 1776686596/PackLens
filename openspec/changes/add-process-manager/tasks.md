## 1. 需求与安全边界
- [x] 1.1 明确 UI 入口与命名（侧边栏新增“进程管理/内存加速”）
- [x] 1.2 明确安全限制：仅同 UID 可结束；禁止结束 packlens；所有操作二次确认
- [x] 1.3 明确结束策略：默认 SIGTERM；可选 SIGKILL；失败与已退出场景处理

## 2. Service：进程扫描与结束
- [x] 2.1 新增 `services/process_manager.rs`：读取 `/proc/meminfo` 生成内存概览
- [x] 2.2 新增 `services/process_manager.rs`：扫描 `/proc/[pid]/status` + `/proc/[pid]/cmdline`，生成进程列表（PID/名称/UID/RSS）
- [x] 2.3 新增 `services/process_manager.rs`：结束进程 API（TERM/KILL），返回结果（成功/失败/无权限/已退出）
- [x] 2.4 性能：扫描放入 `tokio::spawn_blocking`；数据通过 `async-channel` 推送给 UI

## 3. UI：进程管理页
- [x] 3.1 新增 `ui/process_manager.rs`：内存/Swap 概览卡片（Total/Available/Used/SwapUsed）
- [x] 3.2 进程列表：按 RSS 降序，支持搜索、复选框多选、显示占用与命令提示
- [x] 3.3 操作区：结束选中（TERM）+ 强制结束（KILL）；按钮状态与进度提示
- [x] 3.4 二次确认：展示将结束的数量与风险提示；仅确认后执行
- [x] 3.5 执行结果提示：成功/失败计数 + 失败原因（无权限/不存在/系统错误）

## 4. 接入导航与多语言
- [x] 4.1 `ui/mod.rs` 导出新页面模块
- [x] 4.2 `window.rs` 侧边栏新增导航项并接入语言切换重建逻辑
- [x] 4.3 i18n：所有新增文案提供中英文（沿用 `i18n::pick`）

## 5. 测试与校验
- [x] 5.1 单元测试：meminfo 解析、status 解析、排序/过滤逻辑（不依赖真实 `/proc`）
- [x] 5.2 单元测试：结束目标选择规则（同 UID/禁止 self）
- [x] 5.3 运行 `timeout 60s cargo test`
- [x] 5.4 运行 `openspec validate add-process-manager --strict --no-interactive`
