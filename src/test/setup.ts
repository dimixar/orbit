import "@testing-library/jest-dom/vitest";
import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

// jsdom lacks these APIs used by copy buttons / clipboard.
Object.assign(navigator, {
  clipboard: {
    writeText: vi.fn().mockResolvedValue(undefined),
  },
});

// jsdom lacks element.scrollTo used by the auto-scroll logic.
if (!Element.prototype.scrollTo) {
  Element.prototype.scrollTo = function scrollTo() {
    // no-op — jsdom has no layout
  };
}

// jsdom lacks ResizeObserver used by the scroll-follow logic.
if (!globalThis.ResizeObserver) {
  class ResizeObserverMock {
    private callback: ResizeObserverCallback;
    constructor(callback: ResizeObserverCallback) {
      this.callback = callback;
    }
    observe() {}
    unobserve() {}
    disconnect() {}
    // Fire once on observe so mount-time pinning logic runs.
    trigger() {
      this.callback([], {} as ResizeObserver);
    }
  }
  globalThis.ResizeObserver = ResizeObserverMock as unknown as typeof ResizeObserver;
}

// Shiki's JS regex engine needs these in jsdom.
if (!window.matchMedia) {
  window.matchMedia = (query: string) =>
    ({
      matches: false,
      media: query,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
    }) as unknown as MediaQueryList;
}
