"use client";

import {
  FileTrigger as FileTriggerPrimitive,
  type FileTriggerProps,
} from "react-aria-components/FileTrigger";

/** Intent FileTrigger primitive — compose your own trigger child. */
export function FileTrigger(props: FileTriggerProps) {
  return <FileTriggerPrimitive {...props} />;
}

export type { FileTriggerProps };
