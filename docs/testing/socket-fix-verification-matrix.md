# Socket Fix Verification Matrix

> 版本: 1.0.0 | 日期: 2026-03-26 | 作者: tester

本文档定义 runtime socket 修复后的验证测试矩阵。

---

## 1. 前置条件

- [ ] runtime-dev 完成 socket 转发修复 (#2)
- [ ] frontend-dev 完成前端产物重建 (#3)
- [ ] 确保无外部 Docker 运行 (`docker ps` 应失败或返回非 CrateBay 实例)

---

## 2. 测试矩阵

### 2.1 Runtime 启动验证

| 测试项 | 命令/操作 | 预期结果 | 实际结果 | 通过 |
|--------|----------|----------|----------|------|
| GUI 启动 | `pnpm tauri dev` (从 cratebay-gui 目录) | 应用窗口正常打开 | | [ ] |
| Runtime 自动启动 | 观察日志 | 显示 detect → provision → start → Docker ready | | [ ] |
| Docker 连接状态 | Settings 页面 | 显示 "connected: built-in" | | [ ] |

### 2.2 Docker API 基础操作

| 测试项 | 命令/操作 | 预期结果 | 实际结果 | 通过 |
|--------|----------|----------|----------|------|
| docker_status | Tauri invoke | `connected: true, source: "built-in"` | | [ ] |
| container_list (docker ps) | Tauri invoke | 返回容器列表（可能为空）无错误 | | [ ] |
| image_list | Tauri invoke | 返回镜像列表无错误 | | [ ] |

### 2.3 Docker Run 测试

| 测试项 | 命令/操作 | 预期结果 | 实际结果 | 通过 |
|--------|----------|----------|----------|------|
| 创建容器 | `container_create` with alpine:latest | 返回 ContainerInfo | | [ ] |
| 启动容器 | `container_start` | 成功无错误 | | [ ] |
| 执行命令 | `container_exec` echo hello | stdout: "hello" | | [ ] |
| 停止容器 | `container_stop` | 成功无错误 | | [ ] |
| 删除容器 | `container_delete` | 成功无错误 | | [ ] |

### 2.4 Docker Pull 测试

| 测试项 | 命令/操作 | 预期结果 | 实际结果 | 通过 |
|--------|----------|----------|----------|------|
| 拉取小镜像 | `image_pull` hello-world | 进度事件 + 完成事件 | | [ ] |
| 拉取镜像 | `image_pull` alpine:latest | 进度事件 + 完成事件 | | [ ] |
| 镜像验证 | `image_list` | 新拉取的镜像出现在列表中 | | [ ] |

**注意**: Docker pull 需要 VM 有外网连接。如果 VZ NAT 限制导致无法连接，此项预期失败并记录。

### 2.5 并发压力测试

| 测试项 | 命令/操作 | 预期结果 | 实际结果 | 通过 |
|--------|----------|----------|----------|------|
| 并发 3 路 container_list | 同时发起 3 个 invoke | 全部成功返回 | | [ ] |
| 并发混合操作 | docker_status + container_list + image_list 同时 | 全部成功返回 | | [ ] |
| 循环 container_list | 连续 10 次快速调用 | 全部成功无超时 | | [ ] |

### 2.6 GUI 资源加载验证

| 测试项 | 命令/操作 | 预期结果 | 实际结果 | 通过 |
|--------|----------|----------|----------|------|
| 前端资源版本 | DevTools Network 或 Console | 加载最新构建的资源（无 304 缓存旧版） | | [ ] |
| Settings 页面渲染 | 导航到 Settings | Runtime 状态卡片正确渲染 | | [ ] |
| Container 页面渲染 | 导航到 Containers | 列表正确加载（即使为空） | | [ ] |
| Images 页面渲染 | 导航到 Images | 列表正确加载 | | [ ] |

---

## 3. 测试脚本

### 3.1 并发测试脚本 (前端 Console)

```javascript
// 并发 3 路 container_list 测试
async function testConcurrentContainerList() {
  const { invoke } = window.__TAURI__.core;
  const start = Date.now();
  
  const promises = [
    invoke('container_list', { filters: null }),
    invoke('container_list', { filters: null }),
    invoke('container_list', { filters: null }),
  ];
  
  const results = await Promise.all(promises);
  const elapsed = Date.now() - start;
  
  console.log(`并发 3 路 container_list: ${elapsed}ms`);
  console.log('Results:', results.map(r => Array.isArray(r) ? `OK (${r.length} items)` : 'ERROR'));
  return results.every(r => Array.isArray(r));
}

// 循环 container_list 测试
async function testLoopContainerList(iterations = 10) {
  const { invoke } = window.__TAURI__.core;
  const start = Date.now();
  let success = 0;
  let fail = 0;
  
  for (let i = 0; i < iterations; i++) {
    try {
      await invoke('container_list', { filters: null });
      success++;
    } catch (e) {
      fail++;
      console.error(`Iteration ${i} failed:`, e);
    }
  }
  
  const elapsed = Date.now() - start;
  console.log(`循环 ${iterations} 次 container_list: ${elapsed}ms (avg: ${(elapsed/iterations).toFixed(1)}ms)`);
  console.log(`Success: ${success}, Fail: ${fail}`);
  return fail === 0;
}

// 混合并发操作测试
async function testMixedConcurrent() {
  const { invoke } = window.__TAURI__.core;
  const start = Date.now();
  
  const promises = [
    invoke('docker_status'),
    invoke('container_list', { filters: null }),
    invoke('image_list'),
  ];
  
  const [dockerStatus, containers, images] = await Promise.all(promises);
  const elapsed = Date.now() - start;
  
  console.log(`混合并发操作: ${elapsed}ms`);
  console.log('docker_status:', dockerStatus);
  console.log('containers:', containers?.length ?? 'error');
  console.log('images:', images?.length ?? 'error');
  
  return dockerStatus?.connected && Array.isArray(containers) && Array.isArray(images);
}

// 运行全部测试
async function runAllTests() {
  console.log('=== Socket Fix Verification Tests ===');
  
  const results = {
    concurrent3: await testConcurrentContainerList(),
    loop10: await testLoopContainerList(10),
    mixed: await testMixedConcurrent(),
  };
  
  console.log('\n=== Test Results ===');
  console.log(results);
  
  const allPassed = Object.values(results).every(Boolean);
  console.log(`\nOverall: ${allPassed ? 'PASS ✓' : 'FAIL ✗'}`);
  return results;
}

// 执行测试
runAllTests();
```

### 3.2 Docker Run 完整流程测试

```javascript
// Docker 容器完整生命周期测试
async function testContainerLifecycle() {
  const { invoke } = window.__TAURI__.core;
  const testName = `test-${Date.now()}`;
  
  console.log('=== Container Lifecycle Test ===');
  
  try {
    // 1. Create
    console.log('1. Creating container...');
    const created = await invoke('container_create', {
      request: {
        name: testName,
        image: 'alpine:latest',
        command: ['/bin/sh'],
        tty: true,
      }
    });
    console.log('   Created:', created.id);
    
    // 2. Start
    console.log('2. Starting container...');
    await invoke('container_start', { id: created.id });
    console.log('   Started');
    
    // 3. Exec
    console.log('3. Executing command...');
    const execResult = await invoke('container_exec', {
      id: created.id,
      cmd: ['echo', 'hello from container'],
      workingDir: null,
    });
    console.log('   Exec result:', execResult);
    
    // 4. Stop
    console.log('4. Stopping container...');
    await invoke('container_stop', { id: created.id, timeout: null });
    console.log('   Stopped');
    
    // 5. Delete
    console.log('5. Deleting container...');
    await invoke('container_delete', { id: created.id, force: true });
    console.log('   Deleted');
    
    console.log('\n=== Container Lifecycle Test: PASS ✓ ===');
    return true;
  } catch (e) {
    console.error('\n=== Container Lifecycle Test: FAIL ✗ ===');
    console.error('Error:', e);
    return false;
  }
}

testContainerLifecycle();
```

---

## 4. 执行检查清单

执行测试时按以下顺序：

1. **环境准备**
   - [ ] 关闭所有外部 Docker (Colima, Docker Desktop 等)
   - [ ] 清理可能的僵尸 VZ 进程: `ps aux | grep cratebay-vz`
   - [ ] 进入 GUI 目录: `cd crates/cratebay-gui`

2. **启动测试**
   - [ ] 运行 `pnpm tauri dev`
   - [ ] 观察终端日志，确认 runtime 启动序列
   - [ ] 等待 "Docker ready" 日志

3. **基础验证**
   - [ ] 打开 Settings 页面，确认 Docker 状态
   - [ ] 打开 DevTools Console (Cmd+Opt+I)
   - [ ] 运行 `testMixedConcurrent()` 验证基础 API

4. **压力测试**
   - [ ] 运行 `runAllTests()` 执行并发测试

5. **容器测试** (需要 alpine 镜像)
   - [ ] 运行 `testContainerLifecycle()`

6. **Pull 测试** (需要外网)
   - [ ] 手动测试 image_pull

---

## 5. 已知限制

| 限制 | 说明 | 影响 |
|------|------|------|
| VM 无外网 | VZ NAT 限制，VM 内无法访问互联网 | docker pull 会失败 |
| Intel Mac VZ | VZ.framework 在 Intel Mac 上可能静默退出 | 需要检查 VZ 进程状态 |
| 首次启动慢 | VM 首次启动约 45-60 秒 | 需要耐心等待 Docker ready |

---

## 6. 测试结果记录

### 执行日期: ____

**执行者**: tester

**环境**:
- macOS 版本: 
- CPU: 
- 内存: 

**测试通过率**: __ / __ (__ %)

**问题记录**:
1. 
2. 

**结论**: [ ] 通过 / [ ] 需要修复

---

*文档由 tester 准备，等待 runtime-dev 完成 #2 后执行*
