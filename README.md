<div align="center">
  <img src="public/app-icon.png" width="96" height="96" alt="Pivot Logo" />
  <h1>Pivot</h1>
  <p><strong>面向 AI Coding 终端工具的本地轻量路由网关与客户端接管中心</strong></p>

  <p>
    <a href="https://github.com/vsnote/pivot/releases/latest">
      <img src="https://img.shields.io/github/v/release/vsnote/pivot?color=blue&label=Release" alt="Latest Release" />
    </a>
    <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="Platform macOS" />
    <a href="LICENSE">
      <img src="https://img.shields.io/badge/License-MIT-green.svg" alt="License MIT" />
    </a>
  </p>
</div>

---

Pivot 是一个专为 **Claude Code**、**Codex CLI**、**Gemini CLI** 和 **Pi Agent** 等现代 AI 开发终端工具打造的本地桌面网关应用。它在本地启动一个兼容 OpenAI 与 Anthropic 标准的高性能路由网关，自动完成客户端配置无感接管、模型智能分发、协议双向转换以及多服务商容灾切换。

---

## ✨ 核心特性

- **⚡ 统一本地网关**：在本机启动接口服务（默认 `http://127.0.0.1:16888`），同时提供 Anthropic (`POST /v1/messages`) 与 OpenAI (`POST /v1/chat/completions`) 双标准端点。
- **🔄 协议双向转换**：支持 Anthropic ⇋ OpenAI ⇋ Gemini 多协议即时转换，包含流式 (SSE) 与非流式请求。
- **🎯 零配置智能路由**：支持直接通过前缀路由（`provider/model`）、精确模型匹配以及默认模型回退，无需手动维护繁琐规则。
- **🛡️ 智能容灾与备用链**：请求失败时自动尝试备用服务商，自动跳过不支持该模型的节点，日志中保留完整的链路重试过程。
- **🔌 客户端一键接管**：一键接管 Claude Code、Codex CLI、Gemini CLI 与 Pi Agent 的本地配置文件，关闭时自动恢复初始状态并备份。
- **🔒 纯本地优先与隐私安全**：所有 API Key、路由配置与请求日志均保存在本机 SQLite 数据库，无任何数据上传云端。
- **📊 实时调用日志与监控**：记录每次请求的完整尝试链、时延、Token 计费统计与错误摘要。

---

## 📥 下载与安装

请前往 [GitHub Releases](https://github.com/vsnote/pivot/releases/latest) 下载最新的 macOS 安装包：

| 芯片架构 | 适用机型 | 安装包下载 |
| :--- | :--- | :--- |
| **Apple Silicon** | M1 / M2 / M3 / M4 及后续芯片 Mac | `Pivot_*_aarch64.dmg` |
| **Intel** | 采用 Intel 处理器的传统 Mac | `Pivot_*_x64.dmg` |

> **💡 首次打开提示「已损坏」或「无法打开」？**  
> 由于开源分发未内置苹果商业付费证书，macOS Gatekeeper 可能会弹出拦截提示。可通过以下方式解除隔离：
> 1. 打开系统「设置」->「隐私与安全性」，在安全性部分找到提示并点击「仍要打开」。  
> 2. 或在终端中执行：
>    ```bash
>    sudo xattr -r -d com.apple.quarantine /Applications/Pivot.app
>    ```

---

## 🚀 快速上手

1. **添加服务商**：在「服务商」页面选择预设（如 Anthropic、OpenAI、DeepSeek、Gemini、OpenRouter 等）或添加自定义服务商，填入 API 地址、密钥及可用模型。
2. **设置默认路由**：为某家主力服务商设置「默认模型」，未显式匹配的请求将由该模型及其所属服务商承接。
3. **开启客户端接管**：在「客户端」页面点击对应 CLI 工具的开关，Pivot 会自动备份原有配置并将其流量指向本地网关。
4. **尽情编码**：在终端正常使用 `claude` 或其他 CLI 工具，所有流量将在本机通过 Pivot 智能路由并记录。

---

## 🧭 模型路由逻辑

Pivot 会按以下优先级推导并路由每个请求：

1. **`provider/model` 前缀**：显式指定了服务商名称时，直接转发至目标服务商。
2. **高级覆盖规则**：若在「高级设置」中配置了针对特定模型或路径的强制改写规则，优先执行。
3. **精确模型匹配**：若请求的模型名称已在某个服务商的模型声明列表中，自动路由至该服务商。
4. **默认模型兜底**：当请求的模型未在任何服务商处显式声明时，自动改写为全局指定的「默认模型」进行承接。

**容灾备用机制**：当请求失败时，备用链会按优先级依次尝试具备同名模型的服务商，再尝试配置了兜底能力的候选节点，避免无效重试。

---

## 🌐 支持的服务商与生态

### 客户端工具
- **Claude Code**
- **Codex CLI**
- **Gemini CLI**
- **Pi Agent**

### 支持的服务商
- **国际主流平台**：Anthropic、OpenAI、Google Gemini、OpenRouter、Ollama、Groq、Mistral AI、Perplexity、Cerebras 等。
- **国内服务商**：DeepSeek、SiliconFlow (硅基流动)、Moonshot AI (月之暗面)、智谱 AI、通义千问、MiniMax、StepFun (阶跃星辰)、百川智能、零一万物等。
- **自定义兼容节点**：任意兼容 OpenAI / Anthropic 格式的第三方反向代理或自建端点。

---

## 🛠️ 本地开发

### 环境要求
- Node.js 20+
- pnpm 10+
- Rust stable
- macOS (含 Xcode Command Line Tools)

### 安装与启动

```bash
# 1. 安装前端依赖
pnpm install

# 2. 启动前端 Vite 开发服务器
pnpm dev

# 3. 启动桌面端开发调试 (Tauri)
pnpm tauri dev
```

### 测试与构建

```bash
# 前端测试与打包
pnpm test
pnpm build

# Rust 后端检查与单元测试
cargo test --manifest-path src-tauri/Cargo.toml

# 本地打包 macOS 安装包
pnpm tauri build
```

---

## 💾 数据与存储

- 本地配置与日志存储路径：
  ```text
  ~/.pivot/pivot.db
  ```
- 网关服务默认监听：`127.0.0.1:16888`
- 客户端接管配置时，原有配置文件会自动在同目录下生成 `.pivot.backup` 备份，关闭开关即可一键无损还原。

---

## 📄 开源协议

本项目采用 [MIT License](LICENSE) 开源协议。
