"use client";

import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, Inbox, Plus, Search } from "lucide-react";
import { useState } from "react";
import { EmptyState, ErrorNote, PageHeader } from "@/components/bits";
import { TransactionTable } from "@/components/transaction-table";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { api, qs } from "@/lib/api";
import { jakartaInputToIso, nowJakartaInput } from "@/lib/format";
import { useCategories } from "@/lib/hooks";
import type { Page, Transaction } from "@/lib/types";

const PER_PAGE = 20;
const EMPTY = { q: "", from: "", to: "", category_id: "", type: "", source: "" };

export default function TransactionsPage() {
  const [filters, setFilters] = useState(EMPTY);
  const [page, setPage] = useState(1);
  const [adding, setAdding] = useState(false);
  const categories = useCategories();

  const list = useQuery({
    queryKey: ["transactions", filters, page],
    queryFn: () => api<Page<Transaction>>(`/transactions${qs({ ...filters, page, per_page: PER_PAGE })}`),
    placeholderData: keepPreviousData,
  });
  const set = (patch: Partial<typeof EMPTY>) => {
    setFilters((f) => ({ ...f, ...patch }));
    setPage(1);
  };
  const pages = Math.max(1, Math.ceil((list.data?.total ?? 0) / PER_PAGE));
  const filtered = Object.values(filters).some(Boolean);

  return (
    <>
      <PageHeader title="Transaksi">
        <Button onClick={() => setAdding(true)}>
          <Plus aria-hidden />
          Tambah transaksi
        </Button>
      </PageHeader>

      <div className="mb-4 flex flex-wrap items-end gap-2">
        <div className="relative w-full sm:w-60">
          <Search className="pointer-events-none absolute top-2 left-2.5 size-4 text-muted-foreground" aria-hidden />
          <Input
            type="search"
            aria-label="Cari merchant atau deskripsi"
            placeholder="Cari merchant…"
            className="pl-8"
            value={filters.q}
            onChange={(e) => set({ q: e.target.value })}
          />
        </div>
        <input type="date" aria-label="Dari tanggal" className="field" value={filters.from} onChange={(e) => set({ from: e.target.value })} />
        <input type="date" aria-label="Sampai tanggal" className="field" value={filters.to} onChange={(e) => set({ to: e.target.value })} />
        <select aria-label="Kategori" className="field" value={filters.category_id} onChange={(e) => set({ category_id: e.target.value })}>
          <option value="">Semua kategori</option>
          {categories.data?.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
        <select aria-label="Jenis" className="field" value={filters.type} onChange={(e) => set({ type: e.target.value })}>
          <option value="">Semua jenis</option>
          <option value="debit">Keluar</option>
          <option value="credit">Masuk</option>
        </select>
        <select aria-label="Sumber" className="field" value={filters.source} onChange={(e) => set({ source: e.target.value })}>
          <option value="">Semua sumber</option>
          <option value="gmail">Gmail</option>
          <option value="manual">Manual</option>
        </select>
        {filtered && (
          <Button variant="ghost" onClick={() => set(EMPTY)}>
            Reset
          </Button>
        )}
      </div>

      <ErrorNote error={list.error} />
      <Card className="py-0">
        <CardContent className="px-0">
          {!list.data ? (
            <div className="space-y-2 p-4">
              {Array.from({ length: 8 }, (_, i) => (
                <Skeleton key={i} className="h-8" />
              ))}
            </div>
          ) : list.data.items.length ? (
            <TransactionTable items={list.data.items} editable />
          ) : (
            <EmptyState
              icon={Inbox}
              text={filtered ? "Tidak ada transaksi yang cocok dengan filter." : "Belum ada transaksi."}
            >
              {filtered && (
                <Button variant="secondary" onClick={() => set(EMPTY)}>
                  Reset filter
                </Button>
              )}
            </EmptyState>
          )}
        </CardContent>
      </Card>

      <div className="mt-3 flex items-center justify-between text-muted-foreground">
        <span>{list.data ? `${list.data.total} transaksi` : ""}</span>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" aria-label="Halaman sebelumnya" disabled={page <= 1} onClick={() => setPage((p) => p - 1)}>
            <ChevronLeft />
          </Button>
          <span className="px-1 tabular-nums">
            {page} / {pages}
          </span>
          <Button variant="ghost" size="icon" aria-label="Halaman berikutnya" disabled={page >= pages} onClick={() => setPage((p) => p + 1)}>
            <ChevronRight />
          </Button>
        </div>
      </div>

      <Dialog open={adding} onOpenChange={setAdding}>
        <DialogContent>{adding && <AddForm onDone={() => setAdding(false)} />}</DialogContent>
      </Dialog>
    </>
  );
}

function AddForm({ onDone }: { onDone: () => void }) {
  const qc = useQueryClient();
  const categories = useCategories();
  const create = useMutation({
    mutationFn: (form: FormData) =>
      api("/transactions", {
        method: "POST",
        json: {
          occurred_at: jakartaInputToIso(String(form.get("occurred_at"))),
          amount_minor: Math.round(Number(form.get("amount")) * 100),
          type: form.get("type"),
          merchant: String(form.get("merchant")).trim() || null,
          description: String(form.get("description")).trim() || null,
          category_id: form.get("category_id") || null,
        },
      }),
    onSuccess: () => {
      qc.invalidateQueries();
      onDone();
    },
  });

  return (
    <form
      className="grid gap-4"
      onSubmit={(e) => {
        e.preventDefault();
        create.mutate(new FormData(e.currentTarget));
      }}
    >
      <DialogHeader>
        <DialogTitle>Tambah transaksi manual</DialogTitle>
      </DialogHeader>
      <div className="grid gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="tx-amount">Nominal (Rp)</Label>
          <Input id="tx-amount" name="amount" type="number" inputMode="decimal" min="0.01" step="0.01" required autoFocus />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="tx-type">Jenis</Label>
          <select id="tx-type" name="type" className="field" defaultValue="debit">
            <option value="debit">Keluar</option>
            <option value="credit">Masuk</option>
          </select>
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="tx-date">Waktu (WIB)</Label>
          <input id="tx-date" name="occurred_at" type="datetime-local" className="field" defaultValue={nowJakartaInput()} required />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="tx-category">Kategori</Label>
          <select id="tx-category" name="category_id" className="field" defaultValue="">
            <option value="">Tanpa kategori</option>
            {categories.data?.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </div>
        <div className="grid gap-1.5 sm:col-span-2">
          <Label htmlFor="tx-merchant">Merchant</Label>
          <Input id="tx-merchant" name="merchant" maxLength={200} />
        </div>
        <div className="grid gap-1.5 sm:col-span-2">
          <Label htmlFor="tx-desc">Deskripsi</Label>
          <Input id="tx-desc" name="description" maxLength={1000} />
        </div>
      </div>
      <ErrorNote error={create.error} />
      <DialogFooter>
        <Button type="button" variant="ghost" onClick={onDone}>
          Batal
        </Button>
        <Button type="submit" disabled={create.isPending}>
          {create.isPending ? "Menyimpan…" : "Simpan"}
        </Button>
      </DialogFooter>
    </form>
  );
}
