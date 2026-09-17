import Script from "next/script";

import { Closer } from "@/components/sections/closer";
import { Features } from "@/components/sections/features";
import { Gallery } from "@/components/sections/gallery";
import { Guard } from "@/components/sections/guard";
import { Hero } from "@/components/sections/hero";
import { Review } from "@/components/sections/review";
import { Stage } from "@/components/sections/stage";
import { Steps } from "@/components/sections/steps";
import { Workbench } from "@/components/sections/workbench";
import { SiteFooter } from "@/components/site-footer";
import { SiteNav } from "@/components/site-nav";

export default function Home() {
  return (
    <>
      <SiteNav />
      <main className="flex-1">
        <Hero />
        <Stage />
        <Steps />
        <Workbench />
        <Features />
        <Guard />
        <Review />
        <Gallery />
        <Closer />
      </main>
      <SiteFooter />
      <Script
        src="https://tracking.rajeshwar.tech/api/script.js"
        data-site-id="8c362549e036"
        strategy="afterInteractive"
      />
    </>
  );
}
