import type { JsonValue } from "cursor-byok:plugin";
import type { ModelDefinition, ModelSnapshot, ModelSupport } from "cursor-byok:model";
import { accountData } from "./resources.ts";

export const ANTIGRAVITY_PROD_ENDPOINT = "https://cloudcode-pa.googleapis.com";
export const ANTIGRAVITY_DAILY_ENDPOINT = "https://daily-cloudcode-pa.googleapis.com";
export const ANTIGRAVITY_SANDBOX_ENDPOINT = "https://daily-cloudcode-pa.sandbox.googleapis.com";

export const ANTIGRAVITY_ENDPOINTS = [
  ANTIGRAVITY_DAILY_ENDPOINT,
  ANTIGRAVITY_PROD_ENDPOINT,
  ANTIGRAVITY_SANDBOX_ENDPOINT,
];

const FETCH_AVAILABLE_MODELS_PATH = "/v1internal:fetchAvailableModels";
export const ANTIGRAVITY_USER_AGENT =
  "antigravity/hub/2.12.2 (aidev_client; os_type=darwin; arch=arm64; cl=975423596)";

export const ANTIGRAVITY_CLIENT_HEADERS: Record<string, string> = {};

const ANTIGRAVITY_DENYLIST = new Set(["chat_20706", "chat_23310"]);

/**
 * Fallback catalog, used only when the provider cannot be asked for its live list.
 * It deliberately keeps just the newest generation that currently answers; the live
 * `fetchAvailableModels` response is authoritative so retired models never linger.
 */
export const STATIC_ANTIGRAVITY_MODELS: ModelDefinition[] = [
  // Gemini 3.8 Series
  {
    id: "gemini-3.8-flash-high",
    displayName: "Gemini 3.8 Flash (High)",
    capabilities: { images: true },
    maxOutputTokens: 65536,
    privateData: { reasoningEfforts: ["low", "medium", "high"] },
  },
  {
    id: "gemini-3.8-flash-medium",
    displayName: "Gemini 3.8 Flash (Medium)",
    capabilities: { images: true },
    maxOutputTokens: 65536,
    privateData: { reasoningEfforts: ["low", "medium", "high"] },
  },
  {
    id: "gemini-3.8-flash-low",
    displayName: "Gemini 3.8 Flash (Low)",
    capabilities: { images: true },
    maxOutputTokens: 65536,
    privateData: { reasoningEfforts: ["low", "medium", "high"] },
  },
  {
    id: "gemini-3.8-flash-tiered",
    displayName: "Gemini 3.8 Flash (Tiered)",
    capabilities: { images: true },
    maxOutputTokens: 65536,
    privateData: { reasoningEfforts: ["low", "medium", "high"] },
  },
  {
    id: "gemini-3.1-flash-image",
    displayName: "Gemini 3.1 Flash Image",
    capabilities: { images: true },
    maxOutputTokens: 65536,
    privateData: { reasoningEfforts: [] },
  },

  // Claude Series (via Antigravity)
  {
    id: "claude-opus-5-5-high",
    displayName: "Claude Opus 5.5 (High)",
    capabilities: { images: true },
    maxOutputTokens: 128000,
    privateData: { reasoningEfforts: ["low", "medium", "high"] },
  },
  {
    id: "claude-opus-5-5-medium",
    displayName: "Claude Opus 5.5 (Medium)",
    capabilities: { images: true },
    maxOutputTokens: 128000,
    privateData: { reasoningEfforts: ["low", "medium", "high"] },
  },
  {
    id: "claude-opus-5-5-low",
    displayName: "Claude Opus 5.5 (Low)",
    capabilities: { images: true },
    maxOutputTokens: 128000,
    privateData: { reasoningEfforts: ["low", "medium", "high"] },
  },
  {
    id: "claude-sonnet-5-5-high",
    displayName: "Claude Sonnet 5.5 (High)",
    capabilities: { images: true },
    maxOutputTokens: 128000,
    privateData: { reasoningEfforts: ["low", "medium", "high"] },
  },
  {
    id: "claude-sonnet-5-5-medium",
    displayName: "Claude Sonnet 5.5 (Medium)",
    capabilities: { images: true },
    maxOutputTokens: 128000,
    privateData: { reasoningEfforts: ["low", "medium", "high"] },
  },
  {
    id: "claude-sonnet-5-5-low",
    displayName: "Claude Sonnet 5.5 (Low)",
    capabilities: { images: true },
    maxOutputTokens: 128000,
    privateData: { reasoningEfforts: ["low", "medium", "high"] },
  },
  {
    id: "claude-opus-4-6-thinking",
    displayName: "Claude Opus 4.6 Thinking (Antigravity)",
    capabilities: { images: true },
    maxOutputTokens: 64000,
    privateData: { reasoningEfforts: ["low", "medium", "high"] },
  },
];

function object(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function text(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

/**
 * Families Google keeps listing long after they are superseded. Only the newest
 * generation inside each family is published, so the catalog follows the vendor
 * without a code change: when 3.9 ships, 3.8 disappears on the next sync.
 */
const PUBLISHED_FAMILIES = new Set([
  "gemini-flash",
  "claude-opus",
  "claude-sonnet",
]);

/** Splits `gemini-3.8-flash-high` / `claude-opus-5-5-medium` into family + version. */
function familyOf(id: string): { family: string; version: number } | null {
  const gemini = /^gemini-(\d+(?:\.\d+)?)-(flash(?:-image)?)(?:-(.+))?$/.exec(id);
  if (gemini) {
    return { family: `gemini-${gemini[2]}`, version: Number(gemini[1]) };
  }
  const claude = /^claude-(opus|sonnet)-(\d+)-(\d+)(?:-(.+))?$/.exec(id);
  if (claude) {
    return {
      family: `claude-${claude[1]}`,
      version: Number(`${claude[2]}.${claude[3]}`),
    };
  }
  return null;
}

/**
 * Drops retired generations, lite/pro previews, and anything outside the families
 * this provider actually serves, keeping every variant of the newest generation.
 */
function newestGeneration(models: ModelDefinition[]): ModelDefinition[] {
  const newest = new Map<string, number>();
  for (const model of models) {
    const parsed = familyOf(model.id);
    if (!parsed || !PUBLISHED_FAMILIES.has(parsed.family)) continue;
    const best = newest.get(parsed.family) ?? 0;
    if (parsed.version > best) newest.set(parsed.family, parsed.version);
  }
  return models.filter((model) => {
    const parsed = familyOf(model.id);
    if (!parsed || !PUBLISHED_FAMILIES.has(parsed.family)) return false;
    return parsed.version === newest.get(parsed.family);
  });
}

export function parseAntigravityModels(payload: unknown): ModelDefinition[] {
  const root = object(payload);
  const rawModels = object(root?.models);
  if (!rawModels) return [];

  const models: ModelDefinition[] = [];
  const seen = new Set<string>();

  for (const [modelId, raw] of Object.entries(rawModels)) {
    if (ANTIGRAVITY_DENYLIST.has(modelId)) continue;
    const model = object(raw);
    if (!model) continue;

    const id = modelId.trim();
    if (!id || seen.has(id)) continue;
    seen.add(id);

    const displayName = text(model.displayName) ?? id;
    const supportsThinking = model.supportsThinking === true;
    const reasoningEfforts = supportsThinking ? ["low", "medium", "high"] : [];
    const maxOutputTokens = typeof model.maxOutputTokens === "number" && model.maxOutputTokens > 0
      ? model.maxOutputTokens
      : 65_536;

    models.push({
      id,
      displayName,
      capabilities: {
        images: model.supportsImages === true || id.includes("gemini") || id.includes("claude"),
      },
      maxOutputTokens,
      privateData: { reasoningEfforts },
    });
  }

  // The live response is authoritative: retired models must disappear instead of
  // being resurrected from the static fallback list, and only the newest generation
  // of each published family is exposed.
  return newestGeneration(models);
}

export function reasoningEfforts(model: ModelSnapshot): string[] {
  const data = object(model.privateData);
  const efforts = data?.reasoningEfforts;
  return Array.isArray(efforts) ? efforts.filter((item) => typeof item === "string") : [];
}

export const antigravityModels: ModelSupport = {
  list: async ({ resource }, context): Promise<ModelDefinition[]> => {
    if (!resource) return STATIC_ANTIGRAVITY_MODELS;
    let data;
    try {
      data = accountData(resource);
    } catch {
      return STATIC_ANTIGRAVITY_MODELS;
    }

    const payloads = [
      JSON.stringify({ project: data.projectId || "bamboo-precept-lgxtn" }),
      JSON.stringify({}),
    ];

    for (const endpoint of ANTIGRAVITY_ENDPOINTS) {
      for (const bodyPayload of payloads) {
        try {
          const response = await context.network.fetch(
            `${endpoint}${FETCH_AVAILABLE_MODELS_PATH}`,
            {
              method: "POST",
              headers: {
                authorization: `Bearer ${data.accessToken}`,
                "content-type": "application/json",
                "user-agent": ANTIGRAVITY_USER_AGENT,
                ...ANTIGRAVITY_CLIENT_HEADERS,
              },
              body: bodyPayload,
            },
          );
          if (response.status >= 200 && response.status < 300) {
            const body = JSON.parse(response.body);
            const models = parseAntigravityModels(body);
            if (models.length > 0) return models;
          }
        } catch {
          // Continue
        }
      }
    }

    return STATIC_ANTIGRAVITY_MODELS;
  },
};
