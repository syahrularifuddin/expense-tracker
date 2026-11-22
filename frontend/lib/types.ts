export type Page<T> = { items: T[]; total: number; page: number; per_page: number };

export type Transaction = {
  id: string;
  occurred_at: string;
  /** sen */
  amount_minor: number;
  currency: string;
  type: "debit" | "credit";
  merchant: string | null;
  description: string | null;
  category_id: string | null;
  category_name: string | null;
  category_color: string | null;
  category_locked: boolean;
  source: "gmail" | "manual";
  /** Metode pembayaran: bca | jenius | krom | custom | manual */
  provider: string;
  raw_subject: string | null;
};

export type Category = { id: string; name: string; color: string | null; is_default: boolean };

export type Sender = { sender: string; provider: string; is_global: boolean; enabled: boolean };

export type UnparsedEmail = {
  id: string;
  sender: string;
  subject: string;
  snippet: string;
  received_at: string;
  ignored: boolean;
};

export type Summary = {
  month: string;
  total_expense: number;
  total_income: number;
  by_category: { category_id: string | null; name: string; color: string | null; total: number }[];
  by_day: { date: string; expense: number; income: number }[];
};

/** Estimasi tagihan kartu kredit untuk siklus berjalan. */
export type CardBill = {
  provider: string;
  /** YYYY-MM-DD, awal siklus (inklusif) */
  from: string;
  /** YYYY-MM-DD, tanggal tagihan terbit */
  statement_date: string;
  /** sen, debit − refund, hanya IDR */
  total: number;
  count: number;
  foreign_count: number;
};

export type MonthTotal = { month: string; expense: number; income: number };

export type SyncStatus = {
  status: "idle" | "syncing" | "error" | "needs_reauth" | "disconnected";
  last_synced_at: string | null;
  last_error: string | null;
  mock: boolean;
};
