## ADDED Requirements

### Requirement: Cache Size Analysis
系统 SHALL 计算各包管理器缓存目录的磁盘占用大小。缓存目录清单：apt 缓存（/var/cache/apt/archives/）、pip 缓存（~/.cache/pip/）、npm 缓存（~/.npm/）、conda 包缓存（~/anaconda3/pkgs/）、cargo 注册表缓存（~/.cargo/registry/）。Docker 占用通过 `docker system df --format='{{json .}}'` 获取（需先通过 `docker info` 验证 daemon 可用）。对于不存在的缓存目录，系统 SHALL 静默跳过。大小计算使用目录递归 stat 求和（非 du 命令），单位为 bytes，UI 显示为 IEC 格式（KiB/MiB/GiB）。

#### Scenario: Calculate apt cache size
- **GIVEN** /var/cache/apt/archives/ 目录存在且包含缓存的 .deb 文件
- **WHEN** 系统执行缓存大小分析
- **THEN** 创建 CacheInfo(name="apt cache", path="/var/cache/apt/archives/", size=<bytes>, requires_sudo=true)，UI 显示人类可读格式如"1.2 GiB"

#### Scenario: Cache directory does not exist
- **GIVEN** 用户未安装 conda，~/anaconda3/pkgs/ 目录不存在
- **WHEN** 系统执行缓存大小分析
- **THEN** conda 缓存项不出现在分析结果中，不显示错误

#### Scenario: Docker disk usage with running daemon
- **GIVEN** Docker daemon 正在运行，`docker info` 返回成功
- **WHEN** 系统执行 `LC_ALL=C docker system df --format='{{json .}}'`
- **THEN** 解析 JSON 输出，分别显示 Images 和 Containers 的占用大小

#### Scenario: Docker daemon not running
- **GIVEN** Docker 已安装但 daemon 未运行，`docker info` 返回非零退出码
- **WHEN** 系统执行缓存大小分析
- **THEN** Docker 缓存项不出现在结果中，UI 显示 AdwBanner 提示"Docker daemon 未运行，无法获取 Docker 磁盘占用"

#### Scenario: Permission denied on apt cache
- **GIVEN** /var/cache/apt/archives/ 目录存在但部分文件无读取权限
- **WHEN** 系统计算 apt 缓存大小
- **THEN** 统计可读文件的大小总和，CacheInfo.requires_sudo 标记为 true，warning 记录权限受限信息

### Requirement: Package Size Ranking
系统 SHALL 按磁盘占用大小降序排列已安装的软件包，展示占用空间最大的 Top N 个包（默认 N=50，可通过 ~/.config/packlens/config.toml 的 top_n 配置，范围 10-200）。排行榜混合所有来源（apt/snap/flatpak），按 Package.size 统一排序。size 为 None 的包排在末尾。

#### Scenario: Show top 50 largest packages
- **GIVEN** 系统安装了 2680 个 apt 包，配置 top_n=50
- **WHEN** 用户查看包大小排行
- **THEN** 显示 size 最大的 50 个包，每个包显示名称、大小（IEC 格式）、来源

#### Scenario: Cross-source ranking
- **GIVEN** 系统同时有 apt 包（size 已知）、snap 包（size 已知）和 flatpak 应用（size 已知）
- **WHEN** 用户查看包大小排行
- **THEN** 排行榜混合所有来源的包，按 size 降序统一排序

#### Scenario: Package with unknown size
- **GIVEN** 某些包的 size 为 None（包管理器未报告大小）
- **WHEN** 用户查看包大小排行
- **THEN** size 为 None 的包排在列表末尾，大小列显示"--"

#### Scenario: Custom top_n configuration
- **GIVEN** 用户在 config.toml 中设置 top_n=100
- **WHEN** 用户查看包大小排行
- **THEN** 显示 100 个最大的包

### Requirement: Cleanup Suggestions
系统 SHALL 生成可操作的清理建议列表，每条建议为 CleanupSuggestion(description, estimated_bytes, command, requires_sudo, risk_level)。清理操作 SHALL 在用户点击"执行清理"按钮后弹出 AdwMessageDialog 确认对话框（显示将执行的命令、预估回收空间、是否需要 sudo），用户确认后才执行。需要 sudo 的操作通过 pkexec 提权。每次清理操作独立确认，不提供会话级免确认。清理成功后自动重新扫描受影响的缓存项。

#### Scenario: Suggest apt cache cleanup
- **GIVEN** apt 缓存占用 1.2 GiB
- **WHEN** 系统生成清理建议
- **THEN** 建议列表包含 CleanupSuggestion(description="清理 apt 下载缓存", estimated_bytes=1288490188, command="apt clean", requires_sudo=true, risk_level="safe")

#### Scenario: Suggest pip cache cleanup
- **GIVEN** ~/.cache/pip/ 占用 500 MiB
- **WHEN** 系统生成清理建议
- **THEN** 建议列表包含 CleanupSuggestion(description="清理 pip 下载缓存", estimated_bytes=524288000, command="pip3 cache purge", requires_sudo=false, risk_level="safe")

#### Scenario: User confirms cleanup
- **GIVEN** 用户查看清理建议列表
- **WHEN** 用户点击"清理 apt 下载缓存"的"执行清理"按钮
- **THEN** 弹出 AdwMessageDialog 显示：命令 "apt clean"、预估回收 "1.2 GiB"、标注"需要管理员权限"，包含"取消"和"确认执行"按钮

#### Scenario: Cleanup execution with sudo
- **GIVEN** 用户在确认对话框中点击"确认执行"，该操作 requires_sudo=true
- **WHEN** 系统执行清理
- **THEN** 通过 pkexec 执行 `apt clean`，成功后自动重新扫描 apt 缓存大小并更新 UI

#### Scenario: Cleanup execution fails
- **GIVEN** 用户确认执行清理，但 pkexec 被用户取消或命令执行失败
- **WHEN** 清理命令返回非零退出码
- **THEN** 显示 AdwMessageDialog 错误提示"清理失败: <错误信息>"，缓存大小不变，"执行清理"按钮恢复可点击状态

#### Scenario: Button disabled during execution
- **GIVEN** 用户确认执行某条清理建议
- **WHEN** 清理命令正在执行中
- **THEN** 该建议的"执行清理"按钮显示为 AdwSpinner + "执行中..."，不可重复点击

### Requirement: Disk Analysis View Layout
系统 SHALL 在磁盘分析视图中展示三个区域（使用 GtkScrolledWindow 垂直滚动）：顶部为缓存占用概览（每个缓存项使用 AdwActionRow + GtkLevelBar 进度条显示占用大小），中部为最大包排行列表（GtkListView），底部为清理建议列表（每条建议含描述、预估空间、"执行清理"按钮）。视图采用懒加载，进入时触发扫描。

#### Scenario: View layout on first load
- **GIVEN** 用户切换到磁盘分析视图
- **WHEN** 扫描完成后视图加载
- **THEN** 顶部显示各缓存占用的 LevelBar 可视化，中部显示最大包排行，底部显示清理建议及操作按钮

#### Scenario: Loading state
- **GIVEN** 用户首次进入磁盘分析视图
- **WHEN** 缓存适配器正在后台扫描
- **THEN** 视图显示 AdwSpinner 加载指示器，已完成的缓存项立即渲染

#### Scenario: Manual refresh
- **GIVEN** 用户已查看磁盘分析视图
- **WHEN** 用户点击刷新按钮
- **THEN** 清除内存缓存，重新执行所有缓存适配器扫描，更新 UI
