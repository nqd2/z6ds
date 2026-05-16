type IconProps = { size?: number; className?: string };

/** VS Code–style tree twistie (chevron-right, rotates when expanded). */
export function IconTreeChevron({
  size = 16,
  className = "",
  expanded = false,
}: IconProps & { expanded?: boolean }) {
  return (
    <svg
      className={`tree-chevron${expanded ? " tree-chevron-expanded" : ""} ${className}`.trim()}
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
    >
      <path
        d="M6.2 4.5 9.8 8l-3.6 3.5"
        stroke="currentColor"
        strokeWidth="1.25"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function IconFolder({ size = 16, className }: IconProps) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
    >
      <path
        d="M1.5 4.5A1 1 0 0 1 2.5 3.5H6l1.2 1.2H13.5A1 1 0 0 1 14.5 5.5v7a1 1 0 0 1-1 1H2.5a1 1 0 0 1-1-1v-7Z"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function IconCode({ size = 16, className }: IconProps) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
    >
      <path
        d="M5 4.5 2.5 8 5 11.5M11 4.5 13.5 8 11 11.5M9 2.5 7 13.5"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function IconCube({ size = 16, className }: IconProps) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
    >
      <path
        d="M8 1.5 13.5 4.5v7L8 14.5 2.5 11.5v-7L8 1.5Z"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
      <path d="M8 1.5v13M2.5 4.5 8 7.5 13.5 4.5" stroke="currentColor" strokeWidth="1.2" />
    </svg>
  );
}

export function IconPlay({ size = 16, className }: IconProps) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
    >
      <path
        d="M5.5 4.2v7.6c0 .6.7 1 .7 1l5.8-3.8c.5-.3.5-1 0-1.3L6.2 3.2s-.7.4-.7 1Z"
        fill="currentColor"
      />
    </svg>
  );
}

export function IconStop({ size = 16, className }: IconProps) {
  return (
    <svg className={className} width={size} height={size} viewBox="0 0 16 16" aria-hidden>
      <rect x="4.5" y="4.5" width="7" height="7" rx="1" fill="currentColor" />
    </svg>
  );
}

export function IconHammer({ size = 16, className }: IconProps) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
    >
      <path
        d="M3 13 9.5 6.5M10 6 12 4l-2-2-2 2 2 2M6.5 7.5 4 10"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
