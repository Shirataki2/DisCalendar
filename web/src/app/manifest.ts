import type { MetadataRoute } from "next";
import { SITE_DESCRIPTION, SITE_NAME, THEME_COLOR } from "@/lib/site";

// /manifest.webmanifest (旧実装の @nuxtjs/pwa の manifest 相当。オフライン対応の Service Worker は移行しない)
export default function manifest(): MetadataRoute.Manifest {
  return {
    name: SITE_NAME,
    short_name: SITE_NAME,
    description: SITE_DESCRIPTION,
    lang: "ja",
    start_url: "/",
    display: "standalone",
    theme_color: THEME_COLOR,
    // globals.css の .dark --background と同じ
    background_color: "#121212",
    icons: [
      { src: "/icons/icon-192.png", sizes: "192x192", type: "image/png" },
      { src: "/icons/icon-512.png", sizes: "512x512", type: "image/png" },
    ],
  };
}
