# Change: 技术栈从 Python+PyGObject 迁移到 Rust+gtk4-rs

## Why
用户决定放弃 Python 技术栈，改用 Rust 开发本项目。主要动机：Rust 作为编译型语言在系统工具场景下性能更优，类型安全性更强，且 gtk4-rs 绑定在 Linux 桌面生态中已成熟（v0.10.3，2025年12月发布，2026年2月仍活跃维护）。GNOME 社区自身也在从 Python/Vala 向 Rust 迁移，长期维护前景更好。

## What Changes
- **BREAKING** 删除全部 Python 源码（`src/soft_management/` 目录下 8 个 .py 文件）
- **BREAKING** 删除 `pyproject.toml`（Meson+Python 构建配置），替换为 `Cargo.toml`（Rust 构建配置）
- **BREAKING** 技术栈变更：Python 3.10+ / PyGObject / Meson → Rust (edition 2021) / gtk4-rs / libadwaita-rs / Cargo
- 项目结构从 `src/soft_management/*.py` 变更为 `src/` 下的 Rust 模块结构
- 构建系统从 meson-python 变更为 Cargo，可执行文件名保持 `softmgr`
- 分发方式从 PyPI/pipx 变更为 Cargo install + .deb 打包
- 所有功能需求（software-panorama、dev-environment、disk-analysis）的行为规格不变，仅实现语言变更
- 适配器架构从 Python ABC + dataclass 变更为 Rust trait + struct
- 并发模型从 ThreadPoolExecutor + GLib.idle_add 变更为 Rust async (tokio) + glib::spawn_future_local
- 配置解析从 Python tomllib 变更为 Rust toml crate
- 日志从 Python logging 变更为 Rust tracing crate
- i18n 从 Python gettext 变更为 Rust gettext-rs
- 测试从 pytest + hypothesis 变更为 Rust #[test] + proptest

## Impact
- Affected specs: software-panorama (MODIFIED), dev-environment (MODIFIED), disk-analysis (MODIFIED)
- Affected code: 全部现有 Python 代码将被删除并用 Rust 重写
- 现有 `openspec/changes/add-linux-software-manager/` 提案中的 Python 特定实现细节将失效，但功能需求和场景规格保持不变
- `openspec/project.md` 需更新技术栈描述
