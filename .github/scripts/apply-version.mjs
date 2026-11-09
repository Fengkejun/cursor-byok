import { readFile, writeFile } from "node:fs/promises";

const version = (process.argv[2] ?? "").trim().replace(/^v/, "");
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error(`invalid semantic version: ${version || "(empty)"}`);
  process.exit(1);
}

async function patchJson(path, mutate) {
  const value = JSON.parse(await readFile(path, "utf8"));
  mutate(value);
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`);
}

await patchJson("apps/desktop/src-tauri/tauri.conf.json", (value) => {
  value.version = version;
});
await patchJson("apps/desktop/package.json", (value) => {
  value.version = version;
});
await patchJson("apps/desktop/package-lock.json", (value) => {
  value.version = version;
  if (value.packages?.[""]) value.packages[""].version = version;
});

const cargoPath = "apps/desktop/src-tauri/Cargo.toml";
const cargo = await readFile(cargoPath, "utf8");
const patched = cargo.replace(/^version = "[^"]*"/m, `version = "${version}"`);
if (patched === cargo) {
  console.error("apps/desktop/src-tauri/Cargo.toml has no version line to patch");
  process.exit(1);
}
await writeFile(cargoPath, patched);

console.log(`applied release version ${version}`);
