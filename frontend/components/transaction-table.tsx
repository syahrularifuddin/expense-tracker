"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Trash2 } from "lucide-react";
import { MethodBadge, Money, TypeLozenge } from "@/components/bits";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { api } from "@/lib/api";
import { dateTime } from "@/lib/format";
import { useCategories } from "@/lib/hooks";
import type { Transaction } from "@/lib/types";

/** Tabel padat. `editable` = kategori bisa diganti inline dan baris bisa dihapus. */
export function TransactionTable({ items, editable = false }: { items: Transaction[]; editable?: boolean }) {
  const qc = useQueryClient();
  const categories = useCategories();
  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["transactions"] });
    qc.invalidateQueries({ queryKey: ["summary"] });
    qc.invalidateQueries({ queryKey: ["trends"] });
  };
  const setCategory = useMutation({
    mutationFn: ({ id, category_id }: { id: string; category_id: string | null }) =>
      api(`/transactions/${id}`, { method: "PATCH", json: { category_id } }),
    onSuccess: refresh,
  });
  const remove = useMutation({
    mutationFn: (id: string) => api(`/transactions/${id}`, { method: "DELETE" }),
    onSuccess: refresh,
  });

  return (
    <Table>
      <TableHeader>
        <TableRow className="hover:bg-transparent">
          {["Tanggal", "Merchant", "Kategori", "Jenis", "Metode"].map((h) => (
            <TableHead key={h}>{h}</TableHead>
          ))}
          <TableHead className="text-right">Nominal</TableHead>
          {editable && <TableHead className="w-10" />}
        </TableRow>
      </TableHeader>
      <TableBody>
        {items.map((t) => (
          <TableRow key={t.id}>
            <TableCell className="whitespace-nowrap text-muted-foreground">{dateTime(t.occurred_at)}</TableCell>
            <TableCell className="max-w-64 truncate font-medium" title={t.description ?? t.raw_subject ?? undefined}>
              {t.merchant ?? t.description ?? t.raw_subject ?? "—"}
            </TableCell>
            <TableCell>
              {editable ? (
                <select
                  aria-label={`Kategori untuk ${t.merchant ?? "transaksi"}`}
                  className="field h-8 max-w-44 border-transparent bg-transparent hover:border-input"
                  value={t.category_id ?? ""}
                  disabled={setCategory.isPending}
                  onChange={(e) => setCategory.mutate({ id: t.id, category_id: e.target.value || null })}
                >
                  <option value="">Tanpa kategori</option>
                  {categories.data?.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                    </option>
                  ))}
                </select>
              ) : (
                (t.category_name ?? <span className="text-muted-foreground">—</span>)
              )}
            </TableCell>
            <TableCell>
              <TypeLozenge type={t.type} />
            </TableCell>
            <TableCell>
              <MethodBadge provider={t.provider} />
            </TableCell>
            <TableCell className="text-right font-semibold">
              <Money minor={t.amount_minor} type={t.type} currency={t.currency} />
            </TableCell>
            {editable && (
              <TableCell>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label={`Hapus transaksi ${t.merchant ?? ""}`}
                  disabled={remove.isPending}
                  onClick={() => confirm("Hapus transaksi ini?") && remove.mutate(t.id)}
                >
                  <Trash2 />
                </Button>
              </TableCell>
            )}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
