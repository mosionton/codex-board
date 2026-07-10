# Final Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复模型感知推理等级整分支审查发现的导入显式性、空模型上下文和目录条目隔离问题。

**Architecture:** Provider TOML schema 保持不变，以 `Option<String>` 保留导入字段是否有效存在；应用状态缓存 Codex 顶层当前模型，并通过统一的有效模型优先级驱动编辑器、表格、详情和应用。模型目录先解析外层数组，再逐项反序列化和语义校验。

**Tech Stack:** Rust、Serde/serde_json、toml/toml_edit、Ratatui、Cargo tests。

## Global Constraints

- 不修改 provider TOML 字段名、别名或 schema。
- 不新增硬编码模型能力表；继续使用 bundled catalog 和兼容 profile。
- 保留 `gpt-5.6 -> gpt-5.6-sol`、认证行为及现有测试。
- 每个缺陷必须先观察准确 RED，再写最小生产修复并观察 GREEN。
- Rust 修改完成后运行完整 Cargo test、fmt、CI Clippy 和 `git diff --check`。

---

### Task 1: 保留导入推理字段显式性

**Files:**
- Modify: `src/provider_config/codex_import.rs`
- Test: `src/provider_config/codex.rs`
- Test: `src/app/provider_editor.rs`

**Interfaces:**
- Consumes: `ModelCatalog::profile_for`、`ProviderConfig` 的现有可选字段。
- Produces: 有效显式值保存为 `Some(value)`；缺失、空白或不支持值保存为 `None`。

- [ ] 写集成回归测试：从缺失字段的 Sol 配置导入 provider，经 catalog-aware editor 切到 Terra 后普通/Plan 均为 `medium`。
- [ ] 写独立性测试：一个字段为受支持显式值，另一个字段缺失或无效，切换模型后仅显式字段保留。
- [ ] 运行定向测试，确认失败原因为导入结果错误地保存默认值为 `Some`。
- [ ] 修改导入归一化，仅对源中明确存在且受支持的值返回 `Some`，其余返回 `None`；OpenAI 合成复用同一结果。
- [ ] 重跑定向测试和现有 OpenAI `ultra`/`max` 继承测试，确认 GREEN。

### Task 2: 统一空 provider 模型的有效模型

**Files:**
- Modify: `src/provider_config/model_catalog.rs`
- Modify: `src/provider_config/codex_import.rs`
- Modify: `src/provider_config/codex_apply.rs`
- Modify: `src/app/providers_state.rs`
- Modify: `src/app/runtime.rs`
- Modify: `src/app/providers.rs`
- Modify: `src/app/provider_editor.rs`
- Modify: `src/ui/details.rs`
- Modify: `src/ui/tables.rs`
- Test: corresponding module tests above

**Interfaces:**
- Produces: 统一 helper 按“非空 provider model → 当前 Codex model → 兼容 profile”解析模型。
- Produces: `ProvidersState` 缓存当前 Codex model，编辑器只借它选 profile，不把 fallback model 写回 provider。
- Consumes: apply 调用方传入同一当前模型上下文，成功应用非空 provider model 后更新状态。

- [ ] 写 current Sol + empty provider model + `ultra`/`max` 的编辑器、表格/详情和 apply 失败测试。
- [ ] 写 current Luna + empty provider model + `ultra`/`max` 的 UI/editor 默认化与 apply 一致性失败测试。
- [ ] 运行定向测试，确认 UI/editor 使用兼容 profile 而 apply 使用当前模型。
- [ ] 增加当前模型加载和状态缓存，接入统一有效模型 helper。
- [ ] 将 editor、provider display 和 apply 改为使用同一上下文；保存仍保留空 model。
- [ ] 重跑跨层测试，确认 Sol 保留 `ultra`/`max`、Luna 将 `ultra` 归一为 `medium` 且 model 仍为空。

### Task 3: 隔离结构损坏的目录项

**Files:**
- Modify: `src/provider_config/model_catalog.rs`

**Interfaces:**
- Produces: 外层 catalog 成功解析后，每个 `models` 元素独立解码为 `CatalogModel`；单项结构或语义错误只丢弃该项。

- [ ] 写混合目录测试，包含缺字段、错误字段类型和一个有效 sibling。
- [ ] 运行定向测试，确认当前整体强类型反序列化导致全目录报错。
- [ ] 将外层 `models` 改为原始 JSON value 列表并逐项 `from_value`，保留现有语义过滤。
- [ ] 重跑目录测试，并确认无有效项仍返回现有 whole-catalog error。

### Task 4: 验证、报告和提交

**Files:**
- Create: `.superpowers/sdd/final-review-fixes-report.md`
- Modify: `docs/superpowers/plans/2026-07-10-final-review-fixes.md`

- [ ] 运行 `cargo test`。
- [ ] 运行 `cargo fmt --all -- --check`。
- [ ] 运行 CI Clippy 命令。
- [ ] 运行 `git diff --check` 并审查完整 diff。
- [ ] 在报告中记录根因、每项 RED/GREEN、文件、完整验证、自审和 concerns。
- [ ] 使用仓库 Conventional Commit 中文摘要与正文提交。
