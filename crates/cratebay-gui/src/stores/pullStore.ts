import { create } from "zustand";
import { invoke, listen } from "@/lib/tauri";
import { useSettingsStore } from "@/stores/settingsStore";

export interface PullTask {
  id: string;
  image: string;
  progress: number;
  status: string;
  complete: boolean;
  error: string | null;
}

interface ImagePullProgress {
  current_layer: number;
  total_layers: number;
  progress_percent: number;
  status: string;
  complete: boolean;
  error: string | null;
}

interface PullState {
  tasks: PullTask[];
  startPull: (image: string) => Promise<void>;
  removeTask: (id: string) => void;
  clearCompleted: () => void;
  isPulling: (image: string) => boolean;
}

export const usePullStore = create<PullState>()((set, get) => ({
  tasks: [],

  startPull: async (image: string) => {
    const trimmedImage = image.trim();
    if (trimmedImage.length === 0) return;

    // 如果该镜像已在拉取中，不重复拉取
    if (get().tasks.some((t) => t.image === trimmedImage && !t.complete)) {
      return;
    }

    const channelId = `pull-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

    const task: PullTask = {
      id: channelId,
      image: trimmedImage,
      progress: 0,
      status: "准备中...",
      complete: false,
      error: null,
    };

    set((state) => ({ tasks: [task, ...state.tasks] }));

    try {
      const mirrors = useSettingsStore.getState().settings.registryMirrors;
      const hasMirrors = Array.isArray(mirrors) && mirrors.length > 0;

      // 先监听事件，再调用 invoke，防止丢失早期事件
      const unlistenPromise = listen<ImagePullProgress>(
        `image:pull:${channelId}`,
        (progress) => {
          set((state) => ({
            tasks: state.tasks.map((t) =>
              t.id === channelId
                ? {
                    ...t,
                    progress: progress.progress_percent,
                    status: progress.status,
                    complete: progress.complete,
                    error: progress.error,
                  }
                : t,
            ),
          }));
        },
      );

      await invoke<string>("image_pull", {
        image: trimmedImage,
        mirrors: hasMirrors ? mirrors : null,
        channel_id: channelId,
      });

      // invoke 返回后，保持监听直到收到 complete 事件
      // 设置超时保护
      const unlisten = await unlistenPromise;
      const timeoutId = window.setTimeout(() => {
        unlisten();
        set((state) => ({
          tasks: state.tasks.map((t) =>
            t.id === channelId && !t.complete
              ? { ...t, complete: true, error: "拉取超时 (5min)" }
              : t,
          ),
        }));
      }, 300000);

      // 轮询检查完成状态，清理超时和监听
      const checkInterval = window.setInterval(() => {
        const task = get().tasks.find((t) => t.id === channelId);
        if (task?.complete) {
          window.clearTimeout(timeoutId);
          window.clearInterval(checkInterval);
          unlisten();
        }
      }, 500);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      set((state) => ({
        tasks: state.tasks.map((t) =>
          t.id === channelId
            ? {
                ...t,
                complete: true,
                error: message.length > 0 ? message : `拉取 ${trimmedImage} 失败`,
              }
            : t,
        ),
      }));
    }
  },

  removeTask: (id: string) => {
    set((state) => ({ tasks: state.tasks.filter((t) => t.id !== id) }));
  },

  clearCompleted: () => {
    set((state) => ({ tasks: state.tasks.filter((t) => !t.complete) }));
  },

  isPulling: (image: string) => {
    return get().tasks.some((t) => t.image === image && !t.complete);
  },
}));
