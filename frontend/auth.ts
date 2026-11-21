import NextAuth from "next-auth";
import Credentials from "next-auth/providers/credentials";
import Google from "next-auth/providers/google";

const BACKEND = process.env.BACKEND_URL ?? "http://127.0.0.1:8080";
const MOCK = process.env.MOCK_GMAIL === "true";

/** Sesi backend yang disimpan di cookie sesi Auth.js (terenkripsi, tidak sampai ke JS browser kecuali access token). */
type BackendTokens = { access_token: string; refresh_token: string; expires_at: number };

async function backend(path: string, body: unknown = {}): Promise<BackendTokens> {
  const res = await fetch(`${BACKEND}/api/v1${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
    cache: "no-store",
  });
  if (!res.ok) throw new Error(`backend ${path} membalas ${res.status}`);
  return res.json();
}

export const { handlers, auth, signIn, signOut } = NextAuth({
  providers: [
    ...(process.env.AUTH_GOOGLE_ID
      ? [
          Google({
            authorization: {
              params: {
                scope: "openid email profile https://www.googleapis.com/auth/gmail.readonly",
                access_type: "offline",
                // Tanpa ini Google hanya mengirim refresh_token pada persetujuan pertama.
                prompt: "consent",
              },
            },
          }),
        ]
      : []),
    ...(MOCK
      ? [
          Credentials({
            id: "dev",
            name: "User dev",
            credentials: {},
            authorize: async () => ({
              id: "dev",
              name: "User Dev",
              email: "dev@example.com",
              backend: await backend("/auth/dev-login"),
            }),
          }),
        ]
      : []),
  ],
  session: { strategy: "jwt" },
  pages: { signIn: "/login" },
  callbacks: {
    authorized: ({ auth }) => !!auth?.user && !auth.error,
    async jwt({ token, account, user }) {
      const t = token as typeof token & Partial<BackendTokens> & { error?: string };
      if (account?.provider === "google") {
        // Sisi server: refresh token Gmail langsung ke backend, tidak pernah ke browser.
        Object.assign(
          t,
          await backend("/auth/exchange", {
            id_token: account.id_token,
            refresh_token: account.refresh_token,
          }),
        );
        delete t.error;
      } else if (account?.provider === "dev") {
        Object.assign(t, (user as { backend: BackendTokens }).backend);
        delete t.error;
      } else if (t.refresh_token && Date.now() / 1000 > (t.expires_at ?? 0) - 60) {
        try {
          Object.assign(t, await backend("/auth/refresh", { refresh_token: t.refresh_token }));
        } catch {
          t.error = "RefreshFailed";
        }
      }
      return t;
    },
    session({ session, token }) {
      const t = token as Partial<BackendTokens> & { error?: string };
      session.accessToken = t.access_token;
      session.error = t.error;
      return session;
    },
  },
  events: {
    async signOut(message) {
      const token = "token" in message ? (message.token as Partial<BackendTokens> | null) : null;
      if (token?.refresh_token) {
        await backend("/auth/logout", { refresh_token: token.refresh_token }).catch(() => {});
      }
    },
  },
});

declare module "next-auth" {
  interface Session {
    /** Access JWT backend (15 menit). */
    accessToken?: string;
    error?: string;
  }
}
