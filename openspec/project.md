# Project Context

## Purpose
Linux 系统软件与开发环境统一管理工具。解决 Linux 下软件安装来源分散（apt/snap/flatpak/手动安装）、开发环境版本混乱、磁盘占用不透明的问题，提供一个原生 GUI 桌面应用进行统一可视化管理。

## Tech Stack
- Rust (edition 2021, MSRV 1.80)
- GTK4 (gtk4-rs v0.10) + libadwaita (libadwaita-rs v0.8) - native GNOME integration
- tokio (async runtime) + async-channel (tokio-glib bridge)
- Cargo (build system)
- cargo install / cargo-deb (distribution)

## Project Conventions

### Code Style
- Clippy pedantic, enforced in Cargo.toml [lints.clippy]
- 中文注释仅用于领域术语，代码使用英文

### Architecture Patterns
- Adapter pattern: each package manager is an independent adapter implementing a common trait
- Service layer: business logic separated from UI
- RPITIT (return-position impl Trait in trait) for async adapter methods
- Async scanning via tokio::spawn + async-channel + glib::spawn_future_local

### Testing Strategy
- #[test] unit tests in source modules
- proptest for property-based invariants
- Adapter tests use mocked subprocess output (no real system calls in CI)

### Git Workflow
- main branch for stable releases
- Feature branches: `feat/<name>`, `fix/<name>`
- Conventional Commits

## Domain Context
- Linux package managers: apt/dpkg (Debian/Ubuntu), snap (Canonical), flatpak (freedesktop)
- Language package managers: pip/uv/conda (Python), npm/yarn/pnpm (Node.js), cargo (Rust)
- Version managers: nvm (Node), rustup (Rust), conda (Python envs)
- Desktop entries: .desktop files in /usr/share/applications, ~/.local/share/applications, etc.

## Important Constraints
- Read-only by default; write operations (install/uninstall) require explicit user confirmation
- Must NOT require root for read operations
- Cannot use Flatpak for distribution (tool needs host system access to read package databases)
- Must handle missing package managers gracefully (auto-detect available adapters)

## External Dependencies
- System CLI tools: dpkg-query, apt, snap, flatpak, pip3, npm, cargo, docker
- System libraries: GTK4 >= 4.12, libadwaita >= 1.4
