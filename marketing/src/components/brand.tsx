import type { SVGProps } from "react";

export function OrbitMark(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 32 32" fill="none" aria-hidden="true" {...props}>
      <circle cx="16" cy="16" r="6.25" stroke="currentColor" strokeWidth="2" />
      <ellipse
        cx="16"
        cy="16"
        rx="14"
        ry="7.25"
        stroke="currentColor"
        strokeWidth="1.5"
        opacity="0.5"
        transform="rotate(-28 16 16)"
      />
      <circle cx="27.4" cy="9.6" r="2.4" fill="currentColor" />
    </svg>
  );
}
