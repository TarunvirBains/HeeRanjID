import { defineConfig } from "vitest/config";
import { existsSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [
    {
      name: "resolve-ts-from-js",
      resolveId(id, importer) {
        if (!importer || !id.endsWith(".js")) return;
        const base = id.startsWith(".")
          ? resolve(dirname(importer), id.slice(0, -3) + ".ts")
          : null;
        if (base && existsSync(base)) return base;
      },
    },
  ],
  test: {
    include: ["tests/**/*.test.ts"],
  },
});
