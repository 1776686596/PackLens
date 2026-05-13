## Context
项目 packlens 是一个 Linux 软件与开发环境统一管理桌面应用，当前使用 Python + GTK4/libadwaita (PyGObject) 技术栈，已完成项目骨架（入口、窗口框架、数据模型、配置、日志），三个核心功能视图尚为占位符。用户决定在功能实现之前将技术栈迁移到 Rust + gtk4-rs，以获得更好的性能、类型安全和长期维护性。

目标平台：Ubuntu 24.04+ GNOME Wayland 环境。系统存在 apt/snap/flatpak 等包管理器、nvm/rustup/conda 等版本管理器、Docker 容器生态。

## Goals / Non-Goals
- Goals:
  - 将全部代码从 Python 迁移到 Rust，保持功能需求不变
  - 利用 Rust 类型系统提升代码安全性（消除运行时类型错误）
  - 利用编译型语言提升扫描性能（尤其是 2680+ apt 包的解析和磁盘递归 stat）
  - 保持 GNOME 原生体验（gtk4-rs + libadwaita-rs）
  - 保持可执行文件名 `packlens` 不变
- Non-Goals:
  - 不改变任何功能需求或用户可见行为
  - 不增加跨平台支持
  - 不改变 UI 设计（仍为 AdwNavigationSplitView 三视图布局）

## Decisions

### MSRV 与工具链
- **MSRV**: Rust 1.80（Ubuntu 24.04 自带版本）
- **工具链固定**: 项目根目录放置 `rust-toolchain.toml`，内容 `[toolchain]\nchannel = "1.80"`
- **原生 async trait**: MSRV >= 1.75，无需 `async-trait` crate，trait 中直接使用 `async fn`
- **Cargo.lock**: 提交到 git，保证可复现构建
- **Edition**: 2021

### 技术栈: Rust + gtk4-rs + libadwaita-rs
- **决策**: 使用 Rust (edition 2021) 作为开发语言，gtk4-rs + libadwaita-rs 作为 GUI 绑定
- **理由**: gtk4-rs 是 GTK 官方推荐的 Rust 绑定，v0.10.3 (2025-12) 稳定，有官方教程书籍，GNOME 社区正在向 Rust 迁移
- **系统库版本锁定**: GTK4 >= 4.12, libadwaita >= 1.4
- **兼容矩阵**: Ubuntu 24.04+, Fedora 39+, Arch Linux (rolling)

### 完整依赖清单

#### [dependencies]
| crate | 版本 | features | 用途 |
|-------|------|----------|------|
| `gtk4` | `0.10` | `v4_12` | GTK4 绑定 |
| `libadwaita` | `0.8` | `v1_4` | libadwaita 绑定 |
| `tokio` | `1` | `rt-multi-thread, process, time, sync` | 异步运行时 |
| `async-channel` | `2` | — | tokio→glib 跨运行时通信 |
| `serde` | `1` | `derive` | 序列化框架 |
| `serde_json` | `1` | — | JSON 解析（pip/npm/conda/docker 输出） |
| `toml` | `0.8` | — | TOML 配置解析 |
| `tracing` | `0.1` | — | 结构化日志 |
| `tracing-subscriber` | `0.3` | `env-filter, fmt` | 日志输出 |
| `tracing-appender` | `0.2` | — | 日志文件轮转 |
| `thiserror` | `2` | — | 错误类型派生 |
| `walkdir` | `2` | — | 递归目录遍历 |
| `freedesktop-desktop-entry` | `0.8` | — | .desktop 文件解析 |
| `tokio-util` | `0.7` | `rt` | CancellationToken |
| `gettext-rs` | `0.7` | — | i18n |

#### [dev-dependencies]
| crate | 版本 | 用途 |
|-------|------|------|
| `proptest` | `1` | 属性测试 |

### 构建依赖矩阵（系统包）

| 发行版 | 编译期依赖 | 运行时依赖 |
|--------|-----------|-----------|
| Ubuntu 24.04+ | `sudo apt install libgtk-4-dev libadwaita-1-dev pkg-config` | `libgtk-4-1 libadwaita-1-0` |
| Fedora 39+ | `sudo dnf install gtk4-devel libadwaita-devel pkg-config` | `gtk4 libadwaita` |
| Arch Linux | `sudo pacman -S gtk4 libadwaita pkgconf` | `gtk4 libadwaita` |

- **链接模式**: 动态链接（系统 GTK4 库），通过 `pkg-config` 自动发现
- **PKG_CONFIG_PATH**: 使用系统默认路径，不做额外配置

### 错误处理: thiserror 分层模型

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("config: {0}")]
    Config(#[from] ConfigError),
    #[error("adapter: {0}")]
    Adapter(#[from] AdapterError),
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("invalid value: {field} = {value}")]
    Validation { field: String, value: String },
}

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("command failed: {cmd} (exit={code})")]
    CommandFailed { cmd: String, code: i32 },
    #[error("command timed out: {cmd} ({timeout_secs}s)")]
    Timeout { cmd: String, timeout_secs: u64 },
    #[error("parse error: {context}: {detail}")]
    Parse { context: String, detail: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
```

**错误矩阵**:
| 错误类型 | 处理动作 | 重试 | 日志级别 | UI 行为 |
|----------|---------|------|---------|---------|
| `AdapterError::CommandFailed` | 返回空 AdapterResult + warning | 否 | `warn!` | AdwBanner 警告 |
| `AdapterError::Timeout` | 取消任务，返回空结果 + warning | 否 | `warn!` | AdwBanner 警告 |
| `AdapterError::Parse` | 返回空 AdapterResult + warning | 否 | `warn!` | AdwBanner 警告 |
| `AdapterError::Io` | 返回空 AdapterResult + warning | 否 | `warn!` | AdwBanner 警告 |
| `ConfigError::Io` | 使用默认配置 | 否 | `warn!` | 静默 |
| `ConfigError::Parse` | 使用默认配置 | 否 | `warn!` | 静默 |
| `ConfigError::Validation` | clamp 到有效范围 | 否 | `warn!` | 静默 |
| tokio JoinError | 记录错误，视为适配器失败 | 否 | `error!` | AdwBanner 警告 |
| async-channel RecvError | 忽略（发送端已关闭） | 否 | `debug!` | 静默 |

### 并发模型: tokio + async-channel + glib::spawn_future_local

- **架构**: 全局单例 `tokio::runtime::Runtime`，在 `main()` 中创建，通过 `once_cell::sync::Lazy` 持有
- **生命周期**: Runtime 在 `main()` 返回前通过 `runtime.shutdown_background()` 关闭
- **桥接机制**: `async-channel`（替代已移除的 `glib::MainContext::channel()`）
  - 后台: `tokio::spawn` 执行适配器，结果通过 `async_channel::Sender` 发送
  - 前台: `glib::spawn_future_local` 中 `async_channel::Receiver::recv().await` 接收并更新 UI
  - 通道容量: bounded(32)，背压策略为发送端 await（不丢弃）
- **取消机制**: `tokio_util::sync::CancellationToken`
  - 每个视图持有一个 CancellationToken
  - 视图切换时调用 `token.cancel()`，后台任务检查 `token.is_cancelled()` 后提前返回
  - 过期结果（token 已取消）在接收端丢弃
- **线程边界约束**:
  - 后台 tokio 任务仅处理 `Send + 'static` 纯数据（models.rs 中的 struct）
  - GTK Widget / GObject 仅在 glib 主线程访问，绝不传入 tokio::spawn
  - UI 状态通过 `Rc<RefCell<T>>` 持有，仅主线程读写

### 子进程执行: tokio::process::Command
- **决策**: 使用 tokio::process::Command 替代 Python subprocess
- **环境变量**: 所有命令前置 `.env("LC_ALL", "C").env("LANG", "C")`
- **输出处理**: `String::from_utf8_lossy()` 转换 stdout/stderr，非 UTF-8 字节替换为 U+FFFD
- **非零退出码**: 返回 `AdapterError::CommandFailed`，由调用方转为 warning
- **超时**: `tokio::time::timeout(Duration::from_secs(N), cmd.output())`

### canonical_id 规范
- **格式**: `"{source}:{name}"`
- **生成规则**:
  | source | name 来源 | 示例 |
  |--------|----------|------|
  | `apt` | dpkg-query 的 Package 字段（原样保留，含架构后缀如 `:amd64`） | `apt:vim`, `apt:libc6:amd64` |
  | `snap` | snap list 的 Name 列 | `snap:firefox` |
  | `flatpak` | flatpak list 的 Application 列 | `flatpak:org.mozilla.firefox` |
  | `manual` | .desktop 文件 Exec 路径的 basename（去扩展名） | `manual:myapp` |
  | `appimage` | .desktop 文件 Exec 路径的 basename（去 .AppImage 后缀） | `appimage:obsidian` |
- **反解析**: `canonical_id.splitn(2, ':')` → `(source, name)`
- **冲突处理**: 同一 source 内 name 重复时保留第一个，跨 source 允许重复（如 `apt:firefox` 和 `snap:firefox` 共存）

### install_method 判定表（运行时检测）
按优先级从高到低匹配可执行文件路径：
| 路径模式 | install_method |
|----------|---------------|
| 包含 `/.nvm/` | `nvm` |
| 包含 `/.rustup/` | `rustup` |
| 包含 `/anaconda3/` 或 `/miniconda3/` | `conda` |
| 包含 `/.cargo/bin/` | `cargo` |
| 以 `/usr/local/bin/` 开头 | `manual` |
| 以 `/usr/bin/` 或 `/bin/` 开头 | `apt` |
| 包含 `/.local/bin/` | `pipx` |
| 其他 | `unknown` |

### 清理命令白名单
仅允许以下精确命令字符串，拒绝任何其他值：
```
apt clean
pip3 cache purge
npm cache clean --force
conda clean --all -y
cargo cache --autoclean
docker system prune -f
```
- **验证方式**: `CleanupSuggestion.command` 必须完全匹配白名单中的某一项（`==` 比较）
- **拒绝模式**: 不匹配时 panic（构造时校验，属于编程错误而非运行时错误）

### GtkListView 数据承载: BoxedAnyObject
- **决策**: 使用 `glib::BoxedAnyObject` 包装 Rust struct 传入 `gio::ListStore`
- **理由**: 无需为每个模型定义 GObject 子类，减少样板代码
- **用法**: `gio::ListStore::new::<glib::BoxedAnyObject>()` + `SignalListItemFactory`

### .desktop 文件解析: freedesktop-desktop-entry crate
- **决策**: 使用 `freedesktop-desktop-entry` crate (v0.8) 解析 .desktop 文件
- **理由**: 符合 freedesktop 规范，处理转义、locale 等边界情况

### 磁盘遍历: walkdir crate
- **决策**: 使用 `walkdir` crate (v2) 递归遍历缓存目录
- **理由**: 成熟稳定，自动处理符号链接循环、权限错误等边界
- **配置**: `WalkDir::new(path).follow_links(false)`，权限错误时 `filter_map(|e| e.ok())` 跳过

### 日志: tracing 详细配置
- **框架**: `tracing` + `tracing-subscriber` + `tracing-appender`
- **默认级别**: INFO（`--debug` 切换为 DEBUG）
- **stderr 输出**: `tracing_subscriber::fmt::layer()` 输出到 stderr
- **文件输出**: `tracing_appender::rolling::RollingFileAppender`
  - 路径: `~/.local/state/packlens/packlens.log`
  - 轮转策略: 按大小轮转，单文件 5MB 上限，保留 3 个备份
  - 注意: tracing-appender 原生仅支持按时间轮转，按大小轮转需使用 `tracing-appender` 的 `non_blocking` writer 配合自定义逻辑，或退回使用 `file-rotate` crate
- **格式**: `{timestamp} {level} {target}: {message}`

### 配置: toml crate + serde
- **配置路径**: `~/.config/packlens/config.toml`
- **配置项**: `show_all_packages: bool` (默认 false) + `top_n: u32` (默认 50, clamp 到 10-200)
- **缺失文件**: 使用默认值，`debug!` 记录
- **解析失败**: 使用默认值，`warn!` 记录

### 分发与资源安装
- **cargo install**: 仅安装二进制到 `~/.cargo/bin/packlens`，不安装 .desktop/图标
- **.deb 打包**: 使用 `cargo-deb`
  - 二进制: `/usr/bin/packlens`
  - .desktop: `/usr/share/applications/io.github.packlens.PackLens.desktop`
  - 图标: `/usr/share/icons/hicolor/scalable/apps/io.github.packlens.PackLens.svg`
- **手动安装资源**: README 中提供 `install -D` 命令将 data/ 下资源复制到 XDG 标准路径

### 适配器架构: Rust 原生 async trait

由于 MSRV 1.80 >= 1.75，使用原生 async fn in trait，无需 `async-trait` crate：
```rust
pub trait PackageAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    async fn list_packages(&self) -> AdapterResult<Package>;
}

pub trait EnvironmentAdapter: Send + Sync {
    fn name(&self) -> &str;
    async fn detect_runtimes(&self) -> Vec<RuntimeInfo>;
    async fn detect_version_managers(&self) -> Vec<VersionManagerInfo>;
    async fn list_global_packages(&self) -> Vec<GlobalPackageInfo>;
}

pub trait CacheAdapter: Send + Sync {
    fn name(&self) -> &str;
    async fn list_caches(&self) -> Vec<CacheInfo>;
    async fn suggest_cleanups(&self) -> Vec<CleanupSuggestion>;
}
```
注意: 原生 async trait 返回的 Future 默认不是 `Send`。由于适配器需要跨 tokio::spawn 边界，需使用 `#[trait_variant::make(SendPackageAdapter: Send)]` 或手动 `Box::pin` 返回 `Send` future。具体方案在实现时根据 Rust 1.80 的 `async fn in trait` 限制确定，若不可行则回退到 `async-trait` crate。

### 数据模型: Rust struct
```rust
pub struct Package {
    pub canonical_id: String,
    pub name: String,
    pub version: String,
    pub source: String,
    pub size: Option<u64>,
    pub description: String,
    pub icon_name: Option<String>,
    pub desktop_file: Option<String>,
}

pub struct AdapterResult<T> {
    pub items: Vec<T>,
    pub warnings: Vec<String>,
    pub duration_ms: u64,
    pub timestamp: f64,
}

pub struct RuntimeInfo {
    pub language: String,
    pub version: String,
    pub path: String,
    pub install_method: String,
}

pub struct VersionManagerInfo {
    pub name: String,
    pub managed_versions: Vec<ManagedVersion>,
    pub path: String,
}

pub struct ManagedVersion {
    pub version: String,
    pub active: bool,
}

pub struct GlobalPackageInfo {
    pub manager: String,
    pub name: String,
    pub version: String,
}

pub struct CacheInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub requires_sudo: bool,
}

pub struct CleanupSuggestion {
    pub description: String,
    pub estimated_bytes: u64,
    pub command: String,
    pub requires_sudo: bool,
    pub risk_level: RiskLevel,
}

pub enum RiskLevel {
    Safe,
    Moderate,
}
```

### 项目结构
```
soft_management/
├── Cargo.toml
├── Cargo.lock                  # 提交到 git
├── rust-toolchain.toml         # channel = "1.80"
├── src/
│   ├── main.rs                 # 入口: 创建 tokio Runtime + 启动 GTK
│   ├── app.rs                  # AdwApplication (ObjectSubclass)
│   ├── window.rs               # 主窗口 (ObjectSubclass)
│   ├── config.rs               # 配置加载 (serde + toml)
│   ├── error.rs                # AppError / AdapterError / ConfigError
│   ├── models.rs               # 数据模型
│   ├── subprocess.rs           # 子进程执行工具
│   ├── runtime.rs              # 全局 tokio Runtime 单例
│   ├── adapters/
│   │   ├── mod.rs              # trait 定义 + 注册表
│   │   ├── apt.rs
│   │   ├── snap.rs
│   │   ├── flatpak.rs
│   │   ├── desktop_file.rs
│   │   ├── python_env.rs
│   │   ├── node_env.rs
│   │   ├── rust_env.rs
│   │   ├── java_env.rs
│   │   └── cache/
│   │       ├── mod.rs
│   │       ├── apt_cache.rs
│   │       ├── pip_cache.rs
│   │       ├── npm_cache.rs
│   │       ├── conda_cache.rs
│   │       ├── cargo_cache.rs
│   │       └── docker_cache.rs
│   ├── services/
│   │   ├── mod.rs
│   │   ├── discovery.rs        # 软件发现聚合
│   │   ├── environment.rs      # 开发环境聚合
│   │   └── disk.rs             # 磁盘分析聚合
│   └── ui/
│       ├── mod.rs
│       ├── panorama.rs         # 软件全景视图
│       ├── devenv.rs           # 开发环境视图
│       └── disk.rs             # 磁盘分析视图
├── data/
│   ├── io.github.packlens.PackLens.desktop
│   └── icons/
│       └── io.github.packlens.PackLens.svg
└── tests/
    ├── adapter_parsing.rs
    └── proptest_invariants.rs
```

## Migration Plan
1. 删除全部 Python 源码和 pyproject.toml（当前仅骨架代码，无需采集行为基线）
2. 初始化 Cargo 项目，搭建 Rust 骨架（含 tokio-glib 桥接基础设施）
3. 按 tasks.md 顺序实现各模块
4. 实现完成后归档 `add-linux-software-manager` 提案（遵循 OpenSpec "归档在部署后"规则）
5. 回滚方案：git revert 到迁移前的 commit

## Risks / Trade-offs
- **gtk4-rs 学习曲线陡** → 有官方书籍 ([gtk-rs.org/gtk4-rs/stable/latest/book](https://gtk-rs.org/gtk4-rs/stable/latest/book))，且项目 UI 复杂度不高
- **Rust 编译时间长** → 开发期使用 `cargo check` 快速验证，release 构建仅在发布时执行
- **系统仍需 GTK4 运行时库** → 与 Python 版本相同的约束，不是新增风险
- **原生 async trait Send 限制** → 若 Rust 1.80 原生 async trait 无法满足 Send 约束，回退到 `async-trait` crate
- **tracing-appender 不支持按大小轮转** → 可能需要 `file-rotate` crate 替代，或接受按日轮转
- **glib::MainContext::channel() 已移除** → 使用 `async-channel` crate 替代，这是 gtk-rs 社区推荐的迁移方案

## PBT (Property-Based Testing) 属性

### canonical_id 属性
| 属性 | 定义 | 边界条件 | 伪造策略 |
|------|------|---------|---------|
| 可逆性 | `parse(canonical_id(s, n)) == (s, n)` | 包名含 `:` (如 `libc6:amd64`) | 生成含 `:` 的随机包名 |
| 唯一性 | 最终列表中无重复 canonical_id | 多适配器返回同名包 | 构造跨源同名包 |
| 格式一致性 | 所有 canonical_id 匹配 `^[a-z]+:.+$` | 空字符串、特殊字符 | 生成随机 Unicode 字符串 |

### 去重属性
| 属性 | 定义 | 边界条件 | 伪造策略 |
|------|------|---------|---------|
| 幂等性 | `merge(merge(pkgs, desktop)) == merge(pkgs, desktop)` | 空列表、全匹配、全不匹配 | 随机生成包列表和 .desktop 列表 |
| 字段优先级 | 合并后 name/version/size 来自包管理器而非 .desktop | .desktop 有不同 name | 构造冲突字段 |

### 排序属性
| 属性 | 定义 | 边界条件 | 伪造策略 |
|------|------|---------|---------|
| 单调性 | `ranking[i].size >= ranking[i+1].size` (None 在末尾) | 全 None、全相同 size | 生成随机 Option<u64> 列表 |
| top_n 边界 | `ranking.len() <= clamp(top_n, 10, 200)` | top_n=0, 1, 9, 10, 200, 201, u32::MAX | 边界值 + 随机 |
| size 非负 | `pkg.size.map_or(true, |s| s >= 0)` (u64 天然满足) | — | — |

### 过滤属性
| 属性 | 定义 | 边界条件 | 伪造策略 |
|------|------|---------|---------|
| 代数性 | `filter(source) ∩ filter(keyword) == filter(source AND keyword)` | 空关键词、空来源 | 随机组合 |
| 大小写不敏感 | `search("FOO") == search("foo")` | 混合大小写、Unicode | 生成随机大小写变体 |

### 适配器隔离属性
| 属性 | 定义 | 边界条件 | 伪造策略 |
|------|------|---------|---------|
| 顺序无关 | `sort(run([A,B,C])) == sort(run([C,A,B]))` | 1个/全部失败 | 随机打乱适配器顺序 |
| 部分失败隔离 | 单适配器失败不影响其他结果 | 全部失败 | 随机标记适配器为失败 |

### 清理命令安全属性
| 属性 | 定义 | 边界条件 | 伪造策略 |
|------|------|---------|---------|
| 白名单 | 所有 CleanupSuggestion.command ∈ WHITELIST | — | 构造非白名单命令 |
| 无注入 | command 不含 `;`, `|`, `&&`, `$()`, `` ` `` | — | 生成含 shell 元字符的字符串 |

## Open Questions
（全部已解决）
