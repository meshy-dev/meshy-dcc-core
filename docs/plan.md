# Meshy DCC Core：Rust fork 计划

日期：2026-09-28。阶段：R0（fork 骨架）。本仓库是 `dcc-mcp/dcc-mcp-core`
（MIT）`v0.20.36` 的 Meshy 派生开发线，保留上游完整历史。

## 1. 方向（已确认）

- **技术栈**：Rust。直接改造上游 workspace，不再用 Node 包装上游二进制。
  2026-09-22 至 28 的 Node.js 原型（0.3.30）只作为行为参考，归档在本地分支
  `archive/js-prototype-0.3.30`，不继续开发。
- **Agent 入口**：`meshy-dcc` CLI + `skills/meshy-dcc/SKILL.md`。不向 Agent 暴露 MCP
  connector。Meshy API、认证、计费和下载仍由外部已发布的 `meshy-cli` 负责。
- **改名范围**：只改产品面（二进制名、Meshy 自有状态目录与配置、Skill、文档）。
  内部 crate、Python 模块 `dcc_mcp_core`、宿主协议保持上游命名，以便直接使用上游
  adapter 并 cherry-pick 上游修复。
- **DCC 范围**：Meshy 已有 Send To bridge 的 9 个 DCC。Krita 等非 Meshy DCC 不在范围。
- **Send To 统一**：webapp 的 `POST /import` 由 core 的 gateway 接收并路由到选中的宿主
  实例，旧端口 5324–5332 保留为兼容入口；用户每个 DCC 只装一个 Meshy 插件。先在
  Blender 验证。
- **Gateway 与 marketplace**：保留并改造为 Meshy 版本（见第 2 节）。

## 2. 上游可直接复用的部分

上游 CLI（本仓库 `crates/dcc-mcp-cli`）已具备本产品需要的主体能力：

| 能力 | 上游位置 |
|---|---|
| 读 FileRegistry 列出实例、直连宿主 `/mcp` 的 search / describe / load-skill / call | `application/local_instance.rs`、`local_control.rs` |
| 按项目启动宿主并等待就绪，同项目不重复启动 | `start-instance`，`application/instance_launch*` |
| 就绪等待、异步 job 轮询、安全停止 | `wait-ready`、`call --wait`、`stop-instance` |
| adapter 安装计划、诊断 | `install`、`doctor` |
| 宿主内主线程调度、嵌入式 HTTP server | `dcc-mcp-host`、`dcc-mcp-http` 及 Python wheel |

### Gateway：保留为 Meshy 本地 hub

上游宿主 adapter 默认就会选举 gateway：各宿主抢占 9765 端口，先抢到的那个在自己进程内
运行 gateway；它退出后其余宿主重新选举。删掉 daemon 并不能去掉 gateway，只会让它寄居
在某个随时可能关闭的 DCC 里。常驻 daemon 反而更稳定，也正是 Send To 统一所需要的：

- 浏览器读不到 FileRegistry，只能访问固定的本地 HTTP 端口。gateway 可以向 webapp 列出
  正在运行的实例（例如 "Blender — scene.blend"、"Unity — ProjectX"），把 `/import`
  路由到用户选中的那个实例，并对 meshy.ai 来源做 origin 检查。
- 旧端口 5324–5332 作为兼容入口转发到 gateway，webapp 可以先不改。
- 跨实例搜索、异步 job、事件推送也放在 gateway。

Agent 的 CLI 调用两条路径都保留：local profile 直连宿主，延迟最低；经 gateway
则可以使用聚合能力。

### Marketplace：改为 Meshy marketplace

上游 marketplace 的源就是一个 `marketplace.json` URL，包以 zip + SHA-256 校验，已实现
安装、升级、卸载和已安装状态。改造方式：

- 内置源改为托管在 `cdn.meshy.ai` 上的 Meshy catalog；包和 showcase 媒体也放 CDN，
  条目的主页链接回 meshy.ai 的对应页面。
- 首批包是 Meshy 官方 skill：各 DCC 的导入 / PBR 重建，以及"Meshy 资产 → 可玩场景"
  这类跨软件工作流。社区包经审核后发布到同一 catalog。
- 上游 `dcc-mcp/marketplace` 作为可选的第二个源，默认不启用，用户需要
  上游资产源时再添加。
- 上游对 dcc-mcp 发布流程身份的 sigstore 校验改为 Meshy 自己的签名身份；首版可以先只做
  SHA-256 固定。

## 3. 架构

```text
Agent → skills/meshy-dcc (SKILL.md)
      → meshy-dcc（单个 Rust 二进制，由 dcc-mcp-cli 改造）
          ├─ setup / doctor / repair          锁版本 + SHA-256 安装 adapter
          ├─ start-instance / list / wait-ready / stop-instance
          ├─ search / describe / load-skill / call   完整 instance UUID 路由
          ├─ asset check / import             manifest / 字节 / SHA / 格式校验 + 读回
          ├─ marketplace …                    Meshy catalog（cdn.meshy.ai）上的 skill 包
          └─ meshy …                          透传外部 meshy-cli 的 JSON 输出
      → 宿主内 adapter（上游固定版本）
          └─ Meshy skill 包：导入、PBR 材质重建、读回

webapp Send To → gateway daemon（Meshy 本地 hub，列出实例并路由 /import）→ 宿主
```

Meshy 特有能力放在各 DCC 的 skill 包中，这是上游原生的扩展点，不需要 patch
adapter。PBR 通道与色彩空间映射沿用 Send To bridge 已验证的做法（`textures`
分通道 URL，Unity SUP-3544 等修复）。

## 4. 从 JS 原型保留的行为约束

以下约束已在真实宿主上验证，移植为 Rust 测试，不移植 JS 实现：

- 发布物按 `runtime-lock` 的 SHA-256 校验后再解包；安装写 receipt，半成品不算已安装。
- 启动前先写待核对的 launch 记录；宿主已启动但记录写入失败时，保留 PID，禁止同项目
  直接重启或修复。
- 调用必须带完整 live instance UUID；实例失联不改投同类实例；进程存活不等于就绪。
- Blender 原生 dirty 标记不可靠：默认 stop 在状态未知时拒绝退出，不发信号。
- Unity 通过 Editor API 退出并核对进程；断连时不回退到信号。
- 资产交接复制到宿主工程内新目录并复核 SHA；结果未知时保留副本和回执，不重放导入。
- Unity FBX 贴图只接受 manifest 明确列出的单组通道；Godot 导入就绪跟踪 reimport 信号。

## 5. DCC 覆盖

| DCC | 上游 adapter | 路线 | 阶段 |
|---|---|---|---|
| Blender | `dcc-mcp-blender` | 直接接入；首个 Send To 统一验证 | R0/R1 |
| Unity | `dcc-mcp-unity`（C#） | 接入 + Meshy 导入/材质 skill | R1 |
| Godot | `dcc-mcp-godot` | 接入 + 导入就绪逻辑 | R1 |
| ZBrush | `dcc-mcp-zbrush`（需 2026.1+） | sidecar 路线 | R2 |
| Unreal | `dcc-mcp-unreal` | 接入；macOS 需 Xcode/Metal 工具链 | R2 |
| Roblox | 无 | 自研：Luau 插件长轮询 core，或对接 Studio 内置 MCP；资产走 Open Cloud | R2 |
| Maya | `dcc-mcp-maya` | 接入 | 暂缓（无验证宿主） |
| 3ds Max | `dcc-mcp-3dsmax` | 接入；仅 Windows | 暂缓 |
| Omniverse | 无（`dcc-mcp-openusd` 为 headless USD） | 自研：Kit 扩展加载 wheel；仅 Windows/Linux | 暂缓 |

只有真实 OS × 宿主 × adapter 验证通过的组合才进入支持矩阵。

## 6. 阶段

| 阶段 | 交付 | 验收 |
|---|---|---|
| R0 fork 骨架 | 经确认的改名与上游地址改向；锁定 Blender adapter | 真实 Blender：list → search → describe → call → 读回 |
| R1 对等 + Send To | setup/repair、asset check/import、meshy 透传、stop 保护；Unity、Godot；gateway 接收 Send To 并路由到 Blender；Meshy marketplace 首批包 | 达到 0.3.30 的主矩阵验收；webapp Send To 走 core 插件导入 Blender |
| R2 扩宿主 | ZBrush、Unreal、Roblox | 各宿主导入、读回、保存重开 |
| R3 评估 | 与既有 v2 对照同任务 | 单次调用延迟、turn 数、token、耗时；不夸大为质量结论 |

### R0 改造原则

- **默认不删除上游内容。** 没有被我们二进制依赖的 crate 不参与编译，保留没有成本；删除反而会
  在合并上游时产生冲突。只发布需要的二进制，而不是删减源码。
- 可能的改动只有两类：产品面改名，以及把写死的上游地址改指向 Meshy。每一项先说明用途和
  影响，经用户逐条确认后再改。
- 已核查并保留：`dcc-mcp-tunnel-*`（云端到本机的中继，可支撑 Meshy 网页端或托管 Agent
  远程驱动本地 DCC，也是浏览器访问 localhost 受限时的备选通道）、`dcc-mcp-semantic`
  （可选的向量检索 skill，marketplace 包变多后有用）、gateway daemon、marketplace。

## 7. 与上游的关系

跟踪上游 tag，按需 cherry-pick 或合并修复；产品面改名集中在少数文件，降低冲突。
对外说明 "Evolved from dcc-mcp-core"，保留上游 MIT LICENSE 与版权声明。重新分发
上游 adapter 或其依赖前逐一核对许可证。

## 8. 验证

```sh
cargo build -p dcc-mcp-cli
cargo test -p dcc-mcp-cli
```

真实宿主检查只启动测试自己创建的工程和进程，不触碰用户已有项目。本地开发记录在
被忽略的 `docs/local/`，不写入本机路径、实例 UUID 或私人任务日志。
