export type PresenceEventKind =
  | 'launch_greeting'
  | 'context_ready'
  | 'gentle_presence'
  | 'stall_assist'
  | 'recovery_assist'
  | 'completion_follow_up'
  | 'abandoned_goal_rescue';

export type PresenceRequirement =
  | 'app_active'
  | 'not_user_turn'
  | 'not_muted'
  | 'goal_active'
  | 'not_hyperfocus';

export type PresenceCancellation = 'user_speaks' | 'pause' | 'background' | 'task_done' | 'error';

export interface PresenceEvent {
  readonly id: string;
  readonly kind: PresenceEventKind;
  readonly dueAtMs: number;
  readonly expiresAtMs: number;
  readonly priority: number;
  readonly requires: readonly PresenceRequirement[];
  readonly cancelOn: readonly PresenceCancellation[];
  readonly payload?: readonly [string, string][];
}

export interface PresenceSnapshot {
  readonly appActive: boolean;
  readonly userTurn: boolean;
  readonly muted: boolean;
  readonly goalActive: boolean;
  readonly hyperfocus: boolean;
  readonly cancellationSignals?: readonly PresenceCancellation[];
}

export const PRESENCE_GUARD_RETRY_MS = 1_000;
export const ACTIVE_PRESENCE_INITIAL_DELAY_MS = 5_000;
export const ACTIVE_PRESENCE_REPEAT_DELAY_MS = 45_000;

function requirementSatisfied(requirement: PresenceRequirement, snapshot: PresenceSnapshot): boolean {
  if (requirement === 'app_active') return snapshot.appActive;
  if (requirement === 'not_user_turn') return !snapshot.userTurn;
  if (requirement === 'not_muted') return !snapshot.muted;
  if (requirement === 'goal_active') return snapshot.goalActive;
  return !snapshot.hyperfocus;
}

export class PresenceEventQueue {
  readonly #events = new Map<string, PresenceEvent>();

  schedule(event: PresenceEvent): void {
    if (!event.id || !Number.isFinite(event.dueAtMs) || !Number.isFinite(event.expiresAtMs)) {
      throw new Error('presence event requires finite id, dueAtMs, and expiresAtMs');
    }
    if (event.expiresAtMs < event.dueAtMs) {
      throw new Error('presence event expiresAtMs must be >= dueAtMs');
    }
    this.#events.set(event.id, { ...event });
  }

  cancel(id: string): boolean {
    return this.#events.delete(id);
  }

  cancelBy(signal: PresenceCancellation): number {
    let cancelled = 0;
    for (const [id, event] of this.#events) {
      if (!event.cancelOn.includes(signal)) continue;
      this.#events.delete(id);
      cancelled += 1;
    }
    return cancelled;
  }

  cancelAll(): void {
    this.#events.clear();
  }

  pending(): readonly PresenceEvent[] {
    return [...this.#events.values()].sort(compareEvents);
  }

  drain(nowMs: number, snapshot: PresenceSnapshot): readonly PresenceEvent[] {
    const due = [...this.#events.values()]
      .filter((event) => event.dueAtMs <= nowMs)
      .sort(compareEvents);
    const emitted: PresenceEvent[] = [];
    const signals = snapshot.cancellationSignals ?? [];
    for (const event of due) {
      if (event.expiresAtMs < nowMs || event.cancelOn.some((signal) => signals.includes(signal))) {
        this.#events.delete(event.id);
        continue;
      }
      if (!event.requires.every((requirement) => requirementSatisfied(requirement, snapshot))) {
        this.#events.set(event.id, {
          ...event,
          dueAtMs: nowMs + PRESENCE_GUARD_RETRY_MS,
        });
        continue;
      }
      this.#events.delete(event.id);
      emitted.push(event);
    }
    return emitted;
  }
}

function compareEvents(left: PresenceEvent, right: PresenceEvent): number {
  return left.dueAtMs - right.dueAtMs || right.priority - left.priority || left.id.localeCompare(right.id);
}

export function proactiveCheckInEvent(nowMs: number): PresenceEvent {
  return {
    id: 'proactive-check-in',
    kind: 'stall_assist',
    dueAtMs: nowMs + 10_000,
    expiresAtMs: nowMs + 86_400_000,
    priority: 20,
    requires: ['app_active', 'not_user_turn', 'not_muted', 'goal_active', 'not_hyperfocus'],
    cancelOn: ['pause', 'background', 'task_done', 'error'],
  };
}

/**
 * Human-presence cadence for the launch/intake states. This is intentionally separate from the
 * cognitive check-in policy: the policy must be allowed to stay silent while the user is focused,
 * but a newly opened orb needs one bounded sign of life before the user has a task.
 */
export function activePresenceEvent(
  nowMs: number,
  delayMs = ACTIVE_PRESENCE_INITIAL_DELAY_MS,
): PresenceEvent {
  return {
    id: 'active-presence',
    kind: 'gentle_presence',
    dueAtMs: nowMs + delayMs,
    expiresAtMs: nowMs + 86_400_000,
    priority: 25,
    requires: ['app_active', 'not_user_turn', 'not_muted', 'not_hyperfocus'],
    cancelOn: ['user_speaks', 'pause', 'background', 'task_done', 'error'],
  };
}
