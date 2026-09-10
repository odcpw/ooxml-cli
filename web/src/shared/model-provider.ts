import { createProvider } from '@earendil-works/pi-ai';
import { openAIResponsesApi } from '@earendil-works/pi-ai/api/openai-responses.lazy';
import { openaiProvider } from '@earendil-works/pi-ai/providers/openai';
import { setProvider } from '@flue/runtime';

// Flue 2.0.3's bundled catalog predates Astra. Register it through the public
// provider API so reasoning and context compaction retain accurate metadata.
// Source: https://developers.openai.com/api/docs/models/gpt-6-astra
const builtin = openaiProvider();
const models = builtin.getModels();
setProvider(createProvider({
  id: 'openai',
  auth: builtin.auth,
  api: openAIResponsesApi(),
  models: [
    ...models.filter(model => model.id !== 'gpt-6-astra'),
    {
      id: 'gpt-6-astra',
      name: 'GPT-6 Astra',
      api: 'openai-responses',
      provider: 'openai',
      baseUrl: 'https://api.openai.com/v1',
      reasoning: true,
      input: ['text', 'image'],
      contextWindow: 1050000,
      maxTokens: 128000,
      thinkingLevelMap: {
        off: null, minimal: null, low: 'low', medium: 'medium',
        high: 'high', xhigh: 'xhigh', max: 'max',
      },
      cost: {
        input: 10, output: 50, cacheRead: 1, cacheWrite: 12.5,
        tiers: [{ inputTokensAbove: 272000, input: 20, output: 75, cacheRead: 2, cacheWrite: 25 }],
      },
    },
  ],
}));
