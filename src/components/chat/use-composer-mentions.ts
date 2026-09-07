"use client";

import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type KeyboardEvent,
  type RefObject,
} from "react";
import { fetchComposerCommands, fetchWorkspaceFiles } from "@/lib/pi-client";
import type { PiComposerCommand } from "@/lib/pi-agent";
import { appendFileMentions } from "@/lib/composer-attach";
import {
  filterCommands,
  filterFiles,
  findActiveMention,
  mentionInsert,
  replaceMention,
} from "@/lib/composer-mention";
import type { MentionItem } from "./ComposerMentionMenu";

const LISTBOX_ID = "composer-mention-list";

export function useComposerMentions({
  draft,
  setDraft,
  textareaRef,
  workspacePath,
}: {
  draft: string;
  setDraft: (value: string) => void;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  workspacePath?: string;
}) {
  const [caret, setCaret] = useState(0);
  const [dismissed, setDismissed] = useState(false);
  const [highlighted, setHighlighted] = useState(0);
  const [commands, setCommands] = useState<PiComposerCommand[] | null | undefined>(
    undefined,
  );
  const [files, setFiles] = useState<string[] | null | undefined>(undefined);

  const mention = useMemo(() => findActiveMention(draft, caret), [draft, caret]);

  useEffect(() => {
    if (!mention) setDismissed(false);
  }, [mention]);

  useEffect(() => {
    setCommands(undefined);
    setFiles(undefined);
  }, [workspacePath]);

  const open = Boolean(mention) && !dismissed;

  useEffect(() => {
    if (!open || mention?.kind !== "slash" || commands !== undefined) return;
    let cancelled = false;
    void fetchComposerCommands(workspacePath).then((data) => {
      if (!cancelled) setCommands(data);
    });
    return () => {
      cancelled = true;
    };
  }, [open, mention?.kind, workspacePath, commands]);

  useEffect(() => {
    if (!open || mention?.kind !== "file") return;
    if (!workspacePath) {
      if (files === undefined) setFiles([]);
      return;
    }
    if (files !== undefined) return;
    let cancelled = false;
    void fetchWorkspaceFiles(workspacePath).then((data) => {
      if (!cancelled) setFiles(data);
    });
    return () => {
      cancelled = true;
    };
  }, [open, mention?.kind, workspacePath, files]);

  const items = useMemo<MentionItem[]>(() => {
    if (!mention) return [];
    if (mention.kind === "slash") {
      if (!commands) return [];
      return filterCommands(commands, mention.query).map((command) => ({
        id: command.id,
        kind: command.kind,
        name: command.name,
        description: command.description,
        argumentHint: command.argumentHint,
        insert: mentionInsert("/", command.name),
      }));
    }
    if (!files) return [];
    return filterFiles(files, mention.query).map((path) => ({
      id: path,
      kind: "file",
      path,
      insert: mentionInsert("@", path),
    }));
  }, [mention, commands, files]);

  useEffect(() => {
    setHighlighted(0);
  }, [mention?.kind, mention?.query]);

  const highlightedIndex =
    items.length === 0 ? 0 : Math.min(highlighted, items.length - 1);

  const applyDraft = useCallback(
    (next: string, nextCaret: number) => {
      setDraft(next);
      setCaret(nextCaret);
      requestAnimationFrame(() => {
        const el = textareaRef.current;
        if (!el) return;
        el.focus();
        el.setSelectionRange(nextCaret, nextCaret);
        el.style.height = "auto";
        el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
      });
    },
    [setDraft, textareaRef],
  );

  const select = useCallback(
    (item: MentionItem) => {
      if (!mention) return;
      const next = replaceMention(draft, mention, item.insert);
      setDismissed(true);
      applyDraft(next.text, next.caret);
    },
    [applyDraft, draft, mention],
  );

  const insertTrigger = useCallback(
    (trigger: "/" | "@") => {
      const el = textareaRef.current;
      const start = el?.selectionStart ?? draft.length;
      const end = el?.selectionEnd ?? start;
      const current = findActiveMention(draft, start);
      if (current?.trigger === trigger) {
        setDismissed(false);
        el?.focus();
        return;
      }
      const prefix = start > 0 && !/\s/.test(draft.charAt(start - 1)) ? " " : "";
      const inserted = `${prefix}${trigger}`;
      applyDraft(draft.slice(0, start) + inserted + draft.slice(end), start + inserted.length);
      setDismissed(false);
    },
    [applyDraft, draft, textareaRef],
  );

  const insertFileMentions = useCallback(
    (paths: string[]) => {
      const unique = [...new Set(paths.filter(Boolean))];
      if (unique.length === 0) return;
      const next = appendFileMentions(draft, unique);
      applyDraft(next.text, next.caret);
      setDismissed(true);
    },
    [applyDraft, draft],
  );

  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLTextAreaElement>): boolean => {
      if (!open || event.nativeEvent.isComposing) return false;
      if (event.key === "Escape") {
        event.preventDefault();
        setDismissed(true);
        return true;
      }
      if (items.length === 0) return false;
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setHighlighted((index) => (index + 1) % items.length);
        return true;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setHighlighted((index) => (index - 1 + items.length) % items.length);
        return true;
      }
      if ((event.key === "Enter" && !event.shiftKey) || event.key === "Tab") {
        event.preventDefault();
        const item = items[highlightedIndex];
        if (item) select(item);
        return true;
      }
      return false;
    },
    [highlightedIndex, items, open, select],
  );

  const syncCaret = useCallback(() => {
    const el = textareaRef.current;
    if (el) setCaret(el.selectionStart);
  }, [textareaRef]);

  const onDraftChange = useCallback(
    (value: string, selectionStart: number) => {
      setDraft(value);
      setCaret(selectionStart);
    },
    [setDraft],
  );

  const loading =
    (open && mention?.kind === "slash" && commands === undefined) ||
    (open && mention?.kind === "file" && Boolean(workspacePath) && files === undefined);

  const error =
    (mention?.kind === "slash" && commands === null) ||
    (mention?.kind === "file" && files === null);

  const emptyMessage = (() => {
    if (mention?.kind === "file" && !workspacePath) {
      return "Open a folder to mention files.";
    }
    if (error) {
      return mention?.kind === "file"
        ? "Couldn't load project files."
        : "Couldn't load skills and prompts.";
    }
    if (mention?.kind === "file") {
      return mention.query
        ? `No files match “${mention.query}”.`
        : "No project files found.";
    }
    return mention?.query
      ? `No skills or prompts match “${mention.query}”.`
      : "No skills or prompts installed.";
  })();

  return {
    mention,
    open,
    items,
    highlightedIndex,
    loading,
    error,
    emptyMessage,
    listboxId: LISTBOX_ID,
    activeOptionId:
      open && items.length > 0 ? `${LISTBOX_ID}-opt-${highlightedIndex}` : undefined,
    onKeyDown,
    onDraftChange,
    syncCaret,
    insertTrigger,
    insertFileMentions,
    select,
    setHighlighted,
    dismiss: () => setDismissed(true),
  };
}
