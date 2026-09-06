import type { BuiltAudioGraph } from './AudioGraph';

export type AppLifecycleState = 'active' | 'background' | 'inactive' | 'unknown' | 'extension';

export interface AppLifecycleSubscription {
  remove(): void;
}

export interface AppLifecycleSource {
  addEventListener(
    type: 'change',
    listener: (state: AppLifecycleState) => void,
  ): AppLifecycleSubscription;
}

export function bindPresenceToAppLifecycle(
  source: AppLifecycleSource,
  graph: () => BuiltAudioGraph | null,
  onStateChange?: (state: AppLifecycleState) => void,
): AppLifecycleSubscription {
  return source.addEventListener('change', (state) => {
    onStateChange?.(state);
    const current = graph();
    if (!current) return;
    if (state === 'active') {
      // Per blueprints/ADHD-Focus-Orb-L8-Deep-Dive/02-ALWAYS-ON-AUDIO-ENGINE.md §5/§5.1: "App
      // backgrounded / loses focus" is the *user-intent* class (PAUSE), not the *transient*
      // class (phone call, Siri, audio-stack reset) that recovers automatically with a cover.
      // §5.1 lists "Bed stops" as step 1 of the pause contract and states plainly: "Resume is
      // explicit (the user returns and taps)... The session does not silently restart itself
      // when the app comes back to the foreground — the user decides when listening resumes."
      // Do NOT call current.recover() here — that would silently un-pause the bed the instant
      // AppState flips back to 'active', including for a momentary notification-shade
      // dismissal that never even reaches 'background'. The bed is deliberately left paused
      // until a genuine explicit action drives it back (App.tsx's onStart, wired to the user's
      // own tap) or real in-session activity resumes it (the envelope-state effect).
      return;
    }
    current.pause();
  });
}

/**
 * Where a session-envelope update came from. `'user_action'` covers a genuine explicit tap
 * (App.tsx's onStart) and real in-session activity driven by the user's own turn (transcript ->
 * intent -> a new envelope, wired through `AppController.createFocusOrbActions`). `'proactive'`
 * covers the app's own self-triggered nudges — `T0FocusSession.checkIn` (stall_assist) and
 * `.proactivePresence()` (gentle_presence) — which run off a timer with no user action at all.
 */
export type SessionEnvelopeUpdateSource = 'user_action' | 'proactive';

/**
 * Per blueprints/ADHD-Focus-Orb-L8-Deep-Dive/02-ALWAYS-ON-AUDIO-ENGINE.md §5.1: resume from a
 * background-triggered pause is explicit — "the user decides when listening resumes" — not
 * merely delayed. B4 fixed the instant case (AppState -> 'active' must not itself call
 * recover()). This closes the delayed case: `bindPresenceToAppLifecycle` schedules a fresh
 * `proactiveCheckInEvent` the moment the app returns to the foreground (see App.tsx), and if the
 * user doesn't act before that event's 10s delay elapses, `checkIn()` fires on its own, produces
 * a new envelope, and — without this guard — the envelope-state effect in App.tsx would call
 * `recover()` off THAT state change alone. A proactive envelope update is not a user action, so
 * it must never be the thing that un-pauses the bed; only a `'user_action'`-sourced update may.
 */
export function shouldRecoverOnSessionStateChange(
  sessionState: string,
  source: SessionEnvelopeUpdateSource,
): boolean {
  if (sessionState === 'INTERRUPTED') return false;
  return source === 'user_action';
}
