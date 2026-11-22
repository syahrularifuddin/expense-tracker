import type { Metadata } from "next";
import { JetBrains_Mono, Plus_Jakarta_Sans, Space_Grotesk } from "next/font/google";
import { Providers } from "@/components/providers";
import "./globals.css";

const sans = Plus_Jakarta_Sans({ variable: "--font-plus-jakarta-sans", subsets: ["latin"] });
const display = Space_Grotesk({ variable: "--font-space-grotesk", subsets: ["latin"], weight: ["500", "600", "700"] });
const mono = JetBrains_Mono({ variable: "--font-jetbrains-mono", subsets: ["latin"], weight: ["400", "500"] });

export const metadata: Metadata = {
  title: "Expense Tracker",
  description: "Catat pengeluaran otomatis dari notifikasi email",
};

// Pasang kelas .dark sebelum paint pertama agar tidak berkedip.
const themeScript = `try{var t=localStorage.getItem("theme");if(t==="dark"||(!t&&matchMedia("(prefers-color-scheme: dark)").matches))document.documentElement.classList.add("dark")}catch(e){}`;

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html lang="id" className={`${sans.variable} ${display.variable} ${mono.variable} h-full antialiased`} suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: themeScript }} />
      </head>
      <body className="min-h-full">
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
