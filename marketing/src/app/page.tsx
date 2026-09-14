import { Closer } from "@/components/sections/closer";
import { Features } from "@/components/sections/features";
import { Gallery } from "@/components/sections/gallery";
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
        <Review />
        <Gallery />
        <Closer />
      </main>
      <SiteFooter />
    </>
  );
}
