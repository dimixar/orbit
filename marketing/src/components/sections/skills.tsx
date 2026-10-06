import { Shot } from "@/components/shot";
import { Story } from "@/components/sections/story";

export function Skills() {
  return (
    <Story
      id="skills"
      label="Skills"
      title="Teach it your team's playbook."
      copy="Skills are SKILL.md playbooks pi loads at startup — user-level and project-level. Browse them in Orbit, read the full instructions, and enable or disable any of them with one switch."
      points={[
        "Project and user skills, browsable with full SKILL.md preview.",
        "Enable or disable a skill without editing files.",
        "Invoked as /skill: commands right from the composer.",
      ]}
      visual={
        <Shot
          src="/screens/skills.png"
          srcLight="/screens/skills-light.png"
          alt="Orbit's Skills page with the skill list on the left and the full SKILL.md of the selected skill on the right."
          sizes="(max-width: 1024px) 100vw, 560px"
        />
      }
    />
  );
}
