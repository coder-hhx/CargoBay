/**
 * ImagesPage — Docker image management page.
 *
 * Features:
 * - Local images tab: list, inspect, remove local images
 * - Search tab: search Docker Hub, pull images
 * - Inline image details via Dialog
 *
 * Uses Tauri commands: image_list, image_search, image_pull,
 * image_remove, image_inspect, image_tag.
 *
 * @see api-spec.md for Tauri command signatures
 */

import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import type {
  LocalImageInfo,
  ImageSearchResult,
  ImageInspectInfo,
} from "@/types/image";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Layers,
  Search,
  Trash2,
  Download,
  Eye,
  RefreshCw,
  Loader2,
  HardDrive,
  Globe,
  Star,
  ArrowDownToLine,
} from "lucide-react";

export function ImagesPage() {
  const { t } = useI18n();
  const [activeTab, setActiveTab] = useState<"local" | "search">("local");

  return (
    <div className="flex h-full flex-col">
      {/* Toolbar — matches ContainersPage style */}
      <div className="flex items-center gap-3 border-b border-border px-6 py-3">
        {/* Tab pills */}
        <div className="flex items-center gap-1.5">
          <button
            onClick={() => setActiveTab("local")}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium transition-colors focus:outline-none",
              activeTab === "local"
                ? "bg-primary/10 text-primary"
                : "text-muted-foreground hover:bg-muted hover:text-foreground",
            )}
          >
            <HardDrive className="h-3 w-3" />
            {t("images", "localImages")}
          </button>
          <button
            onClick={() => setActiveTab("search")}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium transition-colors focus:outline-none",
              activeTab === "search"
                ? "bg-primary/10 text-primary"
                : "text-muted-foreground hover:bg-muted hover:text-foreground",
            )}
          >
            <Globe className="h-3 w-3" />
            {t("images", "searchImages")}
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto">
        {activeTab === "local" ? <LocalImagesTab /> : <SearchImagesTab />}
      </div>
    </div>
  );
}

/* ========== Local Images Tab ========== */

function LocalImagesTab() {
  const { t } = useI18n();
  const [images, setImages] = useState<LocalImageInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState("");
  const [inspectInfo, setInspectInfo] = useState<ImageInspectInfo | null>(null);
  const [inspectLoading, setInspectLoading] = useState(false);
  const [removeConfirm, setRemoveConfirm] = useState<string | null>(null);
  const [removing, setRemoving] = useState(false);

  const fetchImages = useCallback(async () => {
    setLoading(true);
    try {
      const result = await Promise.race([
        invoke<LocalImageInfo[]>("image_list"),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error("镜像列表加载超时")), 8000),
        ),
      ]);
      setImages(result);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.warn("[ImagesPage] fetchImages failed:", message);
      // Docker not available or timeout — show empty list
      setImages([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchImages();
  }, [fetchImages]);

  const filteredImages = useMemo(() => {
    if (filter.length === 0) return images;
    const q = filter.toLowerCase();
    return images.filter(
      (img) =>
        img.repoTags.some((tag) => tag.toLowerCase().includes(q)) ||
        img.id.toLowerCase().includes(q),
    );
  }, [images, filter]);

  const handleInspect = useCallback(async (id: string) => {
    setInspectLoading(true);
    try {
      const info = await invoke<ImageInspectInfo>("image_inspect", { id });
      setInspectInfo(info);
    } catch {
      // Docker not available — cannot inspect
      setInspectInfo(null);
    } finally {
      setInspectLoading(false);
    }
  }, []);

  const handleRemove = useCallback(async (id: string) => {
    setRemoving(true);
    try {
      await invoke("image_remove", { id, force: false });
    } catch {
      // Docker not available — removal failed
      setRemoving(false);
      setRemoveConfirm(null);
      return;
    }
    setImages((prev) => prev.filter((img) => img.id !== id));
    setRemoving(false);
    setRemoveConfirm(null);
  }, []);

  return (
    <div className="px-6 py-4">
      {/* Toolbar */}
      <div className="mb-4 flex items-center gap-3">
        <div className="relative max-w-xs flex-1">
          <Search className="absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder={t("images", "filterPlaceholder")}
            className="pl-9"
          />
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => void fetchImages()}
          disabled={loading}
        >
          <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
          {t("common", "refresh")}
        </Button>
      </div>

      {/* Image count */}
      <p className="mb-3 text-xs text-muted-foreground">
        {filteredImages.length} {t("images", "imageCount")}
      </p>

      {/* Image list */}
      {loading ? (
        <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
          {t("images", "loadingImages")}
        </div>
      ) : filteredImages.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-16 text-center text-muted-foreground">
          <Layers className="mb-3 h-12 w-12 opacity-20" />
          <h3 className="text-sm font-medium">{t("images", "noImages")}</h3>
          <p className="mt-1 text-xs">{t("images", "noImagesHint")}</p>
        </div>
      ) : (
        <div className="space-y-2">
          {filteredImages.map((img) => (
            <LocalImageRow
              key={img.id}
              image={img}
              onInspect={() => void handleInspect(img.id)}
              onRemove={() => setRemoveConfirm(img.id)}
            />
          ))}
        </div>
      )}

      {/* Inspect Dialog */}
      <Dialog
        open={inspectInfo !== null}
        onOpenChange={(open) => {
          if (!open) setInspectInfo(null);
        }}
      >
        <DialogContent className="sm:max-w-[640px]">
          <DialogHeader>
            <DialogTitle>{t("images", "inspectImage")}</DialogTitle>
            <DialogDescription className="font-mono text-xs break-all">
              {inspectInfo?.id ?? ""}
            </DialogDescription>
          </DialogHeader>

          {inspectLoading ? (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
            </div>
          ) : inspectInfo !== null ? (
            <ScrollArea className="max-h-[50vh] pr-2">
              <div className="grid grid-cols-[140px_1fr] gap-x-4 gap-y-2 text-sm">
                <span className="text-muted-foreground">{t("images", "imageId")}</span>
                <span className="font-mono text-xs break-all">{inspectInfo.id}</span>

                <span className="text-muted-foreground">{t("images", "repoTags")}</span>
                <span>{inspectInfo.repoTags.length > 0 ? inspectInfo.repoTags.join(", ") : "-"}</span>

                <span className="text-muted-foreground">{t("images", "imageSize")}</span>
                <span>{(inspectInfo.sizeBytes / (1024 * 1024)).toFixed(1)} MB</span>

                <span className="text-muted-foreground">{t("images", "imageCreated")}</span>
                <span>{inspectInfo.created}</span>

                <span className="text-muted-foreground">{t("images", "architecture")}</span>
                <span>{inspectInfo.architecture}</span>

                <span className="text-muted-foreground">OS</span>
                <span>{inspectInfo.os}</span>

                <span className="text-muted-foreground">Docker Version</span>
                <span>{inspectInfo.dockerVersion}</span>

                <span className="text-muted-foreground">{t("images", "layers")}</span>
                <span>{inspectInfo.layers}</span>
              </div>
            </ScrollArea>
          ) : null}

          <DialogFooter>
            <Button variant="outline" onClick={() => setInspectInfo(null)}>
              {t("common", "close")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Remove Confirm Dialog */}
      <Dialog
        open={removeConfirm !== null}
        onOpenChange={(open) => {
          if (!open) setRemoveConfirm(null);
        }}
      >
        <DialogContent className="sm:max-w-[400px]">
          <DialogHeader>
            <DialogTitle>{t("images", "removeImage")}</DialogTitle>
            <DialogDescription>{t("images", "confirmRemove")}</DialogDescription>
          </DialogHeader>
          {removeConfirm !== null && (
            <div className="rounded-md border bg-muted px-2 py-1 text-xs font-mono text-foreground break-all">
              {images.find((i) => i.id === removeConfirm)?.repoTags.join(", ") ?? removeConfirm}
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setRemoveConfirm(null)}>
              {t("common", "cancel")}
            </Button>
            <Button
              variant="destructive"
              disabled={removing}
              onClick={() => {
                if (removeConfirm !== null) void handleRemove(removeConfirm);
              }}
            >
              {removing ? `${t("common", "loading")}` : t("common", "delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function LocalImageRow({
  image,
  onInspect,
  onRemove,
}: {
  image: LocalImageInfo;
  onInspect: () => void;
  onRemove: () => void;
}) {
  const { t } = useI18n();
  const mainTag = image.repoTags[0] ?? "<none>";
  const additionalTags = image.repoTags.length > 1 ? image.repoTags.length - 1 : 0;
  const createdDate = new Date(image.created * 1000);

  return (
    <div className="flex items-center gap-3 rounded-lg border border-border bg-card px-4 py-3 transition-colors hover:border-primary/30">
      {/* Icon */}
      <div className="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
        <Layers className="h-[18px] w-[18px]" />
      </div>

      {/* Info */}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate font-mono text-sm font-semibold text-foreground">
            {mainTag}
          </span>
          {additionalTags > 0 && (
            <Badge variant="outline" className="text-[10px]">
              +{additionalTags} {t("images", "tags")}
            </Badge>
          )}
        </div>
        <div className="mt-0.5 flex items-center gap-3 text-xs text-muted-foreground">
          <span>{image.sizeHuman}</span>
          <span>{formatRelativeTime(createdDate)}</span>
          <span className="font-mono">{image.id.slice(7, 19)}</span>
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-1">
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={onInspect}
          title={t("images", "inspectImage")}
        >
          <Eye className="h-3.5 w-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 text-destructive hover:text-destructive"
          onClick={onRemove}
          title={t("images", "removeImage")}
        >
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}

/* ========== Search Images Tab ========== */

function SearchImagesTab() {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ImageSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [pulling, setPulling] = useState<string | null>(null);
  const [pullError, setPullError] = useState<string | null>(null);

  const handleSearch = useCallback(async () => {
    if (query.trim().length === 0) return;
    setSearching(true);
    setPullError(null);
    setSearchError(null);
    try {
      const data = await invoke<ImageSearchResult[]>("image_search", { query: query.trim() });
      setResults(data);
    } catch (err) {
      setResults([]);
      setSearchError(
        typeof err === "string" ? err : t("images", "searchError")
      );
    } finally {
      setSearching(false);
    }
  }, [query]);

  const handlePull = useCallback(async (image: string) => {
    setPulling(image);
    setPullError(null);
    try {
      await invoke("image_pull", { image });
    } catch (err) {
      setPullError(typeof err === "string" ? err : `Failed to pull ${image}`);
    } finally {
      setPulling(null);
    }
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") {
        e.preventDefault();
        void handleSearch();
      }
    },
    [handleSearch],
  );

  return (
    <div className="px-6 py-4">
      {/* Search bar */}
      <div className="mb-4 flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t("images", "searchPlaceholder")}
            className="pl-9"
          />
        </div>
        <Button
          size="sm"
          onClick={() => void handleSearch()}
          disabled={searching || query.trim().length === 0}
        >
          {searching ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <Search className="h-3.5 w-3.5" />
          )}
          {t("common", "search")}
        </Button>
      </div>

      {/* Search error */}
      {searchError !== null && (
        <div className="mb-4 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {searchError}
        </div>
      )}

      {/* Pull error */}
      {pullError !== null && (
        <div className="mb-4 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {pullError}
        </div>
      )}

      {/* Results */}
      {results.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-16 text-center text-muted-foreground">
          <Globe className="mb-3 h-12 w-12 opacity-20" />
          <h3 className="text-sm font-medium">{t("images", "searchHint")}</h3>
          <p className="mt-1 text-xs">Docker Hub &middot; Quay.io &middot; GitHub Container Registry</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 lg:grid-cols-2 xl:grid-cols-3">
          {results.map((result, idx) => (
            <SearchResultCard
              key={`${result.source}-${result.reference}-${idx}`}
              result={result}
              pulling={pulling === result.reference}
              onPull={() => void handlePull(result.reference)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function SearchResultCard({
  result,
  pulling,
  onPull,
}: {
  result: ImageSearchResult;
  pulling: boolean;
  onPull: () => void;
}) {
  const { t } = useI18n();

  return (
    <div className="rounded-xl border border-border bg-card p-4 transition-all hover:border-primary/30">
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 flex-col gap-1">
          <div className="flex items-center gap-2">
            <Badge
              variant="outline"
              className="text-[10px]"
            >
              {result.source}
            </Badge>
            {result.official && (
              <Badge className="border-primary/15 bg-primary/10 text-primary text-[10px]">
                {t("images", "official")}
              </Badge>
            )}
          </div>
          <span className="truncate text-sm font-semibold text-foreground">
            {result.reference}
          </span>
          {result.description.length > 0 && (
            <p className="line-clamp-2 text-xs text-muted-foreground">
              {result.description}
            </p>
          )}
        </div>
      </div>

      <div className="mt-3 flex items-center justify-between gap-2">
        <div className="flex items-center gap-3 text-xs text-muted-foreground">
          <span className="inline-flex items-center gap-1">
            <Star className="h-3.5 w-3.5" />
            {result.stars ?? "-"}
          </span>
          <span className="inline-flex items-center gap-1">
            <ArrowDownToLine className="h-3.5 w-3.5" />
            {formatPulls(result.pulls)}
          </span>
        </div>
        <Button
          size="sm"
          variant="outline"
          className="h-7 gap-1 px-2 text-xs"
          disabled={pulling}
          onClick={onPull}
        >
          {pulling ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <Download className="h-3 w-3" />
          )}
          {t("images", "pull")}
        </Button>
      </div>
    </div>
  );
}

/* ========== Helpers ========== */

function formatPulls(pulls?: number): string {
  if (pulls === undefined || pulls === null) return "-";
  if (pulls >= 1_000_000_000) return `${(pulls / 1_000_000_000).toFixed(1)}B`;
  if (pulls >= 1_000_000) return `${(pulls / 1_000_000).toFixed(1)}M`;
  if (pulls >= 1_000) return `${(pulls / 1_000).toFixed(1)}K`;
  return String(pulls);
}

function formatRelativeTime(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffHour = Math.floor(diffMs / 3600000);
  const diffDay = Math.floor(diffMs / 86400000);

  if (diffHour < 1) return "< 1h ago";
  if (diffHour < 24) return `${diffHour}h ago`;
  if (diffDay < 30) return `${diffDay}d ago`;
  return date.toLocaleDateString();
}
