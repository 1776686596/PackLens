## Context
用户运行 Ubuntu 24.04 GNOME Wayland 环境，系统存在 4 个系统级包管理器（apt/dpkg、snap、flatpak、手动 /opt 安装）、7 个语言级包管理器（pip/uv/conda/npm/yarn/pnpm/cargo）、Docker 容器生态，以及 nvm/rustup/conda 等版本管理器。软件安装路径分散在 /usr/bin、~/.local/bin、~/.nvm/、~/.cargo/bin、~/anaconda3/、~/.bun/bin、/opt/、/snap/bin 等多个位置。磁盘已用 81%（剩余 86GB），清理需求迫切。

## Goals / Non-Goals
- Goals:
  - 提供统一的软件全景视图，聚合所有安装来源
  - 可视化开发环境状态（运行时版本、虚拟环境、全局包）
  - 分析磁盘占用并提供可操作的清理建议
  - 原生 GNOME 体验（GTK4/libadwaita，支持暗色模式、自适应布局）
  - 通用 Linux 支持（适配器自动检测，缺失的包管理器静默跳过）
- Non-Goals:
  - 不替代各包管理器本身（不做 apt install 的 GUI 封装）
  - 不做跨平台（不支持 macOS/Windows）
  - 第一版不做自动更新/批量升级功能
  - 不做软件商店/应用发现功能
  - 第一版不做系统托盘常驻
  - 第一版不做崩溃遥测

## Decisions

### 技术栈: Python + GTK4/libadwaita
- **决策**: 使用 Python 3.10+ 作为开发语言，GTK4 + libadwaita 作为 GUI 框架
- **理由**: 本工具核心价值是系统集成（调用 10+ 种 CLI 工具并解析输出），Python 的 subprocess 生态最适合此场景；GTK4/libadwaita 在 GNOME 桌面提供原生体验
- **版本锁定**: GTK4 >= 4.12, libadwaita >= 1.4, PyGObject >= 3.46（Ubuntu 24.04 LTS 自带版本）
- **兼容矩阵**: Ubuntu 24.04+, Fedora 39+, Arch Linux (rolling)
- **依赖不满足时**: 启动前检测 gi 模块可用性，缺失时打印安装指引并退出（非崩溃）

### 架构: 适配器模式（三类接口）
- **决策**: 拆分为三类适配器接口，由对应 Service 聚合
- **PackageAdapter**: `is_available() -> bool`, `list_packages() -> AdapterResult[Package]`
  - 实现: AptAdapter, SnapAdapter, FlatpakAdapter, DesktopFileAdapter
- **EnvironmentAdapter**: `detect_runtimes() -> list[RuntimeInfo]`, `detect_version_managers() -> list[VersionManagerInfo]`, `list_global_packages() -> list[GlobalPackageInfo]`
  - 实现: PythonEnvAdapter, NodeEnvAdapter, RustEnvAdapter, JavaEnvAdapter
- **CacheAdapter**: `list_caches() -> list[CacheInfo]`, `suggest_cleanups() -> list[CleanupSuggestion]`
  - 实现: AptCacheAdapter, PipCacheAdapter, NpmCacheAdapter, CondaCacheAdapter, CargoCacheAdapter, DockerCacheAdapter

### 数据模型
```python
@dataclass
class Package:
    canonical_id: str      # f"{source}:{name}", e.g. "apt:vim", "snap:firefox"
    name: str              # display name
    version: str           # version string
    source: str            # "apt" | "snap" | "flatpak" | "manual" | "appimage"
    size: int | None       # installed size in bytes, None if unavailable
    description: str       # short description
    icon_name: str | None  # icon name from .desktop or None
    desktop_file: str | None  # path to .desktop file if exists

@dataclass
class AdapterResult[T]:
    items: list[T]
    warnings: list[str]    # non-fatal issues
    duration_ms: int       # scan duration
    timestamp: float       # time.time() of scan completion

@dataclass
class RuntimeInfo:
    language: str          # "python" | "node" | "rust" | "java" | "gcc" | "go"
    version: str
    path: str              # executable path
    install_method: str    # "apt" | "nvm" | "rustup" | "manual" | ...

@dataclass
class VersionManagerInfo:
    name: str              # "nvm" | "rustup" | "conda" | "uv"
    managed_versions: list[dict]  # [{version, active: bool}]
    path: str              # install location

@dataclass
class GlobalPackageInfo:
    manager: str           # "pip3" | "npm" | "cargo"
    name: str
    version: str

@dataclass
class CacheInfo:
    name: str              # "apt cache" | "pip cache" | ...
    path: str              # directory path
    size: int              # bytes
    requires_sudo: bool

@dataclass
class CleanupSuggestion:
    description: str       # human-readable description
    estimated_bytes: int   # estimated reclaimable space
    command: str           # exact command to execute
    requires_sudo: bool
    risk_level: str        # "safe" | "moderate"
```

### 分发: PyPI + pipx
- **决策**: 主要通过 `pipx install soft-management` 分发，辅以 .deb 打包
- **理由**: 工具需要访问宿主系统的包管理器数据库，Flatpak 沙箱会阻断这些访问
- **可执行文件名**: `softmgr`

### UI 结构: 侧边栏导航 + 三视图 + 详情面板
- **决策**: AdwNavigationSplitView 侧边栏，右侧内容区含三个视图，点击列表项展开详情侧边面板
- **默认视图**: 启动时显示"软件全景"视图，不持久化用户上次视图状态
- **软件全景默认范围**: 仅显示有 .desktop 文件的桌面应用，提供"显示全部包"切换开关
- **点击行为**: 点击列表项在右侧展开 AdwNavigationPage 详情面板（名称、版本、来源、大小、描述、依赖列表、文件路径）
- **搜索**: 大小写不敏感，匹配 name + description 字段，与来源筛选为 AND 关系，输入去抖 300ms
- **AppImage**: 仅通过 .desktop 文件发现，不主动扫描文件系统中的 .AppImage 文件

### 并发与超时
- **扫描模型**: `concurrent.futures.ThreadPoolExecutor(max_workers=4)`，每个适配器在独立线程执行
- **UI 回调**: 通过 `GLib.idle_add()` 将结果回传主线程更新 UI
- **适配器超时**: 系统包适配器 10s，Docker 适配器 15s，其他 5s；超时后该适配器标记为 warning 并跳过
- **部分成功**: 已完成的适配器结果立即展示，失败的适配器在 UI 顶部显示 AdwBanner 警告（如"snap 扫描超时，部分结果可能缺失"）
- **页面切换**: 切换视图时取消当前视图未完成的扫描任务（通过 threading.Event 信号）
- **手动刷新**: 每个视图提供刷新按钮，刷新当前视图所有适配器

### 扫描与缓存策略
- **触发时机**: 懒加载——进入视图时触发扫描，非启动时全量扫描
- **内存缓存**: 扫描结果缓存在内存中，切换视图时复用，无 TTL（手动刷新清除缓存）
- **无磁盘持久化**: 第一版不落盘缓存
- **清理后刷新**: 执行清理命令成功后，自动重新扫描受影响的缓存项

### CLI 命令规范（所有命令前置 `LC_ALL=C LANG=C`）
| 适配器 | 命令 | 输出格式 |
|--------|------|----------|
| AptAdapter | `dpkg-query -W -f='${Package}\t${Version}\t${Installed-Size}\t${Description}\n'` | TSV text |
| SnapAdapter | `snap list --color=never` | tabular text |
| FlatpakAdapter | `flatpak list --app --columns=name,application,version,size` | tabular text |
| PipAdapter | `pip3 list --format=json` | JSON |
| NpmAdapter | `npm list -g --json --depth=0` | JSON |
| CargoAdapter | `cargo install --list` | text (parse crate name + version) |
| DockerAdapter | `docker system df --format='{{json .}}'` | JSON per line |
| Python runtime | `python3 --version` | text |
| Node runtime | `node --version` | text |
| Rust runtime | `rustc --version` | text |
| Java runtime | `java --version 2>&1 \| head -1` | text |
| GCC runtime | `gcc --version \| head -1` | text |
| Go runtime | `go version` | text |
| nvm versions | `bash -c 'source "$HOME/.nvm/nvm.sh" 2>/dev/null && nvm list --no-colors'` | text |
| rustup | `rustup show` | text |
| conda envs | `conda env list --json` | JSON |
| conda packages | `conda list -n <env> --json` | JSON |

- **非零退出码处理**: exit code 非 0 时记录 warning，返回空结果，不抛异常
- **Docker daemon 不可用**: 检测 `docker info` 失败时跳过 Docker 适配器并提示"Docker daemon 未运行"
- **cargo 版本获取**: 使用 `cargo install --list` 解析版本，失败时回退为仅显示二进制名称（版本标记为"unknown"）

### 跨源去重
- **策略**: DesktopFileAdapter 为补充源，不独立产生条目
- **合并规则**: 如果 .desktop 文件的 `Exec` 路径或包名匹配已有 apt/snap/flatpak 条目，则将 icon_name 和 desktop_file 合并到已有条目
- **未匹配的 .desktop**: 仅当 .desktop 文件的 Exec 路径指向 /opt/ 或包含 AppImage 关键字时，作为"手动安装"或"AppImage"独立条目
- **字段优先级**: apt/snap/flatpak 的 name/version/size 优先于 .desktop 文件解析结果

### 国际化 (i18n)
- **框架**: GNU gettext，通过 Python `gettext` 模块
- **语言选择**: 跟随系统 locale（`LC_MESSAGES`），回退链: 系统 locale → en_US
- **不支持运行时切换语言**（需重启应用）
- **可翻译范围**: UI 标签、按钮文本、错误提示、清理建议描述模板
- **不翻译**: 包名、版本号、文件路径、CLI 命令
- **大小格式化**: IEC 单位（KiB/MiB/GiB），跟随 locale 数字格式

### 日志
- **框架**: Python `logging` 模块
- **默认级别**: INFO（可通过 `--debug` 启动参数切换为 DEBUG）
- **输出**: stderr（开发时）+ 文件 `~/.local/state/soft-management/softmgr.log`
- **轮转**: 单文件 5MB 上限，保留 3 个备份
- **错误分类**: 用户可见错误通过 AdwBanner/AdwMessageDialog 展示；调试信息仅写入日志文件

### 配置
- **格式**: `~/.config/soft-management/config.toml`
- **第一版可配置项**:
  - `show_all_packages = false` (全景视图是否显示全部包)
  - `top_n = 50` (磁盘排行显示数量, 范围 10-200)
- **不可配置（内部常量）**: 超时时间、并发数、缓存路径列表
- **提权策略**: 每次清理操作独立确认，不提供会话级免确认

## Risks / Trade-offs
- **PyGObject 依赖系统 GTK4 库** → pyproject.toml 文档化系统依赖，启动前检测并提供安装指引
- **subprocess 输出格式可能因版本变化而中断** → 适配器内防御性解析，异常时返回空 AdapterResult + warning
- **不同发行版包管理器差异大** → `is_available()` 自动检测，不可用时静默跳过
- **nvm 是 shell function 非二进制** → 通过 `bash -c 'source nvm.sh && ...'` 调用
- **apt 包列表量大（2680+）** → 默认仅显示桌面应用，全部包视图使用 GtkListView 虚拟化

## Open Questions
（已全部解决，无剩余开放问题）
