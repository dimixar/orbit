import type { Metadata } from "next";
import { Geist_Mono, Sora } from "next/font/google";
import "./globals.css";

const sora = Sora({
  subsets: ["latin"],
  variable: "--font-sora",
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
    "Orbit is a desktop client for the pi coding agent. Sessions, providers, Git, and usage — chat, review, and commit in one native app.",
  keywords: [
    "Orbit",
    "pi coding agent",
    "coding agent",
    "desktop app",
    "workbench",
    "AI coding",
    "providers",
    "git",
  ],
  openGraph: {
    title: "Orbit — a native workbench for the pi coding agent",
    description:
      "A desktop client for the pi coding agent — chat, review, and commit in one window.",
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
      className={`dark ${sora.variable} ${geistMono.variable} h-full`}
    >
      <body className="min-h-full flex flex-col bg-page text-ink">
        {children}
      </body>
    </html>
  );
}
