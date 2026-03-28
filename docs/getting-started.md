# Getting Started

CrateBay is a local AI sandbox. It lets any AI agent run code safely on your machine through the MCP protocol.

## Prerequisites

- macOS 12+ (Apple Silicon or Intel), Linux, or Windows 11
- One of: Claude Desktop, Cursor, Windsurf, or any MCP-compatible AI client

## Install

### macOS

Download from [GitHub Releases](https://github.com/nicepkg/CrateBay/releases) and drag to Applications.

Or build from source:

```bash
git clone https://github.com/nicepkg/CrateBay.git
cd CrateBay
pnpm install
cargo tauri build
```

### First Launch

1. Open CrateBay
2. Wait for the built-in runtime to start (status bar shows "Ready")
3. Sandbox images are automatically loaded on first launch

## Connect Your AI

### Claude Desktop

1. Generate the MCP config:

```bash
cratebay mcp export claude
```

Output:
```json
{
  "mcpServers": {
    "cratebay": {
      "command": "cratebay-mcp"
    }
  }
}
```

2. Open Claude Desktop config file:
   - macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
   - Windows: `%APPDATA%/Claude/claude_desktop_config.json`

3. Merge the JSON into your existing config (add the `"cratebay"` entry to `"mcpServers"`)

4. Restart Claude Desktop

5. Verify: Claude should now show "CrateBay" in its tools list

### Cursor / Windsurf

```bash
cratebay mcp export cursor
```

Add the output to your editor's MCP settings.

## Run Your First Code

In Claude Desktop (or your MCP client), type:

> Run this Python code: print("Hello from CrateBay!")

Claude will:
1. Call `sandbox_run_code` with language="python" and your code
2. CrateBay creates a sandbox, writes the code, executes it
3. Returns the result: `Hello from CrateBay!`

### More Examples

**Install packages and run data analysis:**

> Create a Python sandbox, install pandas and numpy, then generate a summary of random data

**Run JavaScript:**

> Execute this Node.js code: console.log(Array.from({length: 10}, (_, i) => i * i))

**Bash scripting:**

> Run a bash script that lists all files in /etc and counts them

**Multi-step workflow:**

> 1. Create a Python sandbox with cleanup=false
> 2. Install requests and beautifulsoup4
> 3. Write a script that fetches example.com and prints the title
> 4. Run it

## Available MCP Tools

| Tool | Description |
|------|-------------|
| `sandbox_run_code` | One-shot: create sandbox + write code + execute + return result |
| `sandbox_install` | Install packages (pip/npm/cargo/apt) in a sandbox |
| `sandbox_create` | Create a persistent sandbox from template |
| `sandbox_exec` | Execute a command in an existing sandbox |
| `sandbox_list` | List running sandboxes |
| `sandbox_stop` / `sandbox_delete` | Lifecycle management |
| `sandbox_put_path` / `sandbox_get_path` | File transfer in/out of sandbox |
| `sandbox_templates` | List available sandbox templates |

## CLI Usage

```bash
# Runtime management
cratebay runtime status
cratebay runtime start

# Container operations
cratebay container list
cratebay container create mybox --image python:3.12-slim-bookworm
cratebay container exec mybox -- python -c "print('hello')"

# Image management
cratebay image list
cratebay image pull python:3.12-slim-bookworm

# MCP config
cratebay mcp export claude
```

## Troubleshooting

### Runtime won't start

Check status:
```bash
cratebay runtime status
```

If stuck, try manual start:
```bash
cratebay runtime start
```

### MCP connection fails

1. Verify `cratebay-mcp` is in your PATH:
   ```bash
   which cratebay-mcp
   ```

2. Test it manually:
   ```bash
   echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cratebay-mcp
   ```

3. Check Claude Desktop logs for connection errors

### Sandbox creation fails

Ensure Docker is reachable through the runtime:
```bash
cratebay system docker-status
```

If images are missing:
```bash
cratebay image list
cratebay image pull python:3.12-slim-bookworm
```

## Architecture

```
Your AI (Claude/Cursor)
    ↓ MCP protocol (stdio)
cratebay-mcp (MCP Server)
    ↓ Docker API
Built-in VM (VZ/KVM/WSL2)
    ↓
Docker Engine → Containers
```

All code runs inside a lightweight VM on your machine. No cloud, no cost, no data leaving your computer.
