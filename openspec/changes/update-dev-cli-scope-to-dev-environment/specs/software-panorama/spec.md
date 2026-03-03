## ADDED Requirements

### Requirement: Exclude Developer CLI Tools From Software Panorama
系统 SHALL 在软件全景视图中排除“开发环境的全局 CLI 工具”条目（例如来自 `npm/cargo/uv/pipx` 的全局安装工具）。这些条目 SHALL 仅在开发环境视图（dev-environment）中展示，以避免同一对象在多个视图重复出现。

#### Scenario: npm global package is not shown in panorama
- **GIVEN** 系统安装了 npm，且存在全局包 `eslint`
- **WHEN** 用户进入软件全景视图并完成软件发现
- **THEN** 列表中不出现 `canonical_id="npm:eslint"` 的条目

