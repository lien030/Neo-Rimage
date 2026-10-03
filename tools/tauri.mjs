import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const project = fileURLToPath(new URL("../", import.meta.url));
const argumentsList = process.argv.slice(2);
let environment = process.env;

if (process.platform === "win32" && ["dev", "build"].includes(argumentsList[0])) {
  const setup = spawnSync("powershell.exe", [
    "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
    join(project, "tools/build-windows.ps1"),
  ], { cwd: project, stdio: "inherit" });
  if (setup.error) throw setup.error;
  if (setup.status !== 0) process.exit(setup.status ?? 1);

  const installed = join(project, "src-tauri/target/native/vcpkg/installed");
  environment = {
    ...process.env,
    PKG_CONFIG: join(installed, "x64-windows/tools/pkgconf/pkgconf.exe"),
    PKG_CONFIG_PATH: join(installed, "x64-windows-static-md/lib/pkgconfig"),
    SYSTEM_DEPS_DAV1D_LINK: "static",
    SYSTEM_DEPS_DAV1D_BUILD_INTERNAL: "never",
  };
}

const cli = createRequire(import.meta.url).resolve("@tauri-apps/cli/tauri.js");
const result = spawnSync(process.execPath, [cli, ...argumentsList], {
  cwd: project, stdio: "inherit", env: environment,
});
if (result.error) throw result.error;
process.exit(result.status ?? 1);
