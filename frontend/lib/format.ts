const TZ = "Asia/Jakarta";

/** Nominal dalam sen -> "Rp 12.500" / "Rp 12.500,50"; mata uang lain mis. "SGD 1,38". */
export function rupiah(minor: number, currency = "IDR"): string {
  return new Intl.NumberFormat("id-ID", {
    style: "currency",
    currency,
    minimumFractionDigits: minor % 100 ? 2 : 0,
    maximumFractionDigits: 2,
  }).format(minor / 100);
}

/** Untuk sumbu grafik: "1,5 jt". */
export function rupiahCompact(minor: number): string {
  return new Intl.NumberFormat("id-ID", { notation: "compact", maximumFractionDigits: 1 }).format(minor / 100);
}

export function dateTime(iso: string): string {
  return new Intl.DateTimeFormat("id-ID", { timeZone: TZ, dateStyle: "medium", timeStyle: "short" }).format(
    new Date(iso),
  );
}

/** "2026-11-25" -> "25 Nov" */
export function dayMonth(date: string): string {
  return new Intl.DateTimeFormat("id-ID", { timeZone: "UTC", day: "numeric", month: "short" }).format(new Date(date));
}

/** "2026-11-25" -> "2026-11-24" */
export function previousDay(date: string): string {
  return new Date(Date.parse(`${date}T00:00:00Z`) - 86_400_000).toISOString().slice(0, 10);
}

/** Bulan berjalan di Jakarta sebagai "YYYY-MM". */
export function currentMonth(): string {
  return new Intl.DateTimeFormat("en-CA", { timeZone: TZ, year: "numeric", month: "2-digit" }).format(new Date());
}

/** "2026-11" -> "November 2026" */
export function monthLabel(month: string, short = false): string {
  return new Intl.DateTimeFormat("id-ID", { month: short ? "short" : "long", year: short ? "2-digit" : "numeric" }).format(
    new Date(`${month}-15T00:00:00Z`),
  );
}

/** Nilai awal <input type="datetime-local">: jam dinding Jakarta saat ini. */
export function nowJakartaInput(): string {
  return new Date(Date.now() + 7 * 3600_000).toISOString().slice(0, 16);
}

/** Nilai <input type="datetime-local"> dibaca sebagai waktu Jakarta (UTC+7, tanpa DST). */
export function jakartaInputToIso(value: string): string {
  return new Date(`${value}:00+07:00`).toISOString();
}

// Halo palette for the built-in categories; overrides the color stored in the DB.
// ponytail: matched by name, so a custom category named like a default gets its color too.
const CATEGORY_COLORS: Record<string, string> = {
  "Makanan & Minuman": "#A94511",
  Transportasi: "#2A63BD",
  Belanja: "#17794D",
  Tagihan: "#B53844",
  Hiburan: "#6E5AA8",
  Kesehatan: "#3F7F86",
  Transfer: "#57665D",
  "Top Up": "#9AA89F",
  Pemasukan: "#17794D",
  Lainnya: "#9AA89F",
};

export const categoryColor = (name: string, color: string | null) => CATEGORY_COLORS[name] ?? color;
