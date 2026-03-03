# Change: 将 dev-cli 工具从软件全景迁移到开发环境

## Why
当前实现中，“软件全景”通过 DevCliAdapter 列出 npm/cargo/uv/pipx 的全局 CLI 工具；同时“开发环境”页也会按语言列出 npm/cargo 等全局包，导致同一类对象在两个页面重复出现，用户难以理解“应该在哪里管理”。此外 dev-cli 条目会稀释软件全景的核心价值（聚合系统软件与桌面应用），造成列表噪音。

本变更将明确边界：
- 软件全景：系统软件与桌面应用的统一视图
- 开发环境：开发工具链与全局 CLI 工具的统一视图

## What Changes
- **software-panorama**：不再展示 dev-cli 适配器产生的条目（npm/cargo/uv/pipx 全局 CLI 工具）
- **dev-environment**：补齐此前仅在 dev-cli 中出现的工具清单：uv tools 与 pipx apps，并在开发环境中统一展示
- UX：减少重复信息，降低用户认知成本；开发相关的“复制命令”操作聚合到开发环境页

## Impact
- Affected specs: software-panorama (MODIFIED), dev-environment (MODIFIED)
- Affected code (预计): `src/services/discovery.rs`, `src/adapters/dev_cli.rs`, `src/adapters/python_env.rs`, `src/services/environment.rs`, `src/ui/panorama.rs`, `src/ui/devenv.rs`
- 兼容性: 无 breaking API；仅调整 UI 信息归属与默认展示内容

