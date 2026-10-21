/** Durations for Svelte transitions, in milliseconds. They collapse to zero when the user
 *  prefers reduced motion; CSS transitions and animations are zeroed in app.css the same way. */

const query =
  typeof matchMedia === "function" ? matchMedia("(prefers-reduced-motion: reduce)") : null;

/** Whether the system asks for reduced motion right now. */
export function reducedMotion(): boolean {
  return query?.matches ?? false;
}

/** `duration`, or 0 under reduced motion. */
export function ms(duration: number): number {
  return reducedMotion() ? 0 : duration;
}

/** The three durations of the design: hover feedback, element changes, page changes. */
export const FAST = 120;
export const NORMAL = 180;
export const SLOW = 260;
