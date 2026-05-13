# 设计说明：process-manager（进程管理 / 内存加速）

## 数据来源
- 内存概览：读取 `/proc/meminfo`
  - 关键字段：MemTotal、MemAvailable、SwapTotal、SwapFree
  - 计算：MemUsed = MemTotal - MemAvailable；SwapUsed = SwapTotal - SwapFree
- 进程列表：遍历 `/proc` 下的数字目录
  - `/proc/<pid>/status`：解析 Name、Uid、VmRSS（或 RssAnon/RssFile 的回退策略）
  - `/proc/<pid>/cmdline`：用于展示更友好的命令提示（不可读时允许为空）

## 扫描与性能
- 扫描属于 IO 密集型操作，放入 `tokio::spawn_blocking` 执行，避免阻塞 GTK 主线程。
- UI 通过 `async-channel` 接收扫描结果，必要时对列表更新做分批渲染（与软件全景增量加载一致）。

## 结束进程策略
- 默认：SIGTERM（温和结束）
- 可选：SIGKILL（强制结束）
- 安全限制：
  - 仅允许结束当前用户（同 UID）进程
  - 禁止结束应用自身进程（packlens）
  - 必须二次确认，并提示可能造成未保存数据丢失

## 错误处理
- 进程可能在扫描后已退出：结束时返回“已退出/不存在”，UI 视为可忽略失败并提示刷新。
- 无权限：对非同 UID 进程直接禁用按钮；即使用户尝试执行也应返回“无权限”错误。

