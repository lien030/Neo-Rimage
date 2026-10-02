import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const result = spawnSync(
  "cargo",
  [
    "test",
    "--locked",
    "--lib",
    "domain::contracts::typescript_contract_matches_rust",
    "--",
    "--exact",
    "--nocapture",
  ],
  {
    cwd: fileURLToPath(new URL("../src-tauri", import.meta.url)),
    stdio: "inherit",
    env: {
      ...process.env,
      NEO_RIMAGE_GENERATE_CONTRACTS: process.argv.includes("--check") ? "0" : "1",
    },
  },
);

if (result.error) {
  throw result.error;
}

process.exit(result.status ?? 1);
