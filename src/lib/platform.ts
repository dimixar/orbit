/** True on Apple platforms — used to reserve space for the native traffic
 * lights that overlay the webview when the title bar style is "Overlay". */
export const isMac = /Mac|iPhone|iPad/.test(navigator.userAgent)