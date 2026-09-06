/**
 * Use the orb the way a person uses it: a real multi-turn spoken conversation, in ONE session.
 *
 * `synthetic_mic.mjs` proves one turn works. That is not the product. The owner's report was about
 * everything BETWEEN turns:
 *
 *   "it does not have context of what I spoke previously the context or continuing the conversation
 *    end to end... not able to stop the conversation in between, divert or have a conversation.
 *    right now its just one by one messaging not a conversation completely"
 *
 * So this drives a whole conversation on one `session_id` and asserts the properties that make it a
 * conversation rather than a series of unrelated replies:
 *
 *   C1  context carried      — a later turn referring back ("that thing I mentioned") is understood
 *   C2  topic change honoured — "let's talk about something else" is followed, not atomized
 *   C3  no verbatim repeats   — the same sentence twice is the not-being-heard failure
 *   C4  no load handed back   — no "what's the smallest step you could take?"
 *   C5  audible every turn    — a reply with no audio is not a reply
 *   C6  latency from the user's clock, per turn
 *
 * Every turn is REAL: Fish TTS synthesises the utterance, it streams into relay-rs as binary mic
 * frames, Fish STT transcribes it, the model answers, and the answer is spoken back. Nothing is
 * stubbed. Exit 0 only if every check passes; exit 6 lists which failed.
 *
 *   node e2e-human-simulator/conversation_drive.mjs      (whole chain up — scripts/dev.sh)
 */

const VOICE_URL = process.env.ORB_VOICE_URL ?? 'http://127.0.0.1:8083';
const RELAY_WS = process.env.ORB_RELAY_WS_URL ?? 'ws://127.0.0.1:8091';
const RELAY_HTTP = process.env.ORB_RELAY_HTTP_URL ?? 'http://127.0.0.1:8765';
const TENANT = 't0';
const USER = 'conversation-drive-user';
// ONE session for the whole conversation — the point of the exercise. A per-turn session id is
// exactly the bug that loses context on every app reload.
const SESSION = `conversation-drive-${Date.now()}`;

const TURNS = [
  { say: 'I have been thinking about learning to cook properly lately.', why: 'opening: musing, not a task' },
  { say: 'Yeah, my kitchen is a total disaster though.', why: 'follow-on that only makes sense with turn 1' },
  { say: 'Actually, can we talk about something else instead?', why: 'C2 topic change — must be followed' },
  { say: 'What do you think about whether ai actually helps people focus?', why: 'open discussion, needs a real position' },
  { say: 'Wait, stop.', why: 'C2 control — must not become a task step' },
  { say: 'Sorry, what were we saying about cooking earlier?', why: 'C1 callback to turn 1, four turns later' },
];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const stripTags = (t) => t.replace(/\[[^\]]*\]/g, ' ').replace(/\s+/g, ' ').trim();

// Same shape as the production egress predicate. Kept deliberately SIMPLE and independent: if the
// real guard has a hole (nine have been found so far), a copy of the guard would inherit it. This
// asks the blunt question instead — did the reply end by asking the user to produce the step?
const LOAD_BACK = [
  /what(?:'s| is)?\s+the\s+(?:smallest|easiest|first|simplest|biggest)\b/i,
  /what\s+(?:do|would)\s+you\s+(?:think|want|feel)\b[^.?!]*\b(?:should|could|do|start|next|step)\b/i,
  /\b(?:up\s+to\s+you|your\s+call|you\s+tell\s+me)\b/i,
  /\bwhat\s+feels?\s+like\b/i,
  /\bhow\s+would\s+you\s+break\b/i,
];

async function synthesize(text) {
  const res = await fetch(`${VOICE_URL}/v1/tts/synthesize`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ tenant_id: TENANT, session_id: SESSION, text, voice_id: 'orb.warm.v1', emotion: 'warm' }),
  });
  if (!res.ok) throw new Error(`TTS ${res.status}: ${(await res.text()).slice(0, 200)}`);
  const chunks = (await res.json()).audio_chunks ?? [];
  if (chunks.length === 0) throw new Error('TTS returned no audio');
  return Buffer.concat(chunks.map((c) => Buffer.from(c)));
}

/** One spoken turn on a fresh socket — mirrors an app that reconnects per utterance. */
async function speakOneTurn(utterance) {
  const audio = await synthesize(utterance);
  const ws = new WebSocket(RELAY_WS);
  ws.binaryType = 'arraybuffer';
  let transcript = null;
  let audioBack = 0;
  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = () => reject(new Error(`cannot reach ${RELAY_WS}`));
    setTimeout(() => reject(new Error('ws connect timeout')), 8000);
  });
  ws.onmessage = (event) => {
    if (typeof event.data !== 'string') { audioBack += event.data.byteLength ?? 0; return; }
    try {
      const frame = JSON.parse(event.data);
      if (frame.type === 'transcript' && frame.is_final) transcript = frame.text;
    } catch { /* non-JSON control frame */ }
  };
  const startedAt = Date.now();
  ws.send(JSON.stringify({ type: 'start_listening', tenant_id: TENANT, session_id: SESSION }));
  await sleep(200);
  const CHUNK = 1764;
  for (let o = 0; o < audio.length; o += CHUNK) {
    if (ws.readyState !== 1) break;
    ws.send(audio.subarray(o, Math.min(o + CHUNK, audio.length)));
    await sleep(8);
  }
  ws.send(JSON.stringify({ type: 'end_of_turn', tenant_id: TENANT, session_id: SESSION }));

  // Wait for the transcript rather than a fixed sleep, so the latency number means something.
  for (let waited = 0; transcript === null && waited < 15000; waited += 200) await sleep(200);
  const transcribedAt = Date.now();
  // Deliberate empty catch: the socket may already be closed by the relay, and a close() throw must
  // not mask the real finding, which is the `no transcript` result being returned right here.
  if (transcript === null) { try { ws.close(); } catch { /* already closed */ } return { error: 'no transcript' }; }

  const res = await fetch(`${RELAY_HTTP}/v1/respond`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ tenant_id: TENANT, user_id: USER, session_id: SESSION, text: transcript, mode: 'converse' }),
  });
  // Same deliberate empty catch: the HTTP status is the finding being returned; a socket-close error
  // on the way out must not replace it with a less informative failure.
  if (!res.ok) { try { ws.close(); } catch { /* already closed */ } return { error: `respond ${res.status}` }; }
  const body = await res.json();
  const reply = body.text ?? body.reply ?? '';
  const repliedAt = Date.now();
  if (reply && ws.readyState === 1) {
    ws.send(JSON.stringify({ type: 'speak', tenant_id: TENANT, session_id: SESSION, text: reply, voice_id: 'orb.warm.v1', emotion: 'warm' }));
    for (let waited = 0; audioBack === 0 && waited < 12000; waited += 200) await sleep(200);
    await sleep(600); // let a few more frames land so the byte count is not just the first chunk
  }
  try { ws.close(); } catch { /* already closed */ }
  return {
    transcript,
    reply,
    source: body.source,
    degraded: body.degraded,
    degradeReason: body.degrade_reason,
    audioBack,
    sttMs: transcribedAt - startedAt,
    replyMs: repliedAt - transcribedAt,
  };
}

async function main() {
  console.log(`[conversation] session=${SESSION}  (ONE session for all ${TURNS.length} turns)\n`);
  const results = [];
  for (const [index, turn] of TURNS.entries()) {
    const outcome = await speakOneTurn(turn.say);
    results.push({ ...turn, ...outcome, index: index + 1 });
    if (outcome.error) { console.log(`T${index + 1} ERROR ${outcome.error}\n`); continue; }
    console.log(`T${index + 1}  [${turn.why}]`);
    console.log(`   heard : ${outcome.transcript}`);
    console.log(`   orb   : ${outcome.reply}`);
    console.log(`   stt ${outcome.sttMs}ms · reply ${outcome.replyMs}ms · audio ${outcome.audioBack}B`
      + `${outcome.degraded ? ` · DEGRADED ${outcome.degradeReason}` : ''}\n`);
    await sleep(400);
  }

  const ok = results.filter((r) => !r.error && r.reply);
  const failures = [];
  const note = (label, passed, detail) => {
    console.log(`${passed ? 'PASS' : 'FAIL'}  ${label}${detail ? ` — ${detail}` : ''}`);
    if (!passed) failures.push(label);
  };

  console.log('=== CONVERSATION CHECKS ===');
  console.log(`denominator: ${ok.length}/${TURNS.length} turns produced a reply`);
  if (ok.length === 0) { console.log('MEASURED NOTHING — a check over an empty set is a failure.'); return 6; }

  // C1 — the callback turn must mention cooking/kitchen, which only turn 1 and 2 introduced.
  const callback = results.find((r) => r.index === 6);
  const carried = callback?.reply ? /cook|kitchen|food|recipe|meal/i.test(callback.reply) : false;
  note('C1 context carried across 5 turns', carried,
    callback?.reply ? `T6 reply: "${stripTags(callback.reply).slice(0, 90)}"` : 'no T6 reply');

  // C2 — the topic-change turn must not be answered with a task step, and "stop" must not either.
  const divert = results.find((r) => r.index === 3);
  const stop = results.find((r) => r.index === 5);
  const stepish = /\b(?:step|first thing|start with|smallest)\b/i;
  note('C2 topic change not atomized', divert?.reply ? !stepish.test(divert.reply) : false,
    divert?.reply ? `"${stripTags(divert.reply).slice(0, 90)}"` : 'no reply');
  note('C2 "wait, stop" not answered with a task step', stop?.reply ? !stepish.test(stop.reply) : false,
    stop?.reply ? `"${stripTags(stop.reply).slice(0, 90)}"` : 'no reply');

  // C3 — verbatim repeats, compared as spoken words.
  const keys = ok.map((r) => stripTags(r.reply).toLowerCase().replace(/[^\w\s]/g, ''));
  const repeats = keys.length - new Set(keys).size;
  note('C3 no verbatim repeats', repeats === 0, `${repeats} repeat(s) in ${keys.length} replies`);

  // C4 — load direction, on every reply.
  const shifted = ok.filter((r) => LOAD_BACK.some((p) => p.test(stripTags(r.reply))));
  note('C4 no reply hands the thinking back', shifted.length === 0,
    shifted.length ? shifted.map((r) => `T${r.index}: "${stripTags(r.reply).slice(0, 70)}"`).join(' | ') : `${ok.length} replies checked`);

  // C5 — audible.
  const silent = ok.filter((r) => r.audioBack === 0);
  note('C5 every reply came back as audio', silent.length === 0,
    silent.length ? `silent turns: ${silent.map((r) => `T${r.index}`).join(', ')}` : `${ok.length} turns audible`);

  // C6 — latency, reported not asserted: the 250ms budget is a separate audio measurement.
  const lat = ok.map((r) => r.sttMs + r.replyMs).sort((a, b) => a - b);
  console.log(`INFO  end-to-end p50 ${lat[Math.floor(lat.length / 2)]}ms · max ${lat[lat.length - 1]}ms (user's clock, text path)`);
  const degraded = ok.filter((r) => r.degraded);
  if (degraded.length) console.log(`INFO  ${degraded.length} degraded turn(s): ${degraded.map((r) => `T${r.index}=${r.degradeReason}`).join(', ')}`);

  if (ok.length < TURNS.length) failures.push(`${TURNS.length - ok.length} turn(s) produced no reply`);
  console.log(`\n${failures.length === 0 ? 'ALL CHECKS PASSED' : `FAILED: ${failures.join('; ')}`}`);
  return failures.length === 0 ? 0 : 6;
}

main().then((code) => process.exit(code), (error) => { console.error(`ERROR ${error.message}`); process.exit(1); });
