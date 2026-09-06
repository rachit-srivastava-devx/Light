import { describe, expect, it } from 'vitest';
import { statusEcho, echoSpeech } from './StatusEcho';

const SCORE = { checks_total: 14, checks_passed: 9, ratio: 0.643, failed_check_ids: ['R17-DERIV'] };

describe('status echo (P0)', () => {
  it('U5-T1 a refusal echoes the real check ids, not a generic apology', () => {
    const b = statusEcho({ outcome: 'NOT_READY', checked: 14, score: SCORE,
                           reasons: [{ check_id: 'R17-DERIV', detail: 'guarantee 0: calc empty' }] }, 'n1');
    expect(b.kind).toBe('gate_refused');
    expect(b.reasons.map((r: any) => r.check_id)).toContain('R17-DERIV');
    expect(b.score.ratio).toBe(0.643);
  });

  it('U5-T2 a frozen echo declares that nothing consumes the artifact', () => {
    const b = statusEcho({ outcome: 'READY', checked: 14,
                           score: { ...SCORE, checks_passed: 14, ratio: 1, failed_check_ids: [] } },
                         'n1', { freeze_id: 'fz-0000000000000001', content_hash: 'a'.repeat(64), version: 1 });
    expect(b).toMatchObject({ kind: 'frozen', handoff_status: 'artifact_written_no_consumer' });
  });

  it('U5-T3 no echo can claim downstream work that does not exist', () => {
    const forbidden = /\b(PR|pull request|merged|merging|built|building|deployed|shipped|attested)\b/i;
    const blocks = [
      statusEcho({ outcome: 'NOT_READY', checked: 14, score: SCORE, reasons: [] }, 'n1'),
      statusEcho({ outcome: 'READY', checked: 14, score: { ...SCORE, checks_passed: 14, ratio: 1,
                   failed_check_ids: [] } }, 'n1',
                 { freeze_id: 'fz-0000000000000001', content_hash: 'a'.repeat(64), version: 1 }),
    ];
    for (const b of blocks) expect(echoSpeech(b)).not.toMatch(forbidden);
  });
});
