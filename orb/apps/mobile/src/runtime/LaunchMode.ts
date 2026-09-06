import type { OrbMode } from '../router/contracts';

/** Which conversation a session opens in. Turn-level modes ('converse'/'teach') are decided per
 *  turn by the router and are not launchable, so the launch domain is narrower than OrbMode. */
export type LaunchMode = Extract<OrbMode, 'focus' | 'build'>;

/**
 * Launch mode: explicit prop wins, then ORB_LAUNCH_MODE, then focus. An unrecognised env value
 * degrades to focus and warns — it must never crash the app, and it must never be forwarded to the
 * relay, where an unknown mode is a typed 422 (schemas.py:90-96).
 */
export function resolveLaunchMode(prop?: LaunchMode, env?: string): LaunchMode {
  if (prop) return prop;
  const raw = env?.trim().toLowerCase();
  if (raw === 'build' || raw === 'focus') return raw;
  if (raw) console.warn('focus-orb:unknown-launch-mode', { value: raw, falling_back_to: 'focus' });
  return 'focus';
}
