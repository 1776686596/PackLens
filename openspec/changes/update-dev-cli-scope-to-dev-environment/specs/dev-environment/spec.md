## MODIFIED Requirements

### Requirement: Language Package Inventory
系统 SHALL 列出各语言包管理器/工具管理器的全局已安装包：
- pip 通过 `pip3 list --format=json`（serde_json 解析）
- npm 通过 `npm list -g --json --depth=0`（serde_json 解析）
- cargo 通过 `cargo install --list`（文本解析）
- uv tools 通过 `uv tool list`（文本解析）
- pipx apps 通过 `pipx list --json`（serde_json 解析）

每个包 SHALL 显示名称和版本（若无法解析版本则为空字符串或 "unknown"）。未安装的包管理器/工具管理器 SHALL 静默跳过；命令执行失败或解析失败时 SHALL 记录 warning 并跳过对应结果，不影响其他语言的展示。

#### Scenario: List uv tools
- **GIVEN** 系统安装了 uv，`uv tool list` 输出包含 "ruff v0.6.0"
- **WHEN** 系统执行 Python 全局工具清单采集
- **THEN** 创建 GlobalPackageInfo { manager: "uv", name: "ruff", version: "0.6.0" }

#### Scenario: List pipx apps
- **GIVEN** 系统安装了 pipx，`pipx list --json` 返回包含 "black" 的 JSON 结果（版本为 "24.2.0"）
- **WHEN** 系统执行 Python 全局工具清单采集
- **THEN** 创建 GlobalPackageInfo { manager: "pipx", name: "black", version: "24.2.0" }

#### Scenario: pipx not installed
- **GIVEN** 系统未安装 pipx
- **WHEN** 系统执行 Python 全局工具清单采集
- **THEN** pipx apps 列表为空且不显示错误信息

