import assert from "node:assert/strict";
import { execFile, execFileSync } from "node:child_process";
import { readFile, readdir, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { createHash } from "node:crypto";
import { build } from "vite";

const root = fileURLToPath(new URL("../", import.meta.url));
process.chdir(root);
const application = JSON.parse(await readFile("package.json", "utf8"));
const host = execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host: (.+)$/m)[1];
const target = process.env.TAURI_ENV_TARGET_TRIPLE || process.env.CARGO_BUILD_TARGET || host;
const cargo = (args) => execFileSync("cargo", [...args, "--manifest-path", "src-tauri/Cargo.toml"], {
  encoding: "utf8", maxBuffer: 32 * 1024 * 1024,
});
const metadata = JSON.parse(cargo(["metadata", "--locked", "--format-version", "1", "--filter-platform", target]));
const tree = cargo(["tree", "--locked", "--target", target, "--edges", "normal,build", "--prefix", "none", "--format", "{p}"]);
const rustKeys = new Set([...tree.matchAll(/^(\S+) v(\S+)/gm)].map((match) => `${match[1]}@${match[2]}`));
const rustPackages = metadata.packages.filter((entry) => rustKeys.has(`${entry.name}@${entry.version}`));
const lock = await readFile("src-tauri/Cargo.lock", "utf8");
const checksums = new Map(lock.split("[[package]]").flatMap((entry) => {
  const name = entry.match(/^name = "(.+)"$/m)?.[1];
  const version = entry.match(/^version = "(.+)"$/m)?.[1];
  const checksum = entry.match(/^checksum = "(.+)"$/m)?.[1];
  return checksum ? [[`${name}@${version}`, checksum]] : [];
}));
const projectSource = `https://github.com/lien030/Neo-Rimage/tree/v${application.version}`;
const sources = [];
const notices = [];
const licenseTexts = new Map();
const missingLicenses = [];
const runFile = promisify(execFile);

async function upstreamNotices(repository, revision) {
  assert(repository?.startsWith("https://github.com/") && /^[a-f0-9]{40}$/.test(revision || ""), "Missing exact license source");
  const directory = path.join(root, "src-tauri/target/release/license-source-cache", createHash("sha256").update(repository).digest("hex"));
  await mkdir(directory, { recursive: true });
  const git = async (args) => (await runFile("git", ["-C", directory, ...args], {
    encoding: "utf8", maxBuffer: 8 * 1024 * 1024, timeout: 120000,
  })).stdout;
  await git(["init", "--bare", "--quiet"]);
  await git(["config", "remote.origin.url", repository]);
  await git(["config", "remote.origin.promisor", "true"]);
  await git(["config", "remote.origin.partialclonefilter", "blob:none"]);
  try {
    await git(["cat-file", "-e", `${revision}^{commit}`]);
  } catch {
    await git(["fetch", "--quiet", "--filter=blob:none", "--depth=1", "origin", revision]);
  }
  const filenames = (await git(["ls-tree", "--name-only", revision])).trim().split(/\r?\n/)
    .filter((filename) => /^(?:licen[cs]e|copying|copyright|notice)(?:$|[-_.])/i.test(filename));
  const records = [];
  for (const filename of filenames) records.push({ filename: `${repository}/blob/${revision}/${filename}`, text: await git(["show", `${revision}:${filename}`]) });
  return records;
}

async function upstreamSource(published, entry) {
  const repository = (typeof published.repository === "string" ? published.repository : published.repository?.url || "")
    .replace(/^git\+/, "").replace(/^git:/, "https:").replace(/^ssh:\/\/git@github.com\//, "https://github.com/").replace(/\.git$/, "");
  assert(repository.startsWith("https://github.com/"), `Upstream repository must be reviewed for ${entry.name}`);
  let revision = published.gitHead;
  if (`${entry.name}@${entry.version}` === "react-remove-scroll-bar@2.3.8") {
    assert.equal(published.dist.integrity, "sha512-9r+yi9+mgU33AKcj6IbT9oRCO78WriSj6t/cF8DWBZJ9aOGPOTEDvdUDz1FwKim7QXWwmHqtdHnRJfhAxEG46Q==");
    revision = "8ca9ba5ea52de03308fe8ced94f7b159a44d28ff";
  }
  if (`${entry.name}@${entry.version}` === "react-style-singleton@2.2.3") {
    assert.equal(published.dist.integrity, "sha512-b6jSvxvVnyptAiLjbkWLE/lOnR4lfTtDAl+eUC7RZy+QQWc6wRzIV2CE6xBuMmDxc2qIihtDCZD5NPOFl7fRBQ==");
    revision = "0855d4b3478326fda30efff7e0511540be7f57e8";
  }
  if (!/^[a-f0-9]{40}$/.test(revision || "") && published.dist.attestations) {
    const response = await fetch(published.dist.attestations.url, { signal: AbortSignal.timeout(30000) });
    assert(response.ok, `Cannot retrieve source provenance for ${entry.name}`);
    const records = await response.json();
    const record = records.attestations.find((entry) => entry.predicateType === "https://slsa.dev/provenance/v1");
    if (record) {
      const statement = JSON.parse(Buffer.from(record.bundle.dsseEnvelope.payload, "base64").toString("utf8"));
      const integrity = Buffer.from(published.dist.integrity.replace(/^sha512-/, ""), "base64").toString("hex");
      assert(statement.subject.some((subject) => subject.digest.sha512 === integrity), `Source provenance digest differs for ${entry.name}`);
      const dependency = statement.predicate.buildDefinition.resolvedDependencies.find((entry) => entry.uri.startsWith(`git+${repository}@`));
      revision = dependency?.digest.gitCommit;
    }
  }
  if (!/^[a-f0-9]{40}$/.test(revision || "")) {
    const tags = [...new Set([`${entry.name}@${entry.version}`, `${entry.name.split("/").at(-1)}@${entry.version}`, `v${entry.version}`, entry.version])];
    const result = await runFile("git", ["ls-remote", "--tags", repository,
      ...tags.flatMap((tag) => [`refs/tags/${tag}`, `refs/tags/${tag}^{}`])],
      { encoding: "utf8", maxBuffer: 2 * 1024 * 1024, timeout: 30000 });
    const references = new Map(result.stdout.trim().split(/\r?\n/).map((line) => line.split(/\s+/).reverse()));
    const tag = tags.find((candidate) => references.has(`refs/tags/${candidate}`));
    assert(tag, `Exact upstream source commit must be reviewed for ${entry.name}@${entry.version}`);
    revision = references.get(`refs/tags/${tag}^{}`) || references.get(`refs/tags/${tag}`);
  }
  assert(/^[a-f0-9]{40}$/.test(revision), `Invalid upstream commit for ${entry.name}`);
  return `${repository}/archive/${revision}.tar.gz`;
}

async function appendNotices(name, directory, explicitFile, upstream) {
  const entries = await readdir(directory, { recursive: true, withFileTypes: true });
  const filenames = entries.filter((entry) => entry.isFile() &&
    /^(?:licen[cs]e|copying|copyright|notice|patents)(?:$|[-_.])/i.test(entry.name) &&
    !/\.(?:png|jpg|pdf|rs|c|h|json)$/i.test(entry.name))
    .map((entry) => path.join(entry.parentPath, entry.name));
  if (explicitFile) filenames.push(path.resolve(directory, explicitFile));
  const uniqueFiles = [...new Set(filenames)].sort();
  notices.push(`\n${"=".repeat(72)}\n${name}\n${"=".repeat(72)}\n`);
  if (!uniqueFiles.length) {
    if (!upstream) missingLicenses.push(name);
    else {
      const records = await upstreamNotices(upstream.repository, upstream.revision);
      if (!records.length && name.startsWith("simd_helpers@0.1.0")) {
        records.push(...await upstreamNotices(upstream.repository, "82040194cd05affb060bf94d6f19f82a771d07fb"));
      }
      if (!records.length && name.startsWith("selectors@")) {
        const header = (await readFile(path.join(directory, "lib.rs"), "utf8")).match(/^\/\*[\s\S]*?\*\//)?.[0];
        const cssparser = rustPackages.find((entry) => entry.name === "cssparser");
        assert(header?.includes("Mozilla Public") && cssparser, "selectors MPL notice must be reviewed");
        records.push({ filename: "selectors/lib.rs notice and MPL-2.0 license text", text: `${header}\n${await readFile(path.join(path.dirname(cssparser.manifest_path), "LICENSE"), "utf8")}` });
      }
      if (!records.length) missingLicenses.push(name);
      for (const record of records) appendLicense(record.filename, record.text);
    }
  }
  for (const filename of uniqueFiles) {
    appendLicense(path.relative(directory, filename).replaceAll("\\", "/"), await readFile(filename, "utf8"));
  }
}

function appendLicense(filename, text) {
  const normalized = text.replaceAll("\r\n", "\n");
  if (!licenseTexts.has(normalized)) licenseTexts.set(normalized, licenseTexts.size + 1);
  notices.push(`--- ${filename} --- Full text: [${licenseTexts.get(normalized)}]\n`);
}

for (const entry of rustPackages.sort((first, second) => `${first.name}@${first.version}`.localeCompare(`${second.name}@${second.version}`, "en"))) {
  if (entry.name === application.name) continue;
  const key = `${entry.name}@${entry.version}`;
  const source = entry.source?.startsWith("registry+")
    ? `https://static.crates.io/crates/${entry.name}/${entry.name}-${entry.version}.crate`
    : `${projectSource}/src-tauri/vendor/${entry.name}`;
  const checksum = checksums.get(key) || "Included in project source / 包含于项目源码";
  const directory = path.dirname(entry.manifest_path);
  let upstream;
  try {
    const vcs = JSON.parse(await readFile(path.join(directory, ".cargo_vcs_info.json"), "utf8"));
    const repository = entry.repository || (entry.name === "zune-inflate" ? "https://github.com/etemesi254/zune-image" : null);
    if (repository?.startsWith("https://github.com/")) upstream = { repository: repository.split(/\/(?:tree|blob)\//)[0].replace(/\.git$/, ""), revision: vcs.git.sha1 };
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  sources.push(`| Rust | ${key} | ${entry.license || "See source"} | [Source / 源码](${source}) | ${checksum} |`);
  await appendNotices(`${key} — ${entry.license || "See source"}`, directory, entry.license_file, upstream);
}

const native = path.join(root, "src-tauri/target/native");
const dav1dChecksum = "7ee5906640495919462b2242c44a8c3cc577fdda52fc25257792f6df919429d54e4a48c315a7a11759385f044f68d3d6573fd720853f447e2d8520a13693827f";
const dav1dArchive = await readFile(path.join(native, "downloads/videolan-dav1d-1.5.4.tar.gz"));
assert.equal(createHash("sha512").update(dav1dArchive).digest("hex"), dav1dChecksum, "dav1d source archive checksum differs");
const dav1dPc = await readFile(path.join(native, "vcpkg/installed/x64-windows-static-md/lib/pkgconfig/dav1d.pc"), "utf8");
assert.match(dav1dPc, /^Version: 1\.5\.4$/m);
sources.push(`| Native static | dav1d@1.5.4 | BSD-2-Clause AND ISC | [Source](https://github.com/videolan/dav1d/archive/1.5.4.tar.gz), commit 54706fc6bc0cdecab7e9593974a4039cc038fca7; [vcpkg recipe](https://github.com/microsoft/vcpkg/tree/2c60af75f9d1ea85143242f92864ffa0dd2f78e7/ports/dav1d) | SHA512 ${dav1dChecksum} |`);
notices.push(`\n${"=".repeat(72)}\ndav1d@1.5.4 — BSD-2-Clause AND ISC (static library, dynamic CRT)\n${"=".repeat(72)}\n`);
appendLicense("dav1d COPYING and x86inc.asm", await readFile(path.join(native, "vcpkg/installed/x64-windows-static-md/share/dav1d/copyright"), "utf8"));

const modules = new Set();
await build({
  build: { write: false },
  plugins: [{ name: "release-source-inventory", transform(code, id) {
    if (!id.includes("\0") && id.includes("node_modules")) modules.add(id.replace(/\?.*$/, ""));
  } }],
});
const frontendPackages = new Map();
const fontDirectory = path.join(root, "node_modules/@fontsource-variable/geist");
const font = JSON.parse(await readFile(path.join(fontDirectory, "package.json"), "utf8"));
frontendPackages.set(`${font.name}@${font.version}`, { ...font, directory: fontDirectory });
const stylesheet = await readFile("src/index.css", "utf8");
for (const [, imported] of stylesheet.matchAll(/^@import "([^"]+)"/gm)) {
  const name = imported.split("/").slice(0, imported.startsWith("@") ? 2 : 1).join("/");
  const directory = path.join(root, "node_modules", name);
  const entry = JSON.parse(await readFile(path.join(directory, "package.json"), "utf8"));
  frontendPackages.set(`${entry.name}@${entry.version}`, { ...entry, directory });
}
for (const module of [...modules].sort()) {
  let directory = path.dirname(module);
  while (directory.includes("node_modules")) {
    try {
      const entry = JSON.parse(await readFile(path.join(directory, "package.json"), "utf8"));
      if (entry.name && entry.version) {
        frontendPackages.set(`${entry.name}@${entry.version}`, { ...entry, directory });
        break;
      }
    } catch (error) {
      if (error.code !== "ENOENT" && error.code !== "ENOTDIR") throw error;
    }
    directory = path.dirname(directory);
  }
}
const frontendEntries = [...frontendPackages.entries()].sort(([first], [second]) => first.localeCompare(second, "en"));
for (let offset = 0; offset < frontendEntries.length; offset += 6) {
  const batch = await Promise.all(frontendEntries.slice(offset, offset + 6).map(async ([key, entry]) => {
    const response = await fetch(`https://registry.npmjs.org/${encodeURIComponent(entry.name)}/${entry.version}`, {
      signal: AbortSignal.timeout(30000),
    });
    assert(response.ok, `Cannot retrieve published source metadata for ${key}: ${response.status}`);
    const published = await response.json();
    assert.equal(published.name, entry.name);
    assert.equal(published.version, entry.version);
    assert(published.dist?.tarball && published.dist?.integrity, `Missing archive integrity for ${key}`);
    const upstream = await upstreamSource(published, entry);
    return { key, entry, upstream: { repository: upstream.split("/archive/")[0], revision: upstream.split("/archive/")[1].replace(/\.tar\.gz$/, "") }, row: `| Frontend | ${key} | ${published.license || entry.license || "See source"} | [Published package / 发布包](${published.dist.tarball}); [Upstream source / 上游源码](${upstream}) | ${published.dist.integrity} |` };
  }));
  for (const { key, entry, row, upstream } of batch) {
    sources.push(row);
    await appendNotices(`${key} — ${entry.license || "See source"}`, entry.directory, undefined, upstream);
  }
}

const imagequant = rustPackages.find((entry) => entry.name === "imagequant");
assert(imagequant, "The current GPL distribution configuration expects imagequant to be enabled");
assert.equal(imagequant.license, "GPL-3.0-or-later");
assert(frontendPackages.has(`react@${application.dependencies.react.replace(/^\^/, "")}`), "React source was not inventoried");
assert(frontendPackages.has(`${font.name}@${font.version}`), "The bundled font license was not inventoried");
assert.equal(missingLicenses.length, 0, `Published packages without license files need review: ${missingLicenses.join(", ")}`);
const sourceText = `# Corresponding source / 对应源码\n\nVersion / 版本: **${application.version}**. Target / 目标平台: **${target}**.\n\nProject source and local modifications / 项目源码及本地修改: [v${application.version}](${projectSource}); [GitHub source archive / GitHub 源码归档](https://github.com/lien030/Neo-Rimage/archive/refs/tags/v${application.version}.zip).\n\nBuild instructions / 构建说明: [BUILDING.md](BUILDING.md). Distribution terms / 发行许可: [DISTRIBUTION.md](DISTRIBUTION.md).\n\nThe following fixed-version archives complement the project source archive. Rust archives contain crate sources and bundled native sources; npm package archives are the published build inputs, with exact upstream source commits linked separately for modification. Preserve Cargo.lock and pnpm-lock.yaml and verify the listed checksums. General-purpose compilers and system libraries are not included. The Rust inventory includes normal and build dependencies; the frontend inventory includes packages processed by Vite, including code subsequently removed by tree shaking. Each component retains its own license.\n\n下列固定版本源码归档补充项目源码包。Rust 归档包含 crate 源码及其内置原生源码；npm 归档为实际发布的构建输入，并额外链接可供修改的上游精确提交源码。请保留 Cargo.lock、pnpm-lock.yaml，并校验所列哈希。通用编译器与系统库不列入清单。Rust 清单包含普通和构建依赖；前端清单包含 Vite 处理过的包，包括随后被摇树优化移除的代码。各组件保留各自许可证。\n\nThese sources are hosted by their upstream providers. The distributor remains responsible for availability of the complete corresponding source. If an archive becomes unavailable or does not contain the preferred source, report it at https://github.com/lien030/Neo-Rimage/issues so the source can be supplied or the reference corrected. A repository homepage or lockfile alone is not a replacement for corresponding source.\n\n源码由上游托管，发行者仍负责完整对应源码的可获取性。如归档失效或缺少适合修改的源码，请通过 https://github.com/lien030/Neo-Rimage/issues 反馈，以便补充源码或修正链接。仓库首页或锁文件本身不能替代对应源码。\n\n| Kind / 类别 | Component / 组件 | Declared license / 声明许可 | Fixed source / 固定源码 | Archive checksum / 归档校验 |\n| --- | --- | --- | --- | --- |\n${sources.join("\n")}\n`;
const noticeText = `neo-rimage ${application.version} — Third-party notices\n\nOriginal neo-rimage source: MIT. The combined application containing imagequant is distributed under GPL-3.0-or-later. Each third-party component retains its original license. See DISTRIBUTION.md, GPL-3.0.txt, PROJECT-MIT.txt and SOURCES.md.\n\nThe notices below reproduce license and copyright files from the normal/build Rust dependency graph and the packages processed by the frontend build. Build-time or tree-shaken components may also appear; inclusion is not a claim that every component is linked into the executable.\n${notices.join("\n")}\n`;
assert(!sourceText.includes(root) && !noticeText.includes(root), "Local workspace paths must not be shipped");
await mkdir("licenses", { recursive: true });
await writeFile("docs/SOURCES.md", `${sourceText}\n## Reviewed source mapping / 已审核的源码映射\n\nreact-remove-scroll-bar 2.3.8 publishes an unavailable gitHead. All 24 files in its dist directory were verified byte-for-byte identical to the published 2.3.7 package. The linked commit 8ca9ba5ea52de03308fe8ced94f7b159a44d28ff contains the 2.3.7 TypeScript source plus the upstream MIT LICENSE addition; no source changes were made between that release and this commit. The 2.3.8 tarball remains the exact locked build input.\n\nreact-remove-scroll-bar 2.3.8 的 gitHead 无法从上游获取。其 dist 目录内的 24 个文件经逐字节校验，与已发布的 2.3.7 完全一致。所链接的固定提交包含 2.3.7 的 TypeScript 源码及随后增加的上游 MIT LICENSE，该版本至此提交没有源码改动；2.3.8 发布包仍是锁定的实际构建输入。\n`);
await writeFile("docs/SOURCES.md", "\nreact-style-singleton 2.2.3 also publishes an unavailable gitHead. All files in its dist directory were verified byte-for-byte identical to the published 2.2.2 package, whose tagged commit 0855d4b3478326fda30efff7e0511540be7f57e8 is linked as the preferred source. The 2.2.3 tarball is the locked build input.\n\nreact-style-singleton 2.2.3 的 gitHead 同样不可获取。其 dist 目录全部文件与已发布的 2.2.2 经逐字节校验完全一致，因此链接 2.2.2 标签的固定提交源码；实际构建输入仍为 2.2.3 发布包。\n\nsimd_helpers 0.1.0 declares MIT in Cargo.toml but omits the license file. Its notice is taken from upstream commit 82040194cd05affb060bf94d6f19f82a771d07fb, which only adds LICENSE to the published source commit.\n\nsimd_helpers 0.1.0 在 Cargo.toml 中声明 MIT，但缺少许可全文。声明取自上游固定提交 82040194cd05affb060bf94d6f19f82a771d07fb，与发布源码相比仅新增 LICENSE 文件。\n", { flag: "a" });
await writeFile("licenses/THIRD-PARTY-NOTICES.txt", `${noticeText}\nFull license and copyright texts\n${[...licenseTexts].map(([text, number]) => `\n${"=".repeat(72)}\n[${number}]\n${"=".repeat(72)}\n${text}`).join("\n")}`);
await writeFile("licenses/GPL-3.0.txt", await readFile(path.join(path.dirname(imagequant.manifest_path), "COPYRIGHT")));
await writeFile("licenses/PROJECT-MIT.txt", await readFile("LICENSE"));
console.log(`Prepared ${rustPackages.length - 1} Rust and ${frontendPackages.size} frontend source references and third-party notices for ${target}.`);
