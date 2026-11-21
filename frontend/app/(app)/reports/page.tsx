"use client";

import { useQuery } from "@tanstack/react-query";
import { Inbox } from "lucide-react";
import { useState } from "react";
import { EmptyState, ErrorNote, Money, PageHeader } from "@/components/bits";
import { CategoryBars, DailyBars, TrendBars } from "@/components/charts";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { api } from "@/lib/api";
import { currentMonth, monthLabel } from "@/lib/format";
import type { MonthTotal, Summary } from "@/lib/types";

export default function ReportsPage() {
  const [month, setMonth] = useState(currentMonth);
  const [months, setMonths] = useState(6);
  const summary = useQuery({
    queryKey: ["summary", month],
    queryFn: () => api<Summary>(`/reports/summary?month=${month}`),
  });
  const trends = useQuery({
    queryKey: ["trends", months],
    queryFn: () => api<MonthTotal[]>(`/reports/trends?months=${months}`),
  });
  const s = summary.data;

  return (
    <>
      <PageHeader title="Laporan">
        <input
          type="month"
          aria-label="Bulan laporan"
          className="field"
          value={month}
          max={currentMonth()}
          onChange={(e) => e.target.value && setMonth(e.target.value)}
        />
      </PageHeader>
      <ErrorNote error={summary.error ?? trends.error} />

      <div className="grid gap-4 sm:grid-cols-2 sm:gap-5">
        <Card size="sm">
          <CardContent>
            <div className="text-muted-foreground">Pengeluaran {monthLabel(month)}</div>
            <div className="mt-1 text-[28px] leading-9 font-semibold tracking-tight">{s ? <Money minor={s.total_expense} /> : <Skeleton className="h-8 w-36" />}</div>
          </CardContent>
        </Card>
        <Card size="sm">
          <CardContent>
            <div className="text-muted-foreground">Pemasukan {monthLabel(month)}</div>
            <div className="mt-1 text-[28px] leading-9 font-semibold tracking-tight">
              {s ? <Money minor={s.total_income} className="text-[var(--loz-success-fg)]" /> : <Skeleton className="h-8 w-36" />}
            </div>
          </CardContent>
        </Card>
      </div>

      <div className="mt-5 grid gap-4 sm:gap-5 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Per kategori</CardTitle>
          </CardHeader>
          <CardContent>
            {!s ? (
              <Skeleton className="h-48" />
            ) : s.by_category.length ? (
              <CategoryBars data={s.by_category} />
            ) : (
              <EmptyState icon={Inbox} text="Tidak ada pengeluaran di bulan ini." />
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Pengeluaran harian</CardTitle>
          </CardHeader>
          <CardContent>{s ? <DailyBars month={month} data={s.by_day} /> : <Skeleton className="h-56" />}</CardContent>
        </Card>
      </div>

      <Card className="mt-5">
        <CardHeader className="flex items-center justify-between">
          <CardTitle>Tren bulanan</CardTitle>
          <select aria-label="Rentang tren" className="field" value={months} onChange={(e) => setMonths(Number(e.target.value))}>
            <option value={3}>3 bulan</option>
            <option value={6}>6 bulan</option>
            <option value={12}>12 bulan</option>
          </select>
        </CardHeader>
        <CardContent>{trends.data ? <TrendBars data={trends.data} /> : <Skeleton className="h-72" />}</CardContent>
      </Card>
    </>
  );
}
