import { describe, expect, it } from 'vitest';
import { createStaticAtomizerPort } from '../runtime/AtomizerPort';
import { resolveLaunchMode } from '../runtime/LaunchMode';
import { BUILD_GREETING, createT0FocusSession } from '../runtime/T0FocusSession';

const INPUT = { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' };
const step = () => createStaticAtomizerPort({ step_text: 'Open the tax portal', est_min: 1, done_signal: 'it is open' });

describe('F01 — build-mode launch', () => {
  // --- A1: the mode is real on the real envelope, not just a local variable -------------------
  it('A1 launching in build mode tags the launch envelope mode "build"', async () => {
    const runtime = createT0FocusSession(INPUT, step(), 0, undefined, { sessionMode: 'build' });
    expect((await runtime.start()).mode).toBe('build');
  });

  // --- A2: distinguishable, and distinguishable from the greeting THIS launch would otherwise
  //         have produced (a fixed-string compare could pass by luck; this cannot) -------------
  it('A2 the build opening differs from the focus opening at every greeting index', async () => {
    for (const index of [0, 1, 2, 3, 4, 17]) {
      const focus = await createT0FocusSession(INPUT, step(), index).start();
      const build = await createT0FocusSession(INPUT, step(), index, undefined, { sessionMode: 'build' }).start();
      expect(build.speech?.text).toBe(BUILD_GREETING);
      expect(build.speech?.text).not.toBe(focus.speech?.text);
      expect(focus.mode).toBe('focus');   // the focus launch is untouched by the build launch
    }
  });

  // --- A2b: the opening is not a fabricated cache hit -----------------------------------------
  it('A2b the build opening resolves to novel audio, not a phrase_id with no recording', async () => {
    const envelope = await createT0FocusSession(INPUT, step(), 0, undefined, { sessionMode: 'build' }).start();
    expect(envelope.speech?.audio).toEqual({ mode: 'novel' });
  });

  // --- A4: the default is preserved — omitting the option changes nothing ---------------------
  it('A4 omitting sessionMode still launches focus with the existing greeting', async () => {
    const envelope = await createT0FocusSession(INPUT, step(), 0).start();
    expect(envelope.mode).toBe('focus');
    expect(envelope.speech?.text).toBe("I'm here. Tell me the task.");
  });

  // --- A5: same FSM, reused. Build mode must NOT introduce a session state or a new cost. ------
  it('A5 build launch enters the same session state as focus and costs nothing extra', async () => {
    const focus = await createT0FocusSession(INPUT, step(), 0).start();
    const build = await createT0FocusSession(INPUT, step(), 0, undefined, { sessionMode: 'build' }).start();
    expect(build.session.state).toBe(focus.session.state);
    expect(build.meta.source).toBe(focus.meta.source);
    expect(build.meta.cost_paise).toBe(0);
  });

  // --- A6: §2.4's killed alternative is asserted, not merely asked for ------------------------
  it('A6 the build opening was not appended to the focus GREETINGS rotation', async () => {
    const rotation = await Promise.all(
      [0, 1, 2, 3, 4, 5, 6, 7].map((i) => createT0FocusSession(INPUT, step(), i).start()),
    );
    for (const envelope of rotation) {
      expect(envelope.speech?.text).not.toBe(BUILD_GREETING);
      expect(envelope.mode).toBe('focus');
    }
    // exactly four distinct focus greetings, i.e. the array's length did not change
    expect(new Set(rotation.map((e) => e.speech?.text)).size).toBe(4);
  });

  // --- A7: unenumerated launch-mode input, explicitly handled ---------------------------------
  it('A7 an unrecognised ORB_LAUNCH_MODE degrades to focus rather than reaching the relay', () => {
    // imported from the leaf module (§3.2b), NOT from App.tsx — App drags in react-native
    expect(resolveLaunchMode(undefined, 'build')).toBe('build');
    expect(resolveLaunchMode(undefined, ' BUILD ')).toBe('build');
    expect(resolveLaunchMode(undefined, 'teach')).toBe('focus');   // turn-level, not launchable
    expect(resolveLaunchMode(undefined, 'wat')).toBe('focus');
    expect(resolveLaunchMode(undefined, undefined)).toBe('focus');
    expect(resolveLaunchMode('build', 'focus')).toBe('build');     // explicit prop wins
  });
});
