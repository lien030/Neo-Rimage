# Neo Rimage v0.1.1 — draft / 草稿

## English

- Added static SDR AVIF, single-page TIFF, SVG and SVGZ input.
- Unified preparation, worker allocation and shared memory admission in JobManager. Memory-waiting images stay queued without taking a worker; small images can proceed, with an eight-bypass starvation barrier.
- SVG leading resize now renders vectors at the requested resolution, sharing cached system fonts. Local resources are restricted to the input directory tree; scripts/network access, excessive nesting and expanded resources are rejected.
- Added multilingual memory-wait status and structured format errors/warnings. The homepage brand now reads **Neo Rimage**, without changing the layout or application identifiers.
- Windows x64 statically links dav1d 1.5.4 with dynamic CRT. Users do not need dav1d or vcpkg installed. Bundles include the additional BSD/ISC notices and exact native source references.
- IPC schema is now 2; task configuration remains version 1. Existing output rules, backups and the 10 encoders remain unchanged.

### Limits

AVIF animations, grid images and PQ/HLG HDR are rejected. High-depth SDR is converted to RGBA8 with a warning; AVIF ICC profiles are not preserved. Multi-page TIFF and GIF inputs are unsupported. SVG expanded documents/resources have a combined 64 MiB cap and at most 8 nested levels. Memory admission is estimate-based, not an OS-enforced hard limit; it cannot guarantee the absence of OOM.

Windows installers are unsigned. WebView2 is required. Source remains MIT; combined binaries containing imagequant are distributed under GPL-3.0-or-later. See BUILDING.md, DISTRIBUTION.md and SOURCES.md.

## 简体中文

- 新增静态 SDR AVIF、单页 TIFF、SVG 和 SVGZ 输入。
- 在现有 JobManager 内统一预检、worker 分配与共享内存准入。等待内存的图片留在队列，不占 worker；小图可先执行，大图被绕过 8 次后形成防饥饿屏障。
- SVG 首个前置缩放直接按目标分辨率矢量渲染，复用系统字体缓存。资源限制在输入目录树内，明确拒绝网络、脚本、过深嵌套及展开内容超限。
- 增加中英日文等待内存状态及格式错误/警告。首页品牌改为 **Neo Rimage**，不改变布局或应用标识。
- Windows x64 静态链接 dav1d 1.5.4，CRT 保持动态链接；用户无需安装 dav1d 或 vcpkg。发行包附带新增 BSD/ISC 声明及固定原生源码清单。
- IPC schema 升为 2，任务配置版本仍为 1；输出规则、备份及 10 个编码器保持不变。

### 限制

拒绝 AVIF 动画、grid 拼图与 PQ/HLG HDR。高位深 SDR 转为 RGBA8 并提示，不保留 AVIF ICC 配置。不支持多页 TIFF 与 GIF 输入。SVG 展开内容及资源累计上限 64 MiB，嵌套最多 8 层。内存预算属于估算式准入，不是操作系统硬上限，不能保证绝不 OOM。

Windows 安装器未签名，运行需要 WebView2。原创源码维持 MIT；包含 imagequant 的组合二进制按 GPL-3.0-or-later 分发。详见 BUILDING.md、DISTRIBUTION.md、SOURCES.md。

This document is a release draft only. No tag, push or GitHub Release is created by this change.
本文件仅为发行说明草稿；本次改动不自动 push、打 tag 或发布 GitHub Release。
