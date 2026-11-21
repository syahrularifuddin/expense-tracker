// Pengalihan optimistis ke /login untuk yang belum masuk (callback `authorized` di auth.ts).
// Otorisasi sebenarnya ada di backend: setiap endpoint memeriksa JWT.
export { auth as proxy } from "@/auth";

export const config = {
  matcher: ["/((?!api|_next/static|_next/image|favicon.ico|login).*)"],
};
