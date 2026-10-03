# Build the release / 构建发行版

## English

Release **0.1.1** targets Windows x64 (`x86_64-pc-windows-msvc`). Use the source revision matching the binary, including vendored rimage and both lockfiles. This version does not upgrade the compiler, framework or installer toolchain.

Validated tools: Node.js **24.19.0**, pnpm **10.30.3**, Rust/Cargo **1.96.1**, MSVC C++ Build Tools with Windows SDK, NASM, and Windows VBScript for MSI generation. See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). Installer tools remain NSIS **3.11**, WiX **3.14.1**, nsis-tauri-utils **0.5.3**; sources/notices are in `licenses/INSTALLER-NOTICES.txt`. WebView2 is required at runtime, but no separate rimage, dav1d or vcpkg installation is needed.

### Native dependency

`tools/build-windows.ps1` pins vcpkg baseline **2c60af75f9d1ea85143242f92864ffa0dd2f78e7** and builds **dav1d 1.5.4** using `x64-windows-static-md`: static dav1d with dynamic MSVC CRT. pkgconf locates its library for dav1d-sys. The script changes only the current process environment; downloads, builds and binary caches stay under `src-tauri/target/native/`. Dot-source it in the same PowerShell session used for Cargo/Tauri. It does not modify global PATH or install a newer toolchain.

rimage uses an explicit feature whitelist with AVIF/TIFF/SVG/limits. Its CLI and overall threads feature remain disabled; ravif default features are disabled too. dav1d decoding uses one internal thread. This does not claim every third-party codec has no internal threads.

From the project root:

```powershell
pnpm install --frozen-lockfile
. ./tools/build-windows.ps1 -DependenciesOnly
cargo fetch --locked --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc
node tools/prepare-release.mjs
pnpm test
pnpm contracts:check
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
cargo test --locked --manifest-path src-tauri/Cargo.toml -p rimage --lib
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm tauri build --bundles nsis,msi --ci -- --locked
./tools/package-windows.ps1
```

The build hook runs `pnpm build`. Packaging writes the NSIS installer, MSI, portable ZIP and SHA256SUMS to `src-tauri/target/distribution/0.1.1/`. The ZIP includes the EXE and the same license materials as installers; copying only the EXE omits them. Artifacts are unsigned. These scripts do not push, tag or publish a Release.

### Source and licensing

Cargo/pnpm fetch exact packages from their lockfiles. `tools/prepare-release.mjs` inventories normal/build Rust dependencies, frontend packages processed by Vite, and **native dav1d outside the Cargo graph**. It verifies the native source SHA-512/version, adds BSD/ISC notices, and refreshes `docs/SOURCES.md` and `licenses/THIRD-PARTY-NOTICES.txt`. Run and review it for each release. Committed notices allow ordinary development without license lookups.

Published dependency archives, upstream revisions, native archive/checksum and fixed vcpkg recipe are linked in [SOURCES.md](SOURCES.md). npm repositories may be monorepos; package repository metadata identifies components. Rust archives include build scripts and bundled sources. Preserve notices when rebuilding/modifying. Registry access and first-time native/installer downloads require Internet; offline or byte-identical reproducibility is not claimed. General-purpose compiler executables are not corresponding source.

Original project source remains MIT. Combined binaries including imagequant are distributed under GPL-3.0-or-later; see [DISTRIBUTION.md](DISTRIBUTION.md). Native dav1d retains its own BSD/ISC notices.

### Validation boundary

See [v0.1.1-validation.md](v0.1.1-validation.md) for tests and measured parallel throughput. PE import inspection and running without native build environment variables check the static link on the development host. They are **not** substitutes for startup, conversion and install/uninstall tests on clean Windows without dav1d/vcpkg. Such a VM was unavailable during implementation; verify it before publication. macOS/Linux builds are not validated.

## 简体中文

**0.1.1** 面向 Windows x64。使用与二进制对应的源码提交、内置 rimage 和两个锁文件；本版本不升级编译器、框架或安装器工具链。工具版本及系统前提见上方；运行需要 WebView2，不需要独立 rimage、dav1d 或 vcpkg。

### 原生依赖与构建

`tools/build-windows.ps1` 固定上述 vcpkg baseline，构建 **dav1d 1.5.4 静态库、动态 MSVC CRT**。下载、构建及缓存均放在 `src-tauri/target/native/`，仅设置当前进程环境，不修改全局 PATH、不升级工具链。请在执行 Cargo/Tauri 的同一个 PowerShell 会话中点源运行脚本，再按上方命令验证、构建和打包。

保留 rimage 显式 feature 白名单，增加 AVIF/TIFF/SVG/limits，不启用 CLI 或整体线程特性；ravif 默认 feature 关闭，dav1d 内部解码线程固定为 1。这不表示所有第三方编码器都没有内部线程。

安装器、MSI、包含许可资料的便携 ZIP 与 SHA256SUMS 输出到 `src-tauri/target/distribution/0.1.1/`。安装包未签名。脚本不会 push、打 tag 或发布 GitHub Release。

### 源码、许可与验证

每次发行前运行并审核 `tools/prepare-release.mjs`。它根据锁文件盘点 Rust/前端依赖，并额外盘点 **Cargo 图之外的静态 dav1d**，核对源码 SHA-512 及版本，生成 BSD/ISC 声明、源码清单和第三方许可资料。精确源码、校验值及固定 vcpkg 配方见 [SOURCES.md](SOURCES.md)。保留许可；不宣称离线或逐字节一致的构建。通用编译工具的可执行文件不属于对应源码。

原创源码保持 MIT；包含 imagequant 的组合二进制按 GPL-3.0-or-later 分发，详见 [DISTRIBUTION.md](DISTRIBUTION.md)。便携包必须附带与安装器相同的许可资料，不能只复制 EXE。

测试及混合批处理实测见 [v0.1.1-validation.md](v0.1.1-validation.md)。开发机 PE 导入检查、移除原生构建环境后的启动，不能替代干净 Windows 启动、转换、安装/卸载验证；本次无可用干净虚拟机，请在发布前补验。macOS/Linux 未验证。
