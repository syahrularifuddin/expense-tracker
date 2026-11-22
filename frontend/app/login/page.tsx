import { Wallet } from "lucide-react";
import { signIn } from "@/auth";
import { Button } from "@/components/ui/button";

// Read AUTH_GOOGLE_ID / MOCK_GMAIL per request: the Docker image is built without them.
export const dynamic = "force-dynamic";

export default function LoginPage() {
  const google = !!process.env.AUTH_GOOGLE_ID;
  const mock = process.env.MOCK_GMAIL === "true";

  return (
    <main className="flex min-h-dvh items-center justify-center bg-background p-4">
      <div className="w-full max-w-sm rounded-xl bg-card p-8 text-center shadow-card sm:p-10">
        <span className="mx-auto flex size-12 items-center justify-center rounded-full bg-brand text-white">
          <Wallet className="size-5" aria-hidden />
        </span>
        <h1 className="mt-4 text-2xl font-semibold tracking-tight">Expense Tracker</h1>
        <p className="mt-2 text-muted-foreground">
          Transaksi tercatat otomatis dari notifikasi email bank dan e-wallet di Gmail kamu.
        </p>

        <div className="mt-6 flex flex-col gap-2">
          {google && (
            <form
              action={async () => {
                "use server";
                await signIn("google", { redirectTo: "/" });
              }}
            >
              <Button type="submit" size="lg" className="w-full">
                Masuk dengan Google
              </Button>
            </form>
          )}
          {mock && (
            <form
              action={async () => {
                "use server";
                await signIn("dev", { redirectTo: "/" });
              }}
            >
              <Button type="submit" size="lg" variant={google ? "secondary" : "default"} className="w-full">
                Masuk sebagai user dev
              </Button>
            </form>
          )}
          {!google && !mock && (
            <p role="alert" className="text-destructive">
              Belum ada metode login. Isi AUTH_GOOGLE_ID atau MOCK_GMAIL=true di .env.local.
            </p>
          )}
        </div>

        <p className="mt-6 text-xs text-muted-foreground">
          {mock
            ? "Mode mock: data berasal dari email contoh, bukan Gmail asli."
            : "Akses Gmail hanya-baca. Hanya email dari pengirim yang kamu izinkan yang diproses."}
        </p>
      </div>
    </main>
  );
}
