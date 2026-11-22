import { cn } from "cn";
import { Mail, Wallet, type LucideIcon } from "lucide-react";
import { rupiah } from "@/lib/format";
import type { SyncStatus, Transaction } from "@/lib/types";

export function PageHeader({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="mb-5 flex flex-wrap items-center justify-between gap-3">
      <h1 className="text-[28px] leading-9 font-semibold tracking-tight">{title}</h1>
      <div className="flex items-center gap-2 text-muted-foreground">{children}</div>
    </div>
  );
}

type Tone = "neutral" | "success" | "danger" | "info" | "warning";

export function Lozenge({ tone = "neutral", dot = false, children }: { tone?: Tone; dot?: boolean; children: React.ReactNode }) {
  return (
    <span className="lozenge" data-tone={tone} data-dot={dot || undefined}>
      {children}
    </span>
  );
}

export function TypeLozenge({ type }: { type: Transaction["type"] }) {
  return (
    <Lozenge dot tone={type === "credit" ? "success" : "neutral"}>
      {type === "credit" ? "Masuk" : "Keluar"}
    </Lozenge>
  );
}

const SYNC: Record<SyncStatus["status"], [Tone, string]> = {
  idle: ["success", "Tersinkron"],
  syncing: ["warning", "Menyinkronkan"],
  error: ["danger", "Gagal"],
  needs_reauth: ["warning", "Perlu login ulang"],
  disconnected: ["neutral", "Terputus"],
};

export function SyncLozenge({ status }: { status: SyncStatus["status"] }) {
  const [tone, label] = SYNC[status];
  return (
    <Lozenge dot tone={tone}>
      {label}
    </Lozenge>
  );
}

/** `logo` = ikon resmi di public/logos (dari situs bank); tanpa logo memakai `icon`. */
type Method = { label: string; logo?: string; icon?: LucideIcon; tone?: "neutral" | "info" };

const METHODS: Record<string, Method> = {
  bca: { label: "BCA", logo: "/logos/bca.png" },
  jenius: { label: "Jenius", logo: "/logos/jenius.png" },
  krom: { label: "Krom", logo: "/logos/krom.png" },
  manual: { label: "Manual", icon: Wallet, tone: "neutral" },
  custom: { label: "Lainnya", icon: Mail, tone: "info" },
};

/** Logo kecil + nama metode pembayaran. Provider tak dikenal tampil sebagai "Lainnya". */
export function MethodBadge({ provider }: { provider: string }) {
  const m = METHODS[provider] ?? METHODS.custom;
  return (
    <span className="inline-flex items-center gap-2 whitespace-nowrap">
      {m.logo ? (
        // eslint-disable-next-line @next/next/no-img-element -- ikon statis 24px, tidak perlu optimasi
        <img src={m.logo} alt="" width={24} height={24} className="size-6 shrink-0 rounded-md" />
      ) : (
        <span
          aria-hidden
          className="grid size-6 shrink-0 place-items-center rounded-md"
          style={{ background: `var(--loz-${m.tone}-bg)`, color: `var(--loz-${m.tone}-fg)` }}
        >
          {m.icon && <m.icon className="size-3.5" />}
        </span>
      )}
      {m.label}
    </span>
  );
}

/** Chip pilih-satu metode pembayaran. `value` "" = semua. */
export function MethodFilter({ value, onChange }: { value: string; onChange: (provider: string) => void }) {
  const chip = (id: string, content: React.ReactNode) => (
    <button
      key={id}
      type="button"
      aria-pressed={value === id}
      onClick={() => onChange(id)}
      className={cn(
        "inline-flex h-9 items-center rounded-full border px-3 font-medium transition-colors",
        value === id
          ? "border-brand bg-[var(--loz-info-bg)] text-foreground"
          : "border-input text-muted-foreground hover:bg-muted hover:text-foreground",
      )}
    >
      {content}
    </button>
  );
  return (
    <div role="group" aria-label="Metode pembayaran" className="flex flex-wrap gap-2">
      {chip("", "Semua metode")}
      {Object.keys(METHODS).map((id) => chip(id, <MethodBadge provider={id} />))}
    </div>
  );
}

/** Tanda +/− menyertai warna agar debit/credit tidak hanya dibedakan warna. */
export function Money({
  minor,
  type,
  currency,
  className,
}: {
  minor: number;
  type?: Transaction["type"];
  currency?: string;
  className?: string;
}) {
  return (
    <span className={cn("tabular-nums", type === "credit" && "text-[var(--loz-success-fg)]", className)}>
      {type === "credit" ? "+" : type === "debit" ? "−" : ""}
      {rupiah(minor, currency)}
    </span>
  );
}

/** Bar tipis membulat seperti meter di kartu. `value` 0..1. */
export function Meter({ value, color, label }: { value: number; color?: string; label: string }) {
  const pct = Math.max(0, Math.min(1, value)) * 100;
  return (
    <div
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(pct)}
      className="h-1.5 w-full overflow-hidden rounded-full bg-track"
    >
      <div className="h-full rounded-full" style={{ width: `${Math.max(pct, 1.5)}%`, background: color ?? "var(--brand)" }} />
    </div>
  );
}

export function EmptyState({ icon: Icon, text, children }: { icon: LucideIcon; text: string; children?: React.ReactNode }) {
  return (
    <div className="flex flex-col items-center gap-3 px-4 py-10 text-center text-muted-foreground">
      <Icon className="size-8" strokeWidth={1.5} aria-hidden />
      <p>{text}</p>
      {children}
    </div>
  );
}

export function ErrorNote({ error }: { error: unknown }) {
  if (!error) return null;
  return (
    <p role="alert" className="text-sm text-destructive">
      {error instanceof Error ? error.message : "Terjadi kesalahan"}
    </p>
  );
}
