import {
  useCallback,
  useLayoutEffect,
  useRef,
  type RefObject,
} from "react";

/** Distance from the live edge that still counts as “pinned to latest”. */
export const STICK_TO_LATEST_THRESHOLD_PX = 80;

export function isPinnedToLatest(
  scrollTop: number,
  scrollHeight: number,
  clientHeight: number,
  threshold = STICK_TO_LATEST_THRESHOLD_PX,
): boolean {
  return scrollHeight - scrollTop - clientHeight < threshold;
}

type ScrollToEnd = (options?: { behavior?: ScrollBehavior }) => boolean;

/**
 * Classic chat stick-to-bottom: follow streaming growth while the user is
 * at the live edge, re-pin when they send, and release when they scroll up.
 *
 * MessageScroller alone is not enough — a new scroll-anchor turn is aligned
 * to the *start* of the viewport, which hides tokens as the reply grows.
 */
export function useStickToLatest({
  viewportRef,
  contentRef,
  scrollToEnd,
  lastUserMessageId,
  lastUserFingerprint,
  followKey,
}: {
  viewportRef: RefObject<HTMLDivElement | null>;
  contentRef: RefObject<HTMLDivElement | null>;
  scrollToEnd: ScrollToEnd;
  lastUserMessageId: string | null | undefined;
  lastUserFingerprint: string | null;
  followKey: unknown;
}): {
  onViewportScroll: () => void;
  pinLatest: () => void;
} {
  const pinnedRef = useRef(true);
  const lastUserMessageIdRef = useRef(lastUserMessageId);
  // `undefined` so the first layout pass is not treated as a snapshot rebuild.
  const lastUserFingerprintRef = useRef<string | null | undefined>(undefined);
  const lastScrollTopRef = useRef(0);

  const pinFromViewport = useCallback(() => {
    const viewport = viewportRef.current;
    if (!viewport) return true;
    return isPinnedToLatest(
      viewport.scrollTop,
      viewport.scrollHeight,
      viewport.clientHeight,
    );
  }, [viewportRef]);

  const onViewportScroll = useCallback(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    pinnedRef.current = pinFromViewport();
    lastScrollTopRef.current = viewport.scrollTop;
  }, [pinFromViewport, viewportRef]);

  const pinLatest = useCallback(() => {
    pinnedRef.current = true;
    scrollToEnd({ behavior: "auto" });
  }, [scrollToEnd]);

  const followIfPinned = useCallback(
    (force: boolean) => {
      if (force) pinnedRef.current = true;
      if (!pinnedRef.current) return;
      scrollToEnd({ behavior: "auto" });
    },
    [scrollToEnd],
  );

  const restoreReadingPosition = useCallback(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const nextTop = lastScrollTopRef.current;
    if (Math.abs(viewport.scrollTop - nextTop) <= 1) return;
    viewport.scrollTop = nextTop;
  }, [viewportRef]);

  useLayoutEffect(() => {
    const seenFingerprint = lastUserFingerprintRef.current !== undefined;
    const isRebuild =
      seenFingerprint &&
      lastUserFingerprint !== null &&
      lastUserFingerprint === lastUserFingerprintRef.current;
    lastUserFingerprintRef.current = lastUserFingerprint;

    const sentNewMessage =
      Boolean(lastUserMessageId) &&
      lastUserMessageId !== lastUserMessageIdRef.current &&
      !isRebuild;
    if (lastUserMessageId) {
      lastUserMessageIdRef.current = lastUserMessageId;
    }

    if (isRebuild && !sentNewMessage) {
      if (pinnedRef.current) followIfPinned(false);
      else restoreReadingPosition();
      return;
    }

    followIfPinned(sentNewMessage);
  }, [
    followIfPinned,
    followKey,
    lastUserFingerprint,
    lastUserMessageId,
    restoreReadingPosition,
  ]);

  useLayoutEffect(() => {
    const content = contentRef.current;
    if (!content || typeof ResizeObserver === "undefined") return;

    const observer = new ResizeObserver(() => {
      if (pinnedRef.current) followIfPinned(false);
      else restoreReadingPosition();
    });
    observer.observe(content);
    return () => observer.disconnect();
  }, [contentRef, followIfPinned, restoreReadingPosition]);

  return { onViewportScroll, pinLatest };
}
