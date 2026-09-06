#!/usr/bin/env node
/**
 * Lists the Fish Audio voice models this account can actually use, so a real `reference_id` can be
 * pinned in `.env` instead of guessing one.
 *
 * Exists because a hardcoded reference_id in `backend/voice-provider-sidecar/src/tts/fish.ts` went
 * stale and started returning a real 400 "Reference not found" — the account's voice list is the
 * only source of truth for which ids exist, and it can only be read from the live API.
 *
 * Reads FISH_API_KEY from the environment (never printed). Run: `npm run fish:voices`.
 */

const API_KEY = process.env.FISH_API_KEY;

if (!API_KEY || !API_KEY.trim()) {
  console.error('FISH_API_KEY is not set. Run `npm run setup` first, or export it for this shell.');
  process.exit(1);
}

/**
 * `self=true` lists voices owned by this account (cloned/uploaded); without it the endpoint returns
 * Fish's public library. Both are valid `reference_id` sources, so this queries each in turn and
 * reports them separately — a public-library id works even with no custom voice on the account.
 */
async function listModels(params) {
  const url = new URL('https://api.fish.audio/model');
  for (const [key, value] of Object.entries(params)) url.searchParams.set(key, String(value));
  const response = await fetch(url, { headers: { Authorization: `Bearer ${API_KEY}` } });
  if (!response.ok) {
    throw new Error(`fish list models failed: ${response.status} ${await response.text()}`);
  }
  return response.json();
}

function printItems(label, payload) {
  const items = Array.isArray(payload?.items) ? payload.items : [];
  console.log(`\n== ${label} (${items.length}) ==`);
  if (items.length === 0) {
    console.log('  (none)');
    return [];
  }
  for (const item of items) {
    // `_id` is the value the TTS endpoint wants as `reference_id`.
    console.log(`  ${item._id}  ${item.title ?? '(untitled)'}${item.languages ? `  [${item.languages}]` : ''}`);
  }
  return items;
}

const own = await listModels({ self: true, page_size: 20 });
const ownItems = printItems('Your own voices', own);

const publicPayload = await listModels({ page_size: 10, sort_by: 'task_count' });
const publicItems = printItems('Public library (most used)', publicPayload);

const suggestion = ownItems[0] ?? publicItems[0];
if (suggestion) {
  console.log(
    `\nTo pin this voice, add to .env:\n  FISH_REFERENCE_ID_ORB_WARM_V1=${suggestion._id}\n  (${suggestion.title ?? 'untitled'})`,
  );
} else {
  console.log('\nNo voices returned at all — the API key may lack access, or the account has no voices.');
}
