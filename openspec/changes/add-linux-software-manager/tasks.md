## 1. 项目脚手架
- [ ] 1.1 创建 pyproject.toml（PEP 621，依赖: PyGObject>=3.46, 构建: meson-python, entry_points: softmgr）
- [ ] 1.2 创建 src/soft_management/ 包结构（__init__.py, __main__.py, app.py, window.py, models.py）
- [ ] 1.3 实现 GtkApplication 入口（AdwApplication）与主窗口（AdwApplicationWindow + AdwNavigationSplitView 三视图侧边栏）
- [ ] 1.4 实现启动前依赖检测（检查 gi + Gtk 4.12+ + Adw 1.4+，缺失时打印安装指引并退出）
- [ ] 1.5 实现日志初始化（logging 模块，INFO 默认，--debug 切 DEBUG，文件 ~/.local/state/soft-management/softmgr.log，5MB 轮转保留 3 份）
- [ ] 1.6 实现配置加载（~/.config/soft-management/config.toml，show_all_packages=false, top_n=50）
- [ ] 1.7 验证: `python3 -m soft_management` 启动显示空白主窗口，侧边栏含三个导航项

## 2. 数据模型与适配器基础架构
- [ ] 2.1 定义 models.py 全部数据模型（Package, AdapterResult[T], RuntimeInfo, VersionManagerInfo, GlobalPackageInfo, CacheInfo, CleanupSuggestion），字段与 design.md 一致
- [ ] 2.2 定义三类适配器 ABC（adapters/base.py: PackageAdapter, EnvironmentAdapter, CacheAdapter），方法签名与 design.md 一致
- [ ] 2.3 实现适配器注册表（adapters/__init__.py: 自动检测 is_available()，返回可用适配器列表）
- [ ] 2.4 实现 subprocess 执行器工具函数（前置 LC_ALL=C LANG=C，超时控制，非零退出码→warning+空结果）

## 3. 软件全景视图（software-panorama）
- [ ] 3.1 实现 AptAdapter（命令: `dpkg-query -W -f='${Package}\t${Version}\t${Installed-Size}\t${Description}\n'`，TSV 解析，超时 10s）
- [ ] 3.2 实现 SnapAdapter（命令: `snap list --color=never`，表格解析，超时 10s）
- [ ] 3.3 实现 FlatpakAdapter（命令: `flatpak list --app --columns=name,application,version,size`，表格解析，超时 10s）
- [ ] 3.4 实现 DesktopFileAdapter（扫描 4 个标准 .desktop 目录，合并逻辑: 匹配已有条目→合并 icon/desktop_file，未匹配且 Exec 指向 /opt/→manual，含 AppImage→appimage）
- [ ] 3.5 实现 DiscoveryService（ThreadPoolExecutor(4) 并行执行适配器，GLib.idle_add 回调 UI，增量加载，部分失败→AdwBanner 警告）
- [ ] 3.6 实现全景视图 UI（GtkListView 虚拟化，默认仅桌面应用，"显示全部包"切换开关，来源筛选标签，搜索栏 300ms 去抖，empty-state 占位）
- [ ] 3.7 实现详情侧边面板（点击列表项→AdwNavigationPage 展示完整信息）
- [ ] 3.8 验证: 启动应用进入全景视图，显示桌面应用列表，搜索/筛选/详情面板均可用

## 4. 开发环境管理视图（dev-environment）
- [ ] 4.1 实现运行时检测逻辑（python3/node/rustc/java/gcc/go --version，路径分析判断 install_method，超时 5s）
- [ ] 4.2 实现版本管理器检测（nvm: bash -c source+nvm list, rustup: rustup show, conda: conda env list --json, uv: uv --version）
- [ ] 4.3 实现 PipAdapter（`pip3 list --format=json`，JSON 解析）
- [ ] 4.4 实现 NpmAdapter（`npm list -g --json --depth=0`，JSON 解析）
- [ ] 4.5 实现 CargoAdapter（`cargo install --list` 文本解析，失败回退扫描 ~/.cargo/bin）
- [ ] 4.6 实现 EnvironmentService（聚合运行时+版本管理器+全局包，按语言分组）
- [ ] 4.7 实现开发环境视图 UI（AdwPreferencesGroup 按语言分组，AdwSpinner 加载态，无工具的语言不显示）
- [ ] 4.8 验证: 进入开发环境视图可看到 Python/Node/Rust/Java 分组信息

## 5. 磁盘占用分析视图（disk-analysis）
- [ ] 5.1 实现各 CacheAdapter（apt/pip/npm/conda/cargo 缓存目录递归 stat 求和，Docker 通过 docker info 检测+docker system df JSON 解析，超时 15s）
- [ ] 5.2 实现包大小排行逻辑（混合所有来源，size 降序，None 排末尾，top_n 裁剪 clamp(10,200)）
- [ ] 5.3 实现清理建议生成（CleanupSuggestion 模板: apt clean/pip3 cache purge/npm cache clean --force/conda clean --all/cargo cache --autoclean/docker system prune，标注 requires_sudo 和 risk_level）
- [ ] 5.4 实现磁盘分析视图 UI（GtkScrolledWindow: 顶部 AdwActionRow+GtkLevelBar 缓存概览，中部 GtkListView 排行，底部清理建议+执行按钮）
- [ ] 5.5 实现清理执行流程（点击→AdwMessageDialog 确认→pkexec 提权（如需）→执行→成功刷新/失败提示→按钮状态恢复）
- [ ] 5.6 验证: 进入磁盘分析视图可看到缓存占用、包排行、清理建议，点击清理可执行

## 6. 测试与 PBT
- [ ] 6.1 编写适配器解析单元测试（mock subprocess 输出: dpkg TSV, snap 表格, flatpak 表格, pip JSON, npm JSON, cargo text）
- [ ] 6.2 编写 PBT 核心不变量测试（使用 hypothesis 库）:
  - canonical_id 可逆性（含 `:` 的包名如 libc6:amd64）
  - 去重幂等性（merge_desktop 连续执行两次结果不变）
  - canonical_id 唯一性（最终列表无重复 ID）
  - 适配器执行顺序无关性（打乱顺序结果集合相同）
  - 排行单调性（size 非递增，None 在末尾）
  - top_n 边界（clamp 到 [10,200]，输出长度 <= top_n）
  - size 非负（size is None or size >= 0）
  - 过滤代数（组合过滤 = 分步求交）
  - 搜索大小写不敏感
  - 部分失败隔离（单适配器失败不影响其他结果）
- [ ] 6.3 编写 CleanupSuggestion 安全性测试（command 白名单验证，拒绝注入）

## 7. 集成与分发
- [ ] 7.1 配置 ruff linter + pyproject.toml [tool.ruff]
- [ ] 7.2 实现 i18n 基础设施（gettext, po/ 目录, 中文翻译文件）
- [ ] 7.3 创建 data/ 目录（.desktop 文件, 应用图标, gschema）
- [ ] 7.4 验证: `pipx install .` 可安装并通过 `softmgr` 命令启动完整应用
