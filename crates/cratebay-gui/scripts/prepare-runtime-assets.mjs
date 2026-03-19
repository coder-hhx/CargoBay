import fs from "node:fs"
import path from "node:path"
import { spawnSync } from "node:child_process"
import { fileURLToPath } from "node:url"

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const repoRoot = path.resolve(__dirname, "../../..")
const srcTauriDir = path.resolve(__dirname, "../src-tauri")
const mode = process.argv[2] === "dev" ? "dev" : "build"

if (process.env.CRATEBAY_SKIP_RUNTIME_ASSET_PREP === "1") {
  process.exit(0)
}

function normalizeArch(value) {
  switch ((value || "").toLowerCase()) {
    case "x64":
    case "x86_64":
    case "amd64":
      return "x86_64"
    case "arm64":
    case "aarch64":
      return "aarch64"
    default:
      return ""
  }
}

function detectTargetArch() {
  for (const value of [
    process.env.CRATEBAY_RUNTIME_ASSET_ARCH,
    process.env.CARGO_BUILD_TARGET?.split("-")[0],
    process.env.TAURI_ENV_TARGET_TRIPLE?.split("-")[0],
    process.env.TAURI_ENV_ARCH,
    process.arch,
  ]) {
    const normalized = normalizeArch(value)
    if (normalized) {
      return normalized
    }
  }
  throw new Error(`Unsupported runtime asset arch: ${process.arch}`)
}

function detectMacTargetTriple(arch) {
  for (const value of [process.env.TAURI_ENV_TARGET_TRIPLE, process.env.CARGO_BUILD_TARGET]) {
    if (value) {
      return value
    }
  }

  const rustc = spawnSync("rustc", ["-vV"], {
    cwd: repoRoot,
    env: process.env,
    encoding: "utf8",
  })
  if (rustc.status === 0) {
    const hostLine = rustc.stdout
      .split("\n")
      .find((line) => line.startsWith("host:"))
    const host = hostLine?.split(":")[1]?.trim()
    if (host) {
      return host
    }
  }

  return `${arch}-apple-darwin`
}

function fileHasPlaceholderMarker(filePath) {
  if (!fs.existsSync(filePath)) {
    return false
  }
  const stat = fs.statSync(filePath)
  if (!stat.isFile() || stat.size >= 1024) {
    return false
  }
  const content = fs.readFileSync(filePath, "utf8")
  return (
    content.includes("PLACEHOLDER") ||
    content.includes("version https://git-lfs.github.com/spec/v1")
  )
}

function readyFile(filePath) {
  return fs.existsSync(filePath) && fs.statSync(filePath).isFile() && !fileHasPlaceholderMarker(filePath)
}

function findOnPath(name) {
  const searchPath = process.env.PATH || ""
  for (const entry of searchPath.split(path.delimiter)) {
    if (!entry) {
      continue
    }
    const candidate = path.join(entry, name)
    if (fs.existsSync(candidate) && fs.statSync(candidate).isFile()) {
      return candidate
    }
  }
  return null
}

function resolveBash() {
  if (process.platform !== "win32") {
    return "bash"
  }

  for (const candidate of [
    findOnPath("bash.exe"),
    process.env.ProgramFiles && path.join(process.env.ProgramFiles, "Git", "bin", "bash.exe"),
    process.env.ProgramFiles && path.join(process.env.ProgramFiles, "Git", "usr", "bin", "bash.exe"),
    process.env["ProgramFiles(x86)"] && path.join(process.env["ProgramFiles(x86)"], "Git", "bin", "bash.exe"),
    process.env["ProgramFiles(x86)"] && path.join(process.env["ProgramFiles(x86)"], "Git", "usr", "bin", "bash.exe"),
  ]) {
    if (candidate && fs.existsSync(candidate)) {
      return candidate
    }
  }

  throw new Error("Git Bash is required to prepare bundled runtime assets on Windows")
}

function runRepoScript(scriptName, args) {
  const scriptPath = path.join(repoRoot, "scripts", scriptName)
  const result = spawnSync(resolveBash(), [scriptPath, ...args], {
    cwd: repoRoot,
    stdio: "inherit",
    env: process.env,
  })
  if (result.status !== 0) {
    throw new Error(`${scriptName} failed with exit code ${result.status ?? 1}`)
  }
}

function ensureMacAssets(arch) {
  const imageDir = path.join(srcTauriDir, "runtime-images", `cratebay-runtime-${arch}`)
  if (readyFile(path.join(imageDir, "vmlinuz")) && readyFile(path.join(imageDir, "initramfs"))) {
    return
  }
  runRepoScript("build-runtime-assets-alpine.sh", [arch])
}

function ensureMacExternalBin(arch) {
  const target = detectMacTargetTriple(arch)
  const runner = path.join(srcTauriDir, "bin", `cratebay-vz-${target}`)
  if (readyFile(runner)) {
    return
  }
  runRepoScript("prepare-tauri-external-bins.sh", [target])
}

function ensureWindowsAssets(arch) {
  const rootfs = path.join(
    srcTauriDir,
    "runtime-wsl",
    `cratebay-runtime-wsl-${arch}`,
    "rootfs.tar",
  )
  if (readyFile(rootfs)) {
    return
  }
  runRepoScript("build-runtime-assets-wsl.sh", [arch])
}

function ensureLinuxAssets(arch) {
  const imageDir = path.join(srcTauriDir, "runtime-images", `cratebay-runtime-${arch}`)
  const helperDir = path.join(srcTauriDir, "runtime-linux", `cratebay-runtime-linux-${arch}`)
  const qemuBinary = arch === "aarch64" ? "qemu-system-aarch64" : "qemu-system-x86_64"
  const kernelReady =
    readyFile(path.join(imageDir, "vmlinuz")) && readyFile(path.join(imageDir, "initramfs"))
  const helperReady = readyFile(path.join(helperDir, qemuBinary))

  if (kernelReady && helperReady) {
    return
  }

  if (mode === "dev" && !process.env.CRATEBAY_FORCE_RUNTIME_ASSET_PREP) {
    console.warn("[cratebay] skipping Linux bundled runtime asset build during Tauri dev")
    return
  }

  if (!kernelReady) {
    runRepoScript("build-runtime-assets-alpine.sh", [arch])
  }
  if (!helperReady) {
    runRepoScript("build-runtime-assets-linux.sh", [arch])
  }
}

const arch = detectTargetArch()

switch (process.platform) {
  case "darwin":
    ensureMacAssets(arch)
    ensureMacExternalBin(arch)
    break
  case "win32":
    ensureWindowsAssets(arch)
    break
  case "linux":
    ensureLinuxAssets(arch)
    break
  default:
    break
}
