#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

suffix="$(date +%s)-$$"
container_name="cbx-runtime-smoke-${suffix}"
packaged_image="cbx-runtime-pack:${suffix}"
volume_name="cbx-runtime-volume-${suffix}"
runtime_image="${CRATEBAY_SMOKE_RUNTIME_IMAGE:-nginx:1.27-alpine}"
env_key="CRATEBAY_E2E"
env_value="smoke-${suffix}"

if [[ -x "$repo_root/target/debug/cratebay.exe" ]]; then
  cratebay_bin="$repo_root/target/debug/cratebay.exe"
else
  cratebay_bin="$repo_root/target/debug/cratebay"
fi

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local message="$3"
  if ! printf '%s\n' "$haystack" | grep -Fq -- "$needle"; then
    echo "ASSERTION FAILED: $message"
    echo "--- output ---"
    printf '%s\n' "$haystack"
    exit 1
  fi
}

native_path() {
  local path="$1"
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$path"
  else
    printf '%s' "$path"
  fi
}

smoke_data_dir="$repo_root/target/runtime-smoke-${suffix}"
mkdir -p "$smoke_data_dir"
export CRATEBAY_DATA_DIR="$(native_path "$smoke_data_dir")"

cleanup() {
  set +e
  "$cratebay_bin" docker rm "$container_name" >/dev/null 2>&1 || true
  "$cratebay_bin" image remove "$packaged_image" >/dev/null 2>&1 || true
  "$cratebay_bin" volume remove "$volume_name" >/dev/null 2>&1 || true
  "$cratebay_bin" runtime stop >/dev/null 2>&1 || true
  if [[ "${OS:-}" == "Windows_NT" || -n "${MSYSTEM:-}" ]]; then
    for _ in {1..10}; do
      rm -rf "$smoke_data_dir" >/dev/null 2>&1 && break
      sleep 1
    done
  else
    rm -rf "$smoke_data_dir"
  fi
}
trap cleanup EXIT

echo "== Build cratebay CLI =="
cargo build -p cratebay-cli >/dev/null

if [[ "$(uname -s)" == "Darwin" ]]; then
  echo "== Build cratebay-vz runner =="
  cargo build -p cratebay-vz >/dev/null
fi

if [[ ! -x "$cratebay_bin" ]]; then
  echo "ERROR: built cratebay binary not found at $cratebay_bin"
  exit 1
fi

if [[ "${OS:-}" == "Windows_NT" || -n "${MSYSTEM:-}" ]]; then
  export CRATEBAY_RUNTIME_PROGRESS="${CRATEBAY_RUNTIME_PROGRESS:-1}"
fi

echo "== Start built-in runtime =="
runtime_start_log="$smoke_data_dir/runtime-start.log"
"$cratebay_bin" runtime start >"$runtime_start_log"
runtime_start_output="$(cat "$runtime_start_log")"
printf '%s\n' "$runtime_start_output"
assert_contains "$runtime_start_output" "DOCKER_HOST:" "runtime start should report DOCKER_HOST"

echo "== Inspect runtime environment =="
runtime_env_output="$("$cratebay_bin" runtime env)"
printf '%s\n' "$runtime_env_output"
assert_contains "$runtime_env_output" "DOCKER_HOST" "runtime env should print DOCKER_HOST instructions"

if [[ "${OS:-}" == "Windows_NT" || -n "${MSYSTEM:-}" ]]; then
  runtime_env_bash="$(printf '%s\n' "$runtime_env_output" | sed -n 's/^Bash     : //p' | head -n 1)"
  if [[ -z "$runtime_env_bash" ]]; then
    echo "ERROR: runtime env did not emit Bash instructions on Windows"
    exit 1
  fi
  eval "$runtime_env_bash"
else
  eval "$runtime_env_output"
fi

runtime_status_output="$("$cratebay_bin" runtime status)"
printf '%s\n' "$runtime_status_output"
assert_contains "$runtime_status_output" "Docker engine:" "runtime status should report engine status"

echo "== Run container =="
run_output="$("$cratebay_bin" docker run --pull --name "$container_name" -e "${env_key}=${env_value}" "$runtime_image")"
printf '%s\n' "$run_output"
assert_contains "$run_output" "$container_name" "docker run should report the created container"

echo "== Verify container list =="
ps_output="$("$cratebay_bin" docker ps)"
printf '%s\n' "$ps_output"
assert_contains "$ps_output" "$container_name" "docker ps should list the created container"
assert_contains "$ps_output" "$runtime_image" "docker ps should show the runtime image"

echo "== Verify env and logs =="
env_output="$("$cratebay_bin" docker env "$container_name")"
printf '%s\n' "$env_output"
assert_contains "$env_output" "$env_key" "docker env should include the injected env key"
assert_contains "$env_output" "$env_value" "docker env should include the injected env value"

logs_output="$("$cratebay_bin" docker logs "$container_name" --tail 20)"
printf '%s\n' "$logs_output"

echo "== Stop and start container =="
stop_output="$("$cratebay_bin" docker stop "$container_name")"
printf '%s\n' "$stop_output"
assert_contains "$stop_output" "Stopped container $container_name" "docker stop should succeed"

start_output="$("$cratebay_bin" docker start "$container_name")"
printf '%s\n' "$start_output"
assert_contains "$start_output" "Started container $container_name" "docker start should succeed"

echo "== Package container into image =="
pack_output="$("$cratebay_bin" image pack-container "$container_name" "$packaged_image")"
printf '%s\n' "$pack_output"

image_list_output="$("$cratebay_bin" image list)"
printf '%s\n' "$image_list_output"
assert_contains "$image_list_output" "$packaged_image" "image list should include the packaged image"

image_inspect_output="$("$cratebay_bin" image inspect "$packaged_image")"
printf '%s\n' "$image_inspect_output"
assert_contains "$image_inspect_output" "$packaged_image" "image inspect should include the packaged tag"

echo "== Volume lifecycle =="
volume_create_output="$("$cratebay_bin" volume create "$volume_name")"
printf '%s\n' "$volume_create_output"
assert_contains "$volume_create_output" "$volume_name" "volume create should report the new volume"

volume_list_output="$("$cratebay_bin" volume list)"
printf '%s\n' "$volume_list_output"
assert_contains "$volume_list_output" "$volume_name" "volume list should show the new volume"

volume_inspect_output="$("$cratebay_bin" volume inspect "$volume_name")"
printf '%s\n' "$volume_inspect_output"
assert_contains "$volume_inspect_output" "\"Name\": \"$volume_name\"" "volume inspect should show the created volume"

volume_remove_output="$("$cratebay_bin" volume remove "$volume_name")"
printf '%s\n' "$volume_remove_output"
assert_contains "$volume_remove_output" "$volume_name" "volume remove should report the deleted volume"

echo "== Remove container =="
rm_output="$("$cratebay_bin" docker rm "$container_name")"
printf '%s\n' "$rm_output"
assert_contains "$rm_output" "Removed container $container_name" "docker rm should remove the container"

echo "CLI-only runtime smoke: PASS"
