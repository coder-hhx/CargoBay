import { useCallback, useState } from "react";
import { useContainerStore, type ContainerCreateRequest } from "@/stores/containerStore";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Plus } from "lucide-react";

export function ContainerCreate() {
  const { t } = useI18n();
  const createContainer = useContainerStore((s) => s.createContainer);
  const templates = useContainerStore((s) => s.templates);

  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [image, setImage] = useState("");
  const [templateId, setTemplateId] = useState("");
  const [cpuCores, setCpuCores] = useState(2);
  const [memoryMb, setMemoryMb] = useState(2048);
  const [creating, setCreating] = useState(false);

  const resetForm = useCallback(() => {
    setName("");
    setImage("");
    setTemplateId("");
    setCpuCores(2);
    setMemoryMb(2048);
  }, []);

  const selectedTemplate = templates.find((tmpl) => tmpl.id === templateId);

  const handleTemplateChange = useCallback(
    (id: string) => {
      setTemplateId(id);
      const tmpl = templates.find((t2) => t2.id === id);
      if (tmpl) {
        setImage(tmpl.image);
        setCpuCores(tmpl.defaultCpuCores);
        setMemoryMb(tmpl.defaultMemoryMb);
      }
    },
    [templates],
  );

  const canCreate = image.trim().length > 0;

  const handleCreate = useCallback(async () => {
    if (!canCreate) return;
    setCreating(true);
    try {
      const req: ContainerCreateRequest = {
        templateId: templateId || "custom",
        name: name.trim() || undefined,
        image: image.trim(),
        cpuCores,
        memoryMb,
      };
      await createContainer(req);
      resetForm();
      setOpen(false);
    } finally {
      setCreating(false);
    }
  }, [canCreate, templateId, name, image, cpuCores, memoryMb, createContainer, resetForm]);

  return (
    <Dialog open={open} onOpenChange={(v) => { setOpen(v); if (!v) resetForm(); }}>
      <DialogTrigger asChild>
        <Button size="sm">
          <Plus className="h-4 w-4" />
          {t("containers", "create")}
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("containers", "create")}</DialogTitle>
          <DialogDescription>
            {t("containers", "createDesc")}
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-4 py-4">
          {/* Template */}
          {templates.length > 0 && (
            <div className="flex flex-col gap-1.5">
              <Label>{t("containers", "template")}</Label>
              <Select value={templateId} onValueChange={handleTemplateChange}>
                <SelectTrigger>
                  <SelectValue placeholder={t("containers", "selectTemplate")} />
                </SelectTrigger>
                <SelectContent>
                  {templates.map((tmpl) => (
                    <SelectItem key={tmpl.id} value={tmpl.id}>
                      {tmpl.name} — {tmpl.description}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          )}

          {/* Name */}
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="container-name">{t("containers", "nameOptional")}</Label>
            <Input
              id="container-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="my-container"
            />
          </div>

          {/* Image */}
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="container-image">{t("containers", "image")}</Label>
            <Input
              id="container-image"
              value={image}
              onChange={(e) => setImage(e.target.value)}
              placeholder={selectedTemplate?.image ?? "ubuntu:latest"}
            />
          </div>

          {/* Resources */}
          <div className="grid grid-cols-2 gap-4">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="cpu">{t("containers", "cpuCores")}</Label>
              <Input
                id="cpu"
                type="number"
                min={1}
                max={16}
                value={cpuCores}
                onChange={(e) => setCpuCores(Number(e.target.value) || 2)}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="memory">{t("containers", "memoryMb")}</Label>
              <Input
                id="memory"
                type="number"
                min={256}
                max={65536}
                step={256}
                value={memoryMb}
                onChange={(e) => setMemoryMb(Number(e.target.value) || 2048)}
              />
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)}>
            {t("common", "cancel")}
          </Button>
          <Button onClick={handleCreate} disabled={!canCreate || creating}>
            {creating ? `${t("containers", "creating")}...` : t("containers", "create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
