import { ArrowLeftIcon, ArrowRightIcon } from "@heroicons/react/24/outline";
import { twMerge } from "tailwind-merge";
import { SidebarTrigger, useSidebar } from "@/components/ui/sidebar";
import { isMac } from "@/lib/platform";

/** ⌘ on Apple platforms, Ctrl elsewhere — shown in back/forward tooltips. */
const modKey = /Mac|iPhone|iPad/.test(navigator.userAgent) ? "⌘" : "Ctrl";

/** Square muted icon button matching the sidebar trigger's footprint. */
function TopBarButton({
  label,
  shortcut,
  disabled,
  onPress,
  children,
}: {
  label: string;
  shortcut?: string;
  disabled?: boolean;
  onPress?: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={shortcut ? `${label} (${shortcut})` : label}
      disabled={disabled}
      onClick={onPress}
      className="flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-lg text-muted-fg outline-none transition-colors duration-100 hover:bg-muted hover:text-fg focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40"
    >
      {children}
    </button>
  );
}

/**
 * Minimal app top bar: sidebar expand/collapse, back/forward, and an
 * optional title. Shared by the chat view (session title) and the
 * workbench pages (no title — the page carries its own heading).
 */
export function TopBar({
  title,
  canGoBack = false,
  canGoForward = false,
  onBack,
  onForward,
  trailing,
  className,
}: {
  title?: string;
  canGoBack?: boolean;
  canGoForward?: boolean;
  onBack?: () => void;
  onForward?: () => void;
  /** Right-aligned actions (chips, pickers) — pushed to the bar's end. */
  trailing?: React.ReactNode;
  className?: string;
}) {
  const { state } = useSidebar();
  return (
    // data-tauri-drag-region makes the bar a window drag handle (blank areas
    // only — buttons and menus keep working). On macOS the bar matches the
    // native traffic-lights strip height so its content centers on the same
    // line as the lights and the sidebar lockup; when the sidebar is collapsed
    // to the dock the lights spill ~16px into the bar, so pad past them.
    <header
      data-tauri-drag-region
      className={twMerge(
        "flex h-12 shrink-0 items-center gap-0.5 pr-3.5 pl-3",
        isMac && "h-8",
        isMac && state === "collapsed" && "pl-6",
        className,
      )}
    >
      <SidebarTrigger className="size-7 sm:size-7" />
      <div className="flex items-center gap-0.5">
        <TopBarButton
          label="Back"
          shortcut={`${modKey}[`}
          disabled={!canGoBack}
          onPress={onBack}
        >
          <ArrowLeftIcon className="size-4" strokeWidth={1.6} />
        </TopBarButton>
        <TopBarButton
          label="Forward"
          shortcut={`${modKey}]`}
          disabled={!canGoForward}
          onPress={onForward}
        >
          <ArrowRightIcon className="size-4" strokeWidth={1.6} />
        </TopBarButton>
      </div>

      {title && (
        <h1
          data-tauri-drag-region
          className="ml-1.5 min-w-0 truncate text-[13px] font-medium tracking-[-0.01em] text-fg"
        >
          {title}
        </h1>
      )}

      {trailing && (
        <div className="ml-auto flex shrink-0 items-center gap-1">{trailing}</div>
      )}
    </header>
  );
}