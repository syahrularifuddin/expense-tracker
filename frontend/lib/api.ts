import { getSession, signOut } from "next-auth/react";

const API = process.env.NEXT_PUBLIC_API_URL ?? "http://127.0.0.1:8080";

type Options = { method?: "GET" | "POST" | "PATCH" | "DELETE"; json?: unknown };

/** Panggil backend dengan access JWT dari sesi. Error = `detail` dari problem JSON. */
export async function api<T = void>(path: string, { method = "GET", json }: Options = {}): Promise<T> {
  // ponytail: getSession() per panggilan = satu round-trip ekstra, tapi token selalu segar
  // (Auth.js me-refresh di server). Ganti dengan cache di memori jika terasa lambat.
  const session = await getSession();
  if (!session?.accessToken || session.error) {
    await signOut({ redirectTo: "/login" });
    throw new Error("Sesi berakhir");
  }
  const res = await fetch(`${API}/api/v1${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${session.accessToken}`,
      ...(json !== undefined && { "content-type": "application/json" }),
    },
    body: json !== undefined ? JSON.stringify(json) : undefined,
  });
  if (res.status === 401) {
    await signOut({ redirectTo: "/login" });
    throw new Error("Sesi berakhir");
  }
  const text = await res.text();
  const body = text ? JSON.parse(text) : undefined;
  if (!res.ok) throw new Error(body?.detail ?? `HTTP ${res.status}`);
  return body as T;
}

/** Buang nilai kosong lalu susun query string. */
export function qs(params: Record<string, string | number | undefined | null>): string {
  const entries = Object.entries(params).filter(([, v]) => v !== undefined && v !== null && v !== "");
  return entries.length ? `?${new URLSearchParams(entries.map(([k, v]) => [k, String(v)]))}` : "";
}
