import type { Translations } from "@/types/i18n";

const zhCN: Translations = {
  common: {
    confirm: "确认",
    cancel: "取消",
    save: "保存",
    delete: "删除",
    loading: "加载中...",
    error: "发生错误",
  },
  chat: {
    newSession: "新对话",
    placeholder: "输入消息... (用 @ 提及，Shift+Enter 换行)",
    sendButton: "发送",
    thinking: "思考中...",
    toolExecuting: "执行工具中...",
  },
  containers: {
    title: "容器",
    create: "创建容器",
    start: "启动",
    stop: "停止",
    delete: "删除",
    noContainers: "没有找到容器。创建一个开始使用吧。",
  },
  mcp: {
    title: "MCP 服务器",
    addServer: "添加服务器",
    connected: "已连接",
    disconnected: "未连接",
  },
  settings: {
    title: "设置",
    general: "常规",
    providers: "LLM 提供商",
    advanced: "高级",
    language: "语言",
    theme: "主题",
  },
};

export default zhCN;
