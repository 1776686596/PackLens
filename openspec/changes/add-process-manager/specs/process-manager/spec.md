## ADDED Requirements

### Requirement: Memory Overview
系统 SHALL 在进程管理视图中展示系统内存与 Swap 概览信息，数据来源为 `/proc/meminfo`。至少包含：MemTotal、MemAvailable、MemUsed、SwapTotal、SwapUsed；单位为 bytes，UI 以人类可读格式展示。

#### Scenario: Show memory overview on page load
- **GIVEN** `/proc/meminfo` 可读取
- **WHEN** 用户进入进程管理视图
- **THEN** UI 显示内存与 Swap 概览（含 total/available/used），并在后台持续允许用户刷新

#### Scenario: meminfo unavailable
- **GIVEN** `/proc/meminfo` 不可读取（权限或异常环境）
- **WHEN** 用户进入进程管理视图
- **THEN** UI 显示加载失败提示，但不影响应用其他页面使用

### Requirement: Process List Scan
系统 SHALL 扫描 `/proc` 获取进程列表，并展示每个进程的 PID、名称、所属 UID、RSS 内存占用（来自 `/proc/<pid>/status` 的 VmRSS 字段，若不可用则记为未知）。列表默认按 RSS 降序排序，并支持关键词过滤（大小写不敏感）。

#### Scenario: List top processes by RSS
- **GIVEN** 系统存在多个运行中的进程
- **WHEN** 用户进入进程管理视图并完成扫描
- **THEN** UI 按 RSS 从大到小展示进程列表，缺失 RSS 的条目排在末尾

#### Scenario: Filter processes by keyword
- **GIVEN** 进程列表已加载完成
- **WHEN** 用户在搜索栏输入关键字（例如 "chrome"）
- **THEN** 列表仅展示名称或命令行包含该关键字的进程（大小写不敏感）

### Requirement: Terminate Processes With Confirmation
系统 SHALL 允许用户在 UI 中结束进程。结束操作必须二次确认，并明确提示“可能导致未保存数据丢失”。默认使用 SIGTERM；用户可选择强制结束（SIGKILL）。

安全边界：
- 系统 SHALL 仅允许结束当前用户（同 UID）拥有的进程；其他 UID 的进程结束按钮禁用并提示“需要管理员权限”。
- 系统 SHALL 禁止结束应用自身进程（softmgr）。

#### Scenario: Terminate a user-owned process
- **GIVEN** 某进程 PID=1234 且 UID=当前用户
- **WHEN** 用户勾选该进程并在确认后执行“结束进程（SIGTERM）”
- **THEN** 系统向 PID=1234 发送 SIGTERM，并在 UI 显示成功/失败结果；成功后刷新列表

#### Scenario: Force kill after graceful termination fails
- **GIVEN** 用户对 PID=1234 发送 SIGTERM 后该进程仍存活
- **WHEN** 用户再次确认并选择“强制结束（SIGKILL）”
- **THEN** 系统向 PID=1234 发送 SIGKILL，并在 UI 显示执行结果

#### Scenario: Attempt to terminate a non-owned process
- **GIVEN** 某进程 PID=1 且 UID!=当前用户
- **WHEN** 用户查看该进程条目
- **THEN** 结束按钮为禁用状态，并显示“需要管理员权限”的提示

#### Scenario: Prevent terminating softmgr
- **GIVEN** 进程列表包含应用自身 PID
- **WHEN** 用户尝试勾选并结束该 PID
- **THEN** UI 禁止该操作并提示“不可结束自身进程”

