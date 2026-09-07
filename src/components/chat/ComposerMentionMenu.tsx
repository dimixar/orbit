"use client";

/**
 * Mention popover for the prompt composer.
 *
 * THESIS: `/` and `@` stay in the composer — a list above the card, not a
 * command-palette modal.
 * OWN-WORLD: Same overlay, row density, and mono names as the model picker.
 * STORY: Pick a skill, prompt, or file; the token is replaced; typing continues.
 * FIRST VIEWPORT: Popover flush with the composer; Skills then Prompts, or Files.
 * FORM: Local extension of the incumbent composer.
 */

import { DocumentTextIcon, SparklesIcon } from "@heroicons/react/24/outline";
import type { RefObject } from "react";
import { Header } from "react-aria-components/Header";
import { twMerge } from "tailwind-merge";
import { dropdownItemStyles, dropdownSectionStyles } from "@/components/ui/dropdown";
import { menuContentStyles } from "@/components/ui/menu";
import { PopoverContent } from "@/components/ui/popover";
import { fileLabel } from "@/lib/composer-mention";
import { FileTypeIcon } from "./file-type-icon";

export type SlashMentionItem = {
  id: string;
  kind: "skill" | "prompt";
  name: string;
  description: string;
  argumentHint?: string;
  insert: string;
};

export type FileMentionItem = {
  id: string;
  kind: "file";
  path: string;
  insert: string;
};

export type MentionItem = SlashMentionItem | FileMentionItem;

const { header: sectionHeader } = dropdownSectionStyles();

export function ComposerMentionMenu({
  open,
  triggerRef,
  kind,
  items,
  highlightedIndex,
  loading,
  error,
  emptyMessage,
  listboxId,
  onHighlight,
  onSelect,
  onOpenChange,
}: {
  open: boolean;
  triggerRef: RefObject<Element | null>;
  kind: "slash" | "file";
  items: MentionItem[];
  highlightedIndex: number;
  loading: boolean;
  error: boolean;
  emptyMessage: string;
  listboxId: string;
  onHighlight: (index: number) => void;
  onSelect: (item: MentionItem) => void;
  onOpenChange: (open: boolean) => void;
}) {
  const skills = items.filter((item): item is SlashMentionItem => item.kind === "skill");
  const prompts = items.filter((item): item is SlashMentionItem => item.kind === "prompt");
  const files = items.filter((item): item is FileMentionItem => item.kind === "file");

  return (
    <PopoverContent
      isOpen={open}
      onOpenChange={onOpenChange}
      triggerRef={triggerRef}
      placement="top"
      isNonModal
      className="w-[var(--trigger-width)] max-w-none"
    >
      <div
        id={listboxId}
        role="listbox"
        aria-label={kind === "slash" ? "Skills and prompts" : "Project files"}
        aria-busy={loading || undefined}
        className={twMerge(menuContentStyles(), "max-h-72 p-1")}
      >
        {loading ? (
          <div className="space-y-1 p-1" aria-hidden="true">
            {Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="h-8 animate-pulse rounded-md bg-muted" />
            ))}
            <span className="sr-only">Loading</span>
          </div>
        ) : error || items.length === 0 ? (
          <p className="col-span-full px-2.5 py-6 text-center text-[11px] text-muted-fg">
            {emptyMessage}
          </p>
        ) : kind === "slash" ? (
          <>
            {skills.length > 0 && (
              <section className="col-span-full grid grid-cols-[auto_1fr]">
                <Header className={sectionHeader()}>Skills</Header>
                {skills.map((item) => (
                  <MentionRow
                    key={item.id}
                    item={item}
                    index={items.indexOf(item)}
                    highlighted={items[highlightedIndex] === item}
                    listboxId={listboxId}
                    onHighlight={onHighlight}
                    onSelect={onSelect}
                  />
                ))}
              </section>
            )}
            {prompts.length > 0 && (
              <section className="col-span-full grid grid-cols-[auto_1fr]">
                <Header className={sectionHeader()}>Prompts</Header>
                {prompts.map((item) => (
                  <MentionRow
                    key={item.id}
                    item={item}
                    index={items.indexOf(item)}
                    highlighted={items[highlightedIndex] === item}
                    listboxId={listboxId}
                    onHighlight={onHighlight}
                    onSelect={onSelect}
                  />
                ))}
              </section>
            )}
          </>
        ) : (
          files.map((item, index) => (
            <MentionRow
              key={item.id}
              item={item}
              index={index}
              highlighted={index === highlightedIndex}
              listboxId={listboxId}
              onHighlight={onHighlight}
              onSelect={onSelect}
            />
          ))
        )}
      </div>
    </PopoverContent>
  );
}

function MentionRow({
  item,
  index,
  highlighted,
  listboxId,
  onHighlight,
  onSelect,
}: {
  item: MentionItem;
  index: number;
  highlighted: boolean;
  listboxId: string;
  onHighlight: (index: number) => void;
  onSelect: (item: MentionItem) => void;
}) {
  const optionId = `${listboxId}-opt-${index}`;
  return (
    <div
      id={optionId}
      role="option"
      aria-selected={highlighted}
      data-focused={highlighted || undefined}
      className={dropdownItemStyles({ isFocused: highlighted, className: "cursor-pointer" })}
      onMouseEnter={() => onHighlight(index)}
      onMouseDown={(event) => {
        event.preventDefault();
        onSelect(item);
      }}
    >
      {item.kind === "file" ? (
        <FileRow path={item.path} />
      ) : item.kind === "skill" ? (
        <SlashRow
          icon={<SparklesIcon className="size-4" strokeWidth={1.8} />}
          name={`/${item.name}`}
          description={item.description}
          argumentHint={item.argumentHint}
        />
      ) : (
        <SlashRow
          icon={<DocumentTextIcon className="size-4" strokeWidth={1.8} />}
          name={`/${item.name}`}
          description={item.description}
          argumentHint={item.argumentHint}
        />
      )}
    </div>
  );
}

function SlashRow({
  icon,
  name,
  description,
  argumentHint,
}: {
  icon: React.ReactNode;
  name: string;
  description: string;
  argumentHint?: string;
}) {
  return (
    <>
      {icon}
      <span className="min-w-0">
        <span className="flex items-baseline gap-2">
          <span slot="label" className="truncate font-mono text-xs">
            {name}
          </span>
          {argumentHint && (
            <span className="truncate font-mono text-[11px] text-muted-fg">{argumentHint}</span>
          )}
        </span>
        {description && (
          <span slot="description" className="line-clamp-1 text-[11px] text-muted-fg">
            {description}
          </span>
        )}
      </span>
    </>
  );
}

function FileRow({ path }: { path: string }) {
  const { name, dir } = fileLabel(path);
  return (
    <>
      <FileTypeIcon filename={name} className="size-4 shrink-0" />
      <span className="flex min-w-0 items-baseline gap-3">
        <span slot="label" className="max-w-[58%] truncate font-mono text-xs">
          {name}
        </span>
        {dir ? (
          <span className="min-w-0 truncate font-mono text-[11px] text-muted-fg">
            {dir}
          </span>
        ) : null}
      </span>
    </>
  );
}
