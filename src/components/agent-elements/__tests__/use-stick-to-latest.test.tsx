import { describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useRef } from "react";
import {
  isPinnedToLatest,
  useStickToLatest,
} from "../use-stick-to-latest";

describe("isPinnedToLatest", () => {
  it("treats the live edge as pinned", () => {
    expect(isPinnedToLatest(920, 1000, 80)).toBe(true);
  });

  it("releases once the user scrolls away from the latest message", () => {
    expect(isPinnedToLatest(100, 1000, 80)).toBe(false);
  });
});

describe("useStickToLatest", () => {
  function setup(initialFollowKey = "1:user-0:4:idle") {
    const scrollToEnd = vi.fn(() => true);
    const viewport = document.createElement("div");
    const content = document.createElement("div");
    let scrollTop = 920;
    Object.defineProperties(viewport, {
      scrollTop: {
        get: () => scrollTop,
        set: (value: number) => {
          scrollTop = value;
        },
        configurable: true,
      },
      scrollHeight: { get: () => 1000, configurable: true },
      clientHeight: { get: () => 80, configurable: true },
    });

    const hook = renderHook(
      (props: {
        lastUserMessageId: string;
        lastUserFingerprint: string;
        followKey: string;
      }) => {
        const viewportRef = useRef<HTMLDivElement | null>(viewport);
        const contentRef = useRef<HTMLDivElement | null>(content);
        return useStickToLatest({
          viewportRef,
          contentRef,
          scrollToEnd,
          ...props,
        });
      },
      {
        initialProps: {
          lastUserMessageId: "user-0",
          lastUserFingerprint: "hi",
          followKey: initialFollowKey,
        },
      },
    );

    return {
      hook,
      scrollToEnd,
      getScrollTop: () => scrollTop,
      setScrollTop: (value: number) => {
        scrollTop = value;
      },
    };
  }

  it("follows streaming growth while pinned to the bottom", () => {
    const { hook, scrollToEnd } = setup();
    scrollToEnd.mockClear();

    act(() => {
      hook.rerender({
        lastUserMessageId: "user-0",
        lastUserFingerprint: "hi",
        followKey: "2:assistant-1:40:streaming",
      });
    });

    expect(scrollToEnd).toHaveBeenCalledWith({ behavior: "auto" });
  });

  it("does not chase the stream after the user scrolls up", () => {
    const { hook, scrollToEnd, setScrollTop } = setup();

    act(() => {
      setScrollTop(40);
      hook.result.current.onViewportScroll();
    });
    scrollToEnd.mockClear();

    act(() => {
      hook.rerender({
        lastUserMessageId: "user-0",
        lastUserFingerprint: "hi",
        followKey: "2:assistant-1:80:streaming",
      });
    });

    expect(scrollToEnd).not.toHaveBeenCalled();
  });

  it("re-pins to the latest turn when the user sends again", () => {
    const { hook, scrollToEnd, setScrollTop } = setup();

    act(() => {
      setScrollTop(40);
      hook.result.current.onViewportScroll();
    });
    scrollToEnd.mockClear();

    act(() => {
      hook.rerender({
        lastUserMessageId: "user-2",
        lastUserFingerprint: "next question",
        followKey: "3:user-2:13:streaming",
      });
    });

    expect(scrollToEnd).toHaveBeenCalledWith({ behavior: "auto" });
  });

  it("keeps a pinned viewport on the live edge across a same-fingerprint rebuild", () => {
    const { hook, scrollToEnd } = setup();
    scrollToEnd.mockClear();

    act(() => {
      hook.rerender({
        lastUserMessageId: "user-99",
        lastUserFingerprint: "hi",
        followKey: "2:assistant-1:12:idle",
      });
    });

    expect(scrollToEnd).toHaveBeenCalledWith({ behavior: "auto" });
  });

  it("re-engages follow when jump-to-latest pins again", () => {
    const { hook, scrollToEnd, setScrollTop } = setup();

    act(() => {
      setScrollTop(40);
      hook.result.current.onViewportScroll();
    });
    scrollToEnd.mockClear();

    act(() => {
      hook.result.current.pinLatest();
    });

    expect(scrollToEnd).toHaveBeenCalledWith({ behavior: "auto" });
    scrollToEnd.mockClear();

    act(() => {
      hook.rerender({
        lastUserMessageId: "user-0",
        lastUserFingerprint: "hi",
        followKey: "2:assistant-1:120:streaming",
      });
    });

    expect(scrollToEnd).toHaveBeenCalledWith({ behavior: "auto" });
  });

  it("leaves a scrolled-away viewport alone across a same-fingerprint rebuild", () => {
    const { hook, scrollToEnd, setScrollTop } = setup();

    act(() => {
      setScrollTop(40);
      hook.result.current.onViewportScroll();
    });
    scrollToEnd.mockClear();

    act(() => {
      hook.rerender({
        lastUserMessageId: "user-99",
        lastUserFingerprint: "hi",
        followKey: "2:assistant-1:12:idle",
      });
    });

    expect(scrollToEnd).not.toHaveBeenCalled();
  });

  it("puts a scrolled-away viewport back after a snapshot remount jumps it", () => {
    const { hook, scrollToEnd, getScrollTop, setScrollTop } = setup();

    act(() => {
      setScrollTop(40);
      hook.result.current.onViewportScroll();
    });
    scrollToEnd.mockClear();

    act(() => {
      // MessageScroller align-start / spacer fight lands mid-transcript.
      setScrollTop(400);
      hook.rerender({
        lastUserMessageId: "user-99",
        lastUserFingerprint: "hi",
        followKey: "2:assistant-1:12:rebuild",
      });
    });

    expect(scrollToEnd).not.toHaveBeenCalled();
    expect(getScrollTop()).toBe(40);
  });
});
