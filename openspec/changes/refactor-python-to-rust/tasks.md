## 1. 清理与初始化
- [x] 1.1 删除全部 Python 源码（src/soft_management/*.py）和 pyproject.toml
- [x] 1.2 创建 Cargo.toml（完整依赖清单见 design.md，edition=2021，name="soft-management"，[[bin]] name="softmgr"）
- [x] 1.3 创建 rust-toolchain.toml（channel = "1.80"）
- [x] 1.4 创建 src/main.rs 入口：创建全局 tokio Runtime 单例，启动 AdwApplication
- [x] 1.5 创建 src/runtime.rs：全局 tokio Runtime（once_cell::sync::Lazy<Runtime>），提供 spawn 辅助函数
- [x] 1.6 创建 src/error.rs：AppError / AdapterError / ConfigError（thiserror 派生，字段与 design.md 一致）
- [x] 1.7 创建 src/app.rs：AdwApplication（ObjectSubclass 派生宏），注册 quit action + Ctrl+Q 快捷键
- [x] 1.8 创建 src/config.rs：serde + toml 加载 ~/.config/soft-management/config.toml，缺失/解析失败→默认值+warn!
- [x] 1.9 创建 src/subprocess.rs：run_command(cmd, args, timeout_secs) 工具函数，前置 LC_ALL=C LANG=C，String::from_utf8_lossy 输出，超时→AdapterError::Timeout，非零退出→AdapterError::CommandFailed
- [x] 1.10 实现日志初始化（tracing + tracing-subscriber + tracing-appender，INFO 默认，--debug 切 DEBUG，文件 ~/.local/state/soft-management/softmgr.log）
- [x] 1.11 验证: `cargo run` 启动显示空白主窗口

## 2. 主窗口与 tokio-glib 桥接基建
- [x] 2.1 创建 src/window.rs：AdwApplicationWindow（ObjectSubclass），AdwNavigationSplitView 三视图侧边栏 + 响应式断点（600px 折叠）
- [x] 2.2 实现 tokio-glib 桥接最小验证：async-channel bounded(32) + glib::spawn_future_local 接收 + tokio::spawn 发送，验证跨运行时数据传递正常
- [x] 2.3 实现 CancellationToken 基础设施：视图切换时 cancel 旧 token，创建新 token，接收端检查 token 状态丢弃过期结果
- [x] 2.4 验证: `cargo run` 启动显示主窗口，侧边栏含三个导航项，点击可切换视图（占位内容）

## 3. 数据模型与适配器基础架构
- [x] 3.1 创建 src/models.rs：全部数据模型（Package, AdapterResult<T>, RuntimeInfo, VersionManagerInfo, ManagedVersion, GlobalPackageInfo, CacheInfo, CleanupSuggestion, RiskLevel），字段与 design.md 一致
- [x] 3.2 创建 src/adapters/mod.rs：三类适配器 trait（PackageAdapter, EnvironmentAdapter, CacheAdapter），原生 async fn in trait（若 Send 约束不满足则回退 async-trait）
- [x] 3.3 实现适配器注册表（自动检测 is_available()，返回 Vec<Box<dyn PackageAdapter>> 等）
- [x] 3.4 实现 canonical_id 生成与反解析函数（format!("{source}:{name}") + splitn(2, ':')），含构造时断言校验

## 4. 软件全景视图（software-panorama）
- [x] 4.1 实现 AptAdapter（dpkg-query TSV 解析，超时 10s）
- [x] 4.2 实现 SnapAdapter（snap list 表格解析，超时 10s）
- [x] 4.3 实现 FlatpakAdapter（flatpak list 表格解析，超时 10s）
- [x] 4.4 实现 DesktopFileAdapter（freedesktop-desktop-entry crate 解析 4 个 .desktop 目录，合并逻辑，/opt/ 和 AppImage 独立条目）
- [x] 4.5 实现 DiscoveryService（tokio::spawn 并行适配器 + async-channel 发送结果 + glib::spawn_future_local 接收更新 UI，CancellationToken 取消，部分失败→AdwBanner）
- [x] 4.6 实现全景视图 UI src/ui/panorama.rs（gtk4::ListView + BoxedAnyObject + SignalListItemFactory，默认桌面应用，显示全部包开关，来源筛选，搜索 300ms 去抖 glib::timeout_add_local，empty-state）
- [x] 4.7 实现详情侧边面板（adw::NavigationPage 展示完整信息）
- [ ] 4.8 验证: 启动应用进入全景视图，显示桌面应用列表，搜索/筛选/详情面板均可用

## 5. 开发环境管理视图（dev-environment）
- [x] 5.1 实现运行时检测（python3/node/rustc/java/gcc/go --version，install_method 判定表见 design.md，超时 5s）
- [x] 5.2 实现版本管理器检测（nvm: bash -c source+nvm list, rustup show, conda env list --json serde_json 解析, uv --version）
- [x] 5.3 实现 PipAdapter（pip3 list --format=json，serde_json 解析）
- [x] 5.4 实现 NpmAdapter（npm list -g --json --depth=0，serde_json 解析）
- [x] 5.5 实现 CargoAdapter（cargo install --list 文本解析，失败回退 std::fs::read_dir ~/.cargo/bin）
- [x] 5.6 实现 EnvironmentService（聚合运行时+版本管理器+全局包，按语言分组，async-channel + glib 桥接）
- [x] 5.7 实现开发环境视图 UI src/ui/devenv.rs（adw::PreferencesGroup 按语言分组，adw::Spinner 加载态）
- [ ] 5.8 验证: 进入开发环境视图可看到 Python/Node/Rust/Java 分组信息

## 6. 磁盘占用分析视图（disk-analysis）
- [x] 6.1 实现各 CacheAdapter（walkdir 递归 stat 求和 apt/pip/npm/conda/cargo 缓存目录，Docker 通过 docker system df serde_json 解析，超时 15s）
- [x] 6.2 实现包大小排行逻辑（混合所有来源，Vec::sort_by size 降序，None 排末尾，top_n clamp(10,200)）
- [x] 6.3 实现清理建议生成（CleanupSuggestion 模板，白名单校验 design.md 中 6 条命令，标注 requires_sudo 和 risk_level）
- [x] 6.4 实现磁盘分析视图 UI src/ui/disk.rs（gtk4::ScrolledWindow: 顶部 adw::ActionRow+gtk4::LevelBar，中部 gtk4::ListView 排行，底部清理建议+执行按钮）
- [ ] 6.5 实现清理执行流程（adw::AlertDialog 确认→std::process::Command pkexec 提权→执行→成功刷新/失败提示→按钮状态恢复）
- [ ] 6.6 验证: 进入磁盘分析视图可看到缓存占用、包排行、清理建议，点击清理可执行

## 7. 测试
- [ ] 7.1 编写适配器解析单元测试 tests/adapter_parsing.rs（mock subprocess 输出: dpkg TSV, snap 表格, flatpak 表格, pip JSON, npm JSON, cargo text）
- [ ] 7.2 编写 proptest 核心不变量测试 tests/proptest_invariants.rs:
  - canonical_id 可逆性（含 `:` 的包名如 libc6:amd64）
  - canonical_id 格式一致性（匹配 ^[a-z]+:.+$）
  - 去重幂等性（merge_desktop 连续执行两次结果不变）
  - 去重字段优先级（合并后 name/version/size 来自包管理器）
  - canonical_id 唯一性（最终列表无重复 ID）
  - 适配器执行顺序无关性（打乱顺序结果集合相同）
  - 排行单调性（size 非递增，None 在末尾）
  - top_n 边界（clamp 到 [10,200]，输出长度 <= top_n）
  - 过滤代数（组合过滤 = 分步求交）
  - 搜索大小写不敏感
  - 部分失败隔离（单适配器失败不影响其他结果）
- [x] 7.3 编写 CleanupSuggestion 安全性测试（command 白名单验证，拒绝非白名单命令）

## 8. 集成与分发
- [x] 8.1 配置 clippy lint 规则（Cargo.toml [lints] 或 .clippy.toml）
- [ ] 8.2 实现 i18n 基础设施（gettext-rs, po/ 目录, 中文翻译文件）
- [x] 8.3 创建 data/ 目录（.desktop 文件, 应用图标 SVG）
- [x] 8.4 更新 openspec/project.md 技术栈描述（Python→Rust）
- [ ] 8.5 验证: `cargo install --path .` 可安装并通过 `softmgr` 命令启动完整应用
- [ ] 8.6 运行 `openspec validate refactor-python-to-rust --strict --no-interactive` 确认规范通过
