import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Self-contained server for the Docker image (frontend/Dockerfile).
  output: "standalone",
  // cacheComponents / partialPrefetching sengaja mati: semua halaman di balik login
  // dirender di klien (TanStack Query), jadi tidak ada yang diprerender per-request.
  turbopack: {
    rules: {
      "*.css": {
        loaders: ["@tailwindcss/turbopack"],
        as: "*.css",
      },
    },
  },
};

export default nextConfig;
