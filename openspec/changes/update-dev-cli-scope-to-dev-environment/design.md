## Context
当前应用同时在“软件全景”和“开发环境”两处展示开发相关的全局 CLI 工具：
- “软件全景”通过 `DevCliAdapter` 把 `npm -g` / `cargo install` / `uv tool` / `pipx` 的条目当作“软件包”展示，并提供卸载命令。
- “开发环境”按语言展示运行时、版本管理器、全局包（Node 的 npm、Rust 的 cargo 等）。

结果是同一类对象重复出现，且“软件全景”列表被大量 CLI 工具噪音淹没，用户难以判断应该在哪个页面完成开发相关操作。

## Goals / Non-Goals
- Goals:
  - 明确页面职责边界：软件全景聚焦系统软件/桌面应用；开发环境聚焦开发工具链/全局 CLI 工具
  - 消除默认视图中的重复信息与噪音
  - 将 uv tools / pipx apps 纳入开发环境视图，保证功能不回退
- Non-Goals:
  - 不实现自动安装/卸载/提权执行（仍以“生成/复制命令”为主）
  - 不新增新的包管理器支持范围（仅调整展示归属）
  - 不对 UI 做大规模重排（只做必要的内容归类与呈现优化）

## Decisions
- **软件全景不再包含 dev-cli**：
  - `DiscoveryService` 适配器列表移除 `DevCliAdapter`，避免将语言包管理器的全局 CLI 工具混入“软件全景”
- **开发环境补齐 uv/pipx**：
  - 在 `PythonEnvAdapter` 中补齐 `uv tool list` 与 `pipx list --json` 的采集与解析
  - 统一输出到 `GlobalPackageInfo { manager, name, version }`，由开发环境 UI 统一展示
- **去重策略**：
  - 以 `(manager, name)` 为唯一键去重，减少重复展示（例如多来源输出同名工具）

## Risks / Trade-offs
- `pip3 list` 可能返回大量包，叠加 uv/pipx 可能加重 UI 负担；必要时后续增加“只显示工具类/限制条数/搜索过滤”。
- `uv tool list` / `pipx list` 需要外部命令，可能超时或输出格式变更；保持超时控制与解析失败降级为 warning。

