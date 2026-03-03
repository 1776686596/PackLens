## MODIFIED Requirements

### Requirement: Software Discovery
系统 SHALL 在用户进入软件全景视图时，自动检测当前 Linux 系统上可用的包管理器（apt/dpkg、snap、flatpak），并通过对应适配器收集已安装软件列表。对于未安装的包管理器，系统 SHALL 静默跳过而不报错。每个适配器 SHALL 通过 tokio::spawn 在独立异步任务中执行，系统包适配器超时阈值为 10s（tokio::time::timeout），超时后标记为 warning 并跳过。所有子进程调用 SHALL 通过 tokio::process::Command 执行，前置 `.env("LC_ALL", "C").env("LANG", "C")` 以避免 locale 影响输出解析。

#### Scenario: Ubuntu system with apt, snap, and flatpak
- **GIVEN** 系统安装了 apt、snap、flatpak 三个包管理器
- **WHEN** 用户进入软件全景视图触发软件发现
- **THEN** 系统通过 tokio::spawn 并行执行三个适配器，通过 glib::spawn_future_local 将结果回传 GTK 主线程，聚合结果列表，每个条目包含 canonical_id、名称、版本、来源标识、安装大小

#### Scenario: System missing flatpak
- **GIVEN** 系统仅安装了 apt 和 snap，未安装 flatpak
- **WHEN** 用户进入软件全景视图触发软件发现
- **THEN** FlatpakAdapter.is_available() 返回 false，系统仅聚合 apt 和 snap 的结果，不显示错误信息

#### Scenario: Adapter timeout
- **GIVEN** snap 命令因网络问题响应缓慢
- **WHEN** SnapAdapter 执行超过 10s 超时阈值（tokio::time::timeout）
- **THEN** 该适配器被取消，UI 顶部显示 AdwBanner 警告"snap 扫描超时，部分结果可能缺失"，已完成的 apt/flatpak 结果正常展示

#### Scenario: Adapter command returns non-zero exit code
- **GIVEN** dpkg-query 命令因数据库锁定返回非零退出码
- **WHEN** AptAdapter 执行该命令
- **THEN** 适配器记录 warning 到 AdapterResult.warnings，返回空 items Vec，不 panic，UI 显示警告 banner

#### Scenario: Loading state during scan
- **GIVEN** 用户首次进入软件全景视图
- **WHEN** 适配器正在后台扫描
- **THEN** 视图显示 AdwSpinner 加载指示器，已完成的适配器结果立即渲染到列表中（增量加载）

### Requirement: Desktop Application Detection
系统 SHALL 扫描标准 .desktop 文件目录（/usr/share/applications/、~/.local/share/applications/、/var/lib/flatpak/exports/share/applications/、/var/lib/snapd/desktop/applications/）以发现桌面应用。DesktopFileAdapter 为补充源：如果 .desktop 文件的 Exec 路径或包名匹配已有 apt/snap/flatpak 条目，SHALL 将 icon_name 和 desktop_file 字段合并到已有条目；仅当 Exec 路径指向 /opt/ 或包含 AppImage 关键字时，才作为"手动安装"或"AppImage"独立条目。不主动扫描文件系统中无 .desktop 文件的 AppImage。.desktop 文件解析使用 Rust 标准库 std::fs::read_to_string + 文本解析。

#### Scenario: Merge desktop info into apt package
- **GIVEN** apt 已发现包 "google-chrome-stable"，且 /usr/share/applications/google-chrome.desktop 存在
- **WHEN** DesktopFileAdapter 扫描到该 .desktop 文件
- **THEN** 将 .desktop 中的 Icon 和文件路径合并到已有 apt 条目的 icon_name 和 desktop_file 字段，不创建重复条目

#### Scenario: Detect manually installed app in /opt
- **GIVEN** 用户手动安装了应用到 /opt/MyApp/，且 ~/.local/share/applications/ 下存在对应 .desktop 文件，Exec 路径指向 /opt/MyApp/myapp
- **WHEN** DesktopFileAdapter 扫描且该 .desktop 未匹配任何 apt/snap/flatpak 条目
- **THEN** 创建独立条目，canonical_id 为 "manual:myapp"，来源标识为"手动安装"，显示 .desktop 中的 Name 和 Icon

#### Scenario: AppImage with desktop entry
- **GIVEN** 用户的 .desktop 文件 Exec 字段包含 ".AppImage" 关键字
- **WHEN** DesktopFileAdapter 扫描到该文件且未匹配已有条目
- **THEN** 创建独立条目，来源标识为"AppImage"

#### Scenario: AppImage without desktop entry
- **GIVEN** 用户在 ~/Downloads/ 存放了 .AppImage 文件但无对应 .desktop 文件
- **WHEN** 系统执行软件发现
- **THEN** 该 AppImage 不出现在软件列表中（仅通过 .desktop 入口发现）

### Requirement: Unified Software List View
系统 SHALL 在全景视图中以 gtk4::ListView（虚拟化渲染，通过 gtk4::SignalListItemFactory 或 gtk4::BuilderListItemFactory）展示所有已发现的桌面应用（默认），支持"显示全部包"切换开关。列表 SHALL 显示软件名称、版本、来源、安装大小。支持按来源筛选（与搜索为 AND 关系）和按关键词搜索（大小写不敏感，匹配 name + description，输入去抖 300ms 通过 glib::timeout_add_local 实现）。

#### Scenario: Default view shows desktop apps only
- **GIVEN** 系统安装了 2680 个 apt 包，其中约 200 个有 .desktop 文件
- **WHEN** 用户进入全景视图（show_all_packages=false）
- **THEN** 列表仅显示有 desktop_file 字段的条目（约 200 个）

#### Scenario: Toggle to show all packages
- **GIVEN** 用户在全景视图中
- **WHEN** 用户开启"显示全部包"切换开关
- **THEN** 列表展示所有 2680+ 个包，使用 gtk4::ListView 虚拟化渲染保证滚动流畅

#### Scenario: Filter by source
- **GIVEN** 全景视图已加载所有软件
- **WHEN** 用户点击"snap"筛选标签
- **THEN** 列表仅显示 source="snap" 的软件

#### Scenario: Search by keyword
- **GIVEN** 全景视图已加载所有软件
- **WHEN** 用户在搜索栏输入"chrome"
- **THEN** 300ms 去抖后列表实时过滤，仅显示 name 或 description 中包含"chrome"（大小写不敏感）的条目

#### Scenario: Combined search and filter
- **GIVEN** 用户已选择"apt"来源筛选
- **WHEN** 用户在搜索栏输入"python"
- **THEN** 列表仅显示 source="apt" AND (name 或 description 包含"python") 的条目

#### Scenario: Empty search result
- **GIVEN** 全景视图已加载所有软件
- **WHEN** 用户搜索"xyznonexistent"
- **THEN** 列表为空，显示 empty-state 占位文案"未找到匹配的软件"

### Requirement: Software Detail Panel
系统 SHALL 在用户点击软件列表中的某一项时，在右侧展开 adw::NavigationPage 详情面板，显示该软件的完整信息：名称、版本、来源、安装大小、描述、.desktop 文件路径（如有）。

#### Scenario: Click to open detail panel
- **GIVEN** 全景视图列表中显示了 Firefox (snap)
- **WHEN** 用户点击 Firefox 条目
- **THEN** 右侧展开详情面板，显示名称"Firefox"、版本、来源"snap"、安装大小、描述

#### Scenario: Package without desktop file
- **GIVEN** 用户开启"显示全部包"，列表中显示 libssl3 (apt)
- **WHEN** 用户点击 libssl3 条目
- **THEN** 详情面板显示可用信息（名称、版本、来源、大小），desktop_file 字段不显示
