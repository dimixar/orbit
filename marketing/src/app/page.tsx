import { Architecture } from "@/components/sections/architecture";
import { Closer } from "@/components/sections/closer";
import { Features } from "@/components/sections/features";
import { Gallery } from "@/components/sections/gallery";
import { Hero } from "@/components/sections/hero";
import { Missions } from "@/components/sections/missions";
import { Quickstart } from "@/components/sections/quickstart";
import { Review } from "@/components/sections/review";
import { Stage } from "@/components/sections/stage";
import { SiteFooter } from "@/components/site-footer";
import { SiteNav } from "@/components/site-nav";

export default function Home() {
  return (
    <>
      <SiteNav />
      <main className="flex-1">
        <Hero />
        <Stage />
        <Quickstart />
        <Missions />
        <Features />
        <Review />
        <Gallery />
        <Architecture />
        <Closer />
      </main>
      <SiteFooter />
    </>
  );
}
