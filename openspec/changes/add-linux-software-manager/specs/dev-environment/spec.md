## ADDED Requirements

### Requirement: Runtime Detection
系统 SHALL 检测当前系统上已安装的开发语言运行时（Python、Node.js、Rust、Java、GCC/G++、Go），通过执行对应版本命令（如 `python3 --version`、`node --version`）获取信息。所有命令 SHALL 前置 `LC_ALL=C LANG=C`。对于每个检测到的运行时，系统 SHALL 显示版本号、可执行文件路径、安装方式（系统包/版本管理器/手动安装）。命令超时阈值为 5s，超时或非零退出码时记录 warning 并跳过该运行时。

#### Scenario: Detect Python installed via system package
- **GIVEN** 系统通过 apt 安装了 Python 3.12.3，路径为 /usr/bin/python3
- **WHEN** 系统执行 `LC_ALL=C python3 --version` 返回 "Python 3.12.3"
- **THEN** 创建 RuntimeInfo(language="python", version="3.12.3", path="/usr/bin/python3", install_method="apt")

#### Scenario: Detect Node.js managed by nvm
- **GIVEN** 用户通过 nvm 安装了 Node.js v22.21.1，路径为 ~/.nvm/versions/node/v22.21.1/bin/node
- **WHEN** 系统执行 `node --version` 返回 "v22.21.1"，且路径包含 ".nvm/"
- **THEN** 创建 RuntimeInfo(language="node", version="22.21.1", path="~/.nvm/versions/node/v22.21.1/bin/node", install_method="nvm")

#### Scenario: Runtime not installed
- **GIVEN** 系统未安装 Go（`go version` 返回非零退出码）
- **WHEN** 系统执行运行时检测
- **THEN** Go 不出现在运行时列表中，warning 记录到日志

#### Scenario: Runtime command timeout
- **GIVEN** `java --version` 因 JVM 启动缓慢超过 5s
- **WHEN** 系统执行运行时检测
- **THEN** Java 运行时标记为 warning 并跳过，其他运行时正常展示

### Requirement: Version Manager Status
系统 SHALL 检测已安装的版本管理器（nvm、rustup、conda、uv），通过对应命令获取管理的版本列表及当前激活版本。nvm 为 shell function，SHALL 通过 `bash -c 'source "$HOME/.nvm/nvm.sh" 2>/dev/null && nvm list --no-colors'` 调用。conda SHALL 通过 `conda env list --json` 获取环境列表。未安装的版本管理器 SHALL 静默跳过。

#### Scenario: Show nvm managed versions
- **GIVEN** 用户通过 nvm 安装了 Node.js v22.21.1（当前激活）
- **WHEN** 系统执行 `bash -c 'source "$HOME/.nvm/nvm.sh" && nvm list --no-colors'`
- **THEN** 创建 VersionManagerInfo(name="nvm", managed_versions=[{"version": "22.21.1", "active": true}], path="~/.nvm")

#### Scenario: Show conda environments
- **GIVEN** 用户安装了 Anaconda3，`conda env list --json` 返回 base 环境
- **WHEN** 系统检测 conda 状态
- **THEN** 显示 conda 环境列表，base 环境标记为当前激活，并通过 `conda list -n base --json` 获取包数量

#### Scenario: Version manager not installed
- **GIVEN** 系统未安装 pyenv（`pyenv --version` 返回非零退出码）
- **WHEN** 系统执行版本管理器检测
- **THEN** pyenv 不出现在版本管理器列表中

#### Scenario: nvm.sh not found
- **GIVEN** 用户的 ~/.nvm/nvm.sh 文件不存在
- **WHEN** 系统尝试 source nvm.sh
- **THEN** nvm 检测静默失败，不出现在版本管理器列表中，warning 记录到日志

### Requirement: Language Package Inventory
系统 SHALL 列出各语言包管理器的全局已安装包：pip3 通过 `pip3 list --format=json`（JSON 输出），npm 通过 `npm list -g --json --depth=0`（JSON 输出），cargo 通过 `cargo install --list`（文本解析）。每个包显示名称和版本。cargo 解析失败时回退为仅显示 ~/.cargo/bin 下的二进制名称，版本标记为"unknown"。

#### Scenario: List pip global packages
- **GIVEN** `pip3 list --format=json` 返回 157 个包的 JSON 数组
- **WHEN** 用户查看 Python 全局包列表
- **THEN** 显示所有 157 个 GlobalPackageInfo(manager="pip3", name=..., version=...)，按名称排序

#### Scenario: List cargo installed binaries
- **GIVEN** `cargo install --list` 返回 14 个 crate 及版本
- **WHEN** 用户查看 Rust 全局包列表
- **THEN** 解析每行 "crate_name v0.1.0:" 格式，创建 14 个 GlobalPackageInfo(manager="cargo", ...)

#### Scenario: Cargo install list fails
- **GIVEN** `cargo install --list` 返回非零退出码
- **WHEN** 用户查看 Rust 全局包列表
- **THEN** 回退扫描 ~/.cargo/bin 目录，列出二进制文件名，版本标记为"unknown"

#### Scenario: npm not installed
- **GIVEN** 系统未安装 npm
- **WHEN** 用户查看开发环境视图
- **THEN** Node.js 全局包列表为空，不显示错误

### Requirement: Dev Environment View Layout
系统 SHALL 在开发环境视图中按语言分组展示信息，每个语言组使用 AdwExpanderRow 或 AdwPreferencesGroup 包含：运行时信息（版本、路径、安装方式）、版本管理器状态（如有，含管理的版本列表）、全局包列表。视图采用懒加载，进入时触发扫描，扫描期间显示 AdwSpinner。

#### Scenario: Python group display
- **GIVEN** 系统安装了 Python 3.12.3（apt）、conda（管理 base 环境）、uv、pip3（157 个全局包）
- **WHEN** 用户进入开发环境视图查看 Python 分组
- **THEN** 依次显示：运行时版本（3.12.3, /usr/bin/python3, apt）、版本管理器（conda: base 环境; uv: 已安装）、全局包列表（157 个 pip 包）

#### Scenario: Language with no tools installed
- **GIVEN** 系统未安装 Go 运行时、无 Go 版本管理器、无 Go 全局包
- **WHEN** 用户进入开发环境视图
- **THEN** Go 分组不出现在视图中

#### Scenario: Loading state
- **GIVEN** 用户首次进入开发环境视图
- **WHEN** 适配器正在后台扫描
- **THEN** 视图显示 AdwSpinner，已完成的语言分组立即渲染
