# TypeBridge

## What This Is

一个自托管的 Wi-Fi 远程键盘——手机浏览器打开网页即可向 PC 发送键盘输入。无需安装 App，无需蓝牙，完全运行在局域网内。支持任何手机输入法（包括语音输入）。基于 Python 原版 [TypeBridge](https://github.com/Hacker-Shohan/TypeBridge) 用 Rust 重写。

## Core Value

手机打字 → PC 输入，零摩擦。可靠、安全、即开即用。

## Requirements

### Validated

- ✓ QR 码连接 — v0.3.0
- ✓ 系统托盘控制（暂停/恢复/退出） — v0.3.0
- ✓ 历史记录（最近发送，点击回填） — v0.3.0
- ✓ 局域网键盘控制安全加固 — v0.3.0
- ✓ macOS .app 打包 — v0.3.0

### Active

- [ ] HTTP API 替代 Socket.IO
- [ ] Bearer token 鉴权
- [ ] 前端 fetch + Promise 命令队列
- [ ] 按钮语义修正
- [ ] 安全头加固
- [ ] AppState 收敛全局状态
- [ ] Backspace 改为普通 Backspace

### Out of Scope

- 多客户端同时连接 — 使用场景不需要
- 蓝牙/WebRTC 等非 HTTP 传输 — HTTP 足够简单可靠
- Cookie-based 鉴权 — 引入 CSRF 风险，不需要
- 实时双向推送 — HTTP polling 足够

## Context

现有架构：axum + socketioxide（Socket.IO），手机通过长连接发送 type_text/backspace/press_key/clear_input 事件。键盘执行通过 enigo + arboard 完成。主线程 winit event loop 从 bounded sync_channel drain + merge + execute 命令。

重构目标：去掉 socketioxide 依赖，改为纯 HTTP API。保留主线程事件循环和键盘队列边界。收紧鉴权（Bearer token）、安全头、前端命令顺序。

## Constraints

- **兼容性**: macOS / Windows / Linux 三平台
- **安全**: 局域网内使用，token 鉴权，不启用 CORS
- **性能**: 命令延迟 <100ms（从手机发送到 PC 执行）
- **依赖**: 移除 socketioxide，不新增重量级框架

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| HTTP-only 替代 Socket.IO | 更简单协议，更强安全边界，更好测试 | — Pending |
| Bearer token 替代 URL query token | token 不出现在日志/历史，不生 CSRF 风险 | — Pending |
| 前端 Promise 队列保证顺序 | 多个 fetch 可能并发乱序 | — Pending |
| Backspace 改回单字符 | 原 Option/Ctrl+Backspace 删词行为不符合直觉 | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-06-08 after milestone v1.0 started*
