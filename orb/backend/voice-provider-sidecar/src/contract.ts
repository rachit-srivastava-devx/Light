/**
 * The JSON-over-HTTP contract this sidecar implements. This is not an independent design — it is
 * the server side of the contract `backend/relay-rs/src/provider.rs`'s `HttpContractProvider`
 * already speaks and tests (`start_contract_server` in that file's test module is the reference
 * fixture). Do not add fields the Rust client doesn't send/read; do not rename these.
 *
 *   POST /v1/stt/push        { session_id: string, audio_bytes: number[] }
 *                             -> { text: string | null, is_final: boolean }
 *   POST /v1/stt/end         { session_id: string }
 *                             -> { text: string, is_final: true }
 *   POST /v1/tts/synthesize  { text: string, voice_id: string, emotion: string }
 *                             -> `Transfer-Encoding: chunked`, one HTTP chunk per audio chunk,
 *                                each chunk's payload `{ "chunk": number[] }` (see
 *                                `writeTtsChunks` in index.ts). NOT a single JSON body — A7/A9
 *                                fixed "TTS fully buffered before send" by making this endpoint
 *                                stream, so the relay can forward audio before the last chunk has
 *                                even been written.
 */

export interface SttPushRequest {
  session_id: string;
  audio_bytes: number[];
}

export interface SttPushResponse {
  text: string | null;
  is_final: boolean;
}

export interface SttEndRequest {
  session_id: string;
}

export interface SttEndResponse {
  text: string;
  is_final: true;
}

export interface TtsSynthesizeRequest {
  text: string;
  voice_id: string;
  emotion: string;
}

/** One HTTP chunk's JSON payload in the streamed `/v1/tts/synthesize` response. */
export interface TtsChunkFrame {
  chunk: number[];
}

/** Byte-array -> chunked-array-of-arrays, matching what `HttpContractProvider::synthesize` parses. */
export function chunkAudioBytes(bytes: Uint8Array, chunkSize = 3200): number[][] {
  const chunks: number[][] = [];
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    chunks.push(Array.from(bytes.subarray(offset, offset + chunkSize)));
  }
  // A zero-length synthesis is a provider bug, not a valid "silence" response under this contract
  // — the caller (session loop) expects at least one chunk to exist so playback lifecycle fires.
  if (chunks.length === 0) {
    chunks.push([]);
  }
  return chunks;
}
