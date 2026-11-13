# 在 macOS 上安装 Cursor BYOK

本仓库发布的 macOS 版本与本机（Windows）版本功能一致：本地模型网关、Cursor 接管、Codex / Claude Code / DSH 接管、模型插件、外部 API 全部保留。

## 1. 下载

打开 <https://github.com/Fengkejun/cursor-byok/releases/latest>，按芯片选择：

| 你的 Mac | 下载文件 |
| --- | --- |
| Apple Silicon（M1/M2/M3/M4） | `Cursor.BYOK_*_aarch64.dmg` |
| Intel | `Cursor.BYOK_*_x64.dmg` |

判断芯片：左上角  → 关于本机 → 看“芯片”一栏是 Apple 还是 Intel。

## 2. 安装

1. 双击 `.dmg`，把 **Cursor BYOK** 拖进 `Applications`。
2. 第一次打开前，先解除隔离属性（因为本仓库的构建是 ad-hoc 签名，没有 Apple 开发者公证）：

```bash
xattr -dr com.apple.quarantine "/Applications/Cursor BYOK.app"
```

3. 打开 **Cursor BYOK**。若系统提示“无法验证开发者”，也可以改用：在 Finder 里右键 App → **打开** → 再点 **打开**。

## 3. 首次运行要做的事

1. **安装本地 CA**：应用会生成自己的根证书，界面里点“安装证书”会弹出终端执行：

   ```bash
   sudo security add-trusted-cert -d -r trustRoot -p ssl -k /Library/Keychains/System.keychain '<数据目录>/ca/ca.crt'
   ```

   需要输入你的 Mac 登录密码。装完回到应用确认状态变成“已信任”。

2. **开启 Cursor 接管**：应用会把 Cursor 的代理设置写进
   `~/Library/Application Support/Cursor/User/settings.json`，然后自动重启 Cursor
   （`pkill -x Cursor` 再 `open -a Cursor`）。若没有自动重启，手动退出 Cursor 再打开。

3. **添加模型**：在“模型设置”里填服务地址、API Key、模型 ID，点测试通过后回主页启动服务。

4. **在 Cursor 里使用**：完全退出 Cursor 后重新打开，新建对话，选择刚配置的模型。

## 4. 数据与配置位置（macOS）

| 内容 | 路径 |
| --- | --- |
| 数据目录（数据库、CA、日志、插件） | `~/.cursor-byok-v3/` |
| 日志 | `~/.cursor-byok-v3/logs/` |
| Cursor 用户设置 | `~/Library/Application Support/Cursor/User/settings.json` |
| Cursor 账号状态 | `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb` |
| Codex 配置 | `~/.codex/config.toml`、`~/.codex/auth.json` |
| Claude Code 配置 | `~/.claude/settings.json` |
| DSH 配置 | `~/.dsh/profiles/desktop/cordis.patch.yml` |

数据目录结构与 Windows 完全一致，所以备份/迁移只要拷贝 `~/.cursor-byok-v3/`。

## 5. 开机自启与托盘

macOS 使用 LaunchAgent（`~/Library/LaunchAgents/`）实现开机自启，可在设置里开关。
关闭主窗口后应用会留在菜单栏托盘中，转发服务继续运行；点托盘图标可重新打开窗口，选“退出”才真正结束。

## 6. 更新

macOS 走 Tauri 内置更新器（读取同一个 `latest.json`）：

- 菜单/设置里的“检查更新”会对比 GitHub Release 的最新版本；
- 更新包经 minisign 签名校验，签名不匹配会拒绝安装；
- 便携 zip 自动替换只在 Windows 上存在，macOS 用标准的 App 覆盖更新。

如果没有发布新版本，检查更新会直接提示已是最新。

## 7. 常见问题

**打开时提示“已损坏，无法打开”**
说明隔离属性没去掉或签名被改：重新执行第 2 步的 `xattr -dr com.apple.quarantine`。仍不行就重新下载。

**Cursor 重启后仍然走官方接口**
检查 `settings.json` 里是否写入了 `http.proxy`，以及应用首页“Cursor 接管”是否为已开启；确认后完全退出 Cursor（`⌘Q`）再打开。

**证书安装后 Cursor 仍报证书错误**
用“钥匙串访问”打开 `系统` 钥匙串，找到 `Cursor BYOK Local CA`，确认信任设置为“始终信任”。若之前装过旧证书，先删掉旧的再重装。

**端口被占用**
应用默认使用 `127.0.0.1` 上的动态端口（管理服务 61702、代理 64625）；被占用时可在“设置 → 端口”里改。

## 8. 从源码构建（可选）

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Node.js 22+
git clone https://github.com/Fengkejun/cursor-byok.git
cd cursor-byok/apps/desktop
npm ci
npm run tauri build
```

产物：

- `apps/desktop/src-tauri/target/release/bundle/macos/Cursor BYOK.app`
- `apps/desktop/src-tauri/target/release/bundle/dmg/*.dmg`

需要在 CI 之外自行构建带签名的安装包时，设置 `TAURI_SIGNING_PRIVATE_KEY` 后
`npm run tauri build` 才会生成可被自动更新器接受的 `.app.tar.gz` 签名。
