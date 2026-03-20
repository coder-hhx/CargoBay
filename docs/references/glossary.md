# Glossary

> Version: 1.0.0 | Last Updated: 2026-03-20 | Author: product-manager

This glossary defines key terms used throughout CrateBay's documentation and codebase. All project documents should use these terms consistently.

---

## Project & Product

### CrateBay

An open-source desktop AI development control plane. Provides a Chat-First interface for managing containers, AI models, and MCP tools. Built with Tauri v2, React, and Rust.

### Control Plane

A management layer that orchestrates and coordinates underlying resources. In CrateBay, the control plane manages containers, LLM providers, MCP tools, and development workflows through a unified desktop interface.

### Chat-First

The primary interaction paradigm of CrateBay. Users interact with the application through a conversational chat interface, where an AI agent translates natural language requests into system operations (container management, file operations, etc.).

---

## Agent System

### Hybrid Agent Architecture

CrateBay's agent design pattern where TypeScript handles orchestration (LLM interaction, tool selection, conversation state) and Rust handles execution (Docker API, file I/O, storage, MCP). The two layers communicate via Tauri IPC.

### pi-agent-core

`@mariozechner/pi-agent-core` -- A TypeScript agent framework used in CrateBay for LLM orchestration, multi-turn conversation management, tool calling, and streaming. Runs in the frontend (Tauri webview).

### pi-ai

`@mariozechner/pi-ai` -- A TypeScript library providing a unified interface to multiple LLM providers (OpenAI, Anthropic, etc.). Used by pi-agent-core to abstract provider differences.

### AgentTool

A tool definition in pi-agent-core that the AI agent can invoke during a conversation. Each AgentTool has a name, description, parameter schema (TypeBox), and an `execute()` function. In CrateBay, AgentTools are typically thin TypeScript wrappers around Tauri `invoke()` calls.

### StreamFn

A streaming function in pi-agent-core that connects to an LLM provider and returns tokens incrementally. In CrateBay, the custom streamFn routes through the Rust backend (LLM Proxy) for API key security.

### LLM Proxy

The Rust backend component that mediates all LLM API calls. The frontend sends prompts to the Rust backend via Tauri invoke; Rust reads the encrypted API key from SQLite, calls the LLM provider, and streams tokens back via Tauri Events. API keys never reach the frontend.

### System Prompt

The initial instruction set provided to the LLM at the start of each conversation, defining the agent's personality, available tools, safety rules, and behavioral constraints.

---

## MCP (Model Context Protocol)

### MCP

Model Context Protocol -- An open standard for AI tool interoperability. Defines how AI assistants discover and invoke tools provided by external servers. CrateBay supports MCP as both a server and a client.

### MCP Server

A standalone process that exposes tools via the MCP protocol. CrateBay's MCP Server (`cratebay-mcp` binary) exposes container/sandbox management tools, allowing external AI assistants (Claude, Cursor, etc.) to manage CrateBay sandboxes.

### MCP Client

A component that connects to MCP servers and consumes their tools. CrateBay's MCP Client (in `cratebay-core`) connects to external MCP servers configured in `.mcp.json`.

### MCP Bridge

The mechanism that makes external MCP tools available as AgentTools within pi-agent-core. When the MCP Client connects to an external server and discovers its tools, those tools are automatically registered as AgentTools that the built-in agent can invoke.

---

## Frontend

### Streamdown

A streaming markdown renderer by Vercel, designed for AI chat interfaces. Handles partial markdown content gracefully (incomplete code blocks, half-rendered tables) and integrates natively with shadcn/ui components and Tailwind CSS classes.

### shadcn/ui

A component system for React that provides copy-paste UI components built on Radix UI primitives. Unlike traditional component libraries, shadcn/ui components are copied into the project source, giving full ownership and customizability.

### Radix UI

A set of unstyled, accessible React component primitives (Dialog, Dropdown, Tabs, etc.) that serve as the foundation for shadcn/ui. Provides built-in keyboard navigation, screen reader support, and ARIA attributes.

### Zustand Store

A lightweight state management solution for React. CrateBay uses 6 domain-specific Zustand stores: `appStore`, `chatStore`, `containerStore`, `mcpStore`, `settingsStore`, and `workflowStore`. Each store manages a specific domain of application state.

### Tauri Commands

The IPC mechanism in Tauri v2 for calling Rust functions from the frontend. CrateBay defines Tauri commands for container operations, LLM proxy, storage, MCP management, and system status. Commands are grouped by module in `src-tauri/src/commands/`.

### tauri-specta

A code generation tool that auto-generates TypeScript type definitions and invoke wrappers from Rust Tauri command signatures. Ensures type safety across the Rust-TypeScript IPC boundary with zero manual type maintenance.

### Tauri Event

An event system in Tauri v2 for pushing data from the Rust backend to the frontend. Used primarily for streaming LLM tokens and long-running operation progress updates.

---

## Backend & Infrastructure

### bollard

A Rust Docker SDK (`bollard` crate, version 0.18) used by CrateBay to interact with the Docker engine API. Provides async container lifecycle management, image operations, and exec capabilities.

### rusqlite

A Rust binding for SQLite, used as the storage layer in CrateBay. The database file lives at `~/.cratebay/cratebay.db` and stores conversations, API keys (encrypted), settings, container templates, and MCP server configs.

### thiserror

A Rust crate for defining custom error types with derive macros. CrateBay uses `thiserror` to define `AppError` types with structured error variants, enabling consistent error handling across all crates.

### tokio

The async runtime used by CrateBay's Rust backend. Provides the foundation for non-blocking I/O, task spawning, and concurrent execution of Docker API calls, LLM streaming, and MCP communication.

---

## Container Runtime

### Built-in Runtime

CrateBay's zero-dependency container runtime that provisions a lightweight Linux VM with Docker inside, eliminating the need for users to install Docker Desktop separately. Platform implementations: VZ.framework (macOS), KVM/QEMU (Linux), WSL2 (Windows).

### VZ.framework

Apple's Virtualization framework (`Virtualization.framework`) for macOS. CrateBay uses `VZVirtualMachine` to run a lightweight Linux VM on Apple Silicon and Intel Macs, hosting the Docker engine inside the VM.

### KVM/QEMU

Linux virtualization technologies. KVM (Kernel-based Virtual Machine) provides hardware acceleration; QEMU provides the VM emulation layer. CrateBay uses these to run a lightweight Linux VM hosting Docker on Linux hosts.

### WSL2

Windows Subsystem for Linux 2 -- Microsoft's lightweight Linux VM technology for Windows. CrateBay leverages WSL2 to run Docker inside a Linux distribution on Windows, providing container support without Docker Desktop.

### Sandbox

An isolated container environment created from a template. Sandboxes have defined resource limits (CPU, memory), a time-to-live (TTL), and are managed through CrateBay's container lifecycle API. The term is used interchangeably with "managed container" in some contexts.

### Container Template

A predefined configuration for creating sandboxes. Templates specify the base image, startup command, environment variables, and default resource limits. Built-in templates include `node-dev`, `python-dev`, and `rust-dev`.

---

## Development Process

### Spec-Driven

CrateBay's development methodology where specification documents (specs) are the authoritative source for all implementation. The flow is: update spec first, implement code according to spec, then update knowledge base. Code must match the spec; discrepancies are resolved by updating either the spec or the code, never by ignoring the difference.

### Knowledge Base

The complete set of documentation and configuration files that enable any AI Agent to work on the project without human re-briefing. Comprises 6 layers: AGENTS.md, specs, workflow docs, references, `.codebuddy/` configs, and `.mcp.json`.

### ADR (Architecture Decision Record)

A structured document that captures a significant technical decision, its context, alternatives considered, and consequences. CrateBay's ADRs are stored in `docs/references/tech-decisions.md`.

### progress.md

The development progress tracking file (`docs/progress.md`) that enables cross-machine project recovery. Contains current status, completed work, in-progress tasks, blockers, and a "Quick Resume" section for continuing work on a different machine.

---

## Abbreviations

| Abbreviation | Full Form |
|-------------|-----------|
| ADR | Architecture Decision Record |
| API | Application Programming Interface |
| ARIA | Accessible Rich Internet Applications |
| CI/CD | Continuous Integration / Continuous Deployment |
| CLI | Command-Line Interface |
| CSS | Cascading Style Sheets |
| E2E | End-to-End |
| HMR | Hot Module Replacement |
| IPC | Inter-Process Communication |
| LLM | Large Language Model |
| MCP | Model Context Protocol |
| ORM | Object-Relational Mapping |
| SDK | Software Development Kit |
| SemVer | Semantic Versioning |
| SSE | Server-Sent Events |
| TS | TypeScript |
| TTL | Time To Live |
| UI | User Interface |
| VM | Virtual Machine |
| WSL | Windows Subsystem for Linux |
