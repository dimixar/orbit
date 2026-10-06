import { Shot } from "@/components/shot";
import { Story } from "@/components/sections/story";

export function NewTask() {
  return (
    <Story
      id="new-task"
      label="New task"
      title="Pick a workspace. Describe the task."
      copy="Choose Plan, Build, or Ask, point Orbit at a workspace, and say what you want. Model, thinking effort, and access mode sit right in the composer — press send and the run begins in a live session."
      points={[
        "Plan, Build, or Ask — one toggle before the run starts.",
        "Workspace picked from your projects, remembered across sessions.",
        "Access mode, model, and thinking level set beside the composer.",
      ]}
      visual={
        <Shot
          src="/screens/new-task.png"
          srcLight="/screens/new-task-light.png"
          alt="Orbit's New Task page with Plan, Build, and Ask mode toggles, a workspace picker, and the composer below."
          sizes="(max-width: 1024px) 100vw, 560px"
        />
      }
    />
  );
}
