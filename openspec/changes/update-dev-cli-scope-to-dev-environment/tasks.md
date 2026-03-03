## 1. software-panorama：移除 dev-cli 条目
- [x] 1.1 `DiscoveryService`（`src/services/discovery.rs`）适配器列表移除 `DevCliAdapter`
- [x] 1.2 验证：软件全景不再出现 source 为 `npm/cargo/uv/pipx` 的“全局 CLI 工具”条目
- [x] 1.3 补充单元测试：Discovery 适配器列表不包含 dev-cli（避免回归）

## 2. dev-environment：补齐 uv/pipx 全局工具
- [x] 2.1 `PythonEnvAdapter` 增加 uv tools 采集（`uv tool list`），解析为 `GlobalPackageInfo { manager: "uv", ... }`
- [x] 2.2 `PythonEnvAdapter` 增加 pipx apps 采集（`pipx list --json`），解析为 `GlobalPackageInfo { manager: "pipx", ... }`
- [x] 2.3 去重与排序：按 `manager`→`name` 排序；同一 `manager+name` 仅保留一条
- [x] 2.4 补充解析单元测试（mock subprocess 输出）：uv tool list / pipx list --json
- [x] 2.5 验证：开发环境页 Python 分组能展示 uv/pipx 工具；缺失命令时静默跳过并记录 warning

## 3. OpenSpec 校验
- [x] 3.1 运行 `openspec validate update-dev-cli-scope-to-dev-environment --strict --no-interactive`
