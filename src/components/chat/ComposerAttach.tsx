"use client";

/**
 * Staged composer attachments and the drop overlay.
 *
 * THESIS: Files land in the card from any directory — images as chips
 * the model can see, other files as `@` mentions already in the draft.
 * OWN-WORLD: Same chip density, 10px radius, and muted tokens as the composer.
 * STORY: Preview, remove, drop; send stays on the toolbar.
 */

import { PaperClipIcon, XMarkIcon } from "@heroicons/react/24/outline";
import { twMerge } from "tailwind-merge";
import {
  formatAttachSize,
  type AttachNotice,
  type ComposerImage,
} from "@/lib/composer-attach";

export function ComposerAttachChips({
  images,
  onRemove,
}: {
  images: ComposerImage[];
  onRemove: (id: string) => void;
}) {
  if (images.length === 0) return null;
  return (
    <ul
      aria-label="Attached images"
      className="mb-2.5 flex flex-wrap gap-2"
    >
      {images.map((image) => (
        <li key={image.id} className="relative">
          <figure className="flex items-center gap-2 rounded-lg bg-secondary py-1 pr-2 pl-1">
            <img
              src={image.dataUrl}
              alt={image.name}
              className="size-9 shrink-0 rounded-md object-cover"
            />
            <figcaption className="min-w-0">
              <span className="block max-w-36 truncate font-mono text-[11px] text-fg">
                {image.name}
              </span>
              <span className="block text-[10px] text-muted-fg">
                {formatAttachSize(image.size)}
              </span>
            </figcaption>
          </figure>
          <button
            type="button"
            aria-label={`Remove ${image.name}`}
            onClick={() => onRemove(image.id)}
            className="absolute -top-1.5 -right-1.5 grid size-5 place-items-center rounded-full bg-secondary text-muted-fg outline-hidden transition-colors duration-100 hover:bg-muted hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
          >
            <XMarkIcon className="size-3" strokeWidth={2} />
          </button>
        </li>
      ))}
    </ul>
  );
}

export function ComposerAttachNotice({ notice }: { notice: AttachNotice }) {
  if (!notice) return null;
  return (
    <p
      role="status"
      className={twMerge(
        "mt-2 text-[11px] leading-4",
        notice.tone === "danger" ? "text-danger-subtle-fg" : "text-muted-fg",
      )}
    >
      {notice.message}
    </p>
  );
}

export function ComposerDropOverlay({ active }: { active: boolean }) {
  return (
    <div
      aria-hidden={!active}
      className={twMerge(
        "pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-[10px] bg-card/90 transition-opacity duration-150",
        active ? "opacity-100" : "opacity-0",
      )}
    >
      <div className="flex items-center gap-2.5 rounded-lg bg-secondary px-3 py-2 text-xs text-fg">
        <span className="grid size-6.5 place-items-center rounded-lg bg-muted text-muted-fg">
          <PaperClipIcon className="size-3.5" strokeWidth={1.8} />
        </span>
        Drop files from anywhere to attach
      </div>
    </div>
  );
}
