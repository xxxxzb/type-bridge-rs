# TypeBridge ⌨️

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.75%2B-orange?style=flat-square&logo=rust" alt="Rust 1.75+">
  <img src="https://img.shields.io/github/license/xxxxzb/type-bridge-rs?style=flat-square&color=blue" alt="License MIT">
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?style=flat-square" alt="Platform">
</p>

> 用手机浏览器通过 Wi-Fi 在电脑上打字。无需安装 App，无需蓝牙 — 打开网页就能打。支持任何手机输入法，包括语音输入。

一个自托管的远程键盘，把手机浏览器变成 PC 的无线键盘。完全在局域网内运行 — 无需账号、无需云端、手机端零安装。

> 基于 [Hacker-Shohan/TypeBridge](https://github.com/Hacker-Shohan/TypeBridge) 用 Rust 重写。

---

## 为什么用 TypeBridge？

想从房间另一头往电脑打字时，大多数人会选择蓝牙键盘或投屏 App。TypeBridge 跳过了这一切：电脑上跑一个轻量服务，手机打开一个网址，手机就变成了无线键盘。就这么简单。

很适合：
- 用手机滑动输入法在电脑上写长文
- 用手机自带的语音输入往任何 PC 应用里听写
- 控制接在电视或投影上的电脑
- 任何不想掏实体键盘的场合

---

## 原理

```
手机浏览器 ──WiFi──▶ TypeBridge (axum + Socket.IO) ──▶ enigo + arboard ──▶ PC 当前焦点应用
```

1. 在电脑上运行 `type-bridge-rs`
2. 手机上打开终端打印的 URL（手机和电脑需在同一 Wi-Fi）
3. 在手机文本框里打字
4. 点 **Send to PC** — 文字会粘贴到电脑当前激活的窗口中

---

## 安装

### 从源码编译

```bash
git clone <repo-url>
cd type-bridge-rs
cargo build --release
```

编译好的二进制文件在 `target/release/type-bridge-rs`。

### 依赖

编译需要 Rust 工具链（1.75+）。运行时无需外部依赖。

| Crate | 用途 |
|---|---|
| `axum` | HTTP 服务器 |
| `socketioxide` | Socket.IO WebSocket 实时通信 |
| `enigo` | 跨平台键盘模拟 |
| `arboard` | 剪贴板读写（处理所有 Unicode） |
| `tray-icon` | 系统托盘图标 |
| `muda` | 托盘菜单 |
| `clap` | 命令行参数 |
| `tokio` | 异步运行时 |

---

## 使用

```bash
# 默认端口 12345
type-bridge-rs

# 自定义端口
type-bridge-rs --port 8080

# 查看版本
type-bridge-rs --version
```

输出：

```
⌨️  TypeBridge running!
📱 Open on your phone: http://192.168.1.42:12345
```

手机打开这个 URL，然后在电脑上点进任意应用，在手机上打字点发送即可。

---

## 按钮

| 按钮 | 功能 |
|---|---|
| **Send to PC** | 把文本框里的所有内容粘贴到 PC 当前焦点窗口，然后清空 |
| **Backspace** | 删除文本框最后一个字符并发送退格键到 PC |
| **Enter** | 发送回车键到 PC |
| **Clear** | 仅清空文本框，不影响 PC |

**提示：** 在手机键盘上直接按 Enter 也会立即发送。Shift + Enter 可以本地换行而不发送。

---

## 系统托盘

运行后任务栏会出现图标。右键可暂停/恢复输入或退出。

- 绿色图标：输入开启
- 红色图标：已暂停

---

## 平台支持

| 平台 | 状态 |
|---|---|
| macOS | 完全支持（首次运行需授权辅助功能权限） |
| Windows | 完全支持 |
| Linux | 支持（X11/Wayland，Wayland 需 `wl-clipboard`） |

### macOS 辅助功能权限

首次运行时系统会弹出权限请求。去 **系统设置 → 隐私与安全性 → 辅助功能**，勾选你的终端即可。

---

## 与 Python 原版的区别

TypeBridge 是 [TypeBridge](https://github.com/Hacker-Shohan/TypeBridge) 的 Rust 重写版，主要改进：

- **剪贴板恢复** — 粘贴后自动还原剪贴板原有内容
- **异步 I/O** — tokio 驱动的 WebSocket，更低的资源占用
- **单文件二进制** — 编译后零依赖，复制即用
- **结构化日志** — tracing 框架，便于排查问题
- **优雅关闭** — 退出时服务端发送关闭帧

---

## Windows 防火墙

如果手机连不上，放行端口：

```
Windows Defender 防火墙 → 高级设置
→ 入站规则 → 新建规则 → 端口 → TCP → 12345 → 允许
```

---

## FAQ

**支持语音输入吗？**
支持。开启手机输入法的语音输入正常使用即可 — TypeBridge 不管文字是怎么进到文本框里的。

**文字会传到互联网上吗？**
不会。所有数据只在你家局域网内传输，不经过路由器之外。

**支持什么手机浏览器？**
任意现代浏览器 — Chrome、Safari、Firefox、三星浏览器。

**能在没有网络的环境用吗？**
需要电脑和手机连同一个 Wi-Fi。可以开手机热点让电脑连。

---

## License

MIT
