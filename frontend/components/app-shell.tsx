"use client";

import { cn } from "cn";
import { LogOut, Moon, RefreshCw, Sun, Wallet } from "lucide-react";
import { signOut, useSession } from "next-auth/react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useSyncExternalStore } from "react";
import { SyncLozenge } from "@/components/bits";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useSync } from "@/lib/hooks";

const NAV = [
  { href: "/", label: "Dashboard" },
  { href: "/transactions", label: "Transaksi" },
  { href: "/reports", label: "Laporan" },
  { href: "/settings", label: "Pengaturan" },
] as const;

function toggleTheme() {
  const dark = document.documentElement.classList.toggle("dark");
  try {
    localStorage.setItem("theme", dark ? "dark" : "light");
  } catch {}
}

export function AppShell({ children }: { children: React.ReactNode }) {
  const { data: session } = useSession();
  const { start, syncing, status } = useSync();
  const pathname = usePathname();
  const user = session?.user;
  // Halaman bergantung pada sesi, waktu saat ini, dan cache query: render di klien saja
  // supaya HTML server tidak pernah berbeda dari render pertama klien.
  const mounted = useSyncExternalStore(
    () => () => {},
    () => true,
    () => false,
  );

  return (
    <div className="mx-auto min-h-dvh max-w-[1240px] px-3 pt-3 pb-10 sm:px-6 sm:pt-5">
      <header className="flex h-16 items-center gap-2 rounded-full bg-card pr-2 pl-4 shadow-card sm:pl-6">
        <Link href="/" className="flex shrink-0 items-center gap-2.5 rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring">
          <span className="flex size-8 items-center justify-center rounded-full bg-brand text-white">
            <Wallet className="size-4" aria-hidden />
          </span>
          <span className="text-lg font-semibold tracking-tight">
            Expense<span className="hidden font-normal text-muted-foreground sm:inline"> Tracker</span>
          </span>
        </Link>
        <span className="hidden h-5 border-l lg:block" aria-hidden />
        <span className="hidden text-muted-foreground lg:block">dari notifikasi email</span>

        <div className="ml-auto flex items-center gap-1.5">
          <span className="hidden md:block">{status && <SyncLozenge status={syncing ? "syncing" : status.status} />}</span>
          <Button
            variant="secondary"
            aria-label="Sinkronkan email sekarang"
            onClick={() => start.mutate()}
            disabled={syncing || status?.status === "disconnected"}
          >
            <RefreshCw className={cn(syncing && "animate-spin")} aria-hidden />
            <span className="hidden sm:inline">{syncing ? "Menyinkronkan…" : "Sync"}</span>
          </Button>
          <Button variant="ghost" size="icon" aria-label="Ganti tema terang/gelap" onClick={toggleTheme}>
            <Sun className="hidden dark:block" />
            <Moon className="dark:hidden" />
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger render={<Button variant="ghost" size="icon" aria-label="Menu akun" />}>
              <span className="flex size-8 items-center justify-center rounded-full bg-secondary text-sm font-semibold">
                {(user?.name ?? user?.email ?? "?").charAt(0).toUpperCase()}
              </span>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="min-w-56">
              <DropdownMenuGroup>
                <DropdownMenuLabel>
                  <div className="font-medium text-foreground">{user?.name ?? "Pengguna"}</div>
                  <div className="truncate font-normal">{user?.email}</div>
                </DropdownMenuLabel>
              </DropdownMenuGroup>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={() => signOut({ redirectTo: "/login" })}>
                <LogOut aria-hidden />
                Keluar
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </header>

      <nav
        aria-label="Navigasi utama"
        className="no-scrollbar mt-5 flex max-w-full gap-1 overflow-x-auto rounded-full bg-secondary p-1 sm:inline-flex"
      >
        {NAV.map(({ href, label }) => {
          const active = href === "/" ? pathname === "/" : pathname.startsWith(href);
          return (
            <Link
              key={href}
              href={href}
              aria-current={active ? "page" : undefined}
              className={cn(
                "flex h-10 shrink-0 items-center rounded-full px-4 font-medium outline-none focus-visible:ring-2 focus-visible:ring-ring sm:px-5",
                active ? "bg-card text-foreground shadow-card" : "text-muted-foreground hover:text-foreground",
              )}
            >
              {label}
            </Link>
          );
        })}
      </nav>

      <main className="mt-7">{mounted && children}</main>
    </div>
  );
}
