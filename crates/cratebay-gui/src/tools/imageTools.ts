/**
 * Docker image tools for the CrateBay Agent.
 */

import { Type } from "@sinclair/typebox";
import type {
  AgentTool,
  AgentToolResult,
  AgentToolUpdateCallback,
} from "@mariozechner/pi-agent-core";
import { invoke } from "@/lib/tauri";
import type { ImageInspectInfo, ImageSearchResult, LocalImageInfo } from "@/types/image";

const EmptyParams = Type.Object({});

const ImageSearchParams = Type.Object({
  query: Type.String({
    description: "Registry search query, e.g. 'alpine' or 'postgres'",
    minLength: 1,
  }),
  limit: Type.Optional(
    Type.Number({
      description: "Maximum result count",
      minimum: 1,
      maximum: 50,
    }),
  ),
});

const ImagePullParams = Type.Object({
  image: Type.String({
    description: "Image reference to pull, e.g. 'alpine:latest'",
    minLength: 1,
  }),
  mirrors: Type.Optional(
    Type.Array(Type.String(), {
      description: "Optional registry mirror URLs",
    }),
  ),
});

const ImageRemoveParams = Type.Object({
  imageId: Type.String({
    description: "Image ID or image reference",
    minLength: 1,
  }),
  force: Type.Optional(
    Type.Boolean({
      description: "Force removal when image is referenced by stopped containers",
    }),
  ),
});

const ImageInspectParams = Type.Object({
  imageId: Type.String({
    description: "Image ID or image reference",
    minLength: 1,
  }),
});

const ImageTagParams = Type.Object({
  sourceImage: Type.String({
    description: "Source image ID or reference",
    minLength: 1,
  }),
  targetImage: Type.String({
    description: "Target image reference in repo:tag format",
    minLength: 1,
  }),
});

function textResult(text: string): AgentToolResult<undefined> {
  return {
    content: [{ type: "text", text }],
    details: undefined,
  };
}

function primaryTag(image: LocalImageInfo): string {
  if (Array.isArray(image.repoTags) && image.repoTags.length > 0) {
    return image.repoTags[0];
  }
  return image.id;
}

export const imageListTool: AgentTool<typeof EmptyParams> = {
  name: "image_list",
  label: "List Images",
  description: "List local Docker images available to the runtime.",
  parameters: EmptyParams,
  execute: async () => {
    const images = await invoke<LocalImageInfo[]>("image_list");

    if (!Array.isArray(images) || images.length === 0) {
      return textResult("No local images found.");
    }

    const lines = images.map(
      (image) =>
        `- **${primaryTag(image)}** (${image.id.slice(0, 12)}) — ${image.sizeHuman}`,
    );

    return textResult(`Found ${images.length} image(s):\n${lines.join("\n")}`);
  },
};

export const imageSearchTool: AgentTool<typeof ImageSearchParams> = {
  name: "image_search",
  label: "Search Images",
  description: "Search Docker registries for images by keyword.",
  parameters: ImageSearchParams,
  execute: async (_toolCallId, params) => {
    const results = await Promise.race([
      invoke<ImageSearchResult[]>("image_search", {
        query: params.query,
        limit: params.limit ?? 10,
      }),
      new Promise<ImageSearchResult[]>((_, reject) =>
        window.setTimeout(() => reject(new Error("Image search timeout (15s)")), 15000),
      ),
    ]);

    if (!Array.isArray(results) || results.length === 0) {
      return textResult(`No image results found for "${params.query}".`);
    }

    const lines = results.map((result) => {
      const stars = result.stars ?? 0;
      const pulls = result.pulls ?? 0;
      return `- **${result.reference}** (source: ${result.source}, stars: ${stars}, pulls: ${pulls})`;
    });

    return textResult(`Found ${results.length} result(s):\n${lines.join("\n")}`);
  },
};

export const imagePullTool: AgentTool<typeof ImagePullParams> = {
  name: "image_pull",
  label: "Pull Image",
  description:
    "Start pulling an image in background. Returns a channel ID for progress events.",
  parameters: ImagePullParams,
  execute: async (
    _toolCallId,
    params,
    _signal?: AbortSignal,
    onUpdate?: AgentToolUpdateCallback<{ status: string; channelId?: string }>,
  ) => {
    onUpdate?.({
      content: [{ type: "text", text: `Starting pull for ${params.image}...` }],
      details: { status: "starting" },
    });

    const channelId = await invoke<string>("image_pull", {
      image: params.image,
      mirrors: params.mirrors,
    });

    onUpdate?.({
      content: [{ type: "text", text: `Image pull accepted. Channel: ${channelId}` }],
      details: { status: "accepted", channelId },
    });

    return textResult(
      `Image pull started for **${params.image}**.\n` +
      `Progress channel: \`image:pull:${channelId}\``,
    );
  },
};

export const imageRemoveTool: AgentTool<typeof ImageRemoveParams> = {
  name: "image_remove",
  label: "Remove Image",
  description: "Remove a local image by ID or reference.",
  parameters: ImageRemoveParams,
  execute: async (_toolCallId, params) => {
    await invoke("image_remove", {
      id: params.imageId,
      force: params.force ?? false,
    });

    return textResult(`Image ${params.imageId} removed.`);
  },
};

export const imageInspectTool: AgentTool<typeof ImageInspectParams> = {
  name: "image_inspect",
  label: "Inspect Image",
  description: "Inspect a local image and return metadata.",
  parameters: ImageInspectParams,
  execute: async (_toolCallId, params) => {
    const info = await invoke<ImageInspectInfo>("image_inspect", { id: params.imageId });

    const lines = [
      `**Image: ${info.repoTags[0] ?? info.id}**`,
      `- ID: ${info.id}`,
      `- Size: ${(info.sizeBytes / (1024 * 1024)).toFixed(1)} MB`,
      `- Created: ${info.created}`,
      `- OS/Arch: ${info.os}/${info.architecture}`,
      `- Docker Version: ${info.dockerVersion}`,
      `- Layers: ${info.layers}`,
    ];

    return textResult(lines.join("\n"));
  },
};

export const imageTagTool: AgentTool<typeof ImageTagParams> = {
  name: "image_tag",
  label: "Tag Image",
  description: "Create a new repo:tag alias for a local image.",
  parameters: ImageTagParams,
  execute: async (_toolCallId, params) => {
    await invoke("image_tag", {
      source: params.sourceImage,
      target: params.targetImage,
    });

    return textResult(`Tagged ${params.sourceImage} as ${params.targetImage}.`);
  },
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const imageTools: AgentTool<any>[] = [
  imageListTool,
  imageSearchTool,
  imagePullTool,
  imageRemoveTool,
  imageInspectTool,
  imageTagTool,
];
