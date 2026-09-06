/**
 * A stalled TLS handshake or a dead connection to an STT/TTS provider would otherwise hold the
 * awaiting request handler — and the client-facing HTTP connection tied to it — open forever.
 * That is the same "the stop path never actually released it" shape as the SIGTERM/recordVideo
 * bug, here for a socket instead of a subprocess. 10s is generous against this app's own p99
 * voice-to-voice target (<=2.0s for the full round trip): this exists to fail a truly dead
 * connection over to an error, not to enforce the latency budget itself.
 */
export const PROVIDER_REQUEST_TIMEOUT_MS = 10_000;
