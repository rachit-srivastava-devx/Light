import type { ResponseSource } from '../lld/ResponseEnvelope';
import type { OrbMode } from '../router/contracts';
import { noopDevLogger, truncate, type DevLogger } from '../shared/DevLogger';
import type { T0FocusSessionInput } from './T0FocusSession';

export interface ConversationPortInput extends T0FocusSessionInput {
  readonly text: string;
  readonly active_task?: string;
  readonly current_step?: string;
  readonly session_state?: string;
  /**
   * Which brain the relay should answer with: `focus-companion.v1` / `converse.v1` / `teach.v1`,
   * and the token ceiling that goes with it (teach gets 480, the others 180).
   *
   * This MUST be transmitted. The routing layer already computes it and, until 2026-08-28, passed
   * it only to the local response envelope — so the field never reached the wire, `app.py` applied
   * its compatibility default (`ResponseMode.FOCUS`), and **teach and converse modes were
   * unreachable from the real app** even though the prompts, the beat-chunking and the 480-token
   * ceiling were all built and tested. Backend contract: `proxy/schemas.py`'s `ResponseMode`
   * ("converse" | "focus" | "teach"); an unknown value is a typed 422, only omission defaults.
   */
  readonly mode?: OrbMode;
}

export interface ConversationPortResult {
  readonly text: string;
  readonly source: ResponseSource;
  readonly latency_ms: number;
  readonly spent_paise: number;
  /** Present when the response is a local degrade path, so the user hears the real failure. */
  readonly failure_message?: string;
}

export interface ConversationPort {
  readonly respond: (input: ConversationPortInput) => Promise<ConversationPortResult>;
}

export function createStaticConversationPort(): ConversationPort {
  return {
    async respond() {
      return {
        text: "I couldn't reach the conversation service. Please say that again in a moment.",
        source: 'cache_hit',
        latency_ms: 0,
        spent_paise: 0,
        failure_message: 'conversation_backend_unavailable',
      };
    },
  };
}

export interface RelayConversationPortOptions {
  readonly baseUrl: string;
  readonly fetchImpl?: typeof fetch;
  readonly fallback?: ConversationPort;
  readonly devLogger?: DevLogger;
}

/** The mobile client talks to relay-py; relay-py is the only process allowed to call the gateway. */
export function createRelayConversationPort({
  baseUrl,
  fetchImpl = fetch,
  fallback = createStaticConversationPort(),
  devLogger = noopDevLogger,
}: RelayConversationPortOptions): ConversationPort {
  const normalizedBaseUrl = baseUrl.replace(/\/+$/, '');

  return {
    async respond(input) {
      const startedAt = Date.now();
      devLogger.log('conversation_port.request', {
        session_id: input.session_id,
        tenant_id: input.tenant_id,
        text: truncate(input.text),
        // Logged so an absent mode is visible rather than silently becoming FOCUS on the relay.
        mode: input.mode ?? '(absent — relay will default to focus)',
      });
      try {
        const response = await fetchImpl(`${normalizedBaseUrl}/v1/respond`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({
            session_id: input.session_id,
            tenant_id: input.tenant_id,
            user_id: input.user_id,
            text: input.text,
            active_task: input.active_task,
            current_step: input.current_step,
            session_state: input.session_state,
            // Omitted (not sent as undefined) when the caller has no mode, so the relay's
            // documented compatibility default applies rather than a 422 on a null.
            ...(input.mode ? { mode: input.mode } : {}),
          }),
        });
        if (!response.ok) {
          devLogger.log('conversation_port.http_error', {
            session_id: input.session_id,
            status: response.status,
            latency_ms: Date.now() - startedAt,
          }, 'error');
          return fallback.respond(input);
        }
        const body = await response.json() as Record<string, unknown>;
        const text = typeof body.text === 'string' ? body.text.trim() : '';
        if (!text) throw new Error('conversation response had no text');
        const result: ConversationPortResult = {
          text,
          source: body.source === 'model' ? 'model' : 'reuse',
          latency_ms: typeof body.latency_ms === 'number' ? body.latency_ms : Date.now() - startedAt,
          spent_paise: typeof body.spent_paise === 'number' ? body.spent_paise : 0,
        };
        devLogger.log('conversation_port.response', {
          session_id: input.session_id,
          source: result.source,
          response_text: truncate(result.text),
          latency_ms: result.latency_ms,
        });
        return result;
      } catch (error) {
        devLogger.log('conversation_port.fell_back', {
          session_id: input.session_id,
          error: error instanceof Error ? error.message : String(error),
          latency_ms: Date.now() - startedAt,
        }, 'error');
        return fallback.respond(input);
      }
    },
  };
}
