"use client";

import { Bar, BarChart, CartesianGrid, Legend, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { Meter } from "@/components/bits";
import { monthLabel, rupiah, rupiahCompact } from "@/lib/format";
import type { MonthTotal, Summary } from "@/lib/types";

const tooltip = {
  contentStyle: {
    background: "var(--popover)",
    border: "none",
    borderRadius: 14,
    boxShadow: "var(--shadow-pop)",
    color: "var(--popover-foreground)",
    fontSize: 13,
  },
  itemStyle: { color: "var(--popover-foreground)" },
  formatter: (v: unknown) => rupiah(Number(v)),
  cursor: { fill: "var(--muted)", radius: 6 },
};
const axis = { tick: { fill: "var(--muted-foreground)", fontSize: 12 }, tickLine: false, axisLine: false };

/** Satu baris per kategori: label | nominal + persen | meter. */
export function CategoryBars({ data, limit }: { data: Summary["by_category"]; limit?: number }) {
  const total = data.reduce((sum, c) => sum + c.total, 0) || 1;
  return (
    <ul className="grid gap-4">
      {data.slice(0, limit).map((c) => (
        <li key={c.category_id ?? "none"} className="grid grid-cols-[minmax(0,9rem)_1fr] items-center gap-x-4 gap-y-2">
          <span className="row-span-2 truncate text-muted-foreground">{c.name}</span>
          <div className="flex items-baseline justify-between gap-3">
            <span className="font-semibold tabular-nums">{rupiah(c.total)}</span>
            <span className="text-sm text-muted-foreground tabular-nums">{Math.round((c.total / total) * 100)}%</span>
          </div>
          <Meter
            label={`Porsi ${c.name}`}
            value={c.total / total}
            color={c.category_id ? (c.color ?? "var(--chart-2)") : "var(--chart-4)"}
          />
        </li>
      ))}
    </ul>
  );
}

/** Pengeluaran per hari; hari tanpa transaksi tampil sebagai nol. */
export function DailyBars({ month, data }: { month: string; data: Summary["by_day"] }) {
  const [y, m] = month.split("-").map(Number);
  const byDate = new Map(data.map((d) => [d.date, d.expense]));
  const days = Array.from({ length: new Date(y, m, 0).getDate() }, (_, i) => {
    const day = String(i + 1).padStart(2, "0");
    return { day: i + 1, expense: byDate.get(`${month}-${day}`) ?? 0 };
  });
  return (
    <div className="h-60">
      <ResponsiveContainer>
        <BarChart data={days} margin={{ left: 0, right: 4, top: 8 }}>
          <CartesianGrid vertical={false} stroke="var(--border)" />
          <XAxis dataKey="day" {...axis} interval="preserveStartEnd" />
          <YAxis {...axis} tickFormatter={rupiahCompact} width={48} />
          <Tooltip {...tooltip} labelFormatter={(d) => `Tanggal ${d}`} />
          <Bar isAnimationActive={false} dataKey="expense" name="Pengeluaran" fill="var(--chart-1)" radius={[6, 6, 6, 6]} maxBarSize={14} />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}

export function TrendBars({ data }: { data: MonthTotal[] }) {
  return (
    <div className="h-72">
      <ResponsiveContainer>
        <BarChart data={data} margin={{ left: 0, right: 4, top: 8 }} barGap={4}>
          <CartesianGrid vertical={false} stroke="var(--border)" />
          <XAxis dataKey="month" {...axis} tickFormatter={(m) => monthLabel(m, true)} />
          <YAxis {...axis} tickFormatter={rupiahCompact} width={48} />
          <Tooltip {...tooltip} labelFormatter={(m) => monthLabel(String(m))} />
          <Legend iconType="circle" iconSize={8} wrapperStyle={{ fontSize: 13 }} />
          <Bar isAnimationActive={false} dataKey="expense" name="Pengeluaran" fill="var(--chart-1)" radius={[6, 6, 6, 6]} maxBarSize={28} />
          <Bar isAnimationActive={false} dataKey="income" name="Pemasukan" fill="var(--chart-2)" radius={[6, 6, 6, 6]} maxBarSize={28} />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
