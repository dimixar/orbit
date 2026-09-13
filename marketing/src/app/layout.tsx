import type { Metadata } from "next";
import { Geist_Mono, Inter } from "next/font/google";
import "./globals.css";

const inter = Inter({
  subsets: ["latin"],
  variable: "--font-inter",
  display: "swap",
});

const geistMono = Geist_Mono({
  subsets: ["latin"],
  variable: "--font-geist-mono",
  display: "swap",
});

export const metadata: Metadata = {
  title: "Orbit — a native workbench for the pi coding agent",
  description:
    "Orbit is a Rust desktop client for the pi coding agent. Chat sessions, tools, diffs, and usage drawn by GPUI on the GPU. No browser, no webview, no Node.",
  keywords: [
    "Orbit",
    "pi coding agent",
    "GPUI",
    "Rust",
    "native desktop",
    "coding agent",
    "workbench",
  ],
  openGraph: {
    title: "Orbit — a native workbench for the pi coding agent",
    description:
      "A Rust desktop client for the pi coding agent. No browser, no webview, no Node.",
    type: "website",
    siteName: "Orbit",
  },
  icons: {
    icon: [{ url: "/icon.png", type: "image/png" }],
  },
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html
      lang="en"
      data-scroll-behavior="smooth"
      className={`${inter.variable} ${geistMono.variable} h-full`}
    >
      <body className="min-h-full flex flex-col bg-page text-ink">
        {children}
      </body>
    </html>
  );
}
