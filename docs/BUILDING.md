# Build the release / 构建发行版

## English

Release **0.1.0** targets Windows x64 (`x86_64-pc-windows-msvc`). Use the source at tag `v0.1.0`, including the vendored rimage source and both lockfiles. The release preparation only adds packaging/license documentation; image processing features and the UI are unchanged.

Installer tools: NSIS **3.11**, WiX **3.14.1**, nsis-tauri-utils **0.5.3**. Their unchanged source links and notices are in `licenses/INSTALLER-NOTICES.txt`; review this file when upgrading the installer tooling.

Validated tools: Node.js **24.19.0**, pnpm **10.30.3**, Rust/Cargo **1.96.1**, MSVC C++ Build Tools with a Windows SDK, NASM, and the Windows VBScript feature for MSI generation. See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). WebView2 Runtime is required to run the app, not a separate rimage executable. Toolchain executables are general-purpose build tools and are not included as corresponding source.

From the project root:

```powershell
pnpm install --frozen-lockfile
cargo fetch --locked --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc
node tools/prepare-release.mjs
pnpm test
pnpm contracts:check
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
cargo test --locked --manifest-path src-tauri/Cargo.toml -p rimage --lib
pnpm tauri build --bundles nsis,msi --ci -- --locked
```

Cargo and pnpm fetch the exact registry packages specified by the lockfiles. Their published source archives and original upstream source commits are linked in [SOURCES.md](SOURCES.md). Upstream frontend source archives may be monorepos: the npm package's repository metadata identifies the component directory. Use the published npm package archive as the locked build input and the linked upstream source for modifications. Rust crate archives include their build scripts and bundled native sources. Preserve notices if rebuilding or modifying components.

The configured build hook runs `pnpm build`. Run `node tools/prepare-release.mjs` before preparing a new release: it inventories the target's normal/build Rust dependency graph and frontend packages processed by Vite, refreshes `docs/SOURCES.md`, and generates license resources in `licenses/`. These resources are committed so ordinary development and tests do not need network license lookups. The preparation script retrieves public npm source metadata and fails if an exact source commit or a license file needs review. Internet access is needed for registry sources/metadata and Tauri's installer tooling on the first build. This is not an offline-build claim.

The EXE is written to `src-tauri/target/release/neo-rimage.exe`; installers are under `src-tauri/target/release/bundle/`. For a portable archive, include the EXE and the same `licenses` directory as the installers; copying only the EXE omits its accompanying license materials. For a future version, update the application version consistently and regenerate/review source references before tagging. Byte-identical reproducibility is not claimed.

## 简体中文

**0.1.0** 发行版面向 Windows x64（`x86_64-pc-windows-msvc`）。使用 `v0.1.0` 标签中的源码，保留内置 rimage 源码及两个锁文件。本次发行准备只增加打包和许可资料，不改变图像处理功能或 UI。

安装器工具版本：NSIS **3.11**、WiX **3.14.1**、nsis-tauri-utils **0.5.3**。未修改的上游源码链接及许可见 `licenses/INSTALLER-NOTICES.txt`，升级安装器工具后须审核该文件。

已验证工具：Node.js **24.19.0**、pnpm **10.30.3**、Rust/Cargo **1.96.1**、包含 Windows SDK 的 MSVC C++ Build Tools、NASM，以及生成 MSI 所需的 Windows VBScript 功能。系统前提见上方 Tauri 链接。运行应用需要 WebView2 Runtime，不需要额外的 rimage 可执行程序。通用构建工具的可执行文件不列入对应源码。

在项目根目录执行上面的命令。Cargo 和 pnpm 根据锁文件下载精确版本的注册表包；[SOURCES.md](SOURCES.md) 链接了这些源码归档及前端包的上游源码提交。前端上游可能为 monorepo，包的 repository 元数据标明组件目录。已发布的 npm 归档是锁定的构建输入，上游源码用于修改；Rust crate 归档包含构建脚本及内置原生源码。重新构建或修改时请保留许可声明。

构建钩子运行 `pnpm build`。准备新版本发行前执行 `node tools/prepare-release.mjs`，盘点目标平台的普通/构建 Rust 依赖及 Vite 处理过的前端包，刷新 `docs/SOURCES.md`，并在 `licenses/` 生成许可资源。这些资料提交到仓库，日常开发和测试无需联网查询许可。脚本遇到缺少精确源码提交或许可证文件时会要求检查，并读取公开 npm 元数据。首次下载依赖和安装器工具也需要联网，不宣称离线构建。

主程序和安装包分别输出到上方列出的目录。制作便携包时，将 EXE 与安装器中相同的 `licenses` 文件夹一起打包，不能仅复制 EXE 而遗漏许可资料。后续版本须统一更新版本号，并在打标签前重新生成和审核源码清单。不保证逐字节一致的可复现构建。
