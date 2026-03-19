export interface UsageData {
  models: ModelUsage[];
  reset_at: string;
}

export interface ModelUsage {
  model_name: string;
  utilization: number;
  messages_used?: number;
  messages_limit?: number;
  tokens_used?: number;
  cost?: number;
}

export interface HistoryEntry {
  timestamp: string;
  data: UsageData;
}

export type AppState = "loading" | "login" | "connected" | "error";
