# 轻量便签 (Lightweight Sticky Notes)

一个基于 **Tauri 2 + React 19 + Milkdown** 构建的 Windows 半透明 Markdown 便签应用。

## 功能

- **Markdown 编辑** — 所见即所得 (WYSIWYG) 编辑器，支持标题、列表、粗体、代码块、表格等常用语法
- **开机自启动** — 可配置登录后自动启动并最小化到系统托盘
- **窗口置顶** — 一键将便签置顶，不被其他窗口覆盖
- **半透明 Acrylic 材质** — Windows 原生亚克力模糊效果，可调节透明度，减少对桌面内容的遮挡
- **位置记忆** — 关闭时自动保存窗口位置和大小，再次打开时恢复
- **系统托盘** — 关闭窗口后驻留托盘，左键点开 / 右键菜单操作
- **多便签管理** — 侧边栏列表切换、新建、删除便签
- **暗色模式** — 支持浅色/深色主题切换

## 技术栈

| 层级 | 技术 |
| ---- | ---- |
| 桌面框架 | Tauri 2 (Rust) |
| 前端 | React 19 + TypeScript |
| 编辑器 | Milkdown Crepe (ProseMirror) |
| 样式 | CSS (无框架依赖) |
| 状态管理 | Zustand |
| 打包 | NSIS |

## 开发

### 环境要求

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://www.rust-lang.org/) (通过 `rustup` 安装)

### 启动开发模式

```bash
npm install
npm run tauri dev
```

### 构建生产版本

```bash
npm run tauri build
```

构建产物位于 `src-tauri/target/release/`。

## 安装

运行 `LightweightStickyNotes_Setup.exe`，按提示完成安装。默认安装路径：

```text
%LOCALAPPDATA%\Programs\LightweightStickyNotes
```

## 数据存储

便签数据存储在：

```text
%APPDATA%\com.liurr.lightweight-sticky-notes\
├── notes_index.json      # 便签索引
└── notes\
    ├── <uuid>.md         # 便签内容（Markdown 格式）
    └── ...
```

## 项目结构

```text
notebook/
├── src/                  # React 前端
│   ├── components/       # UI 组件
│   ├── stores/           # Zustand 状态管理
│   ├── types/            # TypeScript 类型
│   └── App.tsx           # 根组件
├── src-tauri/            # Rust 后端
│   └── src/
│       ├── main.rs       # 入口
│       └── lib.rs        # 命令与托盘逻辑
└── package.json
```

## License

MIT
