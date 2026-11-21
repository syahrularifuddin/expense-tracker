"use client";

import { useQuery } from "@tanstack/react-query";
import { Inbox } from "lucide-react";
import Link from "next/link";
import { EmptyState, ErrorNote, Money, PageHeader } from "@/components/bits";
import { CategoryBars, DailyBars } from "@/components/charts";
import { TransactionTable } from "@/components/transaction-table";
import { buttonVariants } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { api } from "@/lib/api";
import { currentMonth, monthLabel } from "@/lib/format";
import type { Page, Summary, Transaction } from "@/lib/types";

function Stat({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <Card size="sm">
      <CardContent>
        <div className="text-muted-foreground">{label}</div>
        <div className="mt-1 text-[28px] leading-9 font-semibold tracking-tight">{children}</div>
      </CardContent>
    </Card>
  );
}

export default function DashboardPage() {
  const month = currentMonth();
  const summary = useQuery({
    queryKey: ["summary", month],
    queryFn: () => api<Summary>(`/reports/summary?month=${month}`),
  });
  const recent = useQuery({
    queryKey: ["transactions", "recent"],
    queryFn: () => api<Page<Transaction>>("/transactions?per_page=10"),
  });
  const s = summary.data;
  const net = s ? s.total_income - s.total_expense : 0;

  return (
    <>
      <PageHeader title="Ringkasan">{monthLabel(month)}</PageHeader>
      <ErrorNote error={summary.error ?? recent.error} />

      <div className="grid gap-4 sm:grid-cols-3 sm:gap-5">
        <Stat label="Pengeluaran">{s ? <Money minor={s.total_expense} /> : <Skeleton className="h-9 w-40" />}</Stat>
        <Stat label="Pemasukan">
          {s ? <Money minor={s.total_income} className="text-[var(--loz-success-fg)]" /> : <Skeleton className="h-9 w-40" />}
        </Stat>
        <Stat label="Selisih">
          {s ? (
            <span className="tabular-nums">
              {net < 0 ? "−" : "+"}
              <Money minor={Math.abs(net)} />
            </span>
          ) : (
            <Skeleton className="h-9 w-40" />
          )}
        </Stat>
      </div>

      <div className="mt-5 grid gap-4 sm:gap-5 lg:grid-cols-2">
        <Card>
          <CardHeader className="flex items-center justify-between">
            <CardTitle>Per kategori</CardTitle>
            <Link href="/reports" className={buttonVariants({ variant: "secondary", size: "sm" })}>
              Laporan
            </Link>
          </CardHeader>
          <CardContent>
            {!s ? (
              <Skeleton className="h-60" />
            ) : s.by_category.length ? (
              <CategoryBars data={s.by_category} limit={6} />
            ) : (
              <EmptyState icon={Inbox} text="Belum ada pengeluaran bulan ini." />
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Pengeluaran harian</CardTitle>
          </CardHeader>
          <CardContent>{s ? <DailyBars month={month} data={s.by_day} /> : <Skeleton className="h-60" />}</CardContent>
        </Card>
      </div>

      <div className="mt-10 mb-4 flex items-center justify-between gap-3">
        <h2 className="text-[22px] leading-8 font-semibold tracking-tight">Transaksi terakhir</h2>
        <Link href="/transactions" className={buttonVariants({ variant: "secondary" })}>
          Lihat semua
        </Link>
      </div>
      <Card className="py-2">
        <CardContent className="px-2 sm:px-3">
          {!recent.data ? (
            <div className="space-y-2 p-2">
              {Array.from({ length: 5 }, (_, i) => (
                <Skeleton key={i} className="h-10" />
              ))}
            </div>
          ) : recent.data.items.length ? (
            <TransactionTable items={recent.data.items} />
          ) : (
            <EmptyState icon={Inbox} text="Belum ada transaksi. Tekan Sync untuk menarik email." />
          )}
        </CardContent>
      </Card>
    </>
  );
}
