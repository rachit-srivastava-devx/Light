"""A custom `DeepEvalBaseLLM` over Gemini.

There is no OpenAI key in this environment (only GEMINI_API_KEY and ANTHROPIC_API_KEY — see
`.env`), so DeepEval's `ConversationSimulator(simulator_model=...)` cannot use its OpenAI-string
default. This wraps `google-genai` (the current unified Gemini SDK — NOT the older
`google-generativeai`, which Google is migrating callers away from; verified live on PyPI:
google-genai 2.20.0 vs google-generativeai 0.8.6, 2026-08-28) to satisfy DeepEval's
`DeepEvalBaseLLM` contract.

The exact contract was INTROSPECTED against the installed deepeval==4.2.0, not assumed from
memory (the research brief explicitly warns this shape drifts across versions):
`evals/.venv/lib/python3.12/site-packages/deepeval/models/base_model.py:61-65` —
`DeepEvalBaseLLM.__init__(self, model=None, *args, **kwargs)` stores the name as `self.name`
(NOT `self.model_name`, unlike the sibling `DeepEvalBaseModel` class in the same file) and calls
`self.load_model()` with NO arguments. This subclass keeps its own `self._model_name` rather than
depending on that internal attribute name, so a future rename upstream cannot silently break this
file.
"""

from __future__ import annotations

import os
from typing import TypeVar

from deepeval.models.base_model import DeepEvalBaseLLM
from google import genai
from google.genai import types
from pydantic import BaseModel

DEFAULT_MODEL = "gemini-2.5-flash"
SchemaT = TypeVar("SchemaT", bound=BaseModel)


class GeminiDeepEvalModel(DeepEvalBaseLLM):
    """`self.using_native_model` is left at its base-class default (falsy/unset), so DeepEval's
    internal `generate_schema`/`a_generate_schema` (conversation_simulator.py:1002-1038) always
    takes the "custom model" branch: it calls `self.generate(prompt, schema=schema)` /
    `self.a_generate(prompt, schema=schema)` and expects a PARSED instance of `schema` back
    directly (not a `(result, cost)` tuple — that shape is only for `using_native_model=True`).
    Discovered by running the simulator for real and reading the resulting
    `AttributeError: 'str' object has no attribute 'simulated_input'` traceback, not from docs —
    the schema-aware branch is undocumented in the public DeepEval docs at the time of writing.
    """

    def __init__(self, model: str = DEFAULT_MODEL) -> None:
        api_key = os.environ.get("GEMINI_API_KEY")
        if not api_key:
            raise RuntimeError("GEMINI_API_KEY is required (see .env / scripts/load-env.sh)")
        self._model_name = model
        self._client = genai.Client(api_key=api_key)
        super().__init__(model)

    def load_model(self, *args: object, **kwargs: object) -> GeminiDeepEvalModel:
        return self

    def generate(self, prompt: str, schema: type[SchemaT] | None = None, *args: object, **kwargs: object):
        if schema is not None:
            response = self._client.models.generate_content(
                model=self._model_name,
                contents=prompt,
                config=types.GenerateContentConfig(response_mime_type="application/json", response_schema=schema),
            )
            return response.parsed
        response = self._client.models.generate_content(model=self._model_name, contents=prompt)
        return response.text or ""

    async def a_generate(self, prompt: str, schema: type[SchemaT] | None = None, *args: object, **kwargs: object):
        if schema is not None:
            response = await self._client.aio.models.generate_content(
                model=self._model_name,
                contents=prompt,
                config=types.GenerateContentConfig(response_mime_type="application/json", response_schema=schema),
            )
            return response.parsed
        response = await self._client.aio.models.generate_content(model=self._model_name, contents=prompt)
        return response.text or ""

    def get_model_name(self, *args: object, **kwargs: object) -> str:
        return self._model_name
