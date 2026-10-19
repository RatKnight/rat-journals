import type { JSX } from 'solid-js';

interface RatVaultIconProps {
  size?: number;
  class?: string;
  'aria-hidden'?: JSX.HTMLAttributes<SVGSVGElement>['aria-hidden'];
}

/**
 * The vault's icon: a safe with a round vault door, rat ears on top and a curled tail.
 * Drawn like the lucide icons around it (24×24, `currentColor` stroke, width 2).
 */
export default function RatVaultIcon(props: RatVaultIconProps) {
  const size = () => props.size ?? 24;
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size()}
      height={size()}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      class={props.class}
      aria-hidden={props['aria-hidden'] ?? 'true'}
      data-testid="rat-vault-icon"
    >
      {/* Ears */}
      <path d="M4.5 7a2.2 2.2 0 1 1 4 0" />
      <path d="M13.5 7a2.2 2.2 0 1 1 4 0" />
      {/* Safe */}
      <rect x="3" y="7" width="16" height="13" rx="2" />
      {/* Vault door and its wheel */}
      <circle cx="11" cy="13.5" r="3.5" />
      <path d="M11 11.5v4M9 13.5h4" />
      {/* Hinges */}
      <path d="M5.5 10.5v1.5M5.5 15v1.5" />
      {/* Tail */}
      <path d="M19 17c2 0 3 .8 3 2.2 0 1.6-1.6 2.3-3 1.6" />
    </svg>
  );
}
