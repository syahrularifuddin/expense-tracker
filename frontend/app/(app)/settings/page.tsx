"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { MailWarning, RefreshCw, Trash2 } from "lucide-react";
import { signOut } from "next-auth/react";
import { ErrorNote, EmptyState, Lozenge, PageHeader, SyncLozenge } from "@/components/bits";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { api } from "@/lib/api";
import { dateTime } from "@/lib/format";
import { useCategories, useSync } from "@/lib/hooks";
import type { Page, Sender, UnparsedEmail } from "@/lib/types";

export default function SettingsPage() {
  return (
    <>
      <PageHeader title="Pengaturan" />
      <div className="grid gap-4 sm:gap-5 lg:grid-cols-2">
        <SyncCard />
        <CategoriesCard />
        <SendersCard />
        <UnparsedCard />
      </div>
    </>
  );
}

function SyncCard() {
  const qc = useQueryClient();
  const { status, start, syncing } = useSync();
  const disconnect = useMutation({
    mutationFn: () => api("/auth/disconnect-google", { method: "POST" }),
    onSuccess: () => qc.invalidateQueries(),
  });
  const disconnected = status?.status === "disconnected";

  return (
    <Card>
      <CardHeader>
        <CardTitle>Sinkronisasi Gmail</CardTitle>
        <CardDescription>Email diperiksa otomatis tiap 5 menit.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2">
          <dt className="text-muted-foreground">Status</dt>
          <dd className="flex items-center gap-2">
            {status ? <SyncLozenge status={status.status} /> : "…"}
            {status?.mock && <Lozenge tone="info">Mode mock</Lozenge>}
          </dd>
          <dt className="text-muted-foreground">Terakhir sinkron</dt>
          <dd>{status?.last_synced_at ? dateTime(status.last_synced_at) : "Belum pernah"}</dd>
          {status?.last_error && (
            <>
              <dt className="text-muted-foreground">Galat terakhir</dt>
              <dd className="text-destructive">{status.last_error}</dd>
            </>
          )}
        </dl>
        <ErrorNote error={start.error ?? disconnect.error} />
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => start.mutate()} disabled={syncing || disconnected}>
            <RefreshCw className={syncing ? "animate-spin" : undefined} aria-hidden />
            {syncing ? "Menyinkronkan…" : "Sync sekarang"}
          </Button>
          {disconnected || status?.status === "needs_reauth" ? (
            <Button variant="secondary" onClick={() => signOut({ redirectTo: "/login" })}>
              Login ulang untuk menyambungkan
            </Button>
          ) : (
            <Button
              variant="destructive"
              disabled={disconnect.isPending}
              onClick={() =>
                confirm("Putuskan akses Gmail? Sinkronisasi berhenti, transaksi yang ada tetap disimpan.") &&
                disconnect.mutate()
              }
            >
              Disconnect Google
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function CategoriesCard() {
  const qc = useQueryClient();
  const categories = useCategories();
  const done = () => qc.invalidateQueries();
  const add = useMutation({
    mutationFn: (name: string) => api("/categories", { method: "POST", json: { name } }),
    onSuccess: done,
  });
  const remove = useMutation({
    mutationFn: (id: string) => api(`/categories/${id}`, { method: "DELETE" }),
    onSuccess: done,
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>Kategori</CardTitle>
        <CardDescription>Kategori bawaan tidak bisa dihapus.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3">
        <ul className="flex flex-wrap gap-1.5">
          {categories.data?.map((c) => (
            <li key={c.id} className="flex h-8 items-center gap-2 rounded-full bg-secondary pr-1 pl-3">
              <span className="size-2 rounded-full" style={{ background: c.color ?? "var(--muted-foreground)" }} aria-hidden />
              {c.name}
              {c.is_default ? (
                <span className="w-1" />
              ) : (
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={`Hapus kategori ${c.name}`}
                  onClick={() => confirm(`Hapus kategori "${c.name}"?`) && remove.mutate(c.id)}
                >
                  <Trash2 />
                </Button>
              )}
            </li>
          ))}
        </ul>
        <AddRow label="Nama kategori baru" placeholder="mis. Pendidikan" pending={add.isPending} onAdd={(v) => add.mutate(v)} />
        <ErrorNote error={categories.error ?? add.error ?? remove.error} />
      </CardContent>
    </Card>
  );
}

function SendersCard() {
  const qc = useQueryClient();
  const senders = useQuery({ queryKey: ["senders"], queryFn: () => api<Sender[]>("/senders") });
  const done = () => qc.invalidateQueries({ queryKey: ["senders"] });
  const toggle = useMutation({
    mutationFn: (json: { sender: string; enabled: boolean }) => api("/senders", { method: "PATCH", json }),
    onSuccess: done,
  });
  const add = useMutation({
    mutationFn: (sender: string) => api("/senders", { method: "POST", json: { sender } }),
    onSuccess: done,
  });
  const remove = useMutation({
    mutationFn: (sender: string) => api(`/senders/${encodeURIComponent(sender)}`, { method: "DELETE" }),
    onSuccess: done,
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>Pengirim email</CardTitle>
        <CardDescription>Hanya email dari pengirim aktif yang diproses menjadi transaksi.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3">
        <ul className="divide-y">
          {senders.data?.map((s) => (
            <li key={`${s.is_global}-${s.sender}`} className="flex h-10 items-center gap-2">
              <Switch
                aria-label={`Aktifkan ${s.sender}`}
                checked={s.enabled}
                onCheckedChange={(enabled) => toggle.mutate({ sender: s.sender, enabled })}
              />
              <span className="truncate font-medium">{s.sender}</span>
              <Lozenge>{s.provider}</Lozenge>
              {!s.is_global && (
                <Button
                  variant="ghost"
                  size="icon-sm"
                  className="ml-auto"
                  aria-label={`Hapus pengirim ${s.sender}`}
                  onClick={() => remove.mutate(s.sender)}
                >
                  <Trash2 />
                </Button>
              )}
            </li>
          ))}
        </ul>
        <AddRow label="Domain atau alamat pengirim baru" placeholder="mis. bni.co.id" pending={add.isPending} onAdd={(v) => add.mutate(v)} />
        <ErrorNote error={senders.error ?? toggle.error ?? add.error ?? remove.error} />
      </CardContent>
    </Card>
  );
}

function UnparsedCard() {
  const qc = useQueryClient();
  const emails = useQuery({
    queryKey: ["unparsed"],
    queryFn: () => api<Page<UnparsedEmail>>("/unparsed-emails?per_page=50"),
  });
  const ignore = useMutation({
    mutationFn: (id: string) => api(`/unparsed-emails/${id}`, { method: "PATCH", json: { ignored: true } }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["unparsed"] }),
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>Email gagal diproses</CardTitle>
        <CardDescription>Email dari pengirim aktif yang tidak dikenali sebagai transaksi.</CardDescription>
      </CardHeader>
      <CardContent>
        <ErrorNote error={emails.error ?? ignore.error} />
        {emails.data?.items.length === 0 ? (
          <EmptyState icon={MailWarning} text="Tidak ada email yang gagal diproses." />
        ) : (
          <ul className="divide-y">
            {emails.data?.items.map((e) => (
              <li key={e.id} className="flex items-start gap-3 py-2">
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">{e.subject || "(tanpa subjek)"}</div>
                  <div className="truncate text-xs text-muted-foreground">
                    {e.sender} · {dateTime(e.received_at)}
                  </div>
                  <div className="line-clamp-2 text-xs text-muted-foreground">{e.snippet}</div>
                </div>
                <Button variant="ghost" size="sm" disabled={ignore.isPending} onClick={() => ignore.mutate(e.id)}>
                  Abaikan
                </Button>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}

function AddRow(props: { label: string; placeholder: string; pending: boolean; onAdd: (value: string) => void }) {
  return (
    <form
      className="flex gap-2"
      onSubmit={(e) => {
        e.preventDefault();
        const input = e.currentTarget.elements.namedItem("value") as HTMLInputElement;
        const value = input.value.trim();
        if (!value) return;
        props.onAdd(value);
        input.value = "";
      }}
    >
      <Input name="value" aria-label={props.label} placeholder={props.placeholder} maxLength={100} required />
      <Button type="submit" variant="secondary" disabled={props.pending}>
        Tambah
      </Button>
    </form>
  );
}
