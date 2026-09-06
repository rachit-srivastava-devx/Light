import type { ResponseSource } from '../lld/ResponseEnvelope';
import { noopDevLogger, truncate, type DevLogger } from '../shared/DevLogger';
import {
  EST_MIN_MAX,
  EST_MIN_MIN,
  STEP_TEXT_MAX_CHARS,
  STEPS_TOTAL_MAX,
  STEPS_TOTAL_MIN,
  type AtomizerOutput,
  type AtomizerStep,
} from '../shared/atomizer-schema';
import type { T0FocusSessionInput } from './T0FocusSession';

export interface AtomizerPortInput extends T0FocusSessionInput {
  readonly task: string;
}

export interface AtomizerPortResult {
  readonly output: AtomizerOutput;
  readonly envelope_source: ResponseSource;
  readonly atomizer_source: string;
  readonly spent_paise: number;
  readonly latency_ms: number;
  /** Spoken honestly when the remote planner failed and the local step is only a degrade path. */
  readonly failure_message?: string;
}

export interface AtomizerPort {
  readonly atomize: (input: AtomizerPortInput) => Promise<AtomizerPortResult>;
}

export const T0_STARTER_STEP: AtomizerStep = {
  step_text: 'Open the thing you need for this and look at it for one minute.',
  est_min: 1,
  done_signal: 'the thing is open in front of you',
};

export function outputFromStep(step: AtomizerStep): AtomizerOutput {
  return { steps: [step], steps_total: 1 };
}

export function createStaticAtomizerPort(step: AtomizerStep = T0_STARTER_STEP): AtomizerPort {
  return {
    async atomize() {
      return {
        output: outputFromStep(step),
        envelope_source: 'cache_hit',
        atomizer_source: 'local_t0_starter',
        spent_paise: 0,
        latency_ms: 0,
      };
    },
  };
}

export interface RelayAtomizerPortOptions {
  readonly baseUrl: string;
  readonly fetchImpl?: typeof fetch;
  readonly fallback?: AtomizerPort;
  readonly devLogger?: DevLogger;
}

export function createRelayAtomizerPort({
  baseUrl,
  fetchImpl = fetch,
  fallback = createStaticAtomizerPort(),
  devLogger = noopDevLogger,
}: RelayAtomizerPortOptions): AtomizerPort {
  const normalizedBaseUrl = baseUrl.replace(/\/+$/, '');

  return {
    async atomize(input) {
      const startedAt = Date.now();
      devLogger.log('atomizer_port.request', {
        session_id: input.session_id,
        tenant_id: input.tenant_id,
        task: truncate(input.task),
      });
      try {
        const response = await fetchImpl(`${normalizedBaseUrl}/v1/atomize`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({
            session_id: input.session_id,
            tenant_id: input.tenant_id,
            user_id: input.user_id,
            task: input.task,
          }),
        });

        if (!response.ok) {
          devLogger.log(
            'atomizer_port.http_error',
            { session_id: input.session_id, status: response.status },
            response.status === 402 ? 'error' : 'warn',
          );
          if (response.status === 402) {
            throw new Error('Atomizer budget exhausted');
          }
          const degraded = await fallback.atomize(input);
          return {
            ...degraded,
            failure_message:
              response.status >= 500
                ? `I couldn't reach the task planner. I'll use one tiny fallback: ${degraded.output.steps[0]?.step_text ?? 'open the thing you need.'}`
                : `The task planner rejected that request. I'll use one tiny fallback: ${degraded.output.steps[0]?.step_text ?? 'open the thing you need.'}`,
          };
        }

        const body = asRecord(await response.json(), 'atomize response');
        const output = parseAtomizerOutput(body);
        const source = readString(body, 'source');
        devLogger.log(
          'atomizer_port.response',
          {
            session_id: input.session_id,
            source,
            steps_total: output.steps_total,
            spent_paise: readInteger(body, 'spent_paise') ?? 0,
            latency_ms: Date.now() - startedAt,
          },
          source === 'fallback' ? 'warn' : 'info',
        );
        return {
          output,
          envelope_source: mapRelaySource(source),
          atomizer_source: source,
          spent_paise: readInteger(body, 'spent_paise') ?? 0,
          latency_ms: Date.now() - startedAt,
        };
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        if (message === 'Atomizer budget exhausted') {
          devLogger.log(
            'atomizer_port.budget_exhausted',
            { session_id: input.session_id, latency_ms: Date.now() - startedAt },
            'error',
          );
          throw error;
        }
        // A malformed/unreachable atomize response degrading silently to the local fallback is
        // exactly the "where it broke silently" case the dev log exists to surface.
        devLogger.log(
          'atomizer_port.fell_back',
          { session_id: input.session_id, error: message, latency_ms: Date.now() - startedAt },
          'warn',
        );
        const degraded = await fallback.atomize(input);
        return {
          ...degraded,
          failure_message:
            `I couldn't connect to the task planner. I'll use one tiny fallback: ${degraded.output.steps[0]?.step_text ?? 'open the thing you need.'}`,
        };
      }
    },
  };
}

function parseAtomizerOutput(root: Record<string, unknown>): AtomizerOutput {
  const output = asRecord(root.output, 'atomize output');
  const steps = asArray(output.steps, 'output.steps').map(parseStep);
  const stepsTotal = readRequiredInteger(output, 'steps_total');
  if (stepsTotal < STEPS_TOTAL_MIN || stepsTotal > STEPS_TOTAL_MAX) {
    throw new Error(`Atomizer response steps_total ${stepsTotal} outside bounds`);
  }
  if (steps.length !== stepsTotal) {
    throw new Error(`Atomizer response steps.length ${steps.length} != steps_total ${stepsTotal}`);
  }
  return { steps, steps_total: stepsTotal };
}

function parseStep(raw: unknown): AtomizerStep {
  const step = asRecord(raw, 'atomizer step');
  const stepText = readRequiredString(step, 'step_text').trim();
  const doneSignal = readRequiredString(step, 'done_signal').trim();
  const estMin = readRequiredInteger(step, 'est_min');
  if (stepText.length < 1 || stepText.length > STEP_TEXT_MAX_CHARS) {
    throw new Error(`Atomizer step_text length ${stepText.length} outside bounds`);
  }
  if (doneSignal.length < 1) {
    throw new Error('Atomizer done_signal is empty');
  }
  if (estMin < EST_MIN_MIN || estMin > EST_MIN_MAX) {
    throw new Error(`Atomizer est_min ${estMin} outside bounds`);
  }
  return { step_text: stepText, est_min: estMin, done_signal: doneSignal };
}

function mapRelaySource(source: string): ResponseSource {
  if (source === 'reuse' || source === 'cache_hit') return source;
  if (source === 'model' || source === 'model_repaired') return 'model';
  return 'cache_hit';
}

function asRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  throw new Error(`Expected ${label} to be an object`);
}

function asArray(value: unknown, label: string): readonly unknown[] {
  if (Array.isArray(value)) return value;
  throw new Error(`Expected ${label} to be an array`);
}

function readString(record: Record<string, unknown>, key: string): string {
  const value = record[key];
  return typeof value === 'string' ? value : '';
}

function readRequiredString(record: Record<string, unknown>, key: string): string {
  const value = record[key];
  if (typeof value === 'string') return value;
  throw new Error(`Expected ${key} to be a string`);
}

function readInteger(record: Record<string, unknown>, key: string): number | null {
  const value = record[key];
  return typeof value === 'number' && Number.isInteger(value) ? value : null;
}

function readRequiredInteger(record: Record<string, unknown>, key: string): number {
  const value = readInteger(record, key);
  if (value !== null) return value;
  throw new Error(`Expected ${key} to be an integer`);
}
