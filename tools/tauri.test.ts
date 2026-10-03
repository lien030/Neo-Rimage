import { afterEach, expect, it, vi } from "vitest";

const launch = vi.hoisted(() => vi.fn());
vi.mock("node:child_process", () => ({ spawnSync: launch }));

afterEach(() => {
  vi.unstubAllGlobals();
  launch.mockReset();
  vi.resetModules();
});

it.each(["dev", "build"])("prepares Windows native dependencies for %s without changing the parent environment", async (command) => {
  const originalEnvironment = process.env;
  const argumentsList = [command, "--config", '{"path":"C:/Images/My photos"}', "--", "--locked"];
  const exit = vi.fn();
  vi.stubGlobal("process", {
    ...process, platform: "win32", argv: ["node", "tauri.mjs", ...argumentsList], exit,
  });
  launch.mockReturnValue({ status: 0 });

  await import("./tauri.mjs");

  expect(launch).toHaveBeenCalledTimes(2);
  expect(launch.mock.calls[0][0]).toBe("powershell.exe");
  expect(launch.mock.calls[0][1].at(-1)).toMatch(/tools[\\/]build-windows\.ps1$/);
  const [executable, forwardedArguments, options] = launch.mock.calls[1];
  expect(executable).toBe(process.execPath);
  expect(forwardedArguments.slice(1)).toEqual(argumentsList);
  expect(options.env.PKG_CONFIG).toMatch(/x64-windows[\\/]tools[\\/]pkgconf[\\/]pkgconf\.exe$/);
  expect(options.env.PKG_CONFIG_PATH).toMatch(/x64-windows-static-md[\\/]lib[\\/]pkgconfig$/);
  expect(options.env.SYSTEM_DEPS_DAV1D_LINK).toBe("static");
  expect(options.env.SYSTEM_DEPS_DAV1D_BUILD_INTERNAL).toBe("never");
  expect(process.env).toBe(originalEnvironment);
  expect(exit).toHaveBeenCalledWith(0);
});

it.each([
  ["win32", ["--version"]],
  ["linux", ["dev"]],
])("forwards %s commands that do not require Windows preparation", async (platform, argumentsList) => {
  const exit = vi.fn();
  vi.stubGlobal("process", {
    ...process, platform, argv: ["node", "tauri.mjs", ...argumentsList], exit,
  });
  launch.mockReturnValue({ status: 7 });

  await import("./tauri.mjs");

  expect(launch).toHaveBeenCalledTimes(1);
  expect(launch.mock.calls[0][1].slice(1)).toEqual(argumentsList);
  expect(launch.mock.calls[0][2].env).toBe(process.env);
  expect(exit).toHaveBeenCalledWith(7);
});

it("does not start Tauri if native preparation fails", async () => {
  const failure = new Error("preparation failed");
  vi.stubGlobal("process", {
    ...process, platform: "win32", argv: ["node", "tauri.mjs", "dev"],
    exit: vi.fn(() => { throw failure; }),
  });
  launch.mockReturnValue({ status: 1 });

  await expect(import("./tauri.mjs")).rejects.toThrow(failure);
  expect(launch).toHaveBeenCalledTimes(1);
});
