# Change: 构建 Linux 软件与开发环境统一管理桌面应用

## Why
Linux 系统不像 Windows 提供统一的"程序与功能"面板。软件通过 apt/snap/flatpak/手动安装等多种渠道分散管理，开发环境版本由 nvm/rustup/conda 等各自独立的版本管理器控制，用户无法在一个界面中全局掌握系统上安装了什么、占用了多少空间、开发环境处于什么状态。本项目旨在构建一个原生 GTK4/libadwaita 桌面应用，提供统一的可视化管理入口。

## What Changes
- 新建完整项目：Python + GTK4/libadwaita 桌面应用
- 新增能力 **software-panorama**：统一聚合并展示所有已安装软件（apt/dpkg、snap、flatpak、手动安装的桌面应用），支持搜索、筛选、分类浏览
- 新增能力 **dev-environment**：检测并展示所有开发语言运行时（Python/Node/Rust/Java/Go）、版本管理器（nvm/rustup/conda/uv）、虚拟环境、全局包
- 新增能力 **disk-analysis**：分析各包管理器缓存与已安装软件的磁盘占用，提供清理建议与预估可回收空间
- 适配器架构：每个包管理器实现独立适配器，启动时自动检测可用适配器，缺失的包管理器不影响其他功能
- 默认只读：所有读取操作无需 root 权限，写操作（卸载/清理）需用户显式确认

## Impact
- Affected specs: software-panorama (新增), dev-environment (新增), disk-analysis (新增)
- Affected code: 全新项目，无现有代码受影响
- 技术栈: Python 3.10+, GTK4, libadwaita, PyGObject, Meson build system
- 分发方式: PyPI (pipx install) + 可选 .deb 打包
- 目标平台: 通用 Linux（主要适配 Ubuntu/Fedora/Arch，其他发行版通过适配器自动降级）
